//! Corpus-level import proof facts that need more than one lowered file.
//!
//! Frontends lower a static import as `local = import_binding(module, exported)`.
//! Once the whole corpus is available, a sibling module can prove that this binding
//! names a single immutable literal value. In that narrow case we replace the import
//! fact RHS with a cloned literal subtree, so the existing per-file value-graph
//! module-binding seed can reuse its mutation and canonicalization logic.

use nose_il::{
    stable_symbol_hash, EvidenceAnchor, EvidenceEmitter, EvidenceId, EvidenceKind,
    EvidenceProvenance, EvidenceRecord, EvidenceStatus, Il, ImportEvidenceKind, Interner, Node,
    NodeId, NodeKind, Payload, Span, Symbol, SymbolEvidenceKind, UnitKind,
};
use nose_semantics::{
    import_fact_rhs, java_map_entry_contract, java_map_factory_contract, semantics, ImportFactKind,
    ImportedMapFactoryContract, JavaMapFactoryKind, FIRST_PARTY_PACK_ID,
};
use rustc_hash::{FxHashMap, FxHashSet};
use std::path::Path;

#[derive(Clone)]
struct ExportedBinding {
    file_idx: usize,
    deps: Vec<SubtreeSnapshot>,
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

    let replacements: Vec<Vec<(NodeId, Vec<SubtreeSnapshot>, SubtreeSnapshot)>> = files
        .iter()
        .enumerate()
        .map(|(file_idx, il)| {
            collect_top_level_statements(il)
                .into_iter()
                .filter_map(|stmt| {
                    let local = assignment_name(il, stmt)?;
                    let key = import_binding_key(il, interner, stmt)?;
                    let export = exports.get(&key)?;
                    if export.file_idx == file_idx {
                        return None;
                    }
                    if binding_mutated(il, interner, local, stmt) {
                        return None;
                    }
                    Some((
                        stmt,
                        export.deps.clone(),
                        snapshot_subtree(&files[export.file_idx], export.rhs),
                    ))
                })
                .collect()
        })
        .collect();

