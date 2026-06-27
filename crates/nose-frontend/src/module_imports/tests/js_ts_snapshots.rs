use super::super::bindings::{assignment_name, assignment_rhs, collect_top_level_statements};
use super::support::{
    remove_library_api_evidence_by_rule, resolve_importer, resolve_snapshot_count, snapshot_count,
};
use nose_il::{stable_symbol_hash, EvidenceKind, FileId, Interner, Lang, NodeKind};
use nose_semantics::{
    library_api_contract_evidence_for_call, library_js_like_map_constructor_contract,
    library_js_like_set_constructor_contract, LibraryApiEvidenceStatus,
    JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PACK_ID,
    JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PRODUCER_ID,
};

#[test]
fn js_map_provider_requires_constructor_library_api_evidence_for_snapshot() {
    let interner = Interner::new();
    let (provider, importer) = js_provider_and_importer(
        "export const LOOKUP = new Map([[\"red\", 1], [\"blue\", 2]]);\n",
        "import { LOOKUP } from './tables';\nexport function lookup(key, other) { return LOOKUP.get(key) ?? 0; }\n",
        &interner,
    );
    assert_eq!(
        resolve_snapshot_count(provider.clone(), importer.clone(), &interner),
        1
    );

    let mut missing_api = provider;
    remove_library_api_evidence_by_rule(
        &mut missing_api,
        JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PRODUCER_ID,
    );
    assert_eq!(
        resolve_snapshot_count(missing_api, importer, &interner),
        0,
        "JS import/symbol proof must not prove provider Map construction without LibraryApi evidence"
    );
}

#[test]
fn js_set_provider_requires_constructor_library_api_evidence_for_snapshot() {
    let interner = Interner::new();
    let (provider, importer) = js_provider_and_importer(
        "export const VALUES = new Set([\"red\", \"blue\"]);\n",
        "import { VALUES } from './tables';\nexport function member(value, other) { return VALUES.has(value); }\n",
        &interner,
    );
    assert_eq!(
        resolve_snapshot_count(provider.clone(), importer.clone(), &interner),
        1
    );

    let mut missing_api = provider;
    remove_library_api_evidence_by_rule(
        &mut missing_api,
        JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PRODUCER_ID,
    );
    assert_eq!(
        resolve_snapshot_count(missing_api, importer, &interner),
        0,
        "JS import/symbol proof must not prove provider Set construction without LibraryApi evidence"
    );
}

#[test]
fn js_map_provider_snapshot_replaces_import_used_by_lookup_function() {
    let interner = Interner::new();
    let (provider, importer) = js_provider_and_importer(
        "export const LOOKUP = new Map([[\"red\", 1], [\"blue\", 2]]);\n",
        "import { LOOKUP } from './tables';\nexport function lookup(key, other) { return LOOKUP.get(key) ?? 0; }\n",
        &interner,
    );
    let importer = resolve_importer(provider, importer, &interner);
    assert_eq!(snapshot_count(&importer), 1);
    let rhs = replaced_import_rhs(&importer, &interner, "LOOKUP");
    assert_eq!(importer.kind(rhs), NodeKind::Call);
    assert_eq!(
        importer.node(rhs).span.file,
        FileId(0),
        "copied provider RHS keeps provider source origin"
    );
    let contract =
        library_js_like_map_constructor_contract(Lang::JavaScript, "Map").expect("Map contract");
    assert_eq!(
        library_api_contract_evidence_for_call(
            &importer,
            &interner,
            rhs,
            contract.id,
            contract.callee,
            1
        ),
        LibraryApiEvidenceStatus::Admitted
    );
}

#[test]
fn js_set_provider_snapshot_replaces_import_used_by_membership_function() {
    let interner = Interner::new();
    let (provider, importer) = js_provider_and_importer(
        "export const VALUES = new Set([\"red\", \"blue\"]);\n",
        "import { VALUES } from './tables';\nexport function member(value, other) { return VALUES.has(value); }\n",
        &interner,
    );
    let importer = resolve_importer(provider, importer, &interner);
    assert_eq!(snapshot_count(&importer), 1);
    let rhs = replaced_import_rhs(&importer, &interner, "VALUES");
    assert_eq!(importer.kind(rhs), NodeKind::Call);
    assert_eq!(
        importer.node(rhs).span.file,
        FileId(0),
        "copied provider RHS keeps provider source origin"
    );
    let contract =
        library_js_like_set_constructor_contract(Lang::JavaScript, "Set").expect("Set contract");
    assert_eq!(
        library_api_contract_evidence_for_call(
            &importer,
            &interner,
            rhs,
            contract.id,
            contract.callee,
            1
        ),
        LibraryApiEvidenceStatus::Admitted
    );
}

