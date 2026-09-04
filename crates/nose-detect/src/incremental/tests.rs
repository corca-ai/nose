use super::*;
use crate::candidates::structural_candidates;
use crate::{DetectOptions, StructuralDetector, UnitFeat};
use nose_il::{Corpus, FileId, Interner, Lang};

fn features(source: &[(&str, &str)], opts: &DetectOptions) -> Vec<UnitFeat> {
    features_in_lang(source, Lang::Python, opts)
}

fn features_in_lang(source: &[(&str, &str)], lang: Lang, opts: &DetectOptions) -> Vec<UnitFeat> {
    let interner = Interner::new();
    let files = source
        .iter()
        .enumerate()
        .map(|(index, (path, text))| {
            nose_frontend::lower_source(
                FileId(index as u32),
                path,
                text.as_bytes(),
                lang,
                &interner,
            )
            .expect("Python fixture lowers")
        })
        .collect();
    let corpus = Corpus::new(interner, files);
    crate::corpus_features(&corpus, opts).units
}

#[test]
fn incremental_buckets_match_clean_candidate_generation() {
    let opts = DetectOptions {
        shape_candidates: true,
        ..DetectOptions::default()
    };
    let units = features(
        &[
            ("a.py", "def f(xs):\n    return sum(x * x for x in xs)\n"),
            ("b.py", "def g(xs):\n    return sum(x * x for x in xs)\n"),
            ("c.py", "def h(xs):\n    return sum(x + x for x in xs)\n"),
        ],
        &opts,
    );
    let mut stats = IncrementalDetectionStats::new();
    let prepared = prepare(&units, None, &opts, None, &mut stats);
    assert_eq!(prepared.candidates, structural_candidates(&units, &opts));
}

#[test]
fn dense_bucket_growth_matches_clean_scoring_without_losing_old_pairs() {
    let opts = DetectOptions {
        min_lines: 1,
        min_tokens: 1,
        block_units: false,
        ..Default::default()
    };
    let paths = (0..49)
        .map(|index| format!("{index}.py"))
        .collect::<Vec<_>>();
    let sources = paths
        .iter()
        .map(|path| (path.as_str(), "def f(x):\n    return x + 1\n"))
        .collect::<Vec<_>>();
    let units = features(&sources, &opts);
    assert_eq!(units.len(), 49);
    let detector = StructuralDetector::strict(opts.jaccard_weight);
    let mut stats = IncrementalDetectionStats::new();
    let first = prepare(&units[..48], None, &opts, None, &mut stats);
    let old_pairs = first.candidates.clone();
    let (scored, accepted) = score(&units[..48], &first, &detector, opts.threshold, &mut stats);
    let groups = components(&first, &accepted, opts.threshold, &mut stats);
    let state = finish_state(
        first,
        &scored,
        &groups,
        IncrementalConnected::default(),
        None,
    );
    let next = prepare(&units, None, &opts, Some(state), &mut stats);
    assert_eq!(next.candidates, structural_candidates(&units, &opts));
    assert_eq!(next.candidates.len(), 49 * 48 / 2);
    assert!(old_pairs
        .iter()
        .all(|pair| next.candidates.binary_search(pair).is_ok()));
    let (_, cached) = score(&units, &next, &detector, opts.threshold, &mut stats);
    let fresh = prepare(&units, None, &opts, None, &mut stats);
    let (_, clean) = score(&units, &fresh, &detector, opts.threshold, &mut stats);
    assert_eq!(cached, clean);
}

#[test]
fn unchanged_state_reuses_buckets_and_scores() {
    let opts = DetectOptions::default();
    let units = features(
        &[
            ("a.py", "def f(xs):\n    return sum(x * x for x in xs)\n"),
            ("b.py", "def g(xs):\n    return sum(x * x for x in xs)\n"),
        ],
        &opts,
    );
    let detector = StructuralDetector::strict(opts.jaccard_weight);
    let mut first_stats = IncrementalDetectionStats::new();
    let first = prepare(&units, None, &opts, None, &mut first_stats);
    let (scored, _) = score(&units, &first, &detector, opts.threshold, &mut first_stats);
    let components = components(&first, &[], opts.threshold, &mut first_stats);
    let state = finish_state(
        first,
        &scored,
        &components,
        IncrementalConnected::default(),
        None,
    );

    let mut second_stats = IncrementalDetectionStats::new();
    let second = prepare(&units, None, &opts, Some(state), &mut second_stats);
    let _ = score(
        &units,
        &second,
        &detector,
        opts.threshold,
        &mut second_stats,
    );
    assert!(second_stats.state_hit);
    assert_eq!(second_stats.units_reused, units.len());
    assert_eq!(second_stats.buckets_rebuilt, 0);
    assert_eq!(second_stats.scores_evaluated, 0);
    assert_eq!(second_stats.scores_reused, second.candidates.len());
}

#[test]
fn deleting_a_same_unit_branch_removes_its_cached_witness() {
    let opts = DetectOptions {
        threshold: 0.70,
        min_lines: 3,
        min_tokens: 12,
        shape_candidates: true,
        connected_witnesses: true,
        ..DetectOptions::default()
    };
    let repeated = r#"
int set_option(const char *name, const char *value) {
  if (!strcmp(name, "progress")) {
    if (!strcmp(value, "true")) options.progress = 1;
    else if (!strcmp(value, "false")) options.progress = 0;
    else return -1;
    return 0;
  }
  if (!strcmp(name, "deepen-relative")) {
    if (!strcmp(value, "true")) options.deepen_relative = 1;
    else if (!strcmp(value, "false")) options.deepen_relative = 0;
    else return -1;
    return 0;
  }
  return 1;
}
"#;
    let single = r#"
int set_option(const char *name, const char *value) {
  if (!strcmp(name, "progress")) {
    if (!strcmp(value, "true")) options.progress = 1;
    else if (!strcmp(value, "false")) options.progress = 0;
    else return -1;
    return 0;
  }
  return 1;
}
"#;

    let first_units = features_in_lang(&[("options.c", repeated)], Lang::C, &opts);
    let mut first_stats = IncrementalDetectionStats::new();
    let first = prepare(&first_units, None, &opts, None, &mut first_stats);
    let first_connected = connected(&first_units, &first, &[], &[], &opts, &mut first_stats);
    assert!(
        !first_connected.same_unit_accepted.is_empty(),
        "fixture must seed a same-unit accepted edge"
    );
    let state = finish_state(first, &[], &[], first_connected, None);

    let second_units = features_in_lang(&[("options.c", single)], Lang::C, &opts);
    let mut second_stats = IncrementalDetectionStats::new();
    let second = prepare(&second_units, None, &opts, Some(state), &mut second_stats);
    let second_connected = connected(&second_units, &second, &[], &[], &opts, &mut second_stats);
    assert!(second_connected.same_unit_accepted.is_empty());
    assert!(second_stats.connected_evaluations_evaluated > 0);
}
