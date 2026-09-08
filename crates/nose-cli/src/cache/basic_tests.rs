use super::*;
use std::os::unix::fs::PermissionsExt;

/// `files` counts the lowered corpus the caller hands in — which already
/// excludes unreadable/parse-failed files (`lower_corpus_filtered` filter_maps
/// them). So a corpus where one file failed to read yields `files == 1`.
#[test]
fn file_count_matches_lowered_corpus() {
    let dir = std::env::temp_dir().join(format!("nose_cache_count_{}", std::process::id()));
    let cache = std::env::temp_dir().join(format!("nose_cache_dir_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::create_dir_all(&cache).unwrap();

    std::fs::write(dir.join("ok.py"), "def f():\n    return 1\n").unwrap();
    let bad = dir.join("bad.py");
    std::fs::write(&bad, "def g():\n    return 2\n").unwrap();
    std::fs::set_permissions(&bad, std::fs::Permissions::from_mode(0o000)).unwrap();

    let readable = std::fs::read(&bad).is_ok();
    let corpus = nose_frontend::lower_corpus_filtered(&[dir.as_path()], &[]);
    let out = build_units_cached(corpus, &DetectOptions::default(), &cache);

    let _ = std::fs::set_permissions(&bad, std::fs::Permissions::from_mode(0o644));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&cache);

    if readable {
        // Running as root (CI sometimes) — the unreadable file is still readable.
        return;
    }
    assert_eq!(out.files, 1, "only the readable file should be counted");
}

#[test]
fn layered_cas_ignores_legacy_schema_14_entries() {
    let root = std::env::temp_dir().join(format!("nose_cache_schema_{}", std::process::id()));
    let source = root.join("source");
    let cache = root.join("cache");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&source).unwrap();
    std::fs::create_dir_all(&cache).unwrap();
    std::fs::write(
        source.join("mixed.ts"),
        "function f(x: boolean) { if (x) { return 1; } }\n",
    )
    .unwrap();

    let corpus = nose_frontend::lower_corpus_filtered(&[source.as_path()], &[]);
    let options = DetectOptions::default();
    let stale = cache.join("v14-deadbeef");
    std::fs::create_dir_all(&stale).unwrap();
    std::fs::write(stale.join("legacy.json"), b"[]").unwrap();
    let first = build_units_cached(corpus.clone(), &options, &cache);
    assert!(!first.units.is_empty());
    let second = build_units_cached(corpus, &options, &cache);
    assert_eq!(second.units.len(), first.units.len());
    assert_eq!(second.stats.hits, 1);
    assert!(cache.join("cas-v2/units-syntax").is_dir());
    assert!(stale.is_dir(), "the legacy bucket should remain untouched");

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn checkout_path_moves_reuse_payload_and_rebind_locations() {
    let root = std::env::temp_dir().join(format!("nose_cache_move_{}", std::process::id()));
    let source_a = root.join("checkout-a");
    let source_b = root.join("checkout-b");
    let cache = root.join("cache");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&source_a).unwrap();
    std::fs::create_dir_all(&source_b).unwrap();
    let source = "def total(xs):\n    return sum(xs)\n";
    std::fs::write(source_a.join("same.py"), source).unwrap();
    std::fs::write(source_b.join("same.py"), source).unwrap();

    let options = DetectOptions::default();
    let first_corpus = nose_frontend::lower_corpus_filtered(&[source_a.as_path()], &[]);
    let first = build_units_cached(first_corpus, &options, &cache);
    assert_eq!((first.stats.hits, first.stats.misses), (0, 1));
    let second_corpus = nose_frontend::lower_corpus_filtered(&[source_b.as_path()], &[]);
    let second = build_units_cached(second_corpus, &options, &cache);
    assert_eq!((second.stats.hits, second.stats.misses), (1, 0));
    assert!(second
        .units
        .iter()
        .all(|unit| unit.path.starts_with(source_b.to_string_lossy().as_ref())));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn evidence_only_change_misses_then_reuses_resolved_artifact() {
    use nose_il::{
        EvidenceAnchor, EvidenceId, EvidenceKind, EvidenceProvenance, EvidenceRecord,
        EvidenceStatus, ParameterShapeEvidenceKind,
    };

    let root = std::env::temp_dir().join(format!("nose_cache_evidence_{}", std::process::id()));
    let source = root.join("source");
    let cache = root.join("cache");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&source).unwrap();
    std::fs::write(source.join("same.py"), "def f(x):\n    return x\n").unwrap();
    let options = DetectOptions::default();
    let corpus = nose_frontend::lower_corpus_filtered(&[source.as_path()], &[]);
    let first = build_units_cached(corpus.clone(), &options, &cache);
    assert_eq!((first.stats.hits, first.stats.misses), (0, 1));

    let mut changed = corpus.clone();
    let il = &mut changed.files[0];
    let span = il.node(il.root).span;
    il.push_evidence(EvidenceRecord::new(
        EvidenceId(il.evidence.len() as u32),
        EvidenceAnchor::param(span),
        EvidenceKind::ParameterShape(ParameterShapeEvidenceKind::NonPlain),
        EvidenceProvenance::builtin("nose.test", "evidence-only"),
        Vec::new(),
        EvidenceStatus::Asserted,
    ));
    let second = build_units_cached(changed.clone(), &options, &cache);
    assert_eq!((second.stats.hits, second.stats.misses), (0, 1));
    let third = build_units_cached(changed, &options, &cache);
    assert_eq!((third.stats.hits, third.stats.misses), (1, 0));
    let _ = std::fs::remove_dir_all(&root);
}