#[test]
fn js_provider_snapshot_rejects_shadowed_map_or_set_global() {
    let interner = Interner::new();
    let (map_provider, map_importer) = js_provider_and_importer(
        "function Map(_entries) { return { get: function() { return 9; } }; }\nexport const LOOKUP = new Map([[\"red\", 1], [\"blue\", 2]]);\n",
        "import { LOOKUP } from './tables';\nexport function lookup(key, other) { return LOOKUP.get(key) ?? 0; }\n",
        &interner,
    );
    assert_eq!(
        resolve_snapshot_count(map_provider, map_importer, &interner),
        0,
        "provider-local Map shadowing must block imported Map snapshots"
    );

    let (set_provider, set_importer) = js_provider_and_importer(
        "function Set(_values) { return { has: function() { return false; } }; }\nexport const VALUES = new Set([\"red\", \"blue\"]);\n",
        "import { VALUES } from './tables';\nexport function member(value, other) { return VALUES.has(value); }\n",
        &interner,
    );
    assert_eq!(
        resolve_snapshot_count(set_provider, set_importer, &interner),
        0,
        "provider-local Set shadowing must block imported Set snapshots"
    );
}

#[test]
fn typescript_map_and_set_providers_snapshot_with_constructor_provenance() {
    let interner = Interner::new();
    let (map_provider, map_importer) = ts_provider_and_importer(
        "export const LOOKUP = new Map<string, number>([[\"red\", 1], [\"blue\", 2]]);\n",
        "import { LOOKUP } from './tables';\nexport function lookup(key: string, other: string): number { return LOOKUP.get(key) ?? 0; }\n",
        &interner,
    );
    assert_eq!(
        resolve_snapshot_count(map_provider, map_importer, &interner),
        1,
        "TypeScript Map constructor snapshots should use the same JS-like constructor proof"
    );

    let (set_provider, set_importer) = ts_provider_and_importer(
        "export const VALUES = new Set<string>([\"red\", \"blue\"]);\n",
        "import { VALUES } from './tables';\nexport function member(value: string, other: string): boolean { return VALUES.has(value); }\n",
        &interner,
    );
    assert_eq!(
        resolve_snapshot_count(set_provider, set_importer, &interner),
        1,
        "TypeScript Set constructor snapshots should use the same JS-like constructor proof"
    );
}

#[test]
fn copied_js_constructor_evidence_uses_collection_constructor_pack() {
    let interner = Interner::new();
    let (provider, importer) = js_provider_and_importer(
        "export const LOOKUP = new Map([[\"red\", 1], [\"blue\", 2]]);\n",
        "import { LOOKUP } from './tables';\nexport function lookup(key, other) { return LOOKUP.get(key) ?? 0; }\n",
        &interner,
    );
    let importer = resolve_importer(provider, importer, &interner);
    let api_rule = stable_symbol_hash(JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PRODUCER_ID);
    let api_pack = stable_symbol_hash(JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PACK_ID);
    assert!(
        importer.evidence.iter().any(|record| {
            matches!(record.kind, EvidenceKind::LibraryApi(_))
                && record.provenance.pack_hash == Some(api_pack)
                && record.provenance.rule_hash == Some(api_rule)
        }),
        "copied JS constructor snapshot should retain LibraryApi constructor evidence"
    );
}

fn js_provider_and_importer(
    provider_src: &str,
    importer_src: &str,
    interner: &Interner,
) -> (nose_il::Il, nose_il::Il) {
    provider_and_importer(
        provider_src,
        importer_src,
        Lang::JavaScript,
        "tables.js",
        "consumer.js",
        interner,
    )
}

fn ts_provider_and_importer(
    provider_src: &str,
    importer_src: &str,
    interner: &Interner,
) -> (nose_il::Il, nose_il::Il) {
    provider_and_importer(
        provider_src,
        importer_src,
        Lang::TypeScript,
        "tables.ts",
        "consumer.ts",
        interner,
    )
}

fn provider_and_importer(
    provider_src: &str,
    importer_src: &str,
    lang: Lang,
    provider_path: &str,
    importer_path: &str,
    interner: &Interner,
) -> (nose_il::Il, nose_il::Il) {
    let provider = crate::lower_source(
        FileId(0),
        provider_path,
        provider_src.as_bytes(),
        lang,
        interner,
    )
    .expect("lower provider");
    let importer = crate::lower_source(
        FileId(1),
        importer_path,
        importer_src.as_bytes(),
        lang,
        interner,
    )
    .expect("lower importer");
    (provider, importer)
}

fn replaced_import_rhs(il: &nose_il::Il, interner: &Interner, name: &str) -> nose_il::NodeId {
    let import_stmt = collect_top_level_statements(il)
        .into_iter()
        .find(|&stmt| assignment_name(il, stmt).is_some_and(|sym| interner.resolve(sym) == name))
        .expect("import assignment should remain as replacement anchor");
    assignment_rhs(il, import_stmt).expect("import assignment rhs")
}
