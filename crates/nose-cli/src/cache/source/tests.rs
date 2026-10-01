use super::*;

#[test]
fn same_size_and_mtime_never_override_content_identity() {
    let first = portable_il::source_digest(Lang::Python, b"return x + 1\n");
    let second = portable_il::source_digest(Lang::Python, b"return x - 1\n");
    assert_ne!(first, second);
}

#[test]
fn legacy_skipped_rust_source_is_admitted_then_reused() {
    let root = std::env::temp_dir().join(format!(
        "nose_cache_legacy_ansi_admission_{}",
        std::process::id()
    ));
    let source = root.join("source");
    let cache = root.join("cache");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&source).unwrap();
    let path = source.join("regression.rs");
    let bytes =
        include_bytes!("../../../../nose-frontend/src/corpus_tests/fixtures/ansi_expected.rs");
    std::fs::write(&path, bytes).unwrap();

    // The old classifier stored a valid empty raw bundle for this exact source.
    // Seed its real address so admission cannot silently restore that omission.
    let legacy_digest = ContentDigest::derive(
        b"nose.source-analysis.v3",
        &[
            portable_il::source_digest(Lang::Rust, bytes).as_bytes(),
            b"rs",
        ],
    );
    let legacy_key = ArtifactKey::derive(
        ArtifactStage::RawIl,
        RAW_IL_SCHEMA,
        &[legacy_digest.as_bytes()],
    );
    let seed = CacheRun::with_limit(&cache, super::super::DEFAULT_MAX_BYTES);
    let payload = rmp_serde::to_vec(&PortableRawBundle {
        schema: RAW_IL_SCHEMA,
        regions: Vec::new(),
    })
    .unwrap();
    seed.cas().store(legacy_key, &payload).unwrap();
    drop(seed);

    let cold = nose_frontend::lower_corpus_raw_filtered(&[source.as_path()], &[]);
    cold.ensure_complete().unwrap();
    assert_eq!(cold.files.len(), 1);
    let expected = portable_il::semantic_digest(&cold.files[0], &cold.interner);
    for attempt in 0..2 {
        let run = CacheRun::with_limit(&cache, super::super::DEFAULT_MAX_BYTES);
        let warm = build_raw_corpus_cached(&[source.as_path()], &[], &run);
        warm.corpus.ensure_complete().unwrap();
        assert!(warm.corpus.skipped_sources.is_empty());
        assert_eq!(warm.corpus.files.len(), 1);
        assert_eq!(
            portable_il::semantic_digest(&warm.corpus.files[0], &warm.corpus.interner),
            expected
        );
        assert_eq!(warm.regions[0].raw_hit, attempt == 1);
    }
    let _ = std::fs::remove_dir_all(&root);
}
