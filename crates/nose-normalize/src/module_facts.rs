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
    il.cid_names.get(cid as usize).copied()
}

pub fn collect_all_node_symbols(il: &Il, node: NodeId, out: &mut FxHashSet<Symbol>) {
    if let Some(symbol) = node_symbol_in(il, node) {
        out.insert(symbol);
    }
    for &child in il.children(node) {
        collect_all_node_symbols(il, child, out);
    }
}

pub fn collect_module_mutations(
    il: &Il,
    interner: &Interner,
    candidates: &FxHashSet<Symbol>,
    is_top_level: &[bool],
) -> FxHashSet<Symbol> {
    let mut mutated = FxHashSet::default();
    if candidates.is_empty() {
        return mutated;
    }
    let shadowed = shadowed_js_like_module_binding_nodes(il, candidates);
    for (idx, node) in il.nodes.iter().enumerate() {
        let node_id = NodeId(idx as u32);
        match node.kind {
            NodeKind::Call if matches!(node.payload, Payload::Builtin(Builtin::Append)) => {
                if let Some(receiver) = il.children(node_id).first().copied() {
                    mark_direct_symbol(il, receiver, candidates, &shadowed, &mut mutated);
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
                    mark_direct_symbol(il, receiver, candidates, &shadowed, &mut mutated);
                }
            }
            NodeKind::Assign if !is_top_level.get(idx).copied().unwrap_or(false) => {
                if let Some(lhs) = il.children(node_id).first().copied() {
                    collect_unshadowed_node_symbols(il, lhs, candidates, &shadowed, &mut mutated);
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
    let mut candidates = FxHashSet::default();
    candidates.insert(name);
    shadowed_js_like_module_binding_nodes(il, &candidates)
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
    out: &mut FxHashSet<Symbol>,
) {
    if let Some(symbol) = node_symbol_in(il, node) {
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
    out: &mut FxHashSet<Symbol>,
) {
    if let Some(symbol) = node_symbol_in(il, node) {
        if candidates.contains(&symbol)
            && !shadowed
                .get(&node)
                .is_some_and(|symbols| symbols.contains(&symbol))
        {
            out.insert(symbol);
        }
    }
    for &child in il.children(node) {
        collect_unshadowed_node_symbols(il, child, candidates, shadowed, out);
    }
}

fn shadowed_js_like_module_binding_nodes(
    il: &Il,
    candidates: &FxHashSet<Symbol>,
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
        &mut out,
    );
    out
}

fn collect_shadowed_js_like_module_binding_nodes(
    il: &Il,
    node: NodeId,
    candidates: &FxHashSet<Symbol>,
    inherited: &FxHashSet<Symbol>,
    out: &mut FxHashMap<NodeId, FxHashSet<Symbol>>,
) {
    let mut shadowed = inherited.clone();
    if matches!(il.kind(node), NodeKind::Func | NodeKind::Lambda) {
        for &child in il.children(node) {
            if il.kind(child) != NodeKind::Param {
                continue;
            }
            if let Some(symbol) = node_symbol_in(il, child) {
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
        collect_shadowed_js_like_module_binding_nodes(il, child, candidates, &shadowed, out);
    }
}

fn js_like_lang(lang: Lang) -> bool {
    matches!(
        lang,
        Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html
    )
}

fn node_symbol_in(il: &Il, node: NodeId) -> Option<Symbol> {
    match il.node(node).payload {
        Payload::Name(symbol) => Some(symbol),
        Payload::Cid(cid) => il.cid_names.get(cid as usize).copied(),
        _ => None,
    }
}
