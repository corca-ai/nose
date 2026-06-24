use crate::{EnclosingUnit, Group, LineSpan, Loc, LocInit, Metrics, Report};
use nose_il::UnitKind::Function;

use super::super::RefactorFamily;

pub(super) fn loc(file: &str, s: u32, e: u32, lang: &str) -> Loc {
    Loc::new(LocInit {
        file: file.into(),
        source_span: LineSpan::new(s, e),
        lang: lang.into(),
        kind: Function,
        origin: Default::default(),
        name: None,
        sem: 50,
        span_tokens: 50,
    })
}
/// A site with explicit kind / value-graph size / name (for discount tests).
pub(super) fn loc_k(file: &str, s: u32, e: u32, kind: nose_il::UnitKind, sem: usize) -> Loc {
    Loc::new(LocInit {
        file: file.into(),
        source_span: LineSpan::new(s, e),
        lang: "rust".into(),
        kind,
        origin: Default::default(),
        name: None,
        sem,
        span_tokens: sem,
    })
}

pub(super) fn fragment_loc(file: &str, s: u32, e: u32) -> Loc {
    fragment_loc_k(file, s, e, crate::FragmentKind::ConditionalGuard)
}

pub(super) fn fragment_loc_k(
    file: &str,
    s: u32,
    e: u32,
    fragment_kind: crate::FragmentKind,
) -> Loc {
    Loc {
        is_fragment: true,
        fragment_kind: Some(fragment_kind),
        reason_code: Some(fragment_kind.reason_code()),
        ..Loc::new(LocInit {
            file: file.into(),
            source_span: LineSpan::new(s, e),
            lang: "rust".into(),
            kind: nose_il::UnitKind::Block,
            origin: Default::default(),
            name: None,
            sem: 50,
            span_tokens: 50,
        })
    }
}

pub(super) fn test_fragment_loc_k(
    file: &str,
    s: u32,
    e: u32,
    fragment_kind: crate::FragmentKind,
) -> Loc {
    let mut loc = fragment_loc_k(file, s, e, fragment_kind);
    loc.enclosing_unit = Some(EnclosingUnit {
        file: file.into(),
        start_line: s.saturating_sub(5).max(1),
        end_line: e + 5,
        kind: Function,
        name: Some("test_scaffold".into()),
        unit_key: format!(
            "{file}:Function:{}-{}:test_scaffold",
            s.saturating_sub(5).max(1),
            e + 5
        ),
    });
    loc
}

/// A family with the given locations and metrics, other fields at neutral values.
pub(super) fn fam(
    value: f64,
    mean_lines: u32,
    shared: u32,
    params: u32,
    locs: Vec<Loc>,
) -> RefactorFamily {
    RefactorFamily {
        value,
        members: locs.len(),
        files: locs
            .iter()
            .map(|l| &l.file)
            .collect::<std::collections::HashSet<_>>()
            .len(),
        modules: 1,
        languages: locs
            .iter()
            .map(|l| &l.lang)
            .collect::<std::collections::HashSet<_>>()
            .len()
            .max(1),
        mean_score: 1.0,
        mean_lines,
        dup_lines: mean_lines,
        shared_lines: shared,
        params,
        shared_weight: shared as f64,
        locations: locs,
        mean_sem: 50.0,
        scope: "prod",
        discount: 1.0,
        abstraction_witness: None,
        witness: None,
        varying_spots: Vec::new(),
        semantic_laws: Vec::new(),
    }
}

pub(super) fn report(groups: Vec<Group>) -> Report {
    Report {
        tool: "nose",
        version: "test",
        detector: "structural".into(),
        duplicates: vec![],
        reinvented: vec![],
        groups,
        metrics: Metrics {
            files: 0,
            units: 0,
            candidate_pairs: 0,
            accepted_pairs: 0,
            groups: 0,
        },
    }
}

pub(super) fn witnessed(mut f: RefactorFamily, kind: &'static str) -> RefactorFamily {
    f.witness = Some(crate::EquivalenceWitness {
        kind,
        value_nodes: None,
        mean_value_jaccard: None,
        mean_shape_jaccard: None,
        graded: None,
        graded_pair: None,
    });
    f
}
