//! Corpus-level import proof facts that need more than one lowered file.
//!
//! Frontends lower a static import as `local = import_binding(module, exported)`.
//! Once the whole corpus is available, a sibling Python module can prove that this
//! binding names a single immutable literal value. In that narrow case we replace
//! the import fact RHS with a cloned literal subtree, so the existing per-file
//! value-graph module-binding seed can reuse its mutation and canonicalization logic.

use nose_il::{
    stable_symbol_hash, Il, Interner, Lang, Node, NodeId, NodeKind, Payload, Span, Symbol,
};
use rustc_hash::{FxHashMap, FxHashSet};
use std::path::Path;

#[derive(Clone, Copy)]
struct ExportedBinding {
    file_idx: usize,
    rhs: NodeId,
}

#[derive(Clone)]
struct SnapshotNode {
    kind: NodeKind,
    payload: Payload,
    span: Span,
    children: Vec<usize>,
}

#[derive(Clone)]
struct SubtreeSnapshot {
    nodes: Vec<SnapshotNode>,
    root: usize,
}

pub(crate) fn resolve_imported_immutable_bindings(files: &mut [Il], interner: &Interner) {
    let exports = collect_python_literal_exports(files, interner);
    if exports.is_empty() {
        return;
    }

    let replacements: Vec<Vec<(NodeId, SubtreeSnapshot)>> = files
        .iter()
        .enumerate()
        .map(|(file_idx, il)| {
            if il.meta.lang != Lang::Python {
                return Vec::new();
            }
            collect_top_level_statements(il)
                .into_iter()
                .filter_map(|stmt| {
                    let key = import_binding_key(il, interner, stmt)?;
                    let export = exports.get(&key)?;
                    if export.file_idx == file_idx {
                        return None;
                    }
                    Some((stmt, snapshot_subtree(&files[export.file_idx], export.rhs)))
                })
                .collect()
        })
        .collect();

    for (file_idx, file_replacements) in replacements.into_iter().enumerate() {
        for (stmt, snapshot) in file_replacements {
            let rhs = append_snapshot(&mut files[file_idx], &snapshot);
            replace_assignment_rhs(&mut files[file_idx], stmt, rhs);
        }
    }
}

fn collect_python_literal_exports(
    files: &[Il],
    interner: &Interner,
) -> FxHashMap<(u64, u64), ExportedBinding> {
    let mut exports = FxHashMap::default();
    let mut ambiguous = FxHashSet::default();
    for (file_idx, il) in files.iter().enumerate() {
        if il.meta.lang != Lang::Python {
            continue;
        }
        let module_hashes = python_module_hashes(&il.meta.path);
        if module_hashes.is_empty() {
            continue;
        }
        let top_level = collect_top_level_statements(il);
        let mut counts: FxHashMap<Symbol, usize> = FxHashMap::default();
        for &stmt in &top_level {
            if let Some(name) = assignment_name(il, stmt) {
                *counts.entry(name).or_insert(0) += 1;
            }
        }
        for &stmt in &top_level {
            let Some(name) = assignment_name(il, stmt) else {
                continue;
            };
            if counts.get(&name).copied().unwrap_or(0) != 1 {
                continue;
            }
            if binding_mutated(il, interner, name, stmt) {
                continue;
            }
            let Some(rhs) = assignment_rhs(il, stmt) else {
                continue;
            };
            if !literal_map_export_safe(il, interner, rhs) {
                continue;
            }
            let exported = stable_symbol_hash(interner.resolve(name));
            for &module in &module_hashes {
                let key = (module, exported);
                if exports
                    .insert(key, ExportedBinding { file_idx, rhs })
                    .is_some()
                {
                    ambiguous.insert(key);
                }
            }
        }
    }
    for key in ambiguous {
        exports.remove(&key);
    }
    exports
}

fn collect_top_level_statements(il: &Il) -> Vec<NodeId> {
    il.children(il.root)
        .iter()
        .copied()
        .fold(Vec::new(), |mut statements, node| {
            match il.kind(node) {
                NodeKind::Block => statements.extend_from_slice(il.children(node)),
                _ => statements.push(node),
            }
            statements
        })
}

fn assignment_name(il: &Il, stmt: NodeId) -> Option<Symbol> {
    if il.kind(stmt) != NodeKind::Assign {
        return None;
    }
    let kids = il.children(stmt);
    if kids.len() != 2 || il.kind(kids[0]) != NodeKind::Var {
        return None;
    }
    match il.node(kids[0]).payload {
        Payload::Name(name) => Some(name),
        _ => None,
    }
}

fn assignment_rhs(il: &Il, stmt: NodeId) -> Option<NodeId> {
    (il.kind(stmt) == NodeKind::Assign)
        .then(|| il.children(stmt))
        .and_then(|kids| (kids.len() == 2).then_some(kids[1]))
}

fn import_binding_key(il: &Il, interner: &Interner, stmt: NodeId) -> Option<(u64, u64)> {
    let rhs = assignment_rhs(il, stmt)?;
    if il.kind(rhs) != NodeKind::Seq {
        return None;
    }
    let Payload::Name(tag) = il.node(rhs).payload else {
        return None;
    };
    if interner.resolve(tag) != "import_binding" {
        return None;
    }
    let kids = il.children(rhs);
    if kids.len() != 2 {
        return None;
    }
    Some((
        literal_string_hash(il, kids[0])?,
        literal_string_hash(il, kids[1])?,
    ))
}

fn literal_string_hash(il: &Il, node: NodeId) -> Option<u64> {
    match il.node(node).payload {
        Payload::LitStr(hash) => Some(hash),
        _ => None,
    }
}

