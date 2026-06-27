use super::bindings::{
    assignment_name, assignment_rhs, collect_statements_for_root, import_dependency_snapshots,
    BindingUseIndex,
};
use super::modules::java_class_module_hashes;
use super::{ExportedBinding, FileImportContext};
use nose_il::{stable_symbol_hash, Il, Interner, NodeId, Symbol, UnitKind};
use nose_semantics::{imported_literal_export_safe, semantics};
use rustc_hash::{FxHashMap, FxHashSet};

pub(super) struct LiteralExports {
    by_key: FxHashMap<(u64, u64), ExportedBinding>,
    records: Vec<LiteralExportRecord>,
}

struct LiteralExportRecord {
    exported_hash: u64,
    binding: ExportedBinding,
}

impl LiteralExports {
    pub(super) fn is_empty(&self) -> bool {
        self.by_key.is_empty() && self.records.is_empty()
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
            return self.unique_record_match(
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
    }
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
