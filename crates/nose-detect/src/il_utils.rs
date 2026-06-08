use nose_il::{Il, NodeId, NodeKind, Payload};
use rustc_hash::FxHashSet;

pub(crate) fn node_mentions_any_cid(il: &Il, node: NodeId, cids: &FxHashSet<u32>) -> bool {
    if let (NodeKind::Var, Payload::Cid(cid)) = (il.kind(node), il.node(node).payload) {
        if cids.contains(&cid) {
            return true;
        }
    }
    il.children(node)
        .iter()
        .any(|&child| node_mentions_any_cid(il, child, cids))
}

pub(crate) fn local_nontrivial_assignment(il: &Il, node: NodeId) -> Option<(u32, NodeId)> {
    let (lhs, rhs) = il.assignment_var_parts(node)?;
    if matches!(il.kind(rhs), NodeKind::Var | NodeKind::Lit) {
        return None;
    }
    let cid = il.var_cid(lhs)?;
    let mut target = FxHashSet::default();
    target.insert(cid);
    if node_mentions_any_cid(il, rhs, &target) {
        return None;
    }
    Some((cid, rhs))
}

pub(crate) struct LocalTempChain {
    pub second_cid: u32,
    pub first: FxHashSet<u32>,
    pub second: FxHashSet<u32>,
    pub all: FxHashSet<u32>,
}

pub(crate) fn local_nontrivial_assignment_chain(
    il: &Il,
    first_assign: NodeId,
    second_assign: NodeId,
) -> Option<LocalTempChain> {
    let (first_cid, first_rhs) = local_nontrivial_assignment(il, first_assign)?;
    let (second_cid, second_rhs) = local_nontrivial_assignment(il, second_assign)?;
    if first_cid == second_cid {
        return None;
    }
    let mut first = FxHashSet::default();
    first.insert(first_cid);
    let mut second = FxHashSet::default();
    second.insert(second_cid);
    if node_mentions_any_cid(il, first_rhs, &first)
        || node_mentions_any_cid(il, first_rhs, &second)
        || !node_mentions_any_cid(il, second_rhs, &first)
    {
        return None;
    }
    let mut all = first.clone();
    all.insert(second_cid);
    Some(LocalTempChain {
        second_cid,
        first,
        second,
        all,
    })
}