fn literal_map_export_safe(il: &Il, interner: &Interner, node: NodeId) -> bool {
    if il.kind(node) != NodeKind::Seq {
        return false;
    }
    let Payload::Name(tag) = il.node(node).payload else {
        return false;
    };
    if interner.resolve(tag) != "dictionary" {
        return false;
    }
    il.children(node)
        .iter()
        .all(|&child| literal_export_value_safe(il, interner, child))
}

fn literal_export_value_safe(il: &Il, interner: &Interner, node: NodeId) -> bool {
    match il.kind(node) {
        NodeKind::Lit => true,
        NodeKind::Seq => {
            if let Payload::Name(tag) = il.node(node).payload {
                if matches!(interner.resolve(tag), "import_binding" | "import_namespace") {
                    return false;
                }
            }
            il.children(node)
                .iter()
                .all(|&child| literal_export_value_safe(il, interner, child))
        }
        NodeKind::UnOp => il
            .children(node)
            .iter()
            .all(|&child| literal_export_value_safe(il, interner, child)),
        _ => false,
    }
}

fn binding_mutated(il: &Il, interner: &Interner, name: Symbol, defining_stmt: NodeId) -> bool {
    il.nodes.iter().enumerate().any(|(idx, node)| {
        let node_id = NodeId(idx as u32);
        if node_id == defining_stmt {
            return false;
        }
        match node.kind {
            NodeKind::Assign => il
                .children(node_id)
                .first()
                .is_some_and(|&lhs| node_contains_symbol(il, lhs, name)),
            NodeKind::Field => field_mutates_binding(il, interner, node_id, name),
            _ => false,
        }
    })
}

fn field_mutates_binding(il: &Il, interner: &Interner, field: NodeId, name: Symbol) -> bool {
    let Payload::Name(method) = il.node(field).payload else {
        return false;
    };
    if !mutating_method_name(interner.resolve(method)) {
        return false;
    }
    il.children(field)
        .first()
        .is_some_and(|&receiver| node_refers_to_symbol(il, receiver, name))
}

fn mutating_method_name(method: &str) -> bool {
    matches!(
        method,
        "clear" | "pop" | "popitem" | "setdefault" | "update"
    )
}

fn node_refers_to_symbol(il: &Il, node: NodeId, name: Symbol) -> bool {
    match il.node(node).payload {
        Payload::Name(symbol) => symbol == name,
        _ => false,
    }
}

fn node_contains_symbol(il: &Il, node: NodeId, name: Symbol) -> bool {
    node_refers_to_symbol(il, node, name)
        || il
            .children(node)
            .iter()
            .any(|&child| node_contains_symbol(il, child, name))
}

fn python_module_hashes(path: &str) -> Vec<u64> {
    let path = Path::new(path);
    if path.extension().and_then(|ext| ext.to_str()) != Some("py") {
        return Vec::new();
    }
    let mut parts: Vec<String> = path
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .filter(|part| !part.is_empty())
        .map(ToOwned::to_owned)
        .collect();
    let Some(last) = parts.last_mut() else {
        return Vec::new();
    };
    if let Some(stripped) = last.strip_suffix(".py") {
        *last = stripped.to_string();
    }
    if last == "__init__" {
        parts.pop();
    }
    let mut out = Vec::new();
    for start in 0..parts.len() {
        let module = parts[start..].join(".");
        if !module.is_empty() {
            out.push(stable_symbol_hash(&module));
        }
    }
    out
}

fn snapshot_subtree(il: &Il, root: NodeId) -> SubtreeSnapshot {
    fn snapshot_node(il: &Il, node: NodeId, out: &mut Vec<SnapshotNode>) -> usize {
        let children: Vec<usize> = il
            .children(node)
            .iter()
            .map(|&child| snapshot_node(il, child, out))
            .collect();
        let idx = out.len();
        let node_ref = il.node(node);
        out.push(SnapshotNode {
            kind: node_ref.kind,
            payload: node_ref.payload,
            span: node_ref.span,
            children,
        });
        idx
    }

    let mut nodes = Vec::new();
    let root = snapshot_node(il, root, &mut nodes);
    SubtreeSnapshot { nodes, root }
}

fn append_snapshot(il: &mut Il, snapshot: &SubtreeSnapshot) -> NodeId {
    let mut new_ids = vec![NodeId(0); snapshot.nodes.len()];
    for (idx, snapshot_node) in snapshot.nodes.iter().enumerate() {
        let children: Vec<NodeId> = snapshot_node
            .children
            .iter()
            .map(|&child_idx| new_ids[child_idx])
            .collect();
        let child_start = il.edges.len() as u32;
        il.edges.extend_from_slice(&children);
        let mut span = snapshot_node.span;
        span.file = il.file;
        let id = NodeId(il.nodes.len() as u32);
        il.nodes.push(Node {
            kind: snapshot_node.kind,
            payload: snapshot_node.payload,
            span,
            child_start,
            child_len: children.len() as u32,
        });
        new_ids[idx] = id;
    }
    new_ids[snapshot.root]
}

fn replace_assignment_rhs(il: &mut Il, stmt: NodeId, rhs: NodeId) {
    let Some(node) = il.nodes.get(stmt.0 as usize) else {
        return;
    };
    if node.kind != NodeKind::Assign || node.child_len != 2 {
        return;
    }
    let rhs_slot = node.child_start as usize + 1;
    if let Some(slot) = il.edges.get_mut(rhs_slot) {
        *slot = rhs;
    }
}
