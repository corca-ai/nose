//! Corpus-level import proof facts that need more than one lowered file.
//!
//! Frontends lower a static import as an assignment whose RHS carries only module
//! coordinates; `EvidenceKind::Import` records prove those coordinates. Once the
//! whole corpus is available, a sibling module can prove that this binding names a
//! single immutable literal value. In that narrow case we replace the import fact
//! RHS with a cloned literal subtree, so the existing per-file value-graph
//! module-binding seed can reuse its mutation and canonicalization logic.

use nose_il::{
    stable_symbol_hash, EvidenceAnchor, EvidenceEmitter, EvidenceId, EvidenceKind,
    EvidenceProvenance, EvidenceRecord, EvidenceStatus, Il, ImportEvidenceKind, Interner, Node,
    NodeId, NodeKind, Payload, Span, Symbol, UnitKind,
};
use nose_semantics::{
    import_fact_evidence_rhs, imported_binding_symbol, library_api_free_name_shadow_safe,
    library_free_name_map_factory_contract, library_java_map_entry_contract,
    library_java_map_factory_contract, semantics, seq_surface_contract_evidence_for_node,
    ImportFactKind, ImportedMapFactoryContract, JavaMapFactoryKind, LibraryApiCalleeContract,
    LibraryMapFactoryResult, FIRST_PARTY_PACK_ID,
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
struct ImportReplacement {
    stmt: NodeId,
    import_evidence: EvidenceId,
    module_hash: u64,
    exported_hash: u64,
    deps: Vec<SubtreeSnapshot>,
    rhs_snapshot: SubtreeSnapshot,
}

#[derive(Clone, Copy)]
struct ImportBindingProof {
    module_hash: u64,
    exported_hash: u64,
    evidence: EvidenceId,
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
    evidence: Vec<SnapshotEvidence>,
}

#[derive(Clone)]
struct SnapshotEvidence {
    source_id: EvidenceId,
    anchor: EvidenceAnchor,
    kind: EvidenceKind,
    provenance: EvidenceProvenance,
    dependencies: Vec<EvidenceId>,
    status: EvidenceStatus,
}

struct AppendedSnapshot {
    root: NodeId,
    evidence: Vec<EvidenceId>,
}

pub(crate) fn resolve_imported_immutable_bindings(files: &mut [Il], interner: &Interner) {
    let exports = collect_literal_exports(files, interner);
    if exports.is_empty() {
        return;
    }
    for (&(module_hash, exported_hash), export) in &exports {
        record_immutable_literal_export_evidence(
            &mut files[export.file_idx],
            export.rhs,
            module_hash,
            exported_hash,
            &export.deps,
        );
    }

    let replacements: Vec<Vec<ImportReplacement>> = files
        .iter()
        .enumerate()
        .map(|(file_idx, il)| {
            collect_top_level_statements(il)
                .into_iter()
                .filter_map(|stmt| {
                    let local = assignment_name(il, stmt)?;
                    let proof = import_binding_proof(il, stmt)?;
                    let key = (proof.module_hash, proof.exported_hash);
                    let export = exports.get(&key)?;
                    if export.file_idx == file_idx {
                        return None;
                    }
                    if binding_mutated(il, interner, local, stmt) {
                        return None;
                    }
                    Some(ImportReplacement {
                        stmt,
                        import_evidence: proof.evidence,
                        module_hash: proof.module_hash,
                        exported_hash: proof.exported_hash,
                        deps: export.deps.clone(),
                        rhs_snapshot: snapshot_subtree(&files[export.file_idx], export.rhs),
                    })
                })
                .collect()
        })
        .collect();

    for (file_idx, file_replacements) in replacements.into_iter().enumerate() {
        for replacement in file_replacements {
            let mut snapshot_evidence = Vec::new();
            for dep in replacement.deps {
                let dep = append_snapshot(&mut files[file_idx], &dep);
                snapshot_evidence.extend(dep.evidence);
                prepend_root_statement(&mut files[file_idx], dep.root);
            }
            let rhs = append_snapshot(&mut files[file_idx], &replacement.rhs_snapshot);
            snapshot_evidence.extend(rhs.evidence);
            replace_assignment_rhs(&mut files[file_idx], replacement.stmt, rhs.root);
            record_imported_literal_snapshot_evidence(
                &mut files[file_idx],
                rhs.root,
                replacement.module_hash,
                replacement.exported_hash,
                replacement.import_evidence,
                snapshot_evidence,
            );
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
        let deps = import_dependency_snapshots(il, rhs);
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

fn import_dependency_snapshots(il: &Il, rhs: NodeId) -> Vec<SubtreeSnapshot> {
    collect_top_level_statements(il)
        .into_iter()
        .filter(|&stmt| {
            assignment_rhs(il, stmt).is_some_and(|dep_rhs| {
                import_binding_key(il, stmt).is_some() && il.kind(dep_rhs) == NodeKind::Seq
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

fn import_binding_key(il: &Il, stmt: NodeId) -> Option<(u64, u64)> {
    let proof = import_binding_proof(il, stmt)?;
    Some((proof.module_hash, proof.exported_hash))
}

fn import_binding_proof(il: &Il, stmt: NodeId) -> Option<ImportBindingProof> {
    let rhs = assignment_rhs(il, stmt)?;
    let fact = import_fact_evidence_rhs(il, rhs)?;
    if fact.kind != ImportFactKind::Binding {
        return None;
    }
    let exported_hash = fact.exported_hash?;
    let evidence = import_fact_evidence_id_for_rhs(il, rhs)?;
    Some(ImportBindingProof {
        module_hash: fact.module_hash,
        exported_hash,
        evidence,
    })
}

fn import_fact_evidence_id_for_rhs(il: &Il, rhs: NodeId) -> Option<EvidenceId> {
    let span = il.node(rhs).span;
    il.evidence.iter().find_map(|record| {
        if record.status != EvidenceStatus::Asserted
            || record.anchor != EvidenceAnchor::sequence(span)
        {
            return None;
        }
        matches!(
            record.kind,
            EvidenceKind::Import(
                ImportEvidenceKind::Binding { .. } | ImportEvidenceKind::Namespace { .. }
            )
        )
        .then_some(record.id)
    })
}

fn imported_literal_export_safe(il: &Il, interner: &Interner, node: NodeId) -> bool {
    match il.kind(node) {
        NodeKind::Seq => {
            seq_surface_contract_evidence_for_node(il, interner, node)
                .is_some_and(|contract| contract.imported_literal)
                && il
                    .children(node)
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
            if import_fact_evidence_rhs(il, node).is_some() {
                return false;
            }
            if !seq_surface_contract_evidence_for_node(il, interner, node)
                .is_some_and(|contract| contract.exact_tree_safe)
            {
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
    let Some((receiver_node, method)) = field_method_on_var(il, interner, callee, "Map") else {
        return false;
    };
    let Some(contract) = library_java_map_factory_contract(il.meta.lang, "Map", method) else {
        return false;
    };
    let LibraryApiCalleeContract::JavaUtilStaticMember {
        receiver: expected_receiver,
        ..
    } = contract.callee
    else {
        return false;
    };
    if !java_util_static_receiver_safe(il, interner, receiver_node, expected_receiver) {
        return false;
    }
    let LibraryMapFactoryResult::JavaFactory { kind } = contract.result else {
        return false;
    };
    match kind {
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
    let Some((receiver_node, method)) = field_method_on_var(il, interner, kids[0], "Map") else {
        return false;
    };
    let Some(contract) = library_java_map_entry_contract(il.meta.lang, "Map", method) else {
        return false;
    };
    let LibraryApiCalleeContract::JavaUtilStaticMember {
        receiver: expected_receiver,
        ..
    } = contract.callee
    else {
        return false;
    };
    if !java_util_static_receiver_safe(il, interner, receiver_node, expected_receiver) {
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
    let Some(contract) = library_free_name_map_factory_contract(il.meta.lang, name) else {
        return false;
    };
    let LibraryApiCalleeContract::FreeName {
        name: contract_name,
        shadow,
    } = contract.callee
    else {
        return false;
    };
    if !library_api_free_name_shadow_safe(il.meta.lang, contract_name, shadow, |candidate| {
        file_defines_name(il, interner, candidate)
    }) {
        return false;
    }
    let LibraryMapFactoryResult::EntrySequence { .. } = contract.result else {
        return false;
    };
    literal_export_value_safe(il, interner, kids[1])
}

fn field_method_on_var<'a>(
    il: &Il,
    interner: &'a Interner,
    node: NodeId,
    receiver: &str,
) -> Option<(NodeId, &'a str)> {
    if il.kind(node) != NodeKind::Field {
        return None;
    }
    let Payload::Name(method) = il.node(node).payload else {
        return None;
    };
    let receiver_node = il.children(node).first().copied()?;
    var_named(il, interner, receiver_node, receiver)
        .then(|| (receiver_node, interner.resolve(method)))
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

fn java_util_static_receiver_safe(
    il: &Il,
    interner: &Interner,
    receiver: NodeId,
    expected_receiver: &str,
) -> bool {
    !java_file_defines_type_name(il, interner, expected_receiver)
        && imported_binding_symbol(il, interner, receiver, "java.util", expected_receiver)
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
    let evidence = snapshot_evidence(il, &nodes);
    SubtreeSnapshot {
        nodes,
        root,
        evidence,
    }
}

fn snapshot_evidence(il: &Il, nodes: &[SnapshotNode]) -> Vec<SnapshotEvidence> {
    let spans: FxHashSet<Span> = nodes.iter().map(|node| node.span).collect();
    let candidates: FxHashMap<EvidenceId, &EvidenceRecord> = il
        .evidence
        .iter()
        .filter(|record| {
            record.status == EvidenceStatus::Asserted
                && spans.contains(&evidence_anchor_span(record.anchor))
        })
        .map(|record| (record.id, record))
        .collect();
    let mut kept: FxHashSet<EvidenceId> = candidates.keys().copied().collect();
    loop {
        let rejected: Vec<EvidenceId> = kept
            .iter()
            .copied()
            .filter(|id| {
                candidates
                    .get(id)
                    .is_some_and(|record| record.dependencies.iter().any(|dep| !kept.contains(dep)))
            })
            .collect();
        if rejected.is_empty() {
            break;
        }
        for id in rejected {
            kept.remove(&id);
        }
    }
    il.evidence
        .iter()
        .filter(|record| kept.contains(&record.id))
        .map(|record| SnapshotEvidence {
            source_id: record.id,
            anchor: record.anchor,
            kind: record.kind,
            provenance: record.provenance,
            dependencies: record.dependencies.clone(),
            status: record.status,
        })
        .collect()
}

fn record_immutable_literal_export_evidence(
    il: &mut Il,
    rhs: NodeId,
    module_hash: u64,
    exported_hash: u64,
    deps: &[SubtreeSnapshot],
) -> EvidenceId {
    let spans = subtree_spans(il, rhs);
    let mut seen = FxHashSet::default();
    let mut dependencies = Vec::new();
    for id in il.evidence.iter().filter_map(|record| {
        (record.status == EvidenceStatus::Asserted
            && spans.contains(&evidence_anchor_span(record.anchor))
            && !matches!(
                record.kind,
                EvidenceKind::Import(
                    ImportEvidenceKind::ImmutableLiteralExport { .. }
                        | ImportEvidenceKind::ImportedLiteralSnapshot { .. }
                )
            ))
        .then_some(record.id)
    }) {
        if seen.insert(id) {
            dependencies.push(id);
        }
    }
    for id in deps
        .iter()
        .flat_map(|dep| dep.evidence.iter().map(|evidence| evidence.source_id))
    {
        if seen.insert(id) {
            dependencies.push(id);
        }
    }
    push_first_party_evidence_with_dependencies(
        il,
        EvidenceAnchor::node(il.node(rhs).span, il.kind(rhs)),
        EvidenceKind::Import(ImportEvidenceKind::ImmutableLiteralExport {
            module_hash,
            exported_hash,
            root_kind: il.kind(rhs),
        }),
        "module_immutable_literal_export",
        dependencies,
    )
}

fn subtree_spans(il: &Il, root: NodeId) -> FxHashSet<Span> {
    fn collect(il: &Il, node: NodeId, out: &mut FxHashSet<Span>) {
        out.insert(il.node(node).span);
        for &child in il.children(node) {
            collect(il, child, out);
        }
    }

    let mut spans = FxHashSet::default();
    collect(il, root, &mut spans);
    spans
}

fn evidence_anchor_span(anchor: EvidenceAnchor) -> Span {
    match anchor {
        EvidenceAnchor::SourceSpan(span)
        | EvidenceAnchor::Node { span, .. }
        | EvidenceAnchor::Param { span }
        | EvidenceAnchor::Binding { span, .. }
        | EvidenceAnchor::Sequence { span } => span,
    }
}

fn append_snapshot(il: &mut Il, snapshot: &SubtreeSnapshot) -> AppendedSnapshot {
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
    let mut evidence_id_map = FxHashMap::default();
    for (idx, evidence) in snapshot.evidence.iter().enumerate() {
        evidence_id_map.insert(
            evidence.source_id,
            EvidenceId((il.evidence.len() + idx) as u32),
        );
    }
    let mut copied_evidence = Vec::with_capacity(snapshot.evidence.len());
    for evidence in &snapshot.evidence {
        let id = evidence_id_map[&evidence.source_id];
        let dependencies = evidence
            .dependencies
            .iter()
            .map(|dependency| {
                evidence_id_map
                    .get(dependency)
                    .copied()
                    .expect("snapshot evidence dependency must be closed")
            })
            .collect();
        il.evidence.push(EvidenceRecord {
            id,
            anchor: remap_anchor_file(evidence.anchor, il.file),
            kind: evidence.kind,
            provenance: evidence.provenance,
            dependencies,
            status: evidence.status,
        });
        copied_evidence.push(id);
    }
    AppendedSnapshot {
        root: new_ids[snapshot.root],
        evidence: copied_evidence,
    }
}

fn remap_anchor_file(anchor: EvidenceAnchor, file: nose_il::FileId) -> EvidenceAnchor {
    match anchor {
        EvidenceAnchor::SourceSpan(span) => EvidenceAnchor::SourceSpan(remap_span_file(span, file)),
        EvidenceAnchor::Node { span, kind } => EvidenceAnchor::Node {
            span: remap_span_file(span, file),
            kind,
        },
        EvidenceAnchor::Param { span } => EvidenceAnchor::Param {
            span: remap_span_file(span, file),
        },
        EvidenceAnchor::Binding { span, local_hash } => EvidenceAnchor::Binding {
            span: remap_span_file(span, file),
            local_hash,
        },
        EvidenceAnchor::Sequence { span } => EvidenceAnchor::Sequence {
            span: remap_span_file(span, file),
        },
    }
}

fn remap_span_file(mut span: Span, file: nose_il::FileId) -> Span {
    span.file = file;
    span
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

fn record_imported_literal_snapshot_evidence(
    il: &mut Il,
    rhs: NodeId,
    module_hash: u64,
    exported_hash: u64,
    import_evidence: EvidenceId,
    copied_snapshot_evidence: Vec<EvidenceId>,
) {
    let mut dependencies = Vec::with_capacity(copied_snapshot_evidence.len() + 1);
    dependencies.push(import_evidence);
    for evidence in copied_snapshot_evidence {
        if !dependencies.contains(&evidence) {
            dependencies.push(evidence);
        }
    }
    push_first_party_evidence_with_dependencies(
        il,
        EvidenceAnchor::node(il.node(rhs).span, il.kind(rhs)),
        EvidenceKind::Import(ImportEvidenceKind::ImportedLiteralSnapshot {
            module_hash,
            exported_hash,
            root_kind: il.kind(rhs),
        }),
        "module_imported_literal_snapshot",
        dependencies,
    );
}

fn push_first_party_evidence_with_dependencies(
    il: &mut Il,
    anchor: EvidenceAnchor,
    kind: EvidenceKind,
    rule: &str,
    dependencies: Vec<EvidenceId>,
) -> EvidenceId {
    let id = EvidenceId(il.evidence.len() as u32);
    il.evidence.push(EvidenceRecord {
        id,
        anchor,
        kind,
        provenance: EvidenceProvenance {
            emitter: EvidenceEmitter::FirstParty,
            pack_hash: Some(stable_symbol_hash(FIRST_PARTY_PACK_ID)),
            rule_hash: Some(stable_symbol_hash(rule)),
        },
        dependencies,
        status: EvidenceStatus::Asserted,
    });
    id
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
    use nose_il::{FileId, FileMeta, IlBuilder, Lang, SequenceSurfaceKind, SymbolEvidenceKind};

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

    fn coordinate_import_binding_assignment(
        file: FileId,
        lang: Lang,
    ) -> (Il, Interner, Span, NodeId, NodeId) {
        let interner = Interner::new();
        let mut b = IlBuilder::new(file);
        let span = Span::new(file, 0, 1, 1, 1);
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
        let rhs = b.add(NodeKind::Seq, Payload::None, span, &[module, exported]);
        let assign = b.add(NodeKind::Assign, Payload::None, span, &[lhs, rhs]);
        let root = b.add(NodeKind::Module, Payload::None, span, &[assign]);
        let il = b.finish(
            root,
            FileMeta {
                path: "imported.java".into(),
                lang,
            },
            Vec::new(),
            Vec::new(),
        );
        (il, interner, span, assign, rhs)
    }

    fn test_provenance(rule: &str) -> EvidenceProvenance {
        EvidenceProvenance {
            emitter: EvidenceEmitter::External,
            pack_hash: Some(stable_symbol_hash("test.pack")),
            rule_hash: Some(stable_symbol_hash(rule)),
        }
    }

    fn add_import_binding_evidence(il: &mut Il, span: Span, status: EvidenceStatus) -> EvidenceId {
        let id = EvidenceId(il.evidence.len() as u32);
        il.evidence.push(EvidenceRecord {
            id,
            anchor: EvidenceAnchor::sequence(span),
            kind: EvidenceKind::Import(ImportEvidenceKind::Binding {
                module_hash: stable_symbol_hash("java.util"),
                exported_hash: stable_symbol_hash("Map"),
            }),
            provenance: test_provenance("import_binding"),
            dependencies: Vec::new(),
            status,
        });
        id
    }

    #[test]
    fn import_binding_key_requires_asserted_import_evidence() {
        let (mut il, _interner, span, assign, _rhs) =
            coordinate_import_binding_assignment(FileId(0), Lang::Java);
        assert_eq!(
            import_binding_key(&il, assign),
            None,
            "raw import coordinate Seqs must not prove import identity"
        );

        add_import_binding_evidence(&mut il, span, EvidenceStatus::Asserted);
        assert_eq!(
            import_binding_key(&il, assign),
            Some((stable_symbol_hash("java.util"), stable_symbol_hash("Map")))
        );
    }

    #[test]
    fn import_binding_key_rejects_ambiguous_import_evidence_even_with_coordinates() {
        let (mut il, _interner, span, assign, _rhs) =
            coordinate_import_binding_assignment(FileId(0), Lang::Java);
        add_import_binding_evidence(&mut il, span, EvidenceStatus::Ambiguous);

        assert_eq!(
            import_binding_key(&il, assign),
            None,
            "ambiguous import evidence must close the imported literal rewrite"
        );
    }

    #[test]
    fn snapshot_append_does_not_mint_import_or_symbol_evidence_from_coordinates() {
        let (provider, _interner, _span, assign, _rhs) =
            coordinate_import_binding_assignment(FileId(0), Lang::Java);
        let snapshot = snapshot_subtree(&provider, assign);

        let mut b = IlBuilder::new(FileId(1));
        let root_span = Span::new(FileId(1), 0, 0, 1, 1);
        let root = b.add(NodeKind::Module, Payload::None, root_span, &[]);
        let mut importer = b.finish(
            root,
            FileMeta {
                path: "consumer.java".into(),
                lang: Lang::Java,
            },
            Vec::new(),
            Vec::new(),
        );

        let appended = append_snapshot(&mut importer, &snapshot);
        assert!(
            appended.evidence.is_empty(),
            "snapshot append must copy provider evidence, not synthesize import facts from raw tags"
        );
        assert_eq!(importer.kind(appended.root), NodeKind::Assign);
    }

    #[test]
    fn snapshot_append_copies_relevant_evidence_with_remapped_file_spans() {
        let interner = Interner::new();
        let mut b = IlBuilder::new(FileId(0));
        let span = Span::new(FileId(0), 4, 12, 1, 1);
        let lookup = interner.intern("LOOKUP");
        let lhs = b.add(NodeKind::Var, Payload::Name(lookup), span, &[]);
        let tag = interner.intern("dictionary");
        let rhs = b.add(NodeKind::Seq, Payload::Name(tag), span, &[]);
        let assign = b.add(NodeKind::Assign, Payload::None, span, &[lhs, rhs]);
        let root = b.add(NodeKind::Module, Payload::None, span, &[assign]);
        let mut provider = b.finish(
            root,
            FileMeta {
                path: "tables.py".into(),
                lang: Lang::Python,
            },
            Vec::new(),
            Vec::new(),
        );
        provider.evidence.push(EvidenceRecord {
            id: EvidenceId(0),
            anchor: EvidenceAnchor::sequence(span),
            kind: EvidenceKind::SequenceSurface(SequenceSurfaceKind::Map),
            provenance: test_provenance("surface"),
            dependencies: Vec::new(),
            status: EvidenceStatus::Asserted,
        });
        provider.evidence.push(EvidenceRecord {
            id: EvidenceId(1),
            anchor: EvidenceAnchor::node(span, NodeKind::Seq),
            kind: EvidenceKind::Import(ImportEvidenceKind::ImmutableLiteralExport {
                module_hash: stable_symbol_hash("tables"),
                exported_hash: stable_symbol_hash("LOOKUP"),
                root_kind: NodeKind::Seq,
            }),
            provenance: test_provenance("export"),
            dependencies: vec![EvidenceId(0)],
            status: EvidenceStatus::Asserted,
        });
        provider.evidence.push(EvidenceRecord {
            id: EvidenceId(2),
            anchor: EvidenceAnchor::binding(span, stable_symbol_hash("LOOKUP")),
            kind: EvidenceKind::Symbol(SymbolEvidenceKind::ImportedBinding {
                module_hash: stable_symbol_hash("tables"),
                exported_hash: stable_symbol_hash("LOOKUP"),
            }),
            provenance: test_provenance("symbol"),
            dependencies: vec![EvidenceId(0)],
            status: EvidenceStatus::Asserted,
        });
        provider.evidence.push(EvidenceRecord {
            id: EvidenceId(3),
            anchor: EvidenceAnchor::sequence(span),
            kind: EvidenceKind::SequenceSurface(SequenceSurfaceKind::Map),
            provenance: test_provenance("ambiguous_surface"),
            dependencies: Vec::new(),
            status: EvidenceStatus::Ambiguous,
        });
        let snapshot = snapshot_subtree(&provider, assign);

        let mut b = IlBuilder::new(FileId(1));
        let root_span = Span::new(FileId(1), 0, 0, 1, 1);
        let root = b.add(NodeKind::Module, Payload::None, root_span, &[]);
        let mut importer = b.finish(
            root,
            FileMeta {
                path: "consumer.py".into(),
                lang: Lang::Python,
            },
            Vec::new(),
            Vec::new(),
        );
        let appended = append_snapshot(&mut importer, &snapshot);

        assert_eq!(appended.evidence.len(), 3);
        assert!(
            importer
                .evidence
                .iter()
                .all(|record| record.status == EvidenceStatus::Asserted),
            "snapshot append must not copy ambiguous evidence into asserted provenance dependencies"
        );
        let copied_surface = importer
            .evidence
            .iter()
            .find(|record| {
                matches!(
                    record.kind,
                    EvidenceKind::SequenceSurface(SequenceSurfaceKind::Map)
                )
            })
            .unwrap();
        assert_eq!(
            copied_surface.anchor,
            EvidenceAnchor::sequence(Span::new(FileId(1), 4, 12, 1, 1))
        );
        assert_eq!(copied_surface.provenance, test_provenance("surface"));

        let copied_export = importer
            .evidence
            .iter()
            .find(|record| {
                matches!(
                    record.kind,
                    EvidenceKind::Import(ImportEvidenceKind::ImmutableLiteralExport { .. })
                )
            })
            .unwrap();
        assert_eq!(copied_export.dependencies, vec![copied_surface.id]);
    }

    #[test]
    fn resolve_imported_literal_records_snapshot_provenance_dependencies() {
        let interner = Interner::new();
        let provider_span = Span::new(FileId(0), 4, 24, 1, 1);
        let lookup = interner.intern("LOOKUP");
        let mut b = IlBuilder::new(FileId(0));
        let lhs = b.add(NodeKind::Var, Payload::Name(lookup), provider_span, &[]);
        let key = b.add(
            NodeKind::Lit,
            Payload::LitStr(stable_symbol_hash("red")),
            provider_span,
            &[],
        );
        let value = b.add(NodeKind::Lit, Payload::LitInt(1), provider_span, &[]);
        let tag = interner.intern("dictionary");
        let rhs = b.add(
            NodeKind::Seq,
            Payload::Name(tag),
            provider_span,
            &[key, value],
        );
        let assign = b.add(NodeKind::Assign, Payload::None, provider_span, &[lhs, rhs]);
        let root = b.add(NodeKind::Module, Payload::None, provider_span, &[assign]);
        let mut provider = b.finish(
            root,
            FileMeta {
                path: "tables.py".into(),
                lang: Lang::Python,
            },
            Vec::new(),
            Vec::new(),
        );
        provider.evidence.push(EvidenceRecord {
            id: EvidenceId(0),
            anchor: EvidenceAnchor::sequence(provider_span),
            kind: EvidenceKind::SequenceSurface(SequenceSurfaceKind::Map),
            provenance: test_provenance("provider_surface"),
            dependencies: Vec::new(),
            status: EvidenceStatus::Asserted,
        });

        let import_span = Span::new(FileId(1), 0, 24, 1, 1);
        let mut b = IlBuilder::new(FileId(1));
        let lhs = b.add(NodeKind::Var, Payload::Name(lookup), import_span, &[]);
        let module = b.add(
            NodeKind::Lit,
            Payload::LitStr(stable_symbol_hash("tables")),
            import_span,
            &[],
        );
        let exported = b.add(
            NodeKind::Lit,
            Payload::LitStr(stable_symbol_hash("LOOKUP")),
            import_span,
            &[],
        );
        let import_rhs = b.add(
            NodeKind::Seq,
            Payload::None,
            import_span,
            &[module, exported],
        );
        let import_assign = b.add(
            NodeKind::Assign,
            Payload::None,
            import_span,
            &[lhs, import_rhs],
        );
        let root = b.add(
            NodeKind::Module,
            Payload::None,
            import_span,
            &[import_assign],
        );
        let mut importer = b.finish(
            root,
            FileMeta {
                path: "consumer.py".into(),
                lang: Lang::Python,
            },
            Vec::new(),
            Vec::new(),
        );
        importer.evidence.push(EvidenceRecord {
            id: EvidenceId(0),
            anchor: EvidenceAnchor::sequence(import_span),
            kind: EvidenceKind::Import(ImportEvidenceKind::Binding {
                module_hash: stable_symbol_hash("tables"),
                exported_hash: stable_symbol_hash("LOOKUP"),
            }),
            provenance: test_provenance("import"),
            dependencies: Vec::new(),
            status: EvidenceStatus::Asserted,
        });

        let mut files = vec![provider, importer];
        resolve_imported_immutable_bindings(&mut files, &interner);
        let replaced_rhs = assignment_rhs(&files[1], import_assign).unwrap();

        assert_eq!(files[1].kind(replaced_rhs), NodeKind::Seq);
        let provenance = files[1]
            .evidence
            .iter()
            .find(|record| {
                matches!(
                    record.kind,
                    EvidenceKind::Import(ImportEvidenceKind::ImportedLiteralSnapshot {
                        module_hash,
                        exported_hash,
                        root_kind: NodeKind::Seq,
                    }) if module_hash == stable_symbol_hash("tables")
                        && exported_hash == stable_symbol_hash("LOOKUP")
                )
            })
            .unwrap();
        assert!(
            provenance.dependencies.contains(&EvidenceId(0)),
            "snapshot provenance must depend on the importer static import proof"
        );
        assert!(
            provenance.dependencies.iter().any(|id| {
                files[1].evidence.get(id.0 as usize).is_some_and(|record| {
                    matches!(
                        record.kind,
                        EvidenceKind::SequenceSurface(SequenceSurfaceKind::Map)
                    )
                })
            }),
            "snapshot provenance must depend on copied provider surface evidence"
        );
        assert!(
            provenance.dependencies.iter().any(|id| {
                files[1].evidence.get(id.0 as usize).is_some_and(|record| {
                    matches!(
                        record.kind,
                        EvidenceKind::Import(ImportEvidenceKind::ImmutableLiteralExport {
                            module_hash,
                            exported_hash,
                            root_kind: NodeKind::Seq,
                        }) if module_hash == stable_symbol_hash("tables")
                            && exported_hash == stable_symbol_hash("LOOKUP")
                    )
                })
            }),
            "snapshot provenance must depend on copied provider export evidence"
        );
    }
}
