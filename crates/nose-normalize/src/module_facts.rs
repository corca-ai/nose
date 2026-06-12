use nose_il::{Il, Interner, NodeId, NodeKind, Payload, Symbol};
use nose_semantics::semantics;
use nose_semantics::{
    binding_write_target, opaque_argument_escape_args, receiver_mutation_call_receiver,
};
use rustc_hash::{FxHashMap, FxHashSet};

/// Whether the module-level binding `name` is mutated ANYWHERE in the file — a
/// non-top-level assignment (e.g. `global name; name = other` inside a function, a bare
/// reassignment in a JS function), a receiver-mutation call, or an opaque argument
/// escape — excluding occurrences that are locally shadowed. The check the call-target
/// evidence and content-keyed seeding layers gate on: a mutated binding's content is
/// not the `def`'s body (coevo series 6, S2-B).
pub fn module_binding_mutated_in_file(
    il: &Il,
    interner: &Interner,
    name: Symbol,
    local_scope_nodes: &[bool],
    top_level: &[NodeId],
) -> bool {
    let shadowed =
        shadowed_js_like_module_binding_nodes_for_symbol_in_scope(il, name, local_scope_nodes);
    let refers = |node: NodeId| -> bool {
        node_symbol_in_scope(il, node, local_scope_nodes).is_some_and(|symbol| symbol == name)
    };
    fn contains(il: &Il, node: NodeId, refers: &dyn Fn(NodeId) -> bool) -> bool {
        refers(node) || il.children(node).iter().any(|&c| contains(il, c, refers))
    }
    il.nodes.iter().enumerate().any(|(idx, node)| {
        let node_id = NodeId(idx as u32);
        if shadowed.contains(&node_id) {
            return false;
        }
        match node.kind {
            NodeKind::Call => {
                if let Some(receiver) = receiver_mutation_call_receiver(il, interner, node_id) {
                    return refers(receiver);
                }
                if let Some(args) = opaque_argument_escape_args(il, node_id) {
                    return args.iter().any(|&arg| contains(il, arg, &refers));
                }
                false
            }
            NodeKind::Assign if !top_level.contains(&node_id) => {
                match binding_write_target(il, node_id) {
                    Some(lhs) => contains(il, lhs, &refers),
                    None => false,
                }
            }
            _ => false,
        }
    })
}

pub fn top_level_statements_for(il: &Il) -> Vec<NodeId> {
    let mut out = Vec::new();
    for &stmt in il.children(il.root) {
        if il.kind(stmt) == NodeKind::Block {
            out.extend(il.children(stmt).iter().copied());
        } else {
            out.push(stmt);
        }
    }
    out
}

pub fn assignment_name_in(il: &Il, stmt: NodeId) -> Option<Symbol> {
    let local_scope = local_scope_nodes(il);
    assignment_name_in_scope(il, stmt, &local_scope)
}

pub(crate) fn assignment_name_in_scope(
    il: &Il,
    stmt: NodeId,
    local_scope: &[bool],
) -> Option<Symbol> {
    let (lhs, _) = il.assignment_var_parts(stmt)?;
    if !cid_is_module_scoped(lhs, local_scope) {
        return None;
    }
    let cid = il.var_cid(lhs)?;
    il.cid_names.get(cid as usize).copied()
}

pub fn collect_all_node_symbols(il: &Il, node: NodeId, out: &mut FxHashSet<Symbol>) {
    let local_scope = local_scope_nodes(il);
    collect_all_node_symbols_in_scope(il, node, &local_scope, out);
}

pub(crate) fn collect_all_node_symbols_in_scope(
    il: &Il,
    node: NodeId,
    local_scope: &[bool],
    out: &mut FxHashSet<Symbol>,
) {
    if let Some(symbol) = node_symbol_in_scope(il, node, local_scope) {
        out.insert(symbol);
    }
    for &child in il.children(node) {
        collect_all_node_symbols_in_scope(il, child, local_scope, out);
    }
}

pub fn collect_module_mutations(
    il: &Il,
    interner: &Interner,
    candidates: &FxHashSet<Symbol>,
    is_top_level: &[bool],
) -> FxHashSet<Symbol> {
    let local_scope = local_scope_nodes(il);
    collect_module_mutations_in_scope(il, interner, candidates, is_top_level, &local_scope)
}

