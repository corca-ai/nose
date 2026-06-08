//! First-party call-target evidence producer.
//!
//! Consumers must not resolve user calls from raw spelling. This pass is the
//! narrow boundary where the lowered file's binding shape is checked and a
//! direct in-file callable target is materialized as evidence.

use nose_il::{
    CallTargetEvidenceKind, EvidenceAnchor, EvidenceKind, Il, Interner, LoopKind, NodeId, NodeKind,
    Payload, Symbol, UnitKind,
};
use nose_semantics::FIRST_PARTY_PACK_ID;
use rustc_hash::{FxHashMap, FxHashSet};

const DIRECT_FUNCTION_RULE: &str = "normalize.call_target.direct_function";

#[derive(Clone, Copy)]
struct DirectFunctionTarget {
    root: NodeId,
    name_hash: u64,
}

pub(crate) fn run(il: &mut Il, interner: &Interner) {
    let targets = unique_direct_function_targets(il, interner);
    if targets.is_empty() {
        return;
    }
    let function_names = function_unit_names(il);
    let scope_bound = scope_bound_symbols(il, &function_names);
    let mut proven = Vec::new();
    let mut scope_stack = Vec::new();
    collect_call_targets(
        il,
        il.root,
        &mut scope_stack,
        &targets,
        &scope_bound,
        &mut proven,
    );
    for (call, target) in proven {
        il.find_or_push_first_party_evidence(
            EvidenceAnchor::node(il.node(call).span, NodeKind::Call),
            EvidenceKind::CallTarget(CallTargetEvidenceKind::DirectFunction {
                target_span: il.node(target.root).span,
                name_hash: target.name_hash,
            }),
            FIRST_PARTY_PACK_ID,
            DIRECT_FUNCTION_RULE,
            Vec::new(),
        );
    }
}

fn unique_direct_function_targets(
    il: &Il,
    interner: &Interner,
) -> FxHashMap<Symbol, DirectFunctionTarget> {
    let parents = parent_map(il);
    let mut targets = FxHashMap::default();
    let mut ambiguous = FxHashSet::default();
    for unit in &il.units {
        if unit.kind != UnitKind::Function || !is_top_level_function_root(il, &parents, unit.root) {
            continue;
        }
        let Some(name) = unit.name else { continue };
        if ambiguous.contains(&name) {
            continue;
        }
        let target = DirectFunctionTarget {
            root: unit.root,
            name_hash: interner.symbol_hash(name),
        };
        if targets.insert(name, target).is_some() {
            targets.remove(&name);
            ambiguous.insert(name);
        }
    }
    targets
}

fn parent_map(il: &Il) -> Vec<Option<NodeId>> {
    let mut parents = vec![None; il.nodes.len()];
    for (idx, _) in il.nodes.iter().enumerate() {
        let parent = NodeId(idx as u32);
        for &child in il.children(parent) {
            if let Some(slot) = parents.get_mut(child.0 as usize) {
                *slot = Some(parent);
            }
        }
    }
    parents
}

fn is_top_level_function_root(il: &Il, parents: &[Option<NodeId>], root: NodeId) -> bool {
    parents
        .get(root.0 as usize)
        .copied()
        .flatten()
        .is_some_and(|parent| il.kind(parent) == NodeKind::Module)
}

fn function_unit_names(il: &Il) -> FxHashMap<u32, Symbol> {
    let mut names = FxHashMap::default();
    for unit in &il.units {
        if unit.kind == UnitKind::Function {
            if let Some(name) = unit.name {
                names.insert(unit.root.0, name);
            }
        }
    }
    names
}

fn scope_bound_symbols(
    il: &Il,
    function_names: &FxHashMap<u32, Symbol>,
) -> FxHashMap<u32, FxHashSet<Symbol>> {
    let mut out = FxHashMap::default();
    collect_scope_bound_symbols(il, il.root, function_names, &mut out);
    out
}

