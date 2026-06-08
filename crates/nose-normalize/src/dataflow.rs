//! Dataflow normalization, stage 1: copy / expression propagation.
//!
//! Inlines a variable that is **defined once and used once** with a
//! **side-effect-free** right-hand side, provided no statement between the
//! definition and the use writes a variable the RHS reads. This dissolves
//! intermediate temporaries so that, e.g.
//!
//! ```text
//! t = a + b; return t * 2      ≡      return (a + b) * 2
//! ```
//!
//! Chains fold transitively (`a = 1; b = a + 1; return b` → `return 1 + 1`),
//! because the rebuild substitutes recursively.
//!
//! Analysis is scoped per function (cids are reused across functions after
//! alpha-renaming) and keyed by *node id* (globally unique), so results from
//! different scopes merge safely. The rewrite is a deterministic rebuild.

use nose_il::{Il, IlBuilder, NodeId, NodeKind, Payload};
use rustc_hash::{FxHashMap, FxHashSet};

pub(crate) fn run(old: &Il) -> Il {
    let mut analysis = Analysis::default();
    analyze_scope(old, old.root, true, &mut analysis);
    if analysis.inline_at.is_empty() && analysis.drop_defs.is_empty() {
        return old.clone();
    }

    let unit_root_set: FxHashSet<u32> = old.units.iter().map(|u| u.root.0).collect();
    let mut rb = Rebuilder {
        old,
        b: IlBuilder::with_capacity(old.file, old.nodes.len(), old.edges.len()),
        inline_at: analysis.inline_at,
        drop_defs: analysis.drop_defs,
        remap: FxHashMap::default(),
        unit_root_set,
    };
    let new_root = rb.go(old.root);

    crate::finalize_rebuild(old, &rb.remap, rb.b, new_root, old.cid_names.clone())
}

#[derive(Default)]
struct Analysis {
    /// use-Var node id → RHS subtree root (old arena) to inline in its place.
    inline_at: FxHashMap<u32, NodeId>,
    /// Assign statement node ids to drop.
    drop_defs: FxHashSet<u32>,
}

/// Analyze one cid scope (`root` is a `Func` body or the module root). Does not
/// descend into nested `Func`s; those are queued and analyzed as their own
/// scopes.
fn analyze_scope(il: &Il, root: NodeId, is_root: bool, a: &mut Analysis) {
    let mut def_var_nodes: FxHashSet<u32> = FxHashSet::default();
    let mut def_count: FxHashMap<u32, u32> = FxHashMap::default();
    let mut use_count: FxHashMap<u32, u32> = FxHashMap::default();
    let mut use_node: FxHashMap<u32, NodeId> = FxHashMap::default();
    let mut param_cids: FxHashSet<u32> = FxHashSet::default();
    let mut nested_funcs: Vec<NodeId> = Vec::new();

    // Pass A: mark def-write Var nodes (lhs of Assign) and param cids.
    crate::collect_scope(
        il,
        root,
        is_root,
        &mut def_var_nodes,
        &mut param_cids,
        &mut nested_funcs,
    );
    // Pass B: count defs/uses.
    count(
        il,
        root,
        is_root,
        &def_var_nodes,
        &mut def_count,
        &mut use_count,
        &mut use_node,
    );

    find_inlines(
        il,
        root,
        is_root,
        &def_count,
        &use_count,
        &use_node,
        &param_cids,
        a,
    );

    for f in nested_funcs {
        analyze_scope(il, f, true, a);
    }
}

#[allow(clippy::too_many_arguments)]
fn count(
    il: &Il,
    node: NodeId,
    is_root: bool,
    def_vars: &FxHashSet<u32>,
    def_count: &mut FxHashMap<u32, u32>,
    use_count: &mut FxHashMap<u32, u32>,
    use_node: &mut FxHashMap<u32, NodeId>,
) {
    let kind = il.kind(node);
    if kind == NodeKind::Func && !is_root {
        return;
    }
    if kind == NodeKind::Var {
        if let Payload::Cid(c) = il.node(node).payload {
            if def_vars.contains(&node.0) {
                *def_count.entry(c).or_default() += 1;
            } else {
                *use_count.entry(c).or_default() += 1;
                use_node.insert(c, node);
            }
        }
    }
    for &c in il.children(node) {
        count(il, c, false, def_vars, def_count, use_count, use_node);
    }
}