pub(crate) fn collect_module_mutations_in_scope(
    il: &Il,
    interner: &Interner,
    candidates: &FxHashSet<Symbol>,
    is_top_level: &[bool],
    local_scope: &[bool],
) -> FxHashSet<Symbol> {
    let direct_definitions = direct_assignment_definitions_in_scope(il, is_top_level, local_scope);
    collect_module_mutations_in_scope_with_direct_definitions(
        il,
        interner,
        candidates,
        is_top_level,
        local_scope,
        &direct_definitions,
    )
}

pub(crate) fn collect_module_mutations_in_scope_with_direct_definitions(
    il: &Il,
    interner: &Interner,
    candidates: &FxHashSet<Symbol>,
    is_top_level: &[bool],
    local_scope: &[bool],
    direct_definitions: &FxHashSet<NodeId>,
) -> FxHashSet<Symbol> {
    let mut mutated = FxHashSet::default();
    if candidates.is_empty() {
        return mutated;
    }
    let shadowed = shadowed_js_like_module_binding_nodes(il, candidates, local_scope);
    for (idx, node) in il.nodes.iter().enumerate() {
        let node_id = NodeId(idx as u32);
        match node.kind {
            NodeKind::Call => {
                if let Some(receiver) = receiver_mutation_call_receiver(il, interner, node_id) {
                    mark_direct_symbol(
                        il,
                        receiver,
                        candidates,
                        &shadowed,
                        local_scope,
                        &mut mutated,
                    );
                }
                if let Some(args) = opaque_argument_escape_args(il, node_id) {
                    for &arg in args {
                        collect_unshadowed_node_symbols(
                            il,
                            arg,
                            candidates,
                            &shadowed,
                            local_scope,
                            &mut mutated,
                        );
                    }
                }
            }
            NodeKind::Assign => {
                if let Some(lhs) = binding_write_target(il, node_id) {
                    let direct_top_level_definition =
                        is_top_level.get(idx).copied().unwrap_or(false)
                            && direct_definitions.contains(&node_id);
                    if !direct_top_level_definition {
                        collect_unshadowed_node_symbols(
                            il,
                            lhs,
                            candidates,
                            &shadowed,
                            local_scope,
                            &mut mutated,
                        );
                    }
                }
            }
            _ => {}
        }
    }
    mutated
}

fn direct_assignment_definitions_in_scope(
    il: &Il,
    is_top_level: &[bool],
    local_scope: &[bool],
) -> FxHashSet<NodeId> {
    il.nodes
        .iter()
        .enumerate()
        .filter_map(|(idx, _)| {
            let node = NodeId(idx as u32);
            (is_top_level.get(idx).copied().unwrap_or(false)
                && assignment_name_in_scope(il, node, local_scope).is_some())
            .then_some(node)
        })
        .collect()
}

pub fn shadowed_js_like_module_binding_nodes_for_symbol(
    il: &Il,
    name: Symbol,
) -> FxHashSet<NodeId> {
    let local_scope = local_scope_nodes(il);
    shadowed_js_like_module_binding_nodes_for_symbol_in_scope(il, name, &local_scope)
}

pub(crate) fn shadowed_js_like_module_binding_nodes_for_symbol_in_scope(
    il: &Il,
    name: Symbol,
    local_scope: &[bool],
) -> FxHashSet<NodeId> {
    let mut candidates = FxHashSet::default();
    candidates.insert(name);
    shadowed_js_like_module_binding_nodes(il, &candidates, local_scope)
        .into_iter()
        .filter_map(|(node, symbols)| symbols.contains(&name).then_some(node))
        .collect()
}

fn mark_direct_symbol(
    il: &Il,
    node: NodeId,
    candidates: &FxHashSet<Symbol>,
    shadowed: &FxHashMap<NodeId, FxHashSet<Symbol>>,
    local_scope: &[bool],
    out: &mut FxHashSet<Symbol>,
) {
    if let Some(symbol) = node_symbol_in_scope(il, node, local_scope) {
        if candidates.contains(&symbol)
            && !shadowed
                .get(&node)
                .is_some_and(|symbols| symbols.contains(&symbol))
        {
            out.insert(symbol);
        }
    }
}

