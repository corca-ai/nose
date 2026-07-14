use super::*;
use nose_il::{Builtin, FileId, FileMeta, IlBuilder, Lang, Span};

fn unit(loc: &str, reason: &'static str, fp: Vec<u64>, tags: &[&str]) -> CensusUnit {
    let excluded = reason != "interpretable";
    CensusUnit {
        loc: loc.to_string(),
        verify_loc: loc.to_string(),
        language: "python",
        reason,
        fp,
        tags: tags.iter().map(|tag| tag.to_string()).collect(),
        exact_safe: true,
        claimable: true,
        classification: if excluded {
            "missing-oracle-support"
        } else {
            "interpretable"
        },
        obligation_family: if excluded {
            "oracle-capability".to_string()
        } else {
            "interpretable".to_string()
        },
        obligation_subreason: if excluded {
            "value.symbolic-condition".to_string()
        } else {
            "interpretable".to_string()
        },
        first_blocker: excluded.then(|| nose_normalize::InterpreterBlocker {
            category: "value",
            capability_id: "value.symbolic-condition",
            blocker_stack: vec![nose_normalize::InterpreterBlockerFrame {
                role: "eval",
                construct: "kind:If".to_string(),
            }],
        }),
    }
}

fn sample_units() -> Vec<CensusUnit> {
    vec![
        unit("a.py:1", "interpretable", vec![1, 2], &["kind:Loop"]),
        unit(
            "b.py:1",
            "battery-bail",
            vec![1, 2],
            &["kind:Loop", "call:named"],
        ),
        unit("c.py:9", "battery-bail", vec![1, 2], &["builtin:Len"]),
        unit("d.py:1", "interpretable", vec![3], &["kind:Loop"]),
        unit("e.py:1", "interpretable", vec![3], &["kind:Loop"]),
        unit("f.py:1", "empty-fp", vec![], &["kind:Raw"]),
    ]
}

#[test]
fn report_counts_merge_mass_and_priority() {
    let units = sample_units();
    let report = build_report(&units);
    assert_eq!(report.units_total, 6);
    assert_eq!(report.interpretable_units, 3);
    assert_eq!(report.excluded_by_reason["battery-bail"], 2);
    assert_eq!(report.excluded_by_reason["empty-fp"], 1);
    assert_eq!(report.merge_pairs.total, 4);
    assert_eq!(report.merge_pairs.verified, 1);
    assert_eq!(report.merge_pairs.unverified, 3);
    assert_eq!(report.claimable_merge_pairs.total, 4);
    assert_eq!(report.generic_unattributed_exclusions, 0);
    assert_eq!(report.priority.len(), 1);
    assert_eq!(report.priority[0].claimable_pair_mass, 3);
    assert_eq!(report.priority[0].capped_claimable_pair_mass, 3);
    assert_eq!(report.priority[0].priority_score, 9);
    assert_eq!(report.units.len(), units.len());
}

#[test]
fn report_attributes_unverified_mass_to_constructs() {
    let report = build_report(&sample_units());
    let row = |tag: &str| report.tags.iter().find(|row| row.tag == tag).unwrap();
    assert_eq!(row("call:named").unverified_pairs, 3);
    assert_eq!(row("builtin:Len").unverified_pairs, 3);
    assert_eq!(row("kind:Loop").unverified_pairs, 3);
    assert_eq!(row("kind:Loop").interpretable_units, 3);
    assert_eq!(row("kind:Loop").excluded_units, 1);
    assert_eq!(row("kind:Raw").unverified_pairs, 0);
    assert_eq!(row("call:named").example_excluded, vec!["b.py:1"]);
}

#[test]
fn all_excluded_group_counts_without_pair_underflow() {
    let units = vec![
        unit("a.py:1", "battery-bail", vec![9], &["kind:If"]),
        unit("b.py:1", "battery-bail", vec![9], &["kind:If"]),
    ];
    let report = build_report(&units);
    assert_eq!(report.merge_pairs.total, 1);
    assert_eq!(report.merge_pairs.verified, 0);
    assert_eq!(report.merge_pairs.unverified, 1);
    assert_eq!(report.claimable_merge_pairs.unverified, 1);
    assert_eq!(report.priority[0].claimable_pair_mass, 1);
}

#[test]
fn exact_unsafe_cluster_cannot_change_priority() {
    let baseline = vec![
        unit("safe.py:1", "interpretable", vec![1, 2, 3, 4], &[]),
        unit("gap.py:1", "battery-bail", vec![1, 2, 3, 4], &[]),
    ];
    let expected = build_report(&baseline).priority;
    let mut poisoned = baseline;
    for index in 0..100 {
        let mut unsafe_unit = unit(
            &format!("unsafe.py:{index}"),
            "battery-bail",
            vec![9, 9, 9, 9],
            &[],
        );
        unsafe_unit.exact_safe = false;
        unsafe_unit.claimable = false;
        poisoned.push(unsafe_unit);
    }
    assert_eq!(build_report(&poisoned).priority, expected);
}

#[test]
fn census_tags_refine_calls_and_skip_retained_literals() {
    let span = Span::synthetic(FileId(0));
    let mut builder = IlBuilder::new(FileId(0));
    let string = builder.add(NodeKind::Lit, Payload::LitStr(0xABCD), span, &[]);
    let call = builder.add(
        NodeKind::Call,
        Payload::Builtin(Builtin::Len),
        span,
        &[string],
    );
    let ret = builder.add(NodeKind::Return, Payload::None, span, &[call]);
    let func = builder.add(NodeKind::Func, Payload::None, span, &[ret]);
    let il = builder.finish(
        func,
        FileMeta {
            path: "census.rs".into(),
            lang: Lang::Rust,
        },
        Vec::new(),
        Vec::new(),
    );
    let tags = census_tags(&il, func);
    assert!(tags.contains(&"builtin:Len".to_string()));
    assert!(tags.contains(&"kind:Return".to_string()));
    assert!(tags.contains(&"kind:Func".to_string()));
    assert!(!tags.iter().any(|tag| tag.starts_with("lit:")));
    assert!(!tags.contains(&"kind:Call".to_string()));
}
