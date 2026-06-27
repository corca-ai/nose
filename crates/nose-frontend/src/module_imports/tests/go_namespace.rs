use super::super::bindings::collect_top_level_statements;
use super::support::{resolve_importer, resolve_snapshot_count, snapshot_count};
use nose_il::{
    stable_symbol_hash, EvidenceKind, FileId, ImportEvidenceKind, Interner, Lang, NodeKind,
};

fn go_provider_and_importer(
    provider_src: &str,
    importer_src: &str,
    interner: &Interner,
) -> (nose_il::Il, nose_il::Il) {
    let provider = crate::lower_source(
        FileId(0),
        "tables.go",
        provider_src.as_bytes(),
        Lang::Go,
        interner,
    )
    .expect("lower Go provider");
    let importer = crate::lower_source(
        FileId(1),
        "consumer.go",
        importer_src.as_bytes(),
        Lang::Go,
        interner,
    )
    .expect("lower Go importer");
    (provider, importer)
}

#[test]
fn go_namespace_member_snapshot_replaces_imported_map_field() {
    let interner = Interner::new();
    let provider_src = "package tables\nvar Lookup = map[string]int{\"red\": 1, \"blue\": 2}\n";
    let importer_src = "package consumer\nimport \"tables\"\nfunc lookup(key string) int { return tables.Lookup[key] }\n";
    let (provider, importer) = go_provider_and_importer(provider_src, importer_src, &interner);

    let importer = resolve_importer(provider, importer, &interner);
    assert_eq!(snapshot_count(&importer), 1);
    let index = importer
        .nodes
        .iter()
        .enumerate()
        .find_map(|(idx, node)| {
            (node.kind == NodeKind::Index).then_some(nose_il::NodeId(idx as u32))
        })
        .expect("consumer map lookup index");
    let map_operand = importer.children(index)[0];
    assert_eq!(importer.kind(map_operand), NodeKind::Seq);
    assert_eq!(
        importer.node(map_operand).span.file,
        FileId(0),
        "namespace member replacement should point at provider-owned snapshot"
    );
    assert!(
        importer.evidence.iter().any(|record| {
            matches!(
                record.kind,
                EvidenceKind::Import(ImportEvidenceKind::ImportedLiteralSnapshot {
                    module_hash,
                    exported_hash,
                    root_kind: NodeKind::Seq,
                }) if module_hash == stable_symbol_hash("tables")
                    && exported_hash == stable_symbol_hash("Lookup")
            )
        }),
        "Go namespace member snapshots must retain module/export provenance"
    );
}

#[test]
fn go_namespace_member_snapshot_rejects_mutated_provider_binding() {
    let interner = Interner::new();
    let provider_src = "package tables\nvar Lookup = map[string]int{\"red\": 1}\nfunc init() { Lookup[\"blue\"] = 2 }\n";
    let importer_src = "package consumer\nimport \"tables\"\nfunc lookup(key string) int { return tables.Lookup[key] }\n";
    let (provider, importer) = go_provider_and_importer(provider_src, importer_src, &interner);

    assert_eq!(
        resolve_snapshot_count(provider, importer, &interner),
        0,
        "provider-side map mutation must close namespace member provenance"
    );
}

#[test]
fn go_namespace_member_snapshot_rejects_consumer_namespace_shadow() {
    let interner = Interner::new();
    let provider_src = "package tables\nvar Lookup = map[string]int{\"red\": 1}\n";
    let importer_src = "package consumer\nimport \"tables\"\nfunc lookup(tables any, key string) int { return tables.Lookup[key] }\n";
    let (provider, importer) = go_provider_and_importer(provider_src, importer_src, &interner);

    assert_eq!(
        resolve_snapshot_count(provider, importer, &interner),
        0,
        "consumer-local namespace shadowing must not inherit import proof"
    );
}

#[test]
fn go_namespace_import_without_member_use_does_not_snapshot_export() {
    let interner = Interner::new();
    let provider_src = "package tables\nvar Lookup = map[string]int{\"red\": 1}\n";
    let importer_src = "package consumer\nimport \"tables\"\nfunc lookup() int { return 0 }\n";
    let (provider, importer) = go_provider_and_importer(provider_src, importer_src, &interner);
    let importer = resolve_importer(provider, importer, &interner);

    assert_eq!(snapshot_count(&importer), 0);
    assert!(
        collect_top_level_statements(&importer)
            .into_iter()
            .any(|stmt| importer.kind(stmt) == NodeKind::Assign),
        "the namespace import assignment should remain as an import proof anchor"
    );
}
