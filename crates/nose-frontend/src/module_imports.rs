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
    import_fact_evidence_rhs, library_api_contract_evidence_for_call,
    library_free_name_map_factory_contract, library_java_map_entry_contract,
    library_java_map_factory_contract, semantics, seq_surface_contract_evidence_for_node,
    ImportFactKind, ImportedMapFactoryContract, JavaMapFactoryKind, LibraryApiCalleeContract,
    LibraryApiEvidenceStatus, LibraryMapFactoryResult, FIRST_PARTY_PACK_ID,
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
    let contexts: Vec<FileImportContext> = files
        .iter()
        .map(|il| FileImportContext::new(il, interner))
        .collect();
    let exports = collect_literal_exports(files, interner, &contexts);
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
            let context = &contexts[file_idx];
            let Some(top_level) = context.top_level.as_deref() else {
                return Vec::new();
            };
            let Some(binding_uses) = context.binding_uses.as_ref() else {
                return Vec::new();
            };
            top_level
                .iter()
                .copied()
                .filter_map(|stmt| {
                    let local = assignment_name(il, stmt)?;
                    let proof = import_binding_proof(il, stmt)?;
                    let key = (proof.module_hash, proof.exported_hash);
                    let export = exports.get(&key)?;
                    if export.file_idx == file_idx {
                        return None;
                    }
                    if binding_uses.binding_mutated(il, local, stmt) {
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

struct FileImportContext {
    top_level: Option<Vec<NodeId>>,
    module_hashes: Vec<u64>,
    binding_uses: Option<BindingUseIndex>,
}

impl FileImportContext {
    fn new(il: &Il, interner: &Interner) -> Self {
        let module_semantics = semantics(il.meta.lang).modules();
        let participates = module_semantics.sibling_literal_exports()
            || module_semantics.java_class_literal_exports();
        Self {
            top_level: participates.then(|| collect_top_level_statements(il)),
            module_hashes: file_module_hashes(il),
            binding_uses: participates.then(|| BindingUseIndex::new(il, interner)),
        }
    }
}

fn collect_literal_exports(
    files: &[Il],
    interner: &Interner,
    contexts: &[FileImportContext],
) -> FxHashMap<(u64, u64), ExportedBinding> {
    let mut exports = FxHashMap::default();
    let mut ambiguous = FxHashSet::default();
    for (file_idx, il) in files.iter().enumerate() {
        let context = &contexts[file_idx];
        if !context.module_hashes.is_empty() {
            let Some(top_level) = context.top_level.as_deref() else {
                continue;
            };
            let Some(binding_uses) = context.binding_uses.as_ref() else {
                continue;
            };
            collect_statement_exports(
                il,
                interner,
                StatementExportScope {
                    file_idx,
                    statements: top_level,
                    top_level,
                    module_hashes: &context.module_hashes,
                    binding_uses,
                },
                ExportCollections {
                    exports: &mut exports,
                    ambiguous: &mut ambiguous,
                },
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
            let Some(top_level) = context.top_level.as_deref() else {
                continue;
            };
            let Some(binding_uses) = context.binding_uses.as_ref() else {
                continue;
            };
            let statements = collect_statements_for_root(il, unit.root);
            collect_statement_exports(
                il,
                interner,
                StatementExportScope {
                    file_idx,
                    statements: &statements,
                    top_level,
                    module_hashes: &class_module_hashes,
                    binding_uses,
                },
                ExportCollections {
                    exports: &mut exports,
                    ambiguous: &mut ambiguous,
                },
            );
        }
    }
    for key in ambiguous {
        exports.remove(&key);
    }
    exports
}

struct StatementExportScope<'a> {
    file_idx: usize,
    statements: &'a [NodeId],
    top_level: &'a [NodeId],
    module_hashes: &'a [u64],
    binding_uses: &'a BindingUseIndex,
}

struct ExportCollections<'a> {
    exports: &'a mut FxHashMap<(u64, u64), ExportedBinding>,
    ambiguous: &'a mut FxHashSet<(u64, u64)>,
}

fn collect_statement_exports(
    il: &Il,
    interner: &Interner,
    scope: StatementExportScope<'_>,
    out: ExportCollections<'_>,
) {
    let mut counts: FxHashMap<Symbol, usize> = FxHashMap::default();
    for &stmt in scope.statements {
        if let Some(name) = assignment_name(il, stmt) {
            *counts.entry(name).or_insert(0) += 1;
        }
    }
    for &stmt in scope.statements {
        let Some(name) = assignment_name(il, stmt) else {
            continue;
        };
        if counts.get(&name).copied().unwrap_or(0) != 1 {
            continue;
        }
        if scope.binding_uses.exported_binding_unsafe(il, name, stmt) {
            continue;
        }
        let Some(rhs) = assignment_rhs(il, stmt) else {
            continue;
        };
        if !imported_literal_export_safe(il, interner, rhs) {
            continue;
        }
        let exported = stable_symbol_hash(interner.resolve(name));
        let deps = import_dependency_snapshots(il, rhs, scope.top_level);
        for &module in scope.module_hashes {
            let key = (module, exported);
            if out
                .exports
                .insert(
                    key,
                    ExportedBinding {
                        file_idx: scope.file_idx,
                        deps: deps.clone(),
                        rhs,
                    },
                )
                .is_some()
            {
                out.ambiguous.insert(key);
            }
        }
    }
}

fn import_dependency_snapshots(il: &Il, rhs: NodeId, top_level: &[NodeId]) -> Vec<SubtreeSnapshot> {
    top_level
        .iter()
        .copied()
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
    let (lhs, _) = il.assignment_var_parts(stmt)?;
    il.var_name(lhs)
}

fn assignment_rhs(il: &Il, stmt: NodeId) -> Option<NodeId> {
    il.assignment_parts(stmt).map(|(_, rhs)| rhs)
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
    let Some((_receiver_node, method)) = field_method_on_var(il, interner, callee, "Map") else {
        return false;
    };
    let Some(contract) = library_java_map_factory_contract(il.meta.lang, "Map", method) else {
        return false;
    };
    if !java_util_static_member_evidence_required(il, interner, call, contract.id, contract.callee)
    {
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
    let Some((_receiver_node, method)) = field_method_on_var(il, interner, kids[0], "Map") else {
        return false;
    };
    let Some(contract) = library_java_map_entry_contract(il.meta.lang, "Map", method) else {
        return false;
    };
    if !java_util_static_member_evidence_required(il, interner, call, contract.id, contract.callee)
    {
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
    let LibraryApiCalleeContract::FreeName { .. } = contract.callee else {
        return false;
    };
    if !matches!(
        library_api_contract_evidence_for_call(il, interner, call, contract.id, contract.callee, 1,),
        LibraryApiEvidenceStatus::Admitted
    ) {
        return false;
    }
    let LibraryMapFactoryResult::EntrySequence { .. } = contract.result else {
        return false;
    };
    literal_export_value_safe(il, interner, kids[1])
}

fn java_util_static_member_evidence_required(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    contract_id: nose_semantics::LibraryApiContractId,
    callee: LibraryApiCalleeContract,
) -> bool {
    matches!(
        library_api_contract_evidence_for_call(
            il,
            interner,
            call,
            contract_id,
            callee,
            il.children(call).len().saturating_sub(1),
        ),
        LibraryApiEvidenceStatus::Admitted
    )
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

struct BindingUseIndex {
    assignment_lhs_counts: FxHashMap<Symbol, usize>,
    receiver_mutation_symbols: FxHashSet<Symbol>,
    escaping_call_arg_symbols: FxHashSet<Symbol>,
}

impl BindingUseIndex {
    fn new(il: &Il, interner: &Interner) -> Self {
        let mut out = Self {
            assignment_lhs_counts: FxHashMap::default(),
            receiver_mutation_symbols: FxHashSet::default(),
            escaping_call_arg_symbols: FxHashSet::default(),
        };
        for (idx, node) in il.nodes.iter().enumerate() {
            let node_id = NodeId(idx as u32);
            match node.kind {
                NodeKind::Assign => {
                    if let Some(lhs) = nose_semantics::binding_write_target(il, node_id) {
                        let mut lhs_symbols = FxHashSet::default();
                        collect_symbols_into_set(il, lhs, &mut lhs_symbols);
                        for symbol in lhs_symbols {
                            *out.assignment_lhs_counts.entry(symbol).or_insert(0) += 1;
                        }
                    }
                }
                NodeKind::Call => {
                    out.collect_receiver_mutation_symbol(il, interner, node_id);
                    out.collect_call_argument_escapes(il, node_id);
                }
                _ => {}
            }
        }
        out
    }

    fn binding_mutated(&self, il: &Il, name: Symbol, defining_stmt: NodeId) -> bool {
        let defining_lhs_refs_name = il
            .children(defining_stmt)
            .first()
            .is_some_and(|&lhs| node_contains_symbol(il, lhs, name));
        let own_assignment = usize::from(defining_lhs_refs_name);
        self.assignment_lhs_counts.get(&name).copied().unwrap_or(0) > own_assignment
            || self.receiver_mutation_symbols.contains(&name)
    }

    fn exported_binding_unsafe(&self, il: &Il, name: Symbol, defining_stmt: NodeId) -> bool {
        self.binding_mutated(il, name, defining_stmt)
            || self.escaping_call_arg_symbols.contains(&name)
    }

    fn collect_receiver_mutation_symbol(&mut self, il: &Il, interner: &Interner, call: NodeId) {
        let Some(receiver) = nose_semantics::receiver_mutation_call_receiver(il, interner, call)
        else {
            return;
        };
        if let Payload::Name(name) = il.node(receiver).payload {
            self.receiver_mutation_symbols.insert(name);
        }
    }

    fn collect_call_argument_escapes(&mut self, il: &Il, call: NodeId) {
        let Some(args) = nose_semantics::opaque_argument_escape_args(il, call) else {
            return;
        };
        for &arg in args {
            collect_symbols_into_set(il, arg, &mut self.escaping_call_arg_symbols);
        }
    }
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

fn collect_symbols_into_set(il: &Il, node: NodeId, out: &mut FxHashSet<Symbol>) {
    if let Payload::Name(symbol) = il.node(node).payload {
        out.insert(symbol);
    }
    for &child in il.children(node) {
        collect_symbols_into_set(il, child, out);
    }
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
    let asserted: FxHashMap<EvidenceId, &EvidenceRecord> = il
        .evidence
        .iter()
        .filter(|record| record.status == EvidenceStatus::Asserted)
        .map(|record| (record.id, record))
        .collect();
    let mut kept: FxHashSet<EvidenceId> = asserted
        .values()
        .filter(|record| spans.contains(&evidence_anchor_span(record.anchor)))
        .map(|record| record.id)
        .collect();

    let mut stack: Vec<EvidenceId> = kept.iter().copied().collect();
    while let Some(id) = stack.pop() {
        let Some(record) = asserted.get(&id) else {
            continue;
        };
        for &dependency in &record.dependencies {
            if asserted.contains_key(&dependency) && kept.insert(dependency) {
                stack.push(dependency);
            }
        }
    }

    loop {
        let rejected: Vec<EvidenceId> = kept
            .iter()
            .copied()
            .filter(|id| {
                asserted
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
        let id = NodeId(il.nodes.len() as u32);
        il.nodes.push(Node {
            kind: snapshot_node.kind,
            payload: snapshot_node.payload,
            span: snapshot_node.span,
            child_start,
            child_len: children.len() as u32,
        });
        new_ids[idx] = id;
    }
    let source_evidence: FxHashMap<EvidenceId, &SnapshotEvidence> = snapshot
        .evidence
        .iter()
        .map(|evidence| (evidence.source_id, evidence))
        .collect();
    let mut evidence_id_map = FxHashMap::default();
    let mut copied_evidence = Vec::with_capacity(snapshot.evidence.len());
    for evidence in &snapshot.evidence {
        let id = append_snapshot_evidence(
            il,
            &source_evidence,
            &mut evidence_id_map,
            evidence.source_id,
        );
        if !copied_evidence.contains(&id) {
            copied_evidence.push(id);
        }
    }
    AppendedSnapshot {
        root: new_ids[snapshot.root],
        evidence: copied_evidence,
    }
}

fn append_snapshot_evidence(
    il: &mut Il,
    source_evidence: &FxHashMap<EvidenceId, &SnapshotEvidence>,
    evidence_id_map: &mut FxHashMap<EvidenceId, EvidenceId>,
    source_id: EvidenceId,
) -> EvidenceId {
    if let Some(id) = evidence_id_map.get(&source_id).copied() {
        return id;
    }
    let evidence = source_evidence
        .get(&source_id)
        .copied()
        .expect("snapshot evidence dependency must be closed");
    let dependencies: Vec<EvidenceId> = evidence
        .dependencies
        .iter()
        .map(|dependency| {
            append_snapshot_evidence(il, source_evidence, evidence_id_map, *dependency)
        })
        .collect();
    let anchor = evidence.anchor;
    let id = existing_snapshot_evidence_id(
        il,
        anchor,
        evidence.kind,
        evidence.provenance,
        &dependencies,
        evidence.status,
    )
    .unwrap_or_else(|| {
        let id = EvidenceId(il.evidence.len() as u32);
        il.evidence.push(EvidenceRecord {
            id,
            anchor,
            kind: evidence.kind,
            provenance: evidence.provenance,
            dependencies,
            status: evidence.status,
        });
        id
    });
    evidence_id_map.insert(source_id, id);
    id
}

fn existing_snapshot_evidence_id(
    il: &Il,
    anchor: EvidenceAnchor,
    kind: EvidenceKind,
    provenance: EvidenceProvenance,
    dependencies: &[EvidenceId],
    status: EvidenceStatus,
) -> Option<EvidenceId> {
    il.evidence
        .iter()
        .find(|record| {
            record.anchor == anchor
                && record.kind == kind
                && record.provenance == provenance
                && record.dependencies == dependencies
                && record.status == status
        })
        .map(|record| record.id)
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
    use nose_il::{
        EffectEvidenceKind, FileId, FileMeta, IlBuilder, Lang, SequenceSurfaceKind,
        SymbolEvidenceKind,
    };

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
        let mut il = b.finish(
            root,
            FileMeta {
                path: "tables.js".into(),
                lang: Lang::JavaScript,
            },
            Vec::new(),
            Vec::new(),
        );
        push_first_party_evidence_with_dependencies(
            &mut il,
            EvidenceAnchor::node(span, NodeKind::Assign),
            EvidenceKind::Effect(EffectEvidenceKind::BindingWrite),
            "effect_binding_write_test",
            Vec::new(),
        );
        push_first_party_evidence_with_dependencies(
            &mut il,
            EvidenceAnchor::node(span, NodeKind::Call),
            EvidenceKind::Effect(EffectEvidenceKind::OpaqueArgumentEscape),
            "effect_opaque_argument_escape_test",
            Vec::new(),
        );
        if nose_semantics::module_binding_mutating_method_contract(Lang::JavaScript, method) {
            push_first_party_evidence_with_dependencies(
                &mut il,
                EvidenceAnchor::node(span, NodeKind::Call),
                EvidenceKind::Effect(EffectEvidenceKind::ReceiverMutation),
                "effect_receiver_mutation_test",
                Vec::new(),
            );
        }
        (il, interner, lookup, assign)
    }

    #[test]
    fn module_binding_push_marks_export_unsafe() {
        let (il, interner, lookup, assign) = module_with_binding_method("push");
        let binding_uses = BindingUseIndex::new(&il, &interner);
        assert!(
            binding_uses.exported_binding_unsafe(&il, lookup, assign),
            "exported literal bindings mutated through push must not be imported as immutable"
        );
    }

    #[test]
    fn module_binding_get_is_not_a_mutation() {
        let (il, interner, lookup, assign) = module_with_binding_method("get");
        let binding_uses = BindingUseIndex::new(&il, &interner);
        assert!(
            !binding_uses.binding_mutated(&il, lookup, assign),
            "read-only lookup methods should not block immutable import replacement"
        );
    }

    fn java_provider_and_importer(provider_src: &str, interner: &Interner) -> (Il, Il) {
        java_provider_and_importer_src(
            provider_src,
            "import static Tables.LOOKUP;\nclass Consumer {}",
            interner,
        )
    }

    fn java_provider_and_importer_src(
        provider_src: &str,
        importer_src: &str,
        interner: &Interner,
    ) -> (Il, Il) {
        let provider = crate::lower_source(
            FileId(0),
            "Tables.java",
            provider_src.as_bytes(),
            Lang::Java,
            interner,
        )
        .expect("lower Java provider");
        let importer = crate::lower_source(
            FileId(1),
            "Consumer.java",
            importer_src.as_bytes(),
            Lang::Java,
            interner,
        )
        .expect("lower Java importer");
        (provider, importer)
    }

    fn snapshot_count(il: &Il) -> usize {
        il.evidence
            .iter()
            .filter(|record| {
                matches!(
                    record.kind,
                    EvidenceKind::Import(ImportEvidenceKind::ImportedLiteralSnapshot { .. })
                )
            })
            .count()
    }

    fn resolve_importer(provider: Il, importer: Il, interner: &Interner) -> Il {
        let mut files = vec![provider, importer];
        resolve_imported_immutable_bindings(&mut files, interner);
        files.remove(1)
    }

    fn resolve_snapshot_count(provider: Il, importer: Il, interner: &Interner) -> usize {
        snapshot_count(&resolve_importer(provider, importer, interner))
    }

    fn remove_library_api_evidence_by_rule(il: &mut Il, rule: &str) {
        let rule_hash = stable_symbol_hash(rule);
        il.evidence.retain(|record| {
            !matches!(record.kind, EvidenceKind::LibraryApi(_))
                || record.provenance.rule_hash != Some(rule_hash)
        });
    }

    #[test]
    fn java_map_provider_requires_library_api_evidence_for_snapshot() {
        let interner = Interner::new();
        let provider_src = "import java.util.Map;\nclass Tables { static final Map<String, Integer> LOOKUP = Map.of(\"red\", 1, \"blue\", 2); }\n";
        let (provider, importer) = java_provider_and_importer(provider_src, &interner);
        assert_eq!(
            resolve_snapshot_count(provider.clone(), importer.clone(), &interner),
            1
        );

        let mut missing_api = provider;
        remove_library_api_evidence_by_rule(&mut missing_api, "library_api_java_map_factory");
        assert_eq!(
            resolve_snapshot_count(missing_api, importer, &interner),
            0,
            "java.util import/symbol proof must not prove provider Map.of without LibraryApi evidence"
        );
    }

    #[test]
    fn java_map_provider_snapshot_copies_library_api_dependency_closure() {
        let interner = Interner::new();
        let provider_src = "import java.util.Map;\nclass Tables { static final Map<String, Integer> LOOKUP = Map.of(\"red\", 1, \"blue\", 2); }\n";
        let (provider, importer) = java_provider_and_importer(provider_src, &interner);
        let importer = resolve_importer(provider, importer, &interner);
        let api_rule = stable_symbol_hash("library_api_java_map_factory");
        let api = importer
            .evidence
            .iter()
            .find(|record| {
                matches!(record.kind, EvidenceKind::LibraryApi(_))
                    && record.provenance.rule_hash == Some(api_rule)
            })
            .expect("copied Java Map.of snapshot should retain LibraryApi evidence");

        let occurrence = api
            .dependencies
            .iter()
            .filter_map(|id| importer.evidence.get(id.0 as usize))
            .find(|record| {
                matches!(
                    record.kind,
                    EvidenceKind::Symbol(SymbolEvidenceKind::ImportedBinding {
                        module_hash,
                        exported_hash,
                    }) if module_hash == stable_symbol_hash("java.util")
                        && exported_hash == stable_symbol_hash("Map")
                )
            })
            .expect("copied LibraryApi evidence should keep its imported-symbol dependency");
        assert!(
            occurrence.dependencies.iter().any(|id| {
                importer.evidence.get(id.0 as usize).is_some_and(|record| {
                    matches!(
                        record.kind,
                        EvidenceKind::Symbol(SymbolEvidenceKind::ImportedBinding {
                            module_hash,
                            exported_hash,
                        }) if module_hash == stable_symbol_hash("java.util")
                            && exported_hash == stable_symbol_hash("Map")
                    )
                })
            }),
            "copied occurrence proof should keep the provider binding-anchor dependency"
        );
    }

    #[test]
    fn java_map_provider_snapshot_replaces_import_used_by_lookup_method() {
        let interner = Interner::new();
        let provider_src = "import java.util.Map;\nclass Tables { static final Map<String, Integer> LOOKUP = Map.of(\"red\", 1, \"blue\", 2); }\n";
        let importer_src = "import static Tables.LOOKUP;\nclass Consumer { static int lookup(String key, String other) { return LOOKUP.getOrDefault(key, 0); } }\n";
        let (provider, importer) =
            java_provider_and_importer_src(provider_src, importer_src, &interner);
        let importer = resolve_importer(provider, importer, &interner);
        assert_eq!(snapshot_count(&importer), 1);
        let import_stmt = collect_top_level_statements(&importer)
            .into_iter()
            .find(|&stmt| {
                assignment_name(&importer, stmt)
                    .is_some_and(|name| interner.resolve(name) == "LOOKUP")
            })
            .expect("static import assignment should remain as replacement anchor");
        let rhs = assignment_rhs(&importer, import_stmt).expect("import assignment rhs");
        assert_eq!(importer.kind(rhs), NodeKind::Call);
        assert_eq!(
            importer.node(rhs).span.file,
            FileId(0),
            "copied provider RHS keeps provider source origin so importer-local scopes cannot shadow it"
        );
        let contract =
            library_java_map_factory_contract(Lang::Java, "Map", "of").expect("Map.of contract");
        assert_eq!(
            library_api_contract_evidence_for_call(
                &importer,
                &interner,
                rhs,
                contract.id,
                contract.callee,
                4
            ),
            LibraryApiEvidenceStatus::Admitted
        );
    }

    #[test]
    fn java_map_of_entries_provider_requires_outer_and_entry_library_api_evidence() {
        let interner = Interner::new();
        let provider_src = "import java.util.Map;\nclass Tables { static final Map<String, Integer> LOOKUP = Map.ofEntries(Map.entry(\"red\", 1), Map.entry(\"blue\", 2)); }\n";
        let (provider, importer) = java_provider_and_importer(provider_src, &interner);
        assert_eq!(
            resolve_snapshot_count(provider.clone(), importer.clone(), &interner),
            1
        );

        let mut missing_outer = provider.clone();
        remove_library_api_evidence_by_rule(&mut missing_outer, "library_api_java_map_factory");
        assert_eq!(
            resolve_snapshot_count(missing_outer, importer.clone(), &interner),
            0,
            "Map.ofEntries provider proof must require the outer LibraryApi evidence"
        );

        let mut missing_entry = provider;
        remove_library_api_evidence_by_rule(
            &mut missing_entry,
            "library_api_java_map_entry_factory",
        );
        assert_eq!(
            resolve_snapshot_count(missing_entry, importer, &interner),
            0,
            "Map.ofEntries provider proof must require every nested Map.entry LibraryApi evidence"
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
    fn snapshot_append_copies_relevant_evidence_with_source_origin_spans() {
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

        assert_eq!(
            importer.node(appended.root).span.file,
            FileId(0),
            "copied provider nodes keep provider source origin so importer-local scopes do not shadow them"
        );
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
            EvidenceAnchor::sequence(Span::new(FileId(0), 4, 12, 1, 1))
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
