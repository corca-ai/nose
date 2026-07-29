use super::fragments::collect_extra_unit_roots;
use crate::fragment::FragmentKind;
use nose_il::{Il, Interner, NodeId, Symbol, UnitKind, UnitOrigin};

#[derive(Clone, Copy)]
pub(super) struct UnitRoot {
    pub(super) root: NodeId,
    pub(super) kind: UnitKind,
    pub(super) name: Option<Symbol>,
    pub(super) origin: UnitOrigin,
    /// The exact-fragment classification, when this root was admitted as an exact
    /// sub-function fragment. `None` for ordinary function/method/class/block units.
    /// `Some(_)` is the authoritative "this is an exact fragment" signal.
    pub(super) fragment_kind: Option<FragmentKind>,
}

pub(super) fn collect_unit_roots(
    il: &Il,
    interner: &Interner,
    block_units: bool,
) -> (Vec<UnitRoot>, Option<Vec<Option<NodeId>>>) {
    let mut roots: Vec<UnitRoot> = il
        .units
        .iter()
        .map(|unit| UnitRoot {
            root: unit.root,
            kind: unit.kind,
            name: unit.name,
            origin: unit.origin,
            fragment_kind: None,
        })
        .collect();
    let parents = if block_units {
        let parents = super::tree::build_parent_index(il);
        collect_extra_unit_roots(il, il.root, &parents, interner, &mut roots);
        Some(parents)
    } else {
        None
    };
    (roots, parents)
}

pub(super) fn value_fingerprint_context_for_roots(
    il: &Il,
    interner: &Interner,
    root_count: usize,
) -> Option<nose_normalize::ValueFingerprintContext> {
    (root_count > 1).then(|| nose_normalize::ValueFingerprintContext::new(il, interner))
}
