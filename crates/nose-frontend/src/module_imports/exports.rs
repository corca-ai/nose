use super::bindings::{
    assignment_name, assignment_rhs, collect_statements_for_root, import_dependency_snapshots,
    BindingUseIndex,
};
use super::modules::java_class_module_hashes;
use super::{ExportedBinding, FileImportContext};
use nose_il::{
    stable_symbol_hash, EvidenceAnchor, EvidenceEmitter, EvidenceKind, EvidenceRecord,
    EvidenceStatus, Il, ImportEvidenceKind, Interner, NodeId, Symbol, UnitKind,
};
use nose_semantics::{imported_literal_export_safe, language_core_evidence_provenance, semantics};
use rustc_hash::{FxHashMap, FxHashSet};

pub(super) struct LiteralExports {
    by_key: FxHashMap<(u64, u64), ExportedBinding>,
    records: Vec<LiteralExportRecord>,
    reexports: Vec<ReExportRecord>,
}

struct LiteralExportRecord {
    exported_hash: u64,
    binding: ExportedBinding,
}

pub(super) struct ReExportRecord {
    pub(super) file_idx: usize,
    pub(super) provider_line: u32,
    pub(super) local_exported_hash: u64,
    pub(super) target_module_hash: u64,
    pub(super) target_exported_hash: u64,
}

impl LiteralExports {
    pub(super) fn is_empty(&self) -> bool {
        self.by_key.is_empty() && self.records.is_empty() && self.reexports.is_empty()
    }

    pub(super) fn iter_keyed(&self) -> impl Iterator<Item = (&(u64, u64), &ExportedBinding)> {
        self.by_key.iter()
    }

    pub(super) fn get(
        &self,
        contexts: &[FileImportContext],
        importer_file_idx: usize,
        module_hash: u64,
        exported_hash: u64,
    ) -> Option<&ExportedBinding> {
        if contexts[importer_file_idx].rust_module.is_some() {
            return self.unique_rust_record_match(
                contexts,
                importer_file_idx,
                module_hash,
                exported_hash,
            );
        }
        if let Some(export) = self.by_key.get(&(module_hash, exported_hash)) {
            return Some(export);
        }
        self.unique_record_match(contexts, importer_file_idx, module_hash, exported_hash)
    }

    fn unique_rust_record_match(
        &self,
        contexts: &[FileImportContext],
        importer_file_idx: usize,
        module_hash: u64,
        exported_hash: u64,
    ) -> Option<&ExportedBinding> {
        let direct =
            self.unique_record_match(contexts, importer_file_idx, module_hash, exported_hash);
        let reexport = self.unique_reexport_binding_match(
            contexts,
            importer_file_idx,
            module_hash,
            exported_hash,
        );
        match (direct, reexport) {
            (Some(_), Some(_)) => None,
            (Some(binding), None) | (None, Some(binding)) => Some(binding),
            (None, None) => None,
        }
    }

    fn unique_record_match(
        &self,
        contexts: &[FileImportContext],
        importer_file_idx: usize,
        module_hash: u64,
        exported_hash: u64,
    ) -> Option<&ExportedBinding> {
        let importer_context = &contexts[importer_file_idx];
        let mut matched = self
            .records
            .iter()
            .filter(|record| record.exported_hash == exported_hash)
            .filter(|record| {
                contexts[record.binding.file_idx]
                    .module_matches_import_from(importer_context, module_hash)
            });
        let first = matched.next()?;
        if matched.next().is_some() {
            return None;
        }
        Some(&first.binding)
    }

    fn unique_reexport_binding_match(
        &self,
        contexts: &[FileImportContext],
        importer_file_idx: usize,
        module_hash: u64,
        exported_hash: u64,
    ) -> Option<&ExportedBinding> {
        let reexport =
            self.unique_reexport_record(contexts, importer_file_idx, module_hash, exported_hash)?;
        self.unique_record_match(
            contexts,
            reexport.file_idx,
            reexport.target_module_hash,
            reexport.target_exported_hash,
        )
    }

    pub(super) fn unique_reexport_record(
        &self,
        contexts: &[FileImportContext],
        importer_file_idx: usize,
        module_hash: u64,
        exported_hash: u64,
    ) -> Option<&ReExportRecord> {
        let candidates = self.reexport_record_candidates(
            contexts,
            importer_file_idx,
            module_hash,
            exported_hash,
        );
        if candidates.len() == 1 {
            Some(candidates[0])
        } else {
            None
        }
    }

    pub(super) fn reexport_record_candidates(
        &self,
        contexts: &[FileImportContext],
        importer_file_idx: usize,
        module_hash: u64,
        exported_hash: u64,
    ) -> Vec<&ReExportRecord> {
        let importer_context = &contexts[importer_file_idx];
        self.reexports
            .iter()
            .filter(|record| record.local_exported_hash == exported_hash)
            .filter(|record| {
                contexts[record.file_idx].module_matches_import_from(importer_context, module_hash)
            })
            .collect()
    }

    pub(super) fn get_exact(
        &self,
        module_hash: u64,
        exported_hash: u64,
    ) -> Option<&ExportedBinding> {
        self.by_key.get(&(module_hash, exported_hash))
    }
}

pub(super) fn collect_literal_exports(
    files: &[Il],
    interner: &Interner,
    contexts: &[FileImportContext],
) -> LiteralExports {
    let mut exports = FxHashMap::default();
    let mut ambiguous = FxHashSet::default();
    let mut records = Vec::new();
    let mut reexports = Vec::new();
    for (file_idx, il) in files.iter().enumerate() {
        let context = &contexts[file_idx];
        if context.rust_module.is_some() {
            collect_reexport_records(il, file_idx, &mut reexports);
        }
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
                    records: &mut records,
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
                    records: &mut records,
                },
            );
        }
    }
    for key in ambiguous {
        exports.remove(&key);
    }
    LiteralExports {
        by_key: exports,
        records,
        reexports,
    }
}

fn collect_reexport_records(il: &Il, file_idx: usize, out: &mut Vec<ReExportRecord>) {
    for record in &il.evidence {
        let EvidenceKind::Import(ImportEvidenceKind::ReExportBinding {
            target_module_hash,
            target_exported_hash,
        }) = record.kind
        else {
            continue;
        };
        let EvidenceAnchor::Binding { span, local_hash } = record.anchor else {
            continue;
        };
        if !trusted_language_core_record(il, record) {
            continue;
        }
        out.push(ReExportRecord {
            file_idx,
            provider_line: span.start_line,
            local_exported_hash: local_hash,
            target_module_hash,
            target_exported_hash,
        });
    }
}

fn trusted_language_core_record(il: &Il, record: &EvidenceRecord) -> bool {
    if record.status != EvidenceStatus::Asserted
        || record.provenance.emitter != EvidenceEmitter::Builtin
        || !il.evidence_dependencies_asserted(record)
    {
        return false;
    }
    let (pack_id, producer_id) = language_core_evidence_provenance(il.meta.lang);
    record.provenance.pack_hash == Some(stable_symbol_hash(pack_id))
        && record.provenance.rule_hash == Some(stable_symbol_hash(producer_id))
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
    records: &'a mut Vec<LiteralExportRecord>,
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
        out.records.push(LiteralExportRecord {
            exported_hash: exported,
            binding: ExportedBinding {
                file_idx: scope.file_idx,
                deps: deps.clone(),
                rhs,
            },
        });
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
