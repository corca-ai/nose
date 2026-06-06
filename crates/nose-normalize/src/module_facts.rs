use nose_il::{Builtin, Il, Interner, Lang, NodeId, NodeKind, Payload, Symbol};
use rustc_hash::{FxHashMap, FxHashSet};

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
    if il.kind(stmt) != NodeKind::Assign {
        return None;
    }
    let kids = il.children(stmt);
    if kids.len() != 2 || il.kind(kids[0]) != NodeKind::Var {
        return None;
    }
    let Payload::Cid(cid) = il.node(kids[0]).payload else {
        return None;
    };
    if !cid_is_module_scoped(kids[0], local_scope) {
        return None;
    }
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
    let mut mutated = FxHashSet::default();
    if candidates.is_empty() {
        return mutated;
    }
    let shadowed = shadowed_js_like_module_binding_nodes(il, candidates, local_scope);
    for (idx, node) in il.nodes.iter().enumerate() {
        let node_id = NodeId(idx as u32);
        match node.kind {
            NodeKind::Call if matches!(node.payload, Payload::Builtin(Builtin::Append)) => {
                if let Some(receiver) = il.children(node_id).first().copied() {
                    mark_direct_symbol(
                        il,
                        receiver,
                        candidates,
                        &shadowed,
                        local_scope,
                        &mut mutated,
                    );
                }
            }
            NodeKind::Field => {
                let Payload::Name(method) = node.payload else {
                    continue;
                };
                if !mutating_method_name(interner.resolve(method)) {
                    continue;
                }
                if let Some(receiver) = il.children(node_id).first().copied() {
                    mark_direct_symbol(
                        il,
                        receiver,
                        candidates,
                        &shadowed,
                        local_scope,
                        &mut mutated,
                    );
                }
            }
            NodeKind::Assign if !is_top_level.get(idx).copied().unwrap_or(false) => {
                if let Some(lhs) = il.children(node_id).first().copied() {
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
            _ => {}
        }
    }
    mutated
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

pub fn mutating_method_name(method: &str) -> bool {
    matches!(
        method,
        "add"
            | "addAll"
            | "append"
            | "delete"
            | "clear"
            | "compute"
            | "computeIfAbsent"
            | "computeIfPresent"
            | "merge"
            | "pop"
            | "push"
            | "put"
            | "putAll"
            | "remove"
            | "removeAll"
            | "removeIf"
            | "replace"
            | "replaceAll"
            | "retainAll"
            | "shift"
            | "sort"
            | "splice"
            | "unshift"
            | "set"
    )
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
    if candidates.is_empty() || !js_like_lang(il.meta.lang) {
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

fn js_like_lang(lang: Lang) -> bool {
    matches!(
        lang,
        Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html
    )
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
    use nose_il::{FileId, FileMeta, IlBuilder, Span, Unit, UnitKind};

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

        let il = b.finish(
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
}
