use super::*;
use nose_normalize::VgSinkKind;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct SinkSignature {
    kind: SemanticSinkKind,
    hash: u64,
    effect_ord: Option<u32>,
}

pub(super) fn sink_signatures(dag: &ValueDag) -> Vec<SinkSignature> {
    let mut signatures = dag
        .sinks
        .iter()
        .take(MAX_NODES_PER_UNIT)
        .filter_map(|sink| {
            Some(SinkSignature {
                kind: semantic_sink_kind(sink.kind),
                hash: dag.nodes.get(sink.value as usize)?.hash,
                effect_ord: sink.effect_ord,
            })
        })
        .collect::<Vec<_>>();
    signatures.sort();
    signatures
}

fn semantic_sink_kind(kind: VgSinkKind) -> SemanticSinkKind {
    match kind {
        VgSinkKind::Return => SemanticSinkKind::Return,
        VgSinkKind::Cond => SemanticSinkKind::Cond,
        VgSinkKind::Effect => SemanticSinkKind::Effect,
        VgSinkKind::Break => SemanticSinkKind::Break,
        VgSinkKind::Throw => SemanticSinkKind::Throw,
    }
}

pub(super) fn sink_deltas(
    before: &[SinkSignature],
    after: &[SinkSignature],
) -> Vec<SemanticSinkDelta> {
    let mut kinds = BTreeMap::<SemanticSinkKind, (Vec<SinkSignature>, Vec<SinkSignature>)>::new();
    for &sink in before {
        kinds.entry(sink.kind).or_default().0.push(sink);
    }
    for &sink in after {
        kinds.entry(sink.kind).or_default().1.push(sink);
    }
    kinds
        .into_iter()
        .filter_map(|(kind, (before, after))| {
            let removed = multiset_removed(&before, &after);
            let inserted = multiset_removed(&after, &before);
            (removed > 0 || inserted > 0).then_some(SemanticSinkDelta {
                kind,
                removed,
                inserted,
            })
        })
        .collect()
}
