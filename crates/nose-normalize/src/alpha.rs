//! Alpha-renaming: replace identifier names with canonical ids (`v0, v1, …`)
//! assigned by first-occurrence order, so that two functions that differ only in
//! identifier names produce identical IL (alpha-equivalence).
//!
//! The cid space resets at every `Func` boundary, so two clone functions number
//! their locals independently and therefore identically — which is exactly what
//! the detector needs. Field names are *not* renamed (they carry API meaning).
//! Lambdas share their enclosing function's scope so closures stay consistent.

use rustc_hash::{FxHashMap, FxHashSet};
use nose_il::{Il, LoopKind, NodeId, NodeKind, Payload, Symbol};

pub(crate) fn run(il: &mut Il) {
    let root = il.root;
    let mut names: Vec<Symbol> = Vec::new();
    let mut scope = Scope::default();
    let bound = compute_bound(il, root);
    rename(il, root, &mut scope, &bound, &mut names);
    il.cid_names = names;
}

/// Symbols *bound* within one scope (params + assignment targets + `for`-pattern
/// vars), NOT descending into nested functions. A name used but not bound here is a
/// *free* variable (a global, or an un-canonicalized function callee) — it must keep
/// its name identity, never an alpha-renamed positional id, or two functions calling
/// different globals (`foo(x)` vs `bar(x)`, `max(a,b)` vs `min(a,b)`) collapse.
fn compute_bound(il: &Il, scope_root: NodeId) -> FxHashSet<Symbol> {
    let mut bound = FxHashSet::default();
    for &c in il.children(scope_root) {
        collect_bound(il, c, &mut bound);
    }
    bound
}

fn collect_bound(il: &Il, id: NodeId, bound: &mut FxHashSet<Symbol>) {
    let kind = il.kind(id);
    if kind == NodeKind::Func {
        return; // nested function: its own scope
    }
    match kind {
        NodeKind::Param => {
            if let Payload::Name(s) = il.node(id).payload {
                bound.insert(s);
            }
        }
        NodeKind::Assign => {
            if let Some(&lhs) = il.children(id).first() {
                collect_targets(il, lhs, bound);
            }
        }
        NodeKind::Loop if matches!(il.node(id).payload, Payload::Loop(LoopKind::ForEach)) => {
            if let Some(&pat) = il.children(id).first() {
                collect_targets(il, pat, bound);
            }
        }
        _ => {}
    }
    for &c in il.children(id) {
        collect_bound(il, c, bound);
    }
}

/// Assignment/`for`-pattern target symbols: a `Var`, or every `Var` in a destructuring
/// `Seq` (`a, b = …`).
fn collect_targets(il: &Il, id: NodeId, bound: &mut FxHashSet<Symbol>) {
    match il.kind(id) {
        NodeKind::Var => {
            if let Payload::Name(s) = il.node(id).payload {
                bound.insert(s);
            }
        }
        NodeKind::Seq => {
            for &c in il.children(id) {
                collect_targets(il, c, bound);
            }
        }
        _ => {}
    }
}

#[derive(Default)]
struct Scope {
    map: FxHashMap<Symbol, u32>,
    next: u32,
}

impl Scope {
    fn get_or_add(&mut self, s: Symbol, names: &mut Vec<Symbol>) -> u32 {
        if let Some(&c) = self.map.get(&s) {
            return c;
        }
        let c = self.next;
        self.next += 1;
        self.map.insert(s, c);
        // best-effort cid -> name table (first name seen for each index)
        if (c as usize) >= names.len() {
            names.push(s);
        }
        c
    }
}

fn rename(
    il: &mut Il,
    id: NodeId,
    scope: &mut Scope,
    bound: &FxHashSet<Symbol>,
    names: &mut Vec<Symbol>,
) {
    let (kind, payload) = {
        let n = il.node(id);
        (n.kind, n.payload)
    };

    // A `Param` is always a binder; a `Var` is alpha-renamed only when its name is
    // BOUND in this scope. A *free* `Var` (global / function callee) keeps its `Name`
    // payload so its identity is preserved — `node_tag` and the value graph key it by
    // name, so `foo` ≠ `bar` but every `foo` agrees (and converges across files via the
    // shared interner).
    if matches!(kind, NodeKind::Var | NodeKind::Param) {
        if let Payload::Name(s) = payload {
            if kind == NodeKind::Param || bound.contains(&s) {
                let cid = scope.get_or_add(s, names);
                il.nodes[id.0 as usize].payload = Payload::Cid(cid);
            }
        }
    }

    let children: Vec<NodeId> = il.children(id).to_vec();
    if kind == NodeKind::Func {
        // Fresh scope: params (first children) seed v0.., then the body.
        let mut inner = Scope::default();
        let inner_bound = compute_bound(il, id);
        for c in children {
            rename(il, c, &mut inner, &inner_bound, names);
        }
    } else {
        for c in children {
            rename(il, c, scope, bound, names);
        }
    }
}
