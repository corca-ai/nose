mod provider;

use self::provider::{provider_miss, ProviderMiss};
use super::bindings::{assignment_name, import_binding_proof};
use super::exports::{collect_literal_exports, LiteralExports};
use super::FileImportContext;
use nose_il::{Corpus, EvidenceKind, Il, ImportEvidenceKind, Interner};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize)]
pub struct ImportSnapshotCensus {
    pub summary: ImportSnapshotSummary,
    pub snapshots_by_language: Vec<ImportSnapshotCount>,
    pub misses_by_reason: Vec<ImportSnapshotCount>,
    pub misses_by_language: Vec<ImportSnapshotCount>,
    pub misses: Vec<ImportSnapshotMiss>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ImportSnapshotSummary {
    pub snapshot_records: usize,
    pub unresolved_binding_imports: usize,
    pub reported_misses: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct ImportSnapshotCount {
    pub key: String,
    pub count: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct ImportSnapshotMiss {
    pub snapshot_kind: &'static str,
    pub reason: &'static str,
    pub language: &'static str,
    pub importer_file: String,
    pub importer_line: u32,
    pub provider_file: Option<String>,
    pub provider_line: Option<u32>,
    pub module_hash: u64,
    pub exported_hash: u64,
}

pub fn imported_immutable_snapshot_census(corpus: &Corpus) -> ImportSnapshotCensus {
    let contexts: Vec<FileImportContext> = corpus
        .files
        .iter()
        .map(|il| FileImportContext::new(il, &corpus.interner))
        .collect();
    let snapshots_by_language = snapshot_records_by_language(&corpus.files);
    let snapshot_records = snapshots_by_language.values().sum();
    let exports = collect_literal_exports(&corpus.files, &corpus.interner, &contexts);
    let misses =
        unresolved_binding_import_misses(&corpus.files, &corpus.interner, &contexts, &exports);
    let unresolved_binding_imports = misses.len();

    ImportSnapshotCensus {
        summary: ImportSnapshotSummary {
            snapshot_records,
            unresolved_binding_imports,
            reported_misses: misses.len(),
        },
        snapshots_by_language: count_rows(snapshots_by_language),
        misses_by_reason: count_rows(count_misses_by(&misses, |miss| miss.reason)),
        misses_by_language: count_rows(count_misses_by(&misses, |miss| miss.language)),
        misses,
    }
}

fn snapshot_records_by_language(files: &[Il]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for il in files {
        let count = il
            .evidence
            .iter()
            .filter(|record| {
                matches!(
                    record.kind,
                    EvidenceKind::Import(ImportEvidenceKind::ImportedLiteralSnapshot { .. })
                )
            })
            .count();
        if count > 0 {
            *counts.entry(il.meta.lang.name().to_string()).or_default() += count;
        }
    }
    counts
}

fn unresolved_binding_import_misses(
    files: &[Il],
    interner: &Interner,
    contexts: &[FileImportContext],
    exports: &LiteralExports,
) -> Vec<ImportSnapshotMiss> {
    let mut misses = Vec::new();
    for (file_idx, il) in files.iter().enumerate() {
        let context = &contexts[file_idx];
        let Some(top_level) = context.top_level.as_deref() else {
            continue;
        };
        let Some(binding_uses) = context.binding_uses.as_ref() else {
            continue;
        };
        for &stmt in top_level {
            let Some(local) = assignment_name(il, stmt) else {
                continue;
            };
            let Some(proof) = import_binding_proof(il, stmt) else {
                continue;
            };
            let provider = if binding_uses.binding_mutated(il, local, stmt) {
                ProviderMiss {
                    reason: "importer-binding-mutated",
                    provider_file: None,
                    provider_line: None,
                }
            } else {
                provider_miss(
                    files,
                    interner,
                    contexts,
                    exports,
                    file_idx,
                    proof.module_hash,
                    proof.exported_hash,
                )
            };
            misses.push(ImportSnapshotMiss {
                snapshot_kind: "binding-import",
                reason: provider.reason,
                language: il.meta.lang.name(),
                importer_file: il.meta.path.clone(),
                importer_line: il.node(stmt).span.start_line,
                provider_file: provider.provider_file,
                provider_line: provider.provider_line,
                module_hash: proof.module_hash,
                exported_hash: proof.exported_hash,
            });
        }
    }
    misses.sort_by(|a, b| {
        a.reason
            .cmp(b.reason)
            .then(a.importer_file.cmp(&b.importer_file))
            .then(a.importer_line.cmp(&b.importer_line))
            .then(a.module_hash.cmp(&b.module_hash))
            .then(a.exported_hash.cmp(&b.exported_hash))
    });
    misses
}

fn count_misses_by(
    misses: &[ImportSnapshotMiss],
    key: impl Fn(&ImportSnapshotMiss) -> &'static str,
) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for miss in misses {
        *counts.entry(key(miss).to_string()).or_default() += 1;
    }
    counts
}

fn count_rows(counts: BTreeMap<String, usize>) -> Vec<ImportSnapshotCount> {
    let mut rows: Vec<_> = counts
        .into_iter()
        .map(|(key, count)| ImportSnapshotCount { key, count })
        .collect();
    rows.sort_by(|a, b| b.count.cmp(&a.count).then(a.key.cmp(&b.key)));
    rows
}