fn collect_unshadowed_node_symbols(
    il: &Il,
    node: NodeId,
    candidates: &FxHashSet<Symbol>,
    shadowed: &FxHashMap<NodeId, FxHashSet<Symbol>>,
    local_scope: &[bool],
    out: &mut FxHashSet<Symbol>,
) {
    mark_direct_symbol(il, node, candidates, shadowed, local_scope, out);
    for &child in il.children(node) {
        collect_unshadowed_node_symbols(il, child, candidates, shadowed, local_scope, out);
    }
}

fn shadowed_js_like_module_binding_nodes(
    il: &Il,
    candidates: &FxHashSet<Symbol>,
    local_scope: &[bool],
) -> FxHashMap<NodeId, FxHashSet<Symbol>> {
    let mut out = FxHashMap::default();
    if candidates.is_empty()
        || !semantics(il.meta.lang)
            .modules()
            .js_like_shadowed_module_bindings()
    {
        return out;
    }
    collect_shadowed_js_like_module_binding_nodes(
        il,
        il.root,
        candidates,
        &FxHashSet::default(),
        local_scope,
        &mut out,
    );
    out
}

fn collect_shadowed_js_like_module_binding_nodes(
    il: &Il,
    node: NodeId,
    candidates: &FxHashSet<Symbol>,
    inherited: &FxHashSet<Symbol>,
    local_scope: &[bool],
    out: &mut FxHashMap<NodeId, FxHashSet<Symbol>>,
) {
    let mut shadowed = inherited.clone();
    if matches!(il.kind(node), NodeKind::Func | NodeKind::Lambda) {
        for &child in il.children(node) {
            if il.kind(child) != NodeKind::Param {
                continue;
            }
            if let Some(symbol) = node_symbol_in_scope(il, child, local_scope) {
                if candidates.contains(&symbol) {
                    shadowed.insert(symbol);
                }
            }
        }
    }
    if !shadowed.is_empty() {
        out.insert(node, shadowed.clone());
    }
    for &child in il.children(node) {
        collect_shadowed_js_like_module_binding_nodes(
            il,
            child,
            candidates,
            &shadowed,
            local_scope,
            out,
        );
    }
}

#[cfg(test)]
fn node_symbol_in(il: &Il, node: NodeId) -> Option<Symbol> {
    let local_scope = local_scope_nodes(il);
    node_symbol_in_scope(il, node, &local_scope)
}

pub(crate) fn node_symbol_in_scope(il: &Il, node: NodeId, local_scope: &[bool]) -> Option<Symbol> {
    match il.node(node).payload {
        Payload::Name(symbol) => Some(symbol),
        Payload::Cid(cid) if cid_is_module_scoped(node, local_scope) => {
            il.cid_names.get(cid as usize).copied()
        }
        _ => None,
    }
}

pub(crate) fn local_scope_nodes(il: &Il) -> Vec<bool> {
    let mut local_scope = vec![false; il.nodes.len()];
    mark_local_scope_nodes(il, il.root, false, &mut local_scope);
    local_scope
}

fn mark_local_scope_nodes(il: &Il, node: NodeId, in_local_scope: bool, out: &mut [bool]) {
    if let Some(slot) = out.get_mut(node.0 as usize) {
        *slot = in_local_scope;
    }
    let child_local_scope =
        in_local_scope || matches!(il.kind(node), NodeKind::Func | NodeKind::Lambda);
    for &child in il.children(node) {
        mark_local_scope_nodes(il, child, child_local_scope, out);
    }
}

