//! Dead-code elimination.
//!
//! - **dead-assignment**: an `Assign` to a variable that is never read in its
//!   function scope is dropped (or, if its RHS has effects, reduced to an
//!   `ExprStmt` of the RHS so the effect survives). Clones that differ only by an
//!   unused/debug temporary thereby converge.
//! - **unreachable code**: statements after a `Return`/`Throw`/`Break`/`Continue`
//!   in the same block are dropped.
//!
//! Use-counts are scoped per function (cids reset at `Func`). A deterministic
//! rebuild; unit roots are remapped.

use nose_il::{FileMeta, Il, IlBuilder, NodeId, NodeKind, Payload, Unit};
use rustc_hash::{FxHashMap, FxHashSet};

pub(crate) fn run(old: &Il) -> Il {
    // drop[assign node id] = rhs_is_pure (pure → drop entirely; else → ExprStmt)
    let mut drop: FxHashMap<u32, bool> = FxHashMap::default();
    analyze_scope(old, old.root, true, &mut drop);

    let unit_root_set: FxHashSet<u32> = old.units.iter().map(|u| u.root.0).collect();
    let mut rb = Rebuilder {
        old,
        b: IlBuilder::new(old.file),
        drop,
        remap: FxHashMap::default(),
        unit_root_set,
    };
    let new_root = rb.go(old.root);
    let units: Vec<Unit> = old
        .units
        .iter()
        .filter_map(|u| {
            rb.remap.get(&u.root.0).map(|&r| Unit {
                root: r,
                kind: u.kind,
                name: u.name,
            })
        })
        .collect();
    let meta = FileMeta {
        path: old.meta.path.clone(),
        lang: old.meta.lang,
    };
    rb.b.finish(new_root, meta, units, old.cid_names.clone())
}

fn analyze_scope(il: &Il, root: NodeId, is_root: bool, drop: &mut FxHashMap<u32, bool>) {
    let mut read: FxHashSet<u32> = FxHashSet::default();
    let mut params: FxHashSet<u32> = FxHashSet::default();
    let mut def_vars: FxHashSet<u32> = FxHashSet::default();
    let mut nested: Vec<NodeId> = Vec::new();
    crate::collect_scope(il, root, is_root, &mut def_vars, &mut params, &mut nested);
    collect_reads(il, root, is_root, &def_vars, &mut read);

    // Drop-eligible: an Assign whose lhs cid is never read and is not a param.
    find_dead(il, root, is_root, &read, &params, drop);

    for f in nested {
        analyze_scope(il, f, true, drop);
    }
}

fn collect_reads(
    il: &Il,
    node: NodeId,
    is_root: bool,
    def_vars: &FxHashSet<u32>,
    read: &mut FxHashSet<u32>,
) {
    let kind = il.kind(node);
    if kind == NodeKind::Func && !is_root {
        return;
    }
    if kind == NodeKind::Var && !def_vars.contains(&node.0) {
        if let Payload::Cid(c) = il.node(node).payload {
            read.insert(c);
        }
    }
    for &c in il.children(node) {
        collect_reads(il, c, false, def_vars, read);
    }
}

fn find_dead(
    il: &Il,
    node: NodeId,
    is_root: bool,
    read: &FxHashSet<u32>,
    params: &FxHashSet<u32>,
    drop: &mut FxHashMap<u32, bool>,
) {
    let kind = il.kind(node);
    if kind == NodeKind::Func && !is_root {
        return;
    }
    if kind == NodeKind::Assign {
        let kids = il.children(node);
        if kids.len() == 2 && il.kind(kids[0]) == NodeKind::Var {
            if let Payload::Cid(c) = il.node(kids[0]).payload {
                if !read.contains(&c) && !params.contains(&c) {
                    drop.insert(node.0, is_pure(il, kids[1]));
                }
            }
        }
    }
    for &c in il.children(node) {
        find_dead(il, c, false, read, params, drop);
    }
}

fn is_pure(il: &Il, node: NodeId) -> bool {
    match il.kind(node) {
        NodeKind::Call | NodeKind::HoF | NodeKind::Assign | NodeKind::Throw => false,
        _ => il.children(node).iter().all(|&c| is_pure(il, c)),
    }
}

fn is_terminator(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Return | NodeKind::Throw | NodeKind::Break | NodeKind::Continue
    )
}

struct Rebuilder<'a> {
    old: &'a Il,
    b: IlBuilder,
    drop: FxHashMap<u32, bool>,
    remap: FxHashMap<u32, NodeId>,
    unit_root_set: FxHashSet<u32>,
}

impl Rebuilder<'_> {
    fn go(&mut self, old_id: NodeId) -> NodeId {
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
        let children = self.old.children(old_id).to_vec();
        let mut out = Vec::with_capacity(children.len());
        for s in children {
            // dead assignment → drop, or keep only its effectful RHS
            if let Some(&pure) = self.drop.get(&s.0) {
                if !pure {
                    let rhs = self.old.children(s)[1];
                    let r = self.go(rhs);
                    let sp = self.old.node(s).span;
                    out.push(self.b.add(NodeKind::ExprStmt, Payload::None, sp, &[r]));
                }
                continue;
            }
            let kind = self.old.kind(s);
            out.push(self.go(s));
            if is_terminator(kind) {
                break; // unreachable code after a terminator
            }
        }
        self.b.add(NodeKind::Block, Payload::None, span, &out)
    }

    crate::rebuild_generic!();
}