    for (file_idx, file_replacements) in replacements.into_iter().enumerate() {
        for (stmt, deps, snapshot) in file_replacements {
            for dep in deps {
                let dep_stmt = append_snapshot(&mut files[file_idx], &dep);
                mirror_import_fact_evidence_for_assignment(
                    &mut files[file_idx],
                    interner,
                    dep_stmt,
                );
                prepend_root_statement(&mut files[file_idx], dep_stmt);
            }
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
        if exported_binding_unsafe(il, interner, name, stmt) {
            continue;
        }
        let Some(rhs) = assignment_rhs(il, stmt) else {
            continue;
        };
        if !imported_literal_export_safe(il, interner, rhs) {
            continue;
        }
        let exported = stable_symbol_hash(interner.resolve(name));
        let deps = import_dependency_snapshots(il, interner, rhs);
        for &module in module_hashes {
            let key = (module, exported);
            if exports
                .insert(
                    key,
                    ExportedBinding {
                        file_idx,
                        deps: deps.clone(),
                        rhs,
                    },
                )
                .is_some()
            {
                ambiguous.insert(key);
            }
        }
    }
}

fn import_dependency_snapshots(il: &Il, interner: &Interner, rhs: NodeId) -> Vec<SubtreeSnapshot> {
    collect_top_level_statements(il)
        .into_iter()
        .filter(|&stmt| {
            assignment_rhs(il, stmt).is_some_and(|dep_rhs| {
                import_binding_key(il, interner, stmt).is_some()
                    && il.kind(dep_rhs) == NodeKind::Seq
            })
        })
        .filter(|&stmt| {
            assignment_name(il, stmt).is_some_and(|name| node_contains_symbol(il, rhs, name))
        })
        .map(|stmt| snapshot_subtree(il, stmt))
        .collect()
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
    let fact = import_fact_rhs(il, interner, rhs)?;
    (fact.kind == ImportFactKind::Binding).then_some((fact.module_hash, fact.exported_hash?))
}

fn imported_literal_export_safe(il: &Il, interner: &Interner, node: NodeId) -> bool {
    match il.kind(node) {
        NodeKind::Seq => {
            let Payload::Name(tag) = il.node(node).payload else {
                return false;
            };
            if !nose_semantics::imported_literal_seq_tag_safe(il.meta.lang, interner.resolve(tag)) {
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
            if import_fact_rhs(il, interner, node).is_some() {
                return false;
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

fn exported_binding_unsafe(
    il: &Il,
    interner: &Interner,
    name: Symbol,
    defining_stmt: NodeId,
) -> bool {
    binding_mutated(il, interner, name, defining_stmt)
        || il.nodes.iter().enumerate().any(|(idx, node)| {
            let node_id = NodeId(idx as u32);
            node_id != defining_stmt
                && node.kind == NodeKind::Call
                && call_argument_escapes_binding(il, node_id, name)
        })
}

fn call_argument_escapes_binding(il: &Il, call: NodeId, name: Symbol) -> bool {
    il.children(call)
        .iter()
        .skip(1)
        .any(|&arg| node_contains_symbol(il, arg, name))
}

fn field_mutates_binding(il: &Il, interner: &Interner, field: NodeId, name: Symbol) -> bool {
    let Payload::Name(method) = il.node(field).payload else {
        return false;
    };
    if !nose_semantics::module_binding_mutating_method_name(interner.resolve(method)) {
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

fn mirror_import_fact_evidence_for_assignment(il: &mut Il, interner: &Interner, stmt: NodeId) {
    if il.kind(stmt) != NodeKind::Assign {
        return;
    }
    let kids = il.children(stmt);
    let [lhs, rhs] = kids else {
        return;
    };
    let local = match il.node(*lhs).payload {
        Payload::Name(symbol) => Some(stable_symbol_hash(interner.resolve(symbol))),
        Payload::Cid(cid) => il
            .cid_names
            .get(cid as usize)
            .map(|&symbol| stable_symbol_hash(interner.resolve(symbol))),
        _ => None,
    };
    let Some(local_hash) = local else {
        return;
    };
    let Some(fact) = import_fact_rhs(il, interner, *rhs) else {
        return;
    };
    let import_kind = match fact.kind {
        ImportFactKind::Binding => {
            let Some(exported_hash) = fact.exported_hash else {
                return;
            };
            EvidenceKind::Import(ImportEvidenceKind::Binding {
                module_hash: fact.module_hash,
                exported_hash,
            })
        }
        ImportFactKind::Namespace => EvidenceKind::Import(ImportEvidenceKind::Namespace {
            module_hash: fact.module_hash,
        }),
    };
    let symbol_kind = match fact.kind {
        ImportFactKind::Binding => {
            let Some(exported_hash) = fact.exported_hash else {
                return;
            };
            EvidenceKind::Symbol(SymbolEvidenceKind::ImportedBinding {
                module_hash: fact.module_hash,
                exported_hash,
            })
        }
        ImportFactKind::Namespace => EvidenceKind::Symbol(SymbolEvidenceKind::ImportedNamespace {
            module_hash: fact.module_hash,
        }),
    };
    push_first_party_evidence(
        il,
        EvidenceAnchor::sequence(il.node(*rhs).span),
        import_kind,
        "module_import_snapshot_import",
    );
    push_first_party_evidence(
        il,
        EvidenceAnchor::binding(il.node(stmt).span, local_hash),
        import_kind,
        "module_import_snapshot_binding_import",
    );
    push_first_party_evidence(
        il,
        EvidenceAnchor::binding(il.node(stmt).span, local_hash),
        symbol_kind,
        "module_import_snapshot_symbol",
    );
}

fn push_first_party_evidence(il: &mut Il, anchor: EvidenceAnchor, kind: EvidenceKind, rule: &str) {
    il.evidence.push(EvidenceRecord {
        id: EvidenceId(il.evidence.len() as u32),
        anchor,
        kind,
        provenance: EvidenceProvenance {
            emitter: EvidenceEmitter::FirstParty,
            pack_hash: Some(stable_symbol_hash(FIRST_PARTY_PACK_ID)),
            rule_hash: Some(stable_symbol_hash(rule)),
        },
        dependencies: Vec::new(),
        status: EvidenceStatus::Asserted,
    });
}

fn prepend_root_statement(il: &mut Il, stmt: NodeId) {
    let old_root = il.root;
    let old_root_node = *il.node(old_root);
    let mut children = Vec::with_capacity(il.children(old_root).len() + 1);
    children.push(stmt);
    children.extend_from_slice(il.children(old_root));
    let child_start = il.edges.len() as u32;
    il.edges.extend_from_slice(&children);
    let new_root = NodeId(il.nodes.len() as u32);
    il.nodes.push(Node {
        kind: old_root_node.kind,
        payload: old_root_node.payload,
        span: old_root_node.span,
        child_start,
        child_len: children.len() as u32,
    });
    il.root = new_root;
}

#[cfg(test)]
mod tests {
    use super::*;
    use nose_il::{FileId, FileMeta, IlBuilder, Lang};

    fn module_with_binding_method(method: &str) -> (Il, Interner, Symbol, NodeId) {
        let interner = Interner::new();
        let mut b = IlBuilder::new(FileId(0));
        let span = Span::new(FileId(0), 0, 1, 1, 1);
        let lookup = interner.intern("LOOKUP");
        let lhs = b.add(NodeKind::Var, Payload::Name(lookup), span, &[]);
        let rhs = b.add(
            NodeKind::Seq,
            Payload::Name(interner.intern("array")),
            span,
            &[],
        );
        let assign = b.add(NodeKind::Assign, Payload::None, span, &[lhs, rhs]);
        let receiver = b.add(NodeKind::Var, Payload::Name(lookup), span, &[]);
        let field = b.add(
            NodeKind::Field,
            Payload::Name(interner.intern(method)),
            span,
            &[receiver],
        );
        let arg = b.add(NodeKind::Lit, Payload::LitInt(2), span, &[]);
        let call = b.add(NodeKind::Call, Payload::None, span, &[field, arg]);
        let stmt = b.add(NodeKind::ExprStmt, Payload::None, span, &[call]);
        let root = b.add(NodeKind::Module, Payload::None, span, &[assign, stmt]);
        let il = b.finish(
            root,
            FileMeta {
                path: "tables.js".into(),
                lang: Lang::JavaScript,
            },
            Vec::new(),
            Vec::new(),
        );
        (il, interner, lookup, assign)
    }

    #[test]
    fn module_binding_push_marks_export_unsafe() {
        let (il, interner, lookup, assign) = module_with_binding_method("push");
        assert!(
            exported_binding_unsafe(&il, &interner, lookup, assign),
            "exported literal bindings mutated through push must not be imported as immutable"
        );
    }

    #[test]
    fn module_binding_get_is_not_a_mutation() {
        let (il, interner, lookup, assign) = module_with_binding_method("get");
        assert!(
            !binding_mutated(&il, &interner, lookup, assign),
            "read-only lookup methods should not block immutable import replacement"
        );
    }

    #[test]
    fn import_snapshot_assignment_regenerates_import_and_symbol_evidence() {
        let interner = Interner::new();
        let mut b = IlBuilder::new(FileId(0));
        let span = Span::new(FileId(0), 0, 1, 1, 1);
        let map = interner.intern("Map");
        let lhs = b.add(NodeKind::Var, Payload::Name(map), span, &[]);
        let module = b.add(
            NodeKind::Lit,
            Payload::LitStr(stable_symbol_hash("java.util")),
            span,
            &[],
        );
        let exported = b.add(
            NodeKind::Lit,
            Payload::LitStr(stable_symbol_hash("Map")),
            span,
            &[],
        );
        let tag = interner.intern(nose_semantics::import_fact_tag(ImportFactKind::Binding));
        let rhs = b.add(NodeKind::Seq, Payload::Name(tag), span, &[module, exported]);
        let assign = b.add(NodeKind::Assign, Payload::None, span, &[lhs, rhs]);
        let root = b.add(NodeKind::Module, Payload::None, span, &[assign]);
        let mut il = b.finish(
            root,
            FileMeta {
                path: "imported.java".into(),
                lang: Lang::Java,
            },
            Vec::new(),
            Vec::new(),
        );

        mirror_import_fact_evidence_for_assignment(&mut il, &interner, assign);

        assert!(il.evidence.iter().any(|record| matches!(
            record.kind,
            EvidenceKind::Import(ImportEvidenceKind::Binding {
                module_hash,
                exported_hash,
            }) if record.anchor == EvidenceAnchor::sequence(span)
                && module_hash == stable_symbol_hash("java.util")
                && exported_hash == stable_symbol_hash("Map")
        )));
        assert!(il.evidence.iter().any(|record| matches!(
            record.kind,
            EvidenceKind::Symbol(SymbolEvidenceKind::ImportedBinding {
                module_hash,
                exported_hash,
            }) if record.anchor == EvidenceAnchor::binding(span, stable_symbol_hash("Map"))
                && module_hash == stable_symbol_hash("java.util")
                && exported_hash == stable_symbol_hash("Map")
        )));
    }
}