fn cid_is_module_scoped(node: NodeId, local_scope: &[bool]) -> bool {
    !local_scope.get(node.0 as usize).copied().unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nose_il::{FileId, FileMeta, IlBuilder, Lang, Span, Unit, UnitKind};

    fn sp(line: u32) -> Span {
        Span::new(FileId(0), line, line, line, line)
    }

    struct CidFixture {
        il: Il,
        interner: Interner,
        arr: Symbol,
        top_level_arr: NodeId,
        function_param: NodeId,
    }

    fn cid_reuse_fixture() -> CidFixture {
        let interner = Interner::new();
        let arr = interner.intern("arr");
        let push = interner.intern("push");
        let grow = interner.intern("grow");

        let mut b = IlBuilder::new(FileId(0));
        let top_level_arr = b.add(NodeKind::Var, Payload::Cid(0), sp(1), &[]);
        let empty_array = b.add(NodeKind::Seq, Payload::None, sp(1), &[]);
        let assign_arr = b.add(
            NodeKind::Assign,
            Payload::None,
            sp(1),
            &[top_level_arr, empty_array],
        );

        let function_param = b.add(NodeKind::Param, Payload::Cid(0), sp(2), &[]);
        let receiver = b.add(NodeKind::Var, Payload::Name(arr), sp(3), &[]);
        let field = b.add(NodeKind::Field, Payload::Name(push), sp(3), &[receiver]);
        let arg = b.add(NodeKind::Var, Payload::Cid(0), sp(3), &[]);
        let call = b.add(NodeKind::Call, Payload::None, sp(3), &[field, arg]);
        let expr = b.add(NodeKind::ExprStmt, Payload::None, sp(3), &[call]);
        let body = b.add(NodeKind::Block, Payload::None, sp(3), &[expr]);
        let func = b.add(
            NodeKind::Func,
            Payload::None,
            sp(2),
            &[function_param, body],
        );
        let module = b.add(NodeKind::Module, Payload::None, sp(1), &[assign_arr, func]);

        let mut il = b.finish(
            module,
            FileMeta {
                path: "t.js".to_string(),
                lang: Lang::JavaScript,
            },
            vec![Unit {
                root: func,
                kind: UnitKind::Function,
                name: Some(grow),
            }],
            vec![arr],
        );
        crate::effect_evidence::run(&mut il, &interner);
        CidFixture {
            il,
            interner,
            arr,
            top_level_arr,
            function_param,
        }
    }

    #[test]
    fn node_symbol_does_not_resolve_function_local_cid_through_global_cid_names() {
        let fixture = cid_reuse_fixture();
        assert_eq!(
            node_symbol_in(&fixture.il, fixture.top_level_arr),
            Some(fixture.arr)
        );
        assert_eq!(node_symbol_in(&fixture.il, fixture.function_param), None);
    }

    #[test]
    fn function_param_cid_reuse_does_not_hide_module_binding_mutation() {
        let fixture = cid_reuse_fixture();
        let mut candidates = FxHashSet::default();
        candidates.insert(fixture.arr);
        let top_level = top_level_statements_for(&fixture.il);
        let mut is_top_level = vec![false; fixture.il.nodes.len()];
        for stmt in top_level {
            is_top_level[stmt.0 as usize] = true;
        }
        let mutated =
            collect_module_mutations(&fixture.il, &fixture.interner, &candidates, &is_top_level);
        assert!(mutated.contains(&fixture.arr));
    }

    #[test]
    fn top_level_place_assignment_marks_module_binding_mutated() {
        let interner = Interner::new();
        let arr = interner.intern("arr");
        let mut b = IlBuilder::new(FileId(0));
        let arr_def = b.add(NodeKind::Var, Payload::Cid(0), sp(1), &[]);
        let empty_array = b.add(NodeKind::Seq, Payload::None, sp(1), &[]);
        let assign_arr = b.add(
            NodeKind::Assign,
            Payload::None,
            sp(1),
            &[arr_def, empty_array],
        );

        let arr_ref = b.add(NodeKind::Var, Payload::Name(arr), sp(2), &[]);
        let index = b.add(NodeKind::Lit, Payload::LitInt(0), sp(2), &[]);
        let place = b.add(NodeKind::Index, Payload::None, sp(2), &[arr_ref, index]);
        let value = b.add(NodeKind::Lit, Payload::LitInt(9), sp(2), &[]);
        let write = b.add(NodeKind::Assign, Payload::None, sp(2), &[place, value]);
        let module = b.add(NodeKind::Module, Payload::None, sp(1), &[assign_arr, write]);
        let mut il = b.finish(
            module,
            FileMeta {
                path: "t.js".to_string(),
                lang: Lang::JavaScript,
            },
            Vec::new(),
            vec![arr],
        );
        crate::effect_evidence::run(&mut il, &interner);

        let mut candidates = FxHashSet::default();
        candidates.insert(arr);
        let top_level = top_level_statements_for(&il);
        let mut is_top_level = vec![false; il.nodes.len()];
        for stmt in top_level {
            is_top_level[stmt.0 as usize] = true;
        }
        let mutated = collect_module_mutations(&il, &interner, &candidates, &is_top_level);
        assert!(mutated.contains(&arr));
    }
}