#[allow(clippy::too_many_arguments)]
fn find_inlines(
    il: &Il,
    node: NodeId,
    is_root: bool,
    def_count: &FxHashMap<u32, u32>,
    use_count: &FxHashMap<u32, u32>,
    use_node: &FxHashMap<u32, NodeId>,
    params: &FxHashSet<u32>,
    a: &mut Analysis,
) {
    let kind = il.kind(node);
    if kind == NodeKind::Func && !is_root {
        return;
    }
    if kind == NodeKind::Block {
        let stmts = il.children(node).to_vec();
        // Precompute, in one pass over the block, which statement each node belongs
        // to and the cids each statement writes. This turns the per-candidate
        // "find the use's statement" (was an O(stmts) subtree scan) into an O(1)
        // lookup and the hazard check into cheap set tests — without it a single
        // huge block is O(stmts² · subtree) (e.g. comfy/sd.py: 84ms → ~3ms).
        let mut owner: FxHashMap<u32, usize> = FxHashMap::default();
        let mut writes: Vec<FxHashSet<u32>> = Vec::with_capacity(stmts.len());
        for (idx, &s) in stmts.iter().enumerate() {
            mark_owner(il, s, idx, &mut owner);
            let mut w = FxHashSet::default();
            collect_writes(il, s, &mut w);
            writes.push(w);
        }
        for (idx, &s) in stmts.iter().enumerate() {
            if il.kind(s) != NodeKind::Assign {
                continue;
            }
            let kids = il.children(s);
            if kids.len() != 2 || il.kind(kids[0]) != NodeKind::Var {
                continue;
            }
            let rhs = kids[1];
            let cid = match il.node(kids[0]).payload {
                Payload::Cid(c) => c,
                _ => continue,
            };
            if params.contains(&cid)
                || def_count.get(&cid) != Some(&1)
                || use_count.get(&cid) != Some(&1)
                || !crate::is_pure(il, rhs)
            {
                continue;
            }
            let u = match use_node.get(&cid) {
                Some(&u) => u,
                None => continue,
            };
            // The single use must live in a later statement of this same block.
            let use_idx = match owner.get(&u.0) {
                Some(&j) if j > idx => j,
                _ => continue, // use is nested elsewhere / before — skip conservatively
            };
            // Hazard: no intervening statement may write a variable the RHS reads.
            let mut reads = FxHashSet::default();
            read_cids(il, rhs, &mut reads);
            let hazard = (idx + 1..use_idx).any(|j| reads.iter().any(|c| writes[j].contains(c)));
            if hazard {
                continue;
            }
            a.inline_at.insert(u.0, rhs);
            a.drop_defs.insert(s.0);
        }
    }
    for &c in il.children(node) {
        find_inlines(il, c, false, def_count, use_count, use_node, params, a);
    }
}

fn read_cids(il: &Il, node: NodeId, out: &mut FxHashSet<u32>) {
    if il.kind(node) == NodeKind::Var {
        if let Payload::Cid(c) = il.node(node).payload {
            out.insert(c);
        }
    }
    for &c in il.children(node) {
        read_cids(il, c, out);
    }
}

/// Record `idx` as the owning statement for every node in `node`'s subtree.
fn mark_owner(il: &Il, node: NodeId, idx: usize, owner: &mut FxHashMap<u32, usize>) {
    owner.insert(node.0, idx);
    for &c in il.children(node) {
        mark_owner(il, c, idx, owner);
    }
}

/// Collect the cids written by any `Assign` in `node`'s subtree (lhs `Var` cid).
fn collect_writes(il: &Il, node: NodeId, out: &mut FxHashSet<u32>) {
    if il.kind(node) == NodeKind::Assign {
        if let Some(&lhs) = il.children(node).first() {
            if il.kind(lhs) == NodeKind::Var {
                if let Payload::Cid(c) = il.node(lhs).payload {
                    out.insert(c);
                }
            }
        }
    }
    for &c in il.children(node) {
        collect_writes(il, c, out);
    }
}

struct Rebuilder<'a> {
    old: &'a Il,
    b: IlBuilder,
    inline_at: FxHashMap<u32, NodeId>,
    drop_defs: FxHashSet<u32>,
    remap: FxHashMap<u32, NodeId>,
    unit_root_set: FxHashSet<u32>,
}

impl Rebuilder<'_> {
    fn go(&mut self, old_id: NodeId) -> NodeId {
        // Substitute an inlinable use with its RHS (rebuilt recursively → chains).
        if let Some(&rhs) = self.inline_at.get(&old_id.0) {
            return self.go(rhs);
        }
        let new_id = match self.old.kind(old_id) {
            NodeKind::Block => self.block(old_id),
            _ => self.generic(old_id),
        };
        if self.unit_root_set.contains(&old_id.0) {
            self.remap.insert(old_id.0, new_id);
        }
        new_id
    }

    fn block(&mut self, old_id: NodeId) -> NodeId {
        let span = self.old.node(old_id).span;
        let child_count = self.old.children(old_id).len();
        let mut kids = Vec::with_capacity(child_count);
        for idx in 0..child_count {
            let s = self.old.children(old_id)[idx];
            if self.drop_defs.contains(&s.0) {
                continue;
            }
            kids.push(self.go(s));
        }
        self.b.add(NodeKind::Block, Payload::None, span, &kids)
    }

    crate::rebuild_generic!();
}
