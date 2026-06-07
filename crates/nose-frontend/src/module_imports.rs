//! Corpus-level import proof facts that need more than one lowered file.
//!
//! Frontends lower a static import as `local = import_binding(module, exported)`.
//! Once the whole corpus is available, a sibling module can prove that this binding
//! names a single immutable literal value. In that narrow case we replace the import
//! fact RHS with a cloned literal subtree, so the existing per-file value-graph
//! module-binding seed can reuse its mutation and canonicalization logic.

use nose_il::{
    stable_symbol_hash, Il, Interner, Node, NodeId, NodeKind, Payload, Span, Symbol, UnitKind,
};
use nose_semantics::{
    java_map_entry_contract, java_map_factory_contract, semantics, ImportedMapFactoryContract,
    JavaMapFactoryKind,
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
    let exports = collect_literal_exports(files, interner);
    if exports.is_empty() {
        return;
    }

    let replacements: Vec<Vec<(NodeId, SubtreeSnapshot)>> = files
        .iter()
        .enumerate()
        .map(|(file_idx, il)| {
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

fn collect_literal_exports(
    files: &[Il],
    interner: &Interner,
) -> FxHashMap<(u64, u64), ExportedBinding> {
    let mut exports = FxHashMap::default();
    let mut ambiguous = FxHashSet::default();
    for (file_idx, il) in files.iter().enumerate() {
        let module_hashes = file_module_hashes(il);
        if !module_hashes.is_empty() {
            let top_level = collect_top_level_statements(il);
            collect_statement_exports(
                il,
                interner,
                file_idx,
                &top_level,
                &module_hashes,
                &mut exports,
                &mut ambiguous,
            );
        }

        if !semantics(il.meta.lang)
            .modules()
            .java_class_literal_exports()
        {
            continue;
        }
        for unit in &il.units {
            if unit.kind != UnitKind::Class {
                continue;
            }
            let Some(class_name) = unit.name else {
                continue;
            };
            let class_module_hashes = java_class_module_hashes(il, interner, class_name);
            if class_module_hashes.is_empty() {
                continue;
            }
            let statements = collect_statements_for_root(il, unit.root);
            collect_statement_exports(
                il,
                interner,
                file_idx,
                &statements,
                &class_module_hashes,
                &mut exports,
                &mut ambiguous,
            );
        }
    }
    for key in ambiguous {
        exports.remove(&key);
    }
    exports
}

fn collect_statement_exports(
    il: &Il,
    interner: &Interner,
    file_idx: usize,
    statements: &[NodeId],
    module_hashes: &[u64],
    exports: &mut FxHashMap<(u64, u64), ExportedBinding>,
    ambiguous: &mut FxHashSet<(u64, u64)>,
) {
    let mut counts: FxHashMap<Symbol, usize> = FxHashMap::default();
    for &stmt in statements {
        if let Some(name) = assignment_name(il, stmt) {
            *counts.entry(name).or_insert(0) += 1;
        }
    }
    for &stmt in statements {
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
        if !imported_literal_export_safe(il, interner, rhs) {
            continue;
        }
        let exported = stable_symbol_hash(interner.resolve(name));
        for &module in module_hashes {
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

fn collect_top_level_statements(il: &Il) -> Vec<NodeId> {
    let class_roots: FxHashSet<NodeId> = il
        .units
        .iter()
        .filter_map(|unit| (unit.kind == UnitKind::Class).then_some(unit.root))
        .collect();
    collect_statements_for_root_except(il, il.root, &class_roots)
}

fn collect_statements_for_root(il: &Il, root: NodeId) -> Vec<NodeId> {
    collect_statements_for_root_except(il, root, &FxHashSet::default())
}

fn collect_statements_for_root_except(
    il: &Il,
    root: NodeId,
    non_flattened_blocks: &FxHashSet<NodeId>,
) -> Vec<NodeId> {
    il.children(root)
        .iter()
        .copied()
        .fold(Vec::new(), |mut statements, node| {
            match il.kind(node) {
                NodeKind::Block if non_flattened_blocks.contains(&node) => statements.push(node),
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

fn imported_literal_export_safe(il: &Il, interner: &Interner, node: NodeId) -> bool {
    match il.kind(node) {
        NodeKind::Seq => {
            let Payload::Name(tag) = il.node(node).payload else {
                return false;
            };
            if !nose_semantics::imported_literal_seq_tag_safe(interner.resolve(tag)) {
                return false;
            }
            il.children(node)
                .iter()
                .all(|&child| literal_export_value_safe(il, interner, child))
        }
        NodeKind::Call => imported_map_factory_call_safe(il, interner, node),
        _ => false,
    }
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
        NodeKind::Call => java_map_entry_call_safe(il, interner, node),
        _ => false,
    }
}

fn imported_map_factory_call_safe(il: &Il, interner: &Interner, call: NodeId) -> bool {
    match semantics(il.meta.lang).stdlib().imported_map_factory() {
        Some(ImportedMapFactoryContract::JavaMap) => java_map_factory_call_safe(il, interner, call),
        Some(ImportedMapFactoryContract::RustStdMap) => {
            rust_std_map_factory_call_safe(il, interner, call)
        }
        None => false,
    }
}

fn java_map_factory_call_safe(il: &Il, interner: &Interner, call: NodeId) -> bool {
    let kids = il.children(call);
    let Some((&callee, args)) = kids.split_first() else {
        return false;
    };
    let Some(method) = field_method_on_var(il, interner, callee, "Map") else {
        return false;
    };
    let Some(contract) = java_map_factory_contract(il.meta.lang, "Map", method) else {
        return false;
    };
    if java_file_defines_type_name(il, interner, contract.receiver) {
        return false;
    }
    match contract.kind {
        JavaMapFactoryKind::Of => {
            args.len() % 2 == 0
                && args
                    .iter()
                    .all(|&arg| literal_export_value_safe(il, interner, arg))
        }
        JavaMapFactoryKind::OfEntries => args
            .iter()
            .all(|&arg| java_map_entry_call_safe(il, interner, arg)),
    }
}

fn java_map_entry_call_safe(il: &Il, interner: &Interner, call: NodeId) -> bool {
    if il.kind(call) != NodeKind::Call {
        return false;
    }
    let kids = il.children(call);
    if kids.len() != 3 {
        return false;
    }
    let Some(method) = field_method_on_var(il, interner, kids[0], "Map") else {
        return false;
    };
    if !java_map_entry_contract(il.meta.lang, "Map", method) {
        return false;
    }
    if java_file_defines_type_name(il, interner, "Map") {
        return false;
    }
    literal_export_value_safe(il, interner, kids[1])
        && literal_export_value_safe(il, interner, kids[2])
}

fn rust_std_map_factory_call_safe(il: &Il, interner: &Interner, call: NodeId) -> bool {
    let kids = il.children(call);
    if kids.len() != 2 {
        return false;
    }
    let Some(name) = var_text(il, interner, kids[0]) else {
        return false;
    };
    let factory = semantics(il.meta.lang)
        .collections()
        .free_name_map_factories()
        .find(|factory| factory.names.contains(&name));
    if factory.is_none() || !free_name_factory_shadow_safe(il, interner, name, false) {
        return false;
    }
    literal_export_value_safe(il, interner, kids[1])
}

fn field_method_on_var<'a>(
    il: &Il,
    interner: &'a Interner,
    node: NodeId,
    receiver: &str,
) -> Option<&'a str> {
    if il.kind(node) != NodeKind::Field {
        return None;
    }
    let Payload::Name(method) = il.node(node).payload else {
        return None;
    };
    let receiver_node = il.children(node).first().copied()?;
    var_named(il, interner, receiver_node, receiver).then(|| interner.resolve(method))
}

fn var_named(il: &Il, interner: &Interner, node: NodeId, expected: &str) -> bool {
    var_text(il, interner, node).is_some_and(|name| name == expected)
}

fn var_text<'a>(il: &Il, interner: &'a Interner, node: NodeId) -> Option<&'a str> {
    if il.kind(node) != NodeKind::Var {
        return None;
    }
    let Payload::Name(name) = il.node(node).payload else {
        return None;
    };
    Some(interner.resolve(name))
}

fn free_name_factory_shadow_safe(
    il: &Il,
    interner: &Interner,
    name: &str,
    shadow_guard: bool,
) -> bool {
    if shadow_guard {
        return !file_defines_name(il, interner, name);
    }
    if il.meta.lang == nose_il::Lang::Rust && name.starts_with("std::") {
        return !file_defines_name(il, interner, "std");
    }
    true
}

fn file_defines_name(il: &Il, interner: &Interner, expected: &str) -> bool {
    collect_top_level_statements(il).iter().any(|&stmt| {
        assignment_name(il, stmt).is_some_and(|symbol| interner.resolve(symbol) == expected)
    }) || il.units.iter().any(|unit| {
        unit.name
            .is_some_and(|symbol| interner.resolve(symbol) == expected)
    }) || il.nodes.iter().any(|node| {
        matches!(node.kind, NodeKind::Module | NodeKind::Block)
            && matches!(node.payload, Payload::Name(symbol) if interner.resolve(symbol) == expected)
    })
}

fn java_file_defines_type_name(il: &Il, interner: &Interner, expected: &str) -> bool {
    il.units.iter().any(|unit| {
        unit.kind == UnitKind::Class
            && unit
                .name
                .is_some_and(|name| interner.resolve(name) == expected)
    })
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
    if !nose_semantics::mutating_method_name(interner.resolve(method)) {
        return false;
    }
    il.children(field)
        .first()
        .is_some_and(|&receiver| node_refers_to_symbol(il, receiver, name))
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

fn file_module_hashes(il: &Il) -> Vec<u64> {
    let Some(spec) = semantics(il.meta.lang).modules().path_spec() else {
        return Vec::new();
    };
    let mut hashes = module_hashes_from_path(
        &il.meta.path,
        spec.extensions,
        spec.separator,
        spec.include_relative_dot,
        spec.drop_init_file,
    );
    if spec.rust_crate_self_aliases {
        for module in module_names_from_path(
            &il.meta.path,
            spec.extensions,
            spec.separator,
            spec.drop_init_file,
        ) {
            hashes.push(stable_symbol_hash(&format!("crate::{module}")));
            hashes.push(stable_symbol_hash(&format!("self::{module}")));
        }
    }
    dedupe_hashes(hashes)
}

fn java_class_module_hashes(il: &Il, interner: &Interner, class_name: Symbol) -> Vec<u64> {
    let class_name = interner.resolve(class_name);
    let mut hashes = vec![stable_symbol_hash(class_name)];
    if let Some(mut parts) = path_parts_without_extension(&il.meta.path, &["java"]) {
        if let Some(last) = parts.last_mut() {
            *last = class_name.to_string();
        }
        for module in suffix_module_names(&parts, ".") {
            hashes.push(stable_symbol_hash(&module));
        }
    }
    dedupe_hashes(hashes)
}

fn module_hashes_from_path(
    path: &str,
    extensions: &[&str],
    separator: &str,
    include_relative_dot: bool,
    drop_python_init: bool,
) -> Vec<u64> {
    let hashes = module_names_from_path(path, extensions, separator, drop_python_init)
        .into_iter()
        .flat_map(|module| {
            if include_relative_dot {
                vec![
                    stable_symbol_hash(&module),
                    stable_symbol_hash(&format!("./{module}")),
                ]
            } else {
                vec![stable_symbol_hash(&module)]
            }
        })
        .collect::<Vec<_>>();
    dedupe_hashes(hashes)
}

fn module_names_from_path(
    path: &str,
    extensions: &[&str],
    separator: &str,
    drop_python_init: bool,
) -> Vec<String> {
    let Some(mut parts) = path_parts_without_extension(path, extensions) else {
        return Vec::new();
    };
    if drop_python_init && parts.last().is_some_and(|part| part == "__init__") {
        parts.pop();
    }
    suffix_module_names(&parts, separator)
}

fn path_parts_without_extension(path: &str, extensions: &[&str]) -> Option<Vec<String>> {
    let path = Path::new(path);
    let ext = path.extension().and_then(|ext| ext.to_str())?;
    if !extensions.contains(&ext) {
        return None;
    }
    let mut parts: Vec<String> = path
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .filter(|part| !part.is_empty() && *part != "/")
        .map(ToOwned::to_owned)
        .collect();
    let last = parts.last_mut()?;
    let stem = Path::new(last)
        .file_stem()
        .and_then(|stem| stem.to_str())?
        .to_string();
    *last = stem;
    Some(parts)
}

fn suffix_module_names(parts: &[String], separator: &str) -> Vec<String> {
    let mut out = Vec::new();
    for start in 0..parts.len() {
        let module = parts[start..].join(separator);
        if !module.is_empty() {
            out.push(module);
        }
    }
    out
}

fn dedupe_hashes(hashes: Vec<u64>) -> Vec<u64> {
    let mut seen = FxHashSet::default();
    hashes
        .into_iter()
        .filter(|hash| seen.insert(*hash))
        .collect()
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