fn collect_scope_bound_symbols(
    il: &Il,
    node: NodeId,
    function_names: &FxHashMap<u32, Symbol>,
    out: &mut FxHashMap<u32, FxHashSet<Symbol>>,
) {
    if is_scope(il.kind(node)) {
        let mut bound = FxHashSet::default();
        collect_bound_in_scope(il, node, true, il.kind(node), function_names, &mut bound);
        out.insert(node.0, bound);
    }
    for &child in il.children(node) {
        collect_scope_bound_symbols(il, child, function_names, out);
    }
}

fn collect_bound_in_scope(
    il: &Il,
    node: NodeId,
    is_root: bool,
    scope_kind: NodeKind,
    function_names: &FxHashMap<u32, Symbol>,
    out: &mut FxHashSet<Symbol>,
) {
    if !is_root && is_scope(il.kind(node)) {
        if scope_kind != NodeKind::Module {
            if let Some(&name) = function_names.get(&node.0) {
                out.insert(name);
            }
        }
        return;
    }
    match il.kind(node) {
        NodeKind::Param => {
            if let Payload::Name(name) = il.node(node).payload {
                out.insert(name);
            }
        }
        NodeKind::Assign => {
            if let Some(&lhs) = il.children(node).first() {
                collect_target_symbols(il, lhs, out);
            }
        }
        NodeKind::Loop if matches!(il.node(node).payload, Payload::Loop(LoopKind::ForEach)) => {
            if let Some(&pattern) = il.children(node).first() {
                collect_target_symbols(il, pattern, out);
            }
        }
        _ => {}
    }
    for &child in il.children(node) {
        collect_bound_in_scope(il, child, false, scope_kind, function_names, out);
    }
}

fn collect_target_symbols(il: &Il, node: NodeId, out: &mut FxHashSet<Symbol>) {
    match il.kind(node) {
        NodeKind::Var => {
            if let Payload::Name(name) = il.node(node).payload {
                out.insert(name);
            }
        }
        NodeKind::Seq => {
            for &child in il.children(node) {
                collect_target_symbols(il, child, out);
            }
        }
        _ => {}
    }
}

fn collect_call_targets(
    il: &Il,
    node: NodeId,
    scope_stack: &mut Vec<NodeId>,
    targets: &FxHashMap<Symbol, DirectFunctionTarget>,
    scope_bound: &FxHashMap<u32, FxHashSet<Symbol>>,
    out: &mut Vec<(NodeId, DirectFunctionTarget)>,
) {
    let entered_scope = is_scope(il.kind(node));
    if entered_scope {
        scope_stack.push(node);
    }
    if let Some(target) = direct_call_target(il, node, scope_stack, targets, scope_bound) {
        out.push((node, target));
    }
    for &child in il.children(node) {
        collect_call_targets(il, child, scope_stack, targets, scope_bound, out);
    }
    if entered_scope {
        scope_stack.pop();
    }
}

fn direct_call_target(
    il: &Il,
    node: NodeId,
    scope_stack: &[NodeId],
    targets: &FxHashMap<Symbol, DirectFunctionTarget>,
    scope_bound: &FxHashMap<u32, FxHashSet<Symbol>>,
) -> Option<DirectFunctionTarget> {
    if il.kind(node) != NodeKind::Call || !matches!(il.node(node).payload, Payload::None) {
        return None;
    }
    let callee = *il.children(node).first()?;
    if il.kind(callee) != NodeKind::Var {
        return None;
    }
    let Payload::Name(name) = il.node(callee).payload else {
        return None;
    };
    if scope_stack.iter().any(|scope| {
        scope_bound
            .get(&scope.0)
            .is_some_and(|bound| bound.contains(&name))
    }) {
        return None;
    }
    targets.get(&name).copied()
}

