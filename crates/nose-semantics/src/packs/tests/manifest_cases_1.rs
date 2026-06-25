use super::*;

#[test]
fn legacy_first_party_pack_aliases_match_builtin_names() {
    assert_eq!(FIRST_PARTY_PACK_ID, BUILTIN_COMPAT_PACK_ID);
    assert_eq!(FIRST_PARTY_VALUE_LAW_PACK_ID, VALUE_GRAPH_LAW_PACK_ID);

    let legacy_compat = first_party_semantic_pack();
    let builtin_compat = builtin_compat_semantic_pack();
    assert_eq!(legacy_compat.id, builtin_compat.id);
    assert_eq!(legacy_compat.hash, builtin_compat.hash);
    assert_eq!(legacy_compat.kind, builtin_compat.kind);

    let legacy_laws = first_party_value_law_pack();
    let value_laws = value_graph_law_pack();
    assert_eq!(legacy_laws.id, value_laws.id);
    assert_eq!(legacy_laws.hash, value_laws.hash);
    assert_eq!(legacy_laws.kind, value_laws.kind);
}

#[test]
fn local_manifest_loads_as_metadata_only_opt_in() {
    let dir = unique_dir("load");
    let path = dir.join("pack.json");
    fs::write(&path, manifest("com.example.pack")).unwrap();
    let set = SemanticPackSet::new_local(&[path]).expect("pack loads");
    assert_eq!(set.packs().len(), 49);
    assert_eq!(set.packs()[1].id, PYTHON_LANGUAGE_PACK_ID);
    assert_eq!(set.packs()[2].id, JS_TS_LANGUAGE_PACK_ID);
    assert_eq!(set.packs()[3].id, GO_LANGUAGE_PACK_ID);
    assert_eq!(set.packs()[4].id, RUST_LANGUAGE_PACK_ID);
    assert_eq!(set.packs()[5].id, JAVA_LANGUAGE_PACK_ID);
    assert_eq!(set.packs()[6].id, C_LANGUAGE_PACK_ID);
    assert_eq!(set.packs()[7].id, RUBY_LANGUAGE_PACK_ID);
    assert_eq!(set.packs()[8].id, SWIFT_LANGUAGE_PACK_ID);
    assert_eq!(set.packs()[9].id, CSS_LANGUAGE_PACK_ID);
    assert_eq!(set.packs()[10].id, HTML_EMBEDDED_LANGUAGE_PACK_ID);
    assert_eq!(
        set.packs()[11].id,
        PYTHON_BUILTIN_COLLECTION_FACTORY_PACK_ID
    );
    assert_eq!(set.packs()[12].id, PYTHON_STDLIB_COLLECTION_FACTORY_PACK_ID);
    assert_eq!(set.packs()[13].id, PYTHON_STDLIB_MATH_PACK_ID);
    assert_eq!(set.packs()[14].id, RUBY_STDLIB_SET_PACK_ID);
    assert_eq!(set.packs()[15].id, RUST_STDLIB_VEC_PACK_ID);
    assert_eq!(set.packs()[16].id, RUST_STDLIB_OPTION_PACK_ID);
    assert_eq!(set.packs()[17].id, RUST_STDLIB_RESULT_PACK_ID);
    assert_eq!(set.packs()[18].id, RUST_STDLIB_INTEGER_METHOD_PACK_ID);
    assert_eq!(set.packs()[19].id, RUST_STDLIB_COLLECTION_FACTORY_PACK_ID);
    assert_eq!(set.packs()[20].id, RUST_STDLIB_MAP_FACTORY_PACK_ID);
    assert_eq!(set.packs()[21].id, SWIFT_STDLIB_COLLECTION_FACTORY_PACK_ID);
    assert_eq!(set.packs()[22].id, JAVA_STDLIB_MATH_PACK_ID);
    assert_eq!(set.packs()[23].id, JAVA_STDLIB_MAP_FACTORY_PACK_ID);
    assert_eq!(set.packs()[24].id, JAVA_STDLIB_MAP_ENTRY_PACK_ID);
    assert_eq!(set.packs()[25].id, JAVA_STDLIB_COLLECTION_FACTORY_PACK_ID);
    assert_eq!(
        set.packs()[26].id,
        JAVA_GUAVA_IMMUTABLE_COLLECTION_FACTORY_PACK_ID
    );
    assert_eq!(
        set.packs()[27].id,
        JAVA_STDLIB_COLLECTION_CONSTRUCTOR_PACK_ID
    );
    assert_eq!(
        set.packs()[28].id,
        JAVA_STDLIB_STATIC_COLLECTION_ADAPTER_PACK_ID
    );
    assert_eq!(set.packs()[29].id, MAP_GET_PROTOCOL_PACK_ID);
    assert_eq!(set.packs()[30].id, MAP_GET_DEFAULT_PROTOCOL_PACK_ID);
    assert_eq!(set.packs()[31].id, FREE_FUNCTION_BUILTIN_PROTOCOL_PACK_ID);
    assert_eq!(set.packs()[32].id, PYTHON_ITERATOR_BUILTIN_PROTOCOL_PACK_ID);
    assert_eq!(set.packs()[33].id, RECEIVER_MEMBERSHIP_PROTOCOL_PACK_ID);
    assert_eq!(set.packs()[34].id, MAP_KEY_VIEW_PROTOCOL_PACK_ID);
    assert_eq!(set.packs()[35].id, PROPERTY_BUILTIN_PROTOCOL_PACK_ID);
    assert_eq!(set.packs()[36].id, BUILTIN_METHOD_CALL_PROTOCOL_PACK_ID);
    assert_eq!(set.packs()[37].id, SEQUENCE_HOF_ADAPTER_PROTOCOL_PACK_ID);
    assert_eq!(set.packs()[38].id, GO_STDLIB_NAMESPACE_CALL_PACK_ID);
    assert_eq!(set.packs()[39].id, ITERATOR_IDENTITY_ADAPTER_PACK_ID);
    assert_eq!(set.packs()[40].id, JS_LIKE_BUILTIN_PROMISE_PACK_ID);
    assert_eq!(set.packs()[41].id, JS_LIKE_BUILTIN_ARRAY_PACK_ID);
    assert_eq!(set.packs()[42].id, JS_LIKE_BUILTIN_BOOLEAN_PACK_ID);
    assert_eq!(set.packs()[43].id, JS_LIKE_BUILTIN_REGEX_PACK_ID);
    assert_eq!(
        set.packs()[44].id,
        JS_LIKE_BUILTIN_STATIC_INDEX_MEMBERSHIP_PACK_ID
    );
    assert_eq!(
        set.packs()[45].id,
        JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PACK_ID
    );
    assert_eq!(set.packs()[46].id, PYTHON_STDLIB_TYPE_DOMAIN_PACK_ID);
    assert_eq!(set.packs()[47].id, VALUE_GRAPH_LAW_PACK_ID);
    let external = &set.packs()[48];
    assert_eq!(external.id, "com.example.pack");
    assert_eq!(external.hash, stable_symbol_hash("com.example.pack"));
    assert_eq!(external.trust, PackTrust::ExternalOptIn);
    assert_eq!(external.source, SemanticPackSource::LocalManifest);
    assert_eq!(external.influence, SemanticPackInfluence::MetadataOnly);
    assert_eq!(external.counts.contracts, 1);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn local_manifest_registers_external_value_law_rows_as_data_only() {
    let dir = unique_dir("external_value_law_rows");
    let path = dir.join("pack.json");
    fs::write(&path, manifest_with_value_law("com.example.laws")).unwrap();

    let set = SemanticPackSet::new_local(std::slice::from_ref(&path)).expect("pack loads");
    let external = set.packs().last().expect("external pack summary");
    assert_eq!(external.id, "com.example.laws");
    assert_eq!(external.influence, SemanticPackInfluence::MetadataOnly);
    assert_eq!(external.counts.value_laws, 1);

    let rows = set.external_value_law_rows();
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.pack_id, "com.example.laws");
    assert_eq!(row.pack_hash, stable_symbol_hash("com.example.laws"));
    assert_eq!(row.manifest_path, path.canonicalize().unwrap());
    assert_eq!(row.law_id, "python.example.numeric-law");
    assert_eq!(
        row.law_hash,
        stable_symbol_hash("python.example.numeric-law")
    );
    assert_eq!(row.channel, SemanticPackChannel::ExactProven);
    assert_eq!(row.proof_status, SemanticPackProofStatus::Proven);
    assert_eq!(row.requirements.len(), 1);
    assert_eq!(row.requirements[0].ref_id, "Domain.Number");
    assert_eq!(row.requirements[0].subject, "operands");
    assert!(row.requirements[0].required);
    assert_eq!(row.requirements[0].same_anchor_as.as_deref(), Some("value"));
    assert_eq!(row.conformance_refs, ["positive", "negative"]);
    assert_eq!(row.semantics["law"], "x + 0 == x");

    assert!(SemanticPackSet::builtin_only()
        .external_value_law_rows()
        .is_empty());
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn local_manifest_registers_external_producer_and_contract_rows_as_data_only() {
    let dir = unique_dir("external_contract_rows");
    let path = dir.join("pack.json");
    fs::write(&path, manifest("com.example.contracts")).unwrap();

    let set = SemanticPackSet::new_local(std::slice::from_ref(&path)).expect("pack loads");
    let external = set.packs().last().expect("external pack summary");
    assert_eq!(external.id, "com.example.contracts");
    assert_eq!(external.influence, SemanticPackInfluence::MetadataOnly);
    assert_eq!(external.counts.evidence_producers, 1);
    assert_eq!(external.counts.contracts, 1);

    let producers = set.external_evidence_producer_rows();
    assert_eq!(producers.len(), 1);
    let producer = &producers[0];
    assert_eq!(producer.pack_id, "com.example.contracts");
    assert_eq!(
        producer.pack_hash,
        stable_symbol_hash("com.example.contracts")
    );
    assert_eq!(producer.manifest_path, path.canonicalize().unwrap());
    assert_eq!(producer.producer_id, "python.library-api.example");
    assert_eq!(
        producer.producer_hash,
        stable_symbol_hash("python.library-api.example")
    );
    assert_eq!(producer.kind, "LibraryApi.Contract");
    assert_eq!(producer.anchors, [SemanticPackAnchor::Node]);
    assert_eq!(producer.channel, SemanticPackChannel::ExactEmpirical);
    assert!(producer.emits.is_empty());
    assert!(producer.requirements.is_empty());
    assert_eq!(
        producer.stable_hash_inputs,
        ["pack.id", "producer.id", "call_span"]
    );
    assert_eq!(producer.conflict_policy, "fail-closed");
    assert!(producer.notes.is_none());

    let contracts = set.external_contract_rows();
    assert_eq!(contracts.len(), 1);
    let contract = &contracts[0];
    assert_eq!(contract.pack_id, "com.example.contracts");
    assert_eq!(
        contract.pack_hash,
        stable_symbol_hash("com.example.contracts")
    );
    assert_eq!(contract.manifest_path, path.canonicalize().unwrap());
    assert_eq!(contract.contract_id, "python.example.contract");
    assert_eq!(
        contract.contract_hash,
        stable_symbol_hash("python.example.contract")
    );
    assert_eq!(contract.surface["kind"], "function");
    assert_eq!(contract.requirements.len(), 1);
    assert_eq!(
        contract.requirements[0].ref_id,
        "python.library-api.example"
    );
    assert_eq!(contract.requirements[0].subject, "call");
    assert!(contract.requirements[0].required);
    assert_eq!(contract.semantics["operation"], "Example");
    assert_eq!(contract.channel, SemanticPackChannel::ExactEmpirical);
    assert_eq!(contract.proof_status, SemanticPackProofStatus::Covered);
    assert_eq!(contract.conformance_refs, ["positive", "negative"]);
    assert!(contract.known_unsupported.is_empty());
    assert!(contract.notes.is_none());

    let builtin = SemanticPackSet::builtin_only();
    assert!(builtin.external_evidence_producer_rows().is_empty());
    assert!(builtin.external_contract_rows().is_empty());
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn external_fixed_result_domain_contract_rows_stay_data_only() {
    let dir = unique_dir("external_fixed_result_domain_rows");
    let path = dir.join("pack.json");
    fs::write(
        &path,
        manifest_with_fixed_result_domain("com.example.fixed-result-domain"),
    )
    .unwrap();

    let set = SemanticPackSet::new_local(std::slice::from_ref(&path))
        .expect("fixed result-domain metadata pack loads");
    let external = set.packs().last().expect("external pack summary");
    assert_eq!(external.id, "com.example.fixed-result-domain");
    assert_eq!(external.influence, SemanticPackInfluence::MetadataOnly);
    assert_eq!(external.counts.evidence_producers, 1);
    assert_eq!(external.counts.contracts, 1);

    let contracts = set.external_contract_rows();
    assert_eq!(contracts.len(), 1);
    let contract = &contracts[0];
    assert_eq!(contract.contract_id, "python.example.contract");
    assert_eq!(contract.semantics["result_domain"]["kind"], "fixed");
    assert_eq!(contract.semantics["result_domain"]["domain"], "Collection");
    assert_eq!(contract.semantics["result_domain"]["subject"], "call");
    assert_eq!(
        contract.requirements[0].ref_id,
        "python.library-api.example"
    );
    assert!(contract.requirements[0].required);

    let preflight = set.external_influence_preflight();
    let contract_row = preflight
        .rows
        .iter()
        .find(|row| row.kind == ExternalRowKind::Contract)
        .expect("contract preflight row");
    assert!(contract_row
        .blockers
        .contains(&ExternalInfluenceBlocker::DataOnlyRegistration));
    assert!(contract_row
        .blockers
        .contains(&ExternalInfluenceBlocker::DependencyBackedEvidenceUnavailable));
    assert!(!contract_row.passed());
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn legacy_string_result_domain_metadata_stays_authorable() {
    let dir = unique_dir("legacy_string_result_domain");
    let path = dir.join("pack.json");
    fs::write(
        &path,
        manifest("com.example.legacy-result-domain").replace(
            r#""operation": "Example",
        "demand": { "arguments": "eager-left-to-right" }"#,
            r#""operation": "Example",
        "result_domain": "NumberOrUnknown",
        "demand": { "arguments": "eager-left-to-right" }"#,
        ),
    )
    .unwrap();

    let set = SemanticPackSet::new_local(std::slice::from_ref(&path))
        .expect("legacy string result-domain metadata pack loads");
    let contracts = set.external_contract_rows();
    assert_eq!(contracts.len(), 1);
    assert_eq!(contracts[0].semantics["result_domain"], "NumberOrUnknown");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn external_rows_report_builtin_id_conflicts_without_rejecting_metadata_only_pack() {
    let dir = unique_dir("external_builtin_row_conflicts");
    let path = dir.join("pack.json");
    let mirror = manifest("com.example.python-stdlib-type-domain-mirror")
        .replace(
            "python.library-api.example",
            "python.stdlib.type-domain-alias-domain",
        )
        .replace("LibraryApi.Contract", "Domain.TypeAlias")
        .replace(
            "python.example.contract",
            "python.stdlib.type-domain-alias.contract",
        )
        .replace("Example", "PythonStdlibTypeDomainAlias");
    fs::write(&path, mirror).unwrap();

    let set =
        SemanticPackSet::new_local(std::slice::from_ref(&path)).expect("metadata-only pack loads");
    let external = set.packs().last().expect("external pack summary");
    assert_eq!(external.id, "com.example.python-stdlib-type-domain-mirror");
    assert_eq!(external.influence, SemanticPackInfluence::MetadataOnly);

    let report = set.external_row_conflicts();
    assert_eq!(report.conflict_count(), 2);
    assert!(!report.passed());
    assert!(report.conflicts.iter().any(|conflict| {
        conflict.kind == ExternalRowKind::EvidenceProducer
            && conflict.row_id == "python.stdlib.type-domain-alias-domain"
            && conflict.conflicting_pack_id == PYTHON_STDLIB_TYPE_DOMAIN_PACK_ID
            && conflict.conflicting_source == SemanticPackSource::CompiledBuiltin
            && conflict.conflicting_manifest_path.is_none()
    }));
    assert!(report.conflicts.iter().any(|conflict| {
        conflict.kind == ExternalRowKind::Contract
            && conflict.row_id == "python.stdlib.type-domain-alias.contract"
            && conflict.conflicting_pack_id == PYTHON_STDLIB_TYPE_DOMAIN_PACK_ID
            && conflict.conflicting_source == SemanticPackSource::CompiledBuiltin
            && conflict.conflicting_manifest_path.is_none()
    }));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn external_rows_report_external_duplicate_id_conflicts_without_rejecting_packs() {
    let dir = unique_dir("external_duplicate_row_conflicts");
    let first = dir.join("a.json");
    let second = dir.join("b.json");
    fs::write(&first, manifest("com.example.first")).unwrap();
    fs::write(&second, manifest("com.example.second")).unwrap();

    let set = SemanticPackSet::new_local(&[first.clone(), second.clone()])
        .expect("metadata-only packs load");
    let first = first.canonicalize().unwrap();
    let report = set.external_row_conflicts();
    assert_eq!(report.conflict_count(), 2);
    assert!(report.conflicts.iter().any(|conflict| {
        conflict.kind == ExternalRowKind::EvidenceProducer
            && conflict.row_id == "python.library-api.example"
            && conflict.external_pack_id == "com.example.second"
            && conflict.conflicting_pack_id == "com.example.first"
            && conflict.conflicting_source == SemanticPackSource::LocalManifest
            && conflict.conflicting_manifest_path.as_deref() == Some(first.as_path())
    }));
    assert!(report.conflicts.iter().any(|conflict| {
        conflict.kind == ExternalRowKind::Contract
            && conflict.row_id == "python.example.contract"
            && conflict.external_pack_id == "com.example.second"
            && conflict.conflicting_pack_id == "com.example.first"
            && conflict.conflicting_source == SemanticPackSource::LocalManifest
            && conflict.conflicting_manifest_path.as_deref() == Some(first.as_path())
    }));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn external_rows_report_builtin_value_law_conflicts_without_rejecting_pack() {
    let dir = unique_dir("external_builtin_law_conflicts");
    let path = dir.join("pack.json");
    let manifest = manifest_with_value_law("com.example.law-mirror").replace(
        "python.example.numeric-law",
        "value-graph.factor-distribute.numeric-common-factor",
    );
    fs::write(&path, manifest).unwrap();

    let set = SemanticPackSet::new_local(&[path]).expect("metadata-only pack loads");
    let report = set.external_row_conflicts();
    assert_eq!(report.conflict_count(), 1);
    let conflict = &report.conflicts[0];
    assert_eq!(conflict.kind, ExternalRowKind::ValueLaw);
    assert_eq!(
        conflict.row_id,
        "value-graph.factor-distribute.numeric-common-factor"
    );
    assert_eq!(conflict.external_pack_id, "com.example.law-mirror");
    assert_eq!(conflict.conflicting_pack_id, VALUE_GRAPH_LAW_PACK_ID);
    assert_eq!(
        conflict.conflicting_source,
        SemanticPackSource::CompiledBuiltin
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn external_influence_preflight_blocks_data_only_rows_before_consumer_admission() {
    let dir = unique_dir("external_influence_preflight");
    let path = dir.join("pack.json");
    fs::write(&path, manifest("com.example.preflight")).unwrap();

    let builtin = SemanticPackSet::builtin_only().external_influence_preflight();
    assert!(builtin.rows.is_empty());
    assert!(builtin.passed());

    let set = SemanticPackSet::new_local(&[path]).expect("metadata-only pack loads");
    let report = set.external_influence_preflight();
    assert_eq!(report.rows.len(), 2);
    assert_eq!(report.blocked_count(), 2);
    assert!(!report.passed());
    for row in &report.rows {
        assert_eq!(row.pack_id, "com.example.preflight");
        assert!(row
            .blockers
            .contains(&ExternalInfluenceBlocker::DataOnlyRegistration));
        assert!(row
            .blockers
            .contains(&ExternalInfluenceBlocker::DependencyBackedEvidenceUnavailable));
        assert!(row
            .blockers
            .contains(&ExternalInfluenceBlocker::ExplicitInfluenceTrustGateMissing));
        assert!(row
            .blockers
            .contains(&ExternalInfluenceBlocker::ExecutableConformanceUnavailable));
        assert!(!row
            .blockers
            .contains(&ExternalInfluenceBlocker::RowConflict));
    }
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn external_influence_preflight_marks_conflicting_rows_as_blocked() {
    let dir = unique_dir("external_preflight_conflicts");
    let path = dir.join("pack.json");
    let mirror = manifest("com.example.python-stdlib-type-domain-mirror")
        .replace(
            "python.library-api.example",
            "python.stdlib.type-domain-alias-domain",
        )
        .replace("LibraryApi.Contract", "Domain.TypeAlias")
        .replace(
            "python.example.contract",
            "python.stdlib.type-domain-alias.contract",
        )
        .replace("Example", "PythonStdlibTypeDomainAlias");
    fs::write(&path, mirror).unwrap();

    let set = SemanticPackSet::new_local(&[path]).expect("metadata-only pack loads");
    let report = set.external_influence_preflight();
    assert_eq!(report.rows.len(), 2);
    assert!(report.rows.iter().all(|row| row
        .blockers
        .contains(&ExternalInfluenceBlocker::RowConflict)));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn external_influence_preflight_marks_both_external_duplicate_rows_as_conflicting() {
    let dir = unique_dir("external_preflight_duplicate_conflicts");
    let first = dir.join("a.json");
    let second = dir.join("b.json");
    fs::write(&first, manifest("com.example.first")).unwrap();
    fs::write(&second, manifest("com.example.second")).unwrap();

    let set = SemanticPackSet::new_local(&[first, second]).expect("metadata-only packs load");
    let report = set.external_influence_preflight();
    assert_eq!(report.rows.len(), 4);
    for pack_id in ["com.example.first", "com.example.second"] {
        let rows = report
            .rows
            .iter()
            .filter(|row| row.pack_id == pack_id)
            .collect::<Vec<_>>();
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|row| row
            .blockers
            .contains(&ExternalInfluenceBlocker::RowConflict)));
    }
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn conformance_check_reports_declared_fixture_files() {
    let dir = unique_dir("conformance_ok");
    let fixture_dir = dir.join("fixtures");
    fs::create_dir_all(&fixture_dir).unwrap();
    fs::write(
        fixture_dir.join("positive.py"),
        "import math\nmath.prod([1, 2])\n",
    )
    .unwrap();
    fs::write(
        fixture_dir.join("negative.py"),
        "math = object()\nmath.prod([1, 2])\n",
    )
    .unwrap();
    let path = dir.join("pack.json");
    fs::write(&path, manifest("com.example.pack")).unwrap();

    let report = check_semantic_pack_conformance(&[path]).expect("conformance loads");

    assert!(report.passed());
    assert_eq!(report.manifest_count(), 1);
    assert_eq!(report.positive_fixture_count(), 1);
    assert_eq!(report.hard_negative_count(), 1);
    assert_eq!(report.fixture_issue_count(), 0);
    let fixture_ids = report.manifests[0]
        .fixtures
        .iter()
        .map(|fixture| (fixture.kind.as_str(), fixture.id.as_str(), fixture.passed()))
        .collect::<Vec<_>>();
    assert_eq!(
        fixture_ids,
        vec![
            ("positive", "positive", true),
            ("hard-negative", "negative", true)
        ]
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn conformance_check_fails_closed_on_missing_fixture_files() {
    let dir = unique_dir("conformance_missing");
    let path = dir.join("pack.json");
    fs::write(&path, manifest("com.example.pack")).unwrap();

    let report = check_semantic_pack_conformance(&[path]).expect("manifest is structurally valid");

    assert!(!report.passed());
    assert_eq!(report.fixture_issue_count(), 2);
    let issues = report.manifests[0]
        .fixtures
        .iter()
        .flat_map(|fixture| fixture.issues.iter().map(|issue| issue.as_str()))
        .collect::<Vec<_>>();
    assert_eq!(issues, vec!["missing-file", "missing-file"]);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn conformance_check_requires_fixture_paths_and_expectations() {
    let dir = unique_dir("conformance_metadata");
    let path = dir.join("pack.json");
    fs::write(
        &path,
        manifest("com.example.pack")
            .replace(
                r#",
      "path": "fixtures/positive.py",
      "expectation": "exact-contract-open""#,
                "",
            )
            .replace(
                r#",
      "path": "fixtures/negative.py",
      "expectation": "exact-contract-closed""#,
                "",
            ),
    )
    .unwrap();

    let report = check_semantic_pack_conformance(&[path]).expect("manifest is structurally valid");

    assert!(!report.passed());
    assert_eq!(report.fixture_issue_count(), 4);
    let issues = report.manifests[0]
        .fixtures
        .iter()
        .flat_map(|fixture| fixture.issues.iter().map(|issue| issue.as_str()))
        .collect::<Vec<_>>();
    assert_eq!(
        issues,
        vec![
            "missing-path",
            "missing-expectation",
            "missing-path",
            "missing-expectation"
        ]
    );
    let _ = fs::remove_dir_all(dir);
}
