//! Corpus-level import proof facts that need more than one lowered file.
//!
//! Frontends lower a static import as an assignment whose RHS carries only module
//! coordinates; `EvidenceKind::Import` records prove those coordinates. Once the
//! whole corpus is available, a sibling module can prove that this binding names a
//! single immutable literal value. In that narrow case we replace the import fact
//! RHS with a cloned literal subtree, so the existing per-file value-graph
//! module-binding seed can reuse its mutation and canonicalization logic.

mod bindings;
mod diagnostics;
mod exports;
mod modules;
mod namespace_members;
mod snapshot;

use bindings::{
    assignment_name, collect_top_level_statements, import_binding_proof, BindingUseIndex,
};
pub use diagnostics::{imported_immutable_snapshot_census, ImportSnapshotCensus};
use exports::collect_literal_exports;
use modules::{
    file_module_hashes, rust_importable_module_hashes, rust_module_identity, RustModuleIdentity,
};
use namespace_members::{collect_namespace_member_replacements, NamespaceMemberReplacement};
use nose_il::{EvidenceId, Il, Interner, NodeId};
use nose_semantics::semantics;
use snapshot::{
    append_snapshot, prepend_root_statement, record_immutable_literal_export_evidence,
    record_imported_literal_snapshot_evidence, replace_assignment_rhs, replace_node_references,
    snapshot_subtree, SubtreeSnapshot,
};

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

pub(crate) fn resolve_imported_immutable_bindings(files: &mut [Il], interner: &Interner) {
    let contexts: Vec<FileImportContext> = files
        .iter()
        .map(|il| FileImportContext::new(il, interner))
        .collect();
    let exports = collect_literal_exports(files, interner, &contexts);
    if exports.is_empty() {
        return;
    }
    for (&(module_hash, exported_hash), export) in exports.iter_keyed() {
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
                    let export =
                        exports.get(&contexts, file_idx, proof.module_hash, proof.exported_hash)?;
                    if export.file_idx == file_idx {
                        return None;
                    }
                    if files[export.file_idx].meta.lang != il.meta.lang {
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
    let namespace_replacements =
        collect_namespace_member_replacements(files, interner, &contexts, &exports);

    apply_import_replacements(files, replacements);
    apply_namespace_member_replacements(files, namespace_replacements);
}

fn apply_import_replacements(files: &mut [Il], replacements: Vec<Vec<ImportReplacement>>) {
    for (file_idx, file_replacements) in replacements.into_iter().enumerate() {
        for replacement in file_replacements {
            let (rhs, snapshot_evidence) = append_replacement_snapshot(
                &mut files[file_idx],
                replacement.deps,
                replacement.rhs_snapshot,
            );
            replace_assignment_rhs(&mut files[file_idx], replacement.stmt, rhs);
            record_imported_literal_snapshot_evidence(
                &mut files[file_idx],
                rhs,
                replacement.module_hash,
                replacement.exported_hash,
                replacement.import_evidence,
                snapshot_evidence,
            );
        }
    }
}

fn apply_namespace_member_replacements(
    files: &mut [Il],
    replacements: Vec<Vec<NamespaceMemberReplacement>>,
) {
    for (file_idx, file_replacements) in replacements.into_iter().enumerate() {
        for replacement in file_replacements {
            let (rhs, snapshot_evidence) = append_replacement_snapshot(
                &mut files[file_idx],
                replacement.deps,
                replacement.rhs_snapshot,
            );
            replace_node_references(&mut files[file_idx], replacement.node, rhs);
            record_imported_literal_snapshot_evidence(
                &mut files[file_idx],
                rhs,
                replacement.module_hash,
                replacement.exported_hash,
                replacement.import_evidence,
                snapshot_evidence,
            );
        }
    }
}

fn append_replacement_snapshot(
    il: &mut Il,
    deps: Vec<SubtreeSnapshot>,
    rhs_snapshot: SubtreeSnapshot,
) -> (NodeId, Vec<EvidenceId>) {
    let mut snapshot_evidence = Vec::new();
    for dep in deps {
        let dep = append_snapshot(il, &dep);
        snapshot_evidence.extend(dep.evidence);
        prepend_root_statement(il, dep.root);
    }
    let rhs = append_snapshot(il, &rhs_snapshot);
    snapshot_evidence.extend(rhs.evidence);
    (rhs.root, snapshot_evidence)
}

struct FileImportContext {
    top_level: Option<Vec<NodeId>>,
    module_hashes: Vec<u64>,
    rust_module: Option<RustModuleIdentity>,
    binding_uses: Option<BindingUseIndex>,
}

impl FileImportContext {
    fn new(il: &Il, interner: &Interner) -> Self {
        let module_semantics = semantics(il.meta.lang).modules();
        let participates = module_semantics.sibling_literal_exports()
            || module_semantics.java_class_literal_exports()
            || module_semantics.go_import_namespace_facts();
        Self {
            top_level: participates.then(|| collect_top_level_statements(il)),
            module_hashes: file_module_hashes(il),
            rust_module: rust_module_identity(&il.meta.path),
            binding_uses: participates.then(|| BindingUseIndex::new(il, interner)),
        }
    }

    fn module_matches_import_from(&self, importer: &Self, module_hash: u64) -> bool {
        if let (Some(provider), Some(importer)) = (&self.rust_module, &importer.rust_module) {
            return rust_importable_module_hashes(provider, importer).contains(&module_hash);
        }
        if self.module_hashes.contains(&module_hash) {
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests;