fn is_scope(kind: NodeKind) -> bool {
    matches!(kind, NodeKind::Module | NodeKind::Func | NodeKind::Lambda)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nose_il::{FileId, FileMeta, IlBuilder, Lang, Span, Unit};
    use nose_semantics::direct_function_call_target_at_call;

    fn sp(n: u32) -> Span {
        Span::new(FileId(0), n, n + 1, n, n)
    }

    fn function_with_call(
        interner: &Interner,
        func_name: &str,
        callee_name: &str,
        duplicate_unit: bool,
    ) -> (Il, NodeId, NodeId) {
        let mut b = IlBuilder::new(FileId(0));
        let func_sym = interner.intern(func_name);
        let callee_sym = interner.intern(callee_name);
        let callee = b.add(NodeKind::Var, Payload::Name(callee_sym), sp(10), &[]);
        let call = b.add(NodeKind::Call, Payload::None, sp(11), &[callee]);
        let ret = b.add(NodeKind::Return, Payload::None, sp(12), &[call]);
        let body = b.add(NodeKind::Block, Payload::None, sp(13), &[ret]);
        let func = b.add(NodeKind::Func, Payload::None, sp(14), &[body]);
        let module = b.add(NodeKind::Module, Payload::None, sp(15), &[func]);
        let mut units = vec![Unit {
            root: func,
            kind: UnitKind::Function,
            name: Some(func_sym),
        }];
        if duplicate_unit {
            units.push(Unit {
                root: func,
                kind: UnitKind::Function,
                name: Some(func_sym),
            });
        }
        let il = b.finish(
            module,
            FileMeta {
                path: "t".into(),
                lang: Lang::Python,
            },
            units,
            Vec::new(),
        );
        (il, func, call)
    }

    #[test]
    fn emits_direct_function_call_target_for_unique_unshadowed_function() {
        let interner = Interner::new();
        let (mut il, func, call) = function_with_call(&interner, "f", "f", false);
        run(&mut il, &interner);
        assert!(direct_function_call_target_at_call(&il, call, func));
    }

    #[test]
    fn does_not_emit_when_local_binder_shadows_function_name() {
        let interner = Interner::new();
        let f = interner.intern("f");
        let mut b = IlBuilder::new(FileId(0));
        let param = b.add(NodeKind::Param, Payload::Name(f), sp(1), &[]);
        let callee = b.add(NodeKind::Var, Payload::Name(f), sp(2), &[]);
        let call = b.add(NodeKind::Call, Payload::None, sp(3), &[callee]);
        let ret = b.add(NodeKind::Return, Payload::None, sp(4), &[call]);
        let body = b.add(NodeKind::Block, Payload::None, sp(5), &[ret]);
        let func = b.add(NodeKind::Func, Payload::None, sp(6), &[param, body]);
        let module = b.add(NodeKind::Module, Payload::None, sp(7), &[func]);
        let mut il = b.finish(
            module,
            FileMeta {
                path: "t".into(),
                lang: Lang::Python,
            },
            vec![Unit {
                root: func,
                kind: UnitKind::Function,
                name: Some(f),
            }],
            Vec::new(),
        );

        run(&mut il, &interner);
        assert!(!direct_function_call_target_at_call(&il, call, func));
    }

    #[test]
    fn does_not_emit_for_duplicate_function_names() {
        let interner = Interner::new();
        let (mut il, func, call) = function_with_call(&interner, "f", "f", true);
        run(&mut il, &interner);
        assert!(!direct_function_call_target_at_call(&il, call, func));
    }

    #[test]
    fn does_not_emit_for_method_bare_call() {
        let interner = Interner::new();
        let method_sym = interner.intern("fac");
        let mut b = IlBuilder::new(FileId(0));
        let callee = b.add(NodeKind::Var, Payload::Name(method_sym), sp(20), &[]);
        let call = b.add(NodeKind::Call, Payload::None, sp(21), &[callee]);
        let ret = b.add(NodeKind::Return, Payload::None, sp(22), &[call]);
        let body = b.add(NodeKind::Block, Payload::None, sp(23), &[ret]);
        let method = b.add(NodeKind::Func, Payload::None, sp(24), &[body]);
        let module = b.add(NodeKind::Module, Payload::None, sp(25), &[method]);
        let mut il = b.finish(
            module,
            FileMeta {
                path: "t".into(),
                lang: Lang::Java,
            },
            vec![Unit {
                root: method,
                kind: UnitKind::Method,
                name: Some(method_sym),
            }],
            Vec::new(),
        );

        run(&mut il, &interner);
        assert!(!direct_function_call_target_at_call(&il, call, method));
    }

    #[test]
    fn does_not_emit_for_nested_function_not_visible_as_top_level() {
        let interner = Interner::new();
        let f = interner.intern("f");
        let mut b = IlBuilder::new(FileId(0));
        let nested_body = b.add(NodeKind::Block, Payload::None, sp(1), &[]);
        let nested = b.add(NodeKind::Func, Payload::None, sp(2), &[nested_body]);
        let callee = b.add(NodeKind::Var, Payload::Name(f), sp(3), &[]);
        let call = b.add(NodeKind::Call, Payload::None, sp(4), &[callee]);
        let ret = b.add(NodeKind::Return, Payload::None, sp(5), &[call]);
        let outer_body = b.add(NodeKind::Block, Payload::None, sp(6), &[nested, ret]);
        let outer = b.add(NodeKind::Func, Payload::None, sp(7), &[outer_body]);
        let module = b.add(NodeKind::Module, Payload::None, sp(8), &[outer]);
        let mut il = b.finish(
            module,
            FileMeta {
                path: "t".into(),
                lang: Lang::Python,
            },
            vec![Unit {
                root: nested,
                kind: UnitKind::Function,
                name: Some(f),
            }],
            Vec::new(),
        );

        run(&mut il, &interner);
        assert!(!direct_function_call_target_at_call(&il, call, nested));
    }

    #[test]
    fn does_not_emit_when_enclosing_scope_binds_function_name() {
        let interner = Interner::new();
        let f = interner.intern("f");
        let g = interner.intern("g");
        let mut b = IlBuilder::new(FileId(0));

        let target_body = b.add(NodeKind::Block, Payload::None, sp(1), &[]);
        let target = b.add(NodeKind::Func, Payload::None, sp(2), &[target_body]);

        let shadow_lhs = b.add(NodeKind::Var, Payload::Name(f), sp(3), &[]);
        let shadow_rhs = b.add(NodeKind::Lit, Payload::LitInt(1), sp(4), &[]);
        let shadow = b.add(
            NodeKind::Assign,
            Payload::None,
            sp(5),
            &[shadow_lhs, shadow_rhs],
        );
        let callee = b.add(NodeKind::Var, Payload::Name(f), sp(6), &[]);
        let call = b.add(NodeKind::Call, Payload::None, sp(7), &[callee]);
        let inner_ret = b.add(NodeKind::Return, Payload::None, sp(8), &[call]);
        let inner_body = b.add(NodeKind::Block, Payload::None, sp(9), &[inner_ret]);
        let inner = b.add(NodeKind::Func, Payload::None, sp(10), &[inner_body]);
        let outer_body = b.add(NodeKind::Block, Payload::None, sp(11), &[shadow, inner]);
        let outer = b.add(NodeKind::Func, Payload::None, sp(12), &[outer_body]);
        let module = b.add(NodeKind::Module, Payload::None, sp(13), &[target, outer]);
        let mut il = b.finish(
            module,
            FileMeta {
                path: "t".into(),
                lang: Lang::Python,
            },
            vec![
                Unit {
                    root: target,
                    kind: UnitKind::Function,
                    name: Some(f),
                },
                Unit {
                    root: outer,
                    kind: UnitKind::Function,
                    name: Some(g),
                },
            ],
            Vec::new(),
        );

        run(&mut il, &interner);
        assert!(!direct_function_call_target_at_call(&il, call, target));
    }
}
