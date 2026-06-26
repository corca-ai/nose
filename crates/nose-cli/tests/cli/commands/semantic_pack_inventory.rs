use super::*;

#[test]
fn semantic_pack_inventory_json_reports_builtin_coverage() {
    let out = run(&["semantic-pack", "inventory", "--format", "json"]);
    let json: serde_json::Value =
        serde_json::from_str(&out).expect("inventory must emit valid JSON");

    assert_eq!(json["schema_version"], 1);
    assert_eq!(json["status"], "ok");
    assert_eq!(json["totals"]["packs"], 49);
    assert_eq!(json["totals"]["builtin_packs"], 49);
    assert_eq!(json["totals"]["positive_fixtures"], 192);
    assert_eq!(json["totals"]["hard_negatives"], 175);
    assert_eq!(json["totals"]["conformance_refs"], 367);
    assert_eq!(json["totals"]["packs_needing_coverage"], 0);
    assert_eq!(
        json["evidence_policy"]["product_output"],
        "required-on-implementation-pr"
    );
    assert_eq!(
        json["evidence_policy"]["performance"],
        "required-on-implementation-pr"
    );

    let packs = inventory_packs(&json);
    assert_go_namespace_pack(packs);
    assert_c_language_pack(packs);
    assert_python_type_domain_pack(packs);
    assert_rust_result_pack(packs);
    assert_swift_collection_factory_pack(packs);
    assert_guava_pack(packs);
    assert_sequence_hof_adapter_pack(packs);
    assert_string_affix_predicate_pack(packs);
    assert_python_iterator_builtin_pack(packs);
    assert_compat_pack(packs);
}

fn assert_string_affix_predicate_pack(packs: &[serde_json::Value]) {
    let string_affix = inventory_pack(packs, "nose.protocols.string_affix_predicates");
    assert_eq!(string_affix["kind"], "ProtocolPack");
    assert_eq!(string_affix["audit"]["exact_capable"], true);
    assert_eq!(string_affix["audit"]["coverage_status"], "covered");
    assert_eq!(
        json_array_strings(&string_affix["declarations"], "contracts"),
        vec!["string_affix.predicate"]
    );
    assert_eq!(
        json_array_strings(&string_affix["conformance"], "positive_refs"),
        vec![
            "string-affix-predicate-python-startswith-positive",
            "string-affix-predicate-python-endswith-positive",
            "string-affix-predicate-java-startswith-positive",
            "string-affix-predicate-java-endswith-positive",
            "string-affix-predicate-rust-starts-with-positive",
            "string-affix-predicate-rust-ends-with-positive",
            "string-affix-predicate-swift-has-prefix-positive",
            "string-affix-predicate-swift-has-suffix-positive",
            "string-affix-predicate-typescript-startswith-positive",
            "string-affix-predicate-typescript-endswith-positive",
            "string-affix-predicate-javascript-startswith-positive",
            "string-affix-predicate-javascript-endswith-positive",
            "string-affix-predicate-go-has-prefix-positive",
            "string-affix-predicate-go-has-suffix-positive",
            "string-affix-predicate-ruby-start-with-positive",
            "string-affix-predicate-ruby-end-with-positive",
            "string-affix-predicate-parameter-coordinate-positive",
            "string-affix-predicate-immutable-binding-coordinate-positive"
        ]
    );
    assert_eq!(
        json_array_strings(&string_affix["conformance"], "hard_negative_refs"),
        vec![
            "string-affix-predicate-direction-mismatch-hard-negative",
            "string-affix-predicate-missing-receiver-proof-hard-negative",
            "string-affix-predicate-non-string-receiver-hard-negative",
            "string-affix-predicate-go-missing-import-hard-negative",
            "string-affix-predicate-go-wrong-namespace-hard-negative",
            "string-affix-predicate-wrong-pack-hard-negative",
            "string-affix-predicate-wrong-producer-hard-negative",
            "string-affix-predicate-unsupported-arity-hard-negative",
            "string-affix-predicate-unsupported-offset-hard-negative",
            "string-affix-predicate-javascript-untyped-receiver-hard-negative",
            "string-affix-predicate-javascript-borrowed-prototype-hard-negative",
            "string-affix-predicate-javascript-custom-same-name-hard-negative",
            "string-affix-predicate-typescript-string-object-wrapper-hard-negative",
            "string-affix-predicate-typescript-nullable-receiver-hard-negative",
            "string-affix-predicate-typescript-optional-receiver-hard-negative",
            "string-affix-predicate-typescript-prototype-patching-hard-negative",
            "string-affix-predicate-typescript-conditional-prototype-patching-hard-negative",
            "string-affix-predicate-typescript-define-property-prototype-patching-hard-negative",
            "string-affix-predicate-typescript-nested-param-string-prototype-patching-hard-negative",
            "string-affix-predicate-typescript-nested-param-object-define-property-hard-negative",
            "string-affix-predicate-typescript-block-scoped-string-prototype-patching-hard-negative",
            "string-affix-predicate-typescript-block-scoped-object-define-property-hard-negative",
            "string-affix-predicate-ruby-untyped-receiver-hard-negative",
            "string-affix-predicate-ruby-custom-same-name-hard-negative",
            "string-affix-predicate-ruby-multi-affix-hard-negative",
            "string-affix-predicate-ruby-wrong-receiver-hard-negative",
            "string-affix-predicate-ruby-direction-mismatch-hard-negative",
            "string-affix-predicate-ruby-monkey-patch-hard-negative",
            "string-affix-predicate-ruby-class-eval-monkey-patch-hard-negative",
            "string-affix-predicate-ruby-define-method-monkey-patch-hard-negative",
            "string-affix-predicate-wrong-parameter-coordinate-hard-negative",
            "string-affix-predicate-dynamic-affix-coordinate-hard-negative",
            "string-affix-predicate-mutated-binding-coordinate-hard-negative",
            "string-affix-predicate-python-tuple-affix-unsupported-hard-negative",
            "string-affix-predicate-javascript-offset-unsupported-hard-negative",
            "string-affix-predicate-java-offset-unsupported-hard-negative"
        ]
    );
    assert_eq!(
        json_array_strings(&string_affix["conformance"], "unsupported_refs"),
        vec![
            "string-affix-predicate-unsupported-arity-hard-negative",
            "string-affix-predicate-unsupported-offset-hard-negative",
            "string-affix-predicate-python-tuple-affix-unsupported-hard-negative",
            "string-affix-predicate-javascript-offset-unsupported-hard-negative",
            "string-affix-predicate-java-offset-unsupported-hard-negative"
        ]
    );
}

fn assert_python_iterator_builtin_pack(packs: &[serde_json::Value]) {
    let iterator_builtins = inventory_pack(packs, "nose.protocols.iterator_builtins");
    assert_eq!(iterator_builtins["kind"], "ProtocolPack");
    assert_eq!(iterator_builtins["audit"]["exact_capable"], true);
    assert_eq!(iterator_builtins["audit"]["coverage_status"], "covered");
    assert_eq!(
        json_array_strings(&iterator_builtins["declarations"], "contracts"),
        vec!["iterator_builtin.call", "free_function_hof.call"]
    );
    assert_eq!(
        json_array_strings(&iterator_builtins["conformance"], "positive_refs"),
        vec![
            "python-iterator-builtin-map-positive",
            "python-iterator-builtin-filter-positive",
            "python-iterator-builtin-zip-positive",
            "python-iterator-builtin-enumerate-positive",
            "python-iterator-builtin-any-terminal-positive",
            "python-iterator-builtin-all-terminal-positive",
            "python-iterator-builtin-materializer-positive"
        ]
    );
    assert_eq!(
        json_array_strings(&iterator_builtins["conformance"], "hard_negative_refs"),
        vec![
            "python-iterator-builtin-shadowed-hard-negative",
            "python-iterator-builtin-wildcard-import-hard-negative",
            "python-iterator-builtin-missing-source-proof-hard-negative",
            "python-iterator-builtin-callback-not-lambda-hard-negative",
            "python-iterator-builtin-missing-materializer-proof-hard-negative",
            "python-iterator-builtin-multi-iterable-map-unsupported-hard-negative",
            "python-iterator-builtin-sorted-reversed-unsupported-hard-negative"
        ]
    );
    assert_eq!(
        json_array_strings(&iterator_builtins["conformance"], "unsupported_refs"),
        vec![
            "python-iterator-builtin-multi-iterable-map-unsupported-hard-negative",
            "python-iterator-builtin-sorted-reversed-unsupported-hard-negative"
        ]
    );
}

fn assert_sequence_hof_adapter_pack(packs: &[serde_json::Value]) {
    let sequence_hof = inventory_pack(packs, "nose.protocols.sequence_hof_adapters");
    assert_eq!(sequence_hof["kind"], "ProtocolPack");
    assert_eq!(sequence_hof["audit"]["exact_capable"], true);
    assert_eq!(sequence_hof["audit"]["coverage_status"], "covered");
    assert_eq!(
        json_array_strings(&sequence_hof["declarations"], "contracts"),
        vec!["sequence_hof.method_call"]
    );
    assert_eq!(
        json_array_strings(&sequence_hof["conformance"], "positive_refs"),
        vec![
            "rust-iterator-hof-map-positive",
            "rust-iterator-hof-filter-positive",
            "rust-iterator-hof-filter-map-positive",
            "rust-iterator-hof-flat-map-positive",
            "rust-iterator-hof-any-terminal-positive",
            "rust-iterator-hof-all-terminal-positive",
            "rust-iterator-hof-count-terminal-positive",
            "swift-sequence-hof-map-positive",
            "swift-sequence-hof-filter-positive",
            "swift-sequence-hof-flat-map-positive",
            "ruby-enumerable-hof-map-positive",
            "ruby-enumerable-hof-collect-positive",
            "ruby-enumerable-hof-select-positive",
            "ruby-enumerable-hof-filter-positive",
            "ruby-enumerable-hof-reject-positive"
        ]
    );
    assert_eq!(
        json_array_strings(&sequence_hof["conformance"], "hard_negative_refs"),
        vec![
            "rust-iterator-hof-custom-method-hard-negative",
            "rust-iterator-hof-missing-receiver-proof-hard-negative",
            "rust-iterator-hof-eager-callback-hard-negative",
            "rust-iterator-hof-missing-terminal-proof-hard-negative",
            "rust-iterator-hof-one-shot-reuse-hard-negative",
            "rust-iterator-hof-collect-vec-hard-negative",
            "rust-iterator-hof-find-unsupported-hard-negative",
            "swift-sequence-hof-set-order-hard-negative",
            "swift-sequence-hof-dictionary-order-hard-negative",
            "swift-sequence-hof-lazy-hard-negative",
            "swift-sequence-hof-throwing-closure-hard-negative",
            "swift-sequence-hof-mutating-closure-hard-negative",
            "swift-sequence-hof-any-sequence-reuse-hard-negative",
            "swift-sequence-hof-compact-map-unsupported-hard-negative",
            "ruby-enumerable-hof-no-block-hard-negative",
            "ruby-enumerable-hof-lazy-enumerator-hard-negative",
            "ruby-enumerable-hof-framework-relation-hard-negative",
            "ruby-enumerable-hof-custom-method-hard-negative",
            "ruby-enumerable-hof-hash-order-hard-negative",
            "ruby-enumerable-hof-set-order-hard-negative",
            "ruby-enumerable-hof-mutating-block-hard-negative",
            "ruby-enumerable-hof-flat-map-unsupported-hard-negative"
        ]
    );
    assert_eq!(
        json_array_strings(&sequence_hof["conformance"], "unsupported_refs"),
        vec![
            "rust-iterator-hof-find-unsupported-hard-negative",
            "swift-sequence-hof-compact-map-unsupported-hard-negative",
            "ruby-enumerable-hof-flat-map-unsupported-hard-negative"
        ]
    );
}

fn assert_rust_result_pack(packs: &[serde_json::Value]) {
    let rust_result = inventory_pack(packs, "nose.rust.stdlib.result");
    assert_eq!(rust_result["kind"], "StdlibPack");
    assert_eq!(rust_result["audit"]["exact_capable"], true);
    assert_eq!(rust_result["audit"]["coverage_status"], "covered");
    assert_eq!(
        json_array_strings(&rust_result["declarations"], "contracts"),
        vec![
            "rust.result.ok.constructor",
            "rust.result.err.constructor",
            "rust.result.is_ok",
            "rust.result.is_err"
        ]
    );
    assert_eq!(
        json_array_strings(&rust_result["conformance"], "positive_refs"),
        vec![
            "rust-result-ok-positive",
            "rust-result-err-positive",
            "rust-result-is-ok-positive",
            "rust-result-is-err-positive"
        ]
    );
    assert_eq!(
        json_array_strings(&rust_result["conformance"], "hard_negative_refs"),
        vec![
            "rust-result-ok-shadow-hard-negative",
            "rust-result-err-shadow-hard-negative",
            "rust-result-predicate-non-result-hard-negative",
            "rust-result-local-type-shadow-hard-negative",
            "rust-result-callback-defaulting-hard-negative"
        ]
    );
}

fn inventory_packs(json: &serde_json::Value) -> &[serde_json::Value] {
    json["packs"]
        .as_array()
        .expect("inventory packs should be an array")
}

fn inventory_pack<'a>(packs: &'a [serde_json::Value], id: &str) -> &'a serde_json::Value {
    packs
        .iter()
        .find(|pack| pack["id"] == id)
        .unwrap_or_else(|| panic!("{id} builtin pack should be reported"))
}

fn assert_go_namespace_pack(packs: &[serde_json::Value]) {
    let go_namespace = inventory_pack(packs, "nose.go.stdlib.namespace_calls");
    assert_eq!(go_namespace["kind"], "StdlibPack");
    assert_eq!(go_namespace["audit"]["exact_capable"], true);
    assert_eq!(go_namespace["audit"]["coverage_status"], "covered");
    assert_eq!(
        json_array_strings(&go_namespace["declarations"], "contracts"),
        vec!["go.stdlib.namespace_call"]
    );
    assert_eq!(
        json_array_strings(&go_namespace["conformance"], "positive_refs"),
        vec![
            "go-stdlib-namespace-call-fmt-print-positive",
            "go-stdlib-namespace-call-slices-contains-positive",
            "go-stdlib-namespace-call-strings-contains-positive"
        ]
    );
    assert_eq!(
        json_array_strings(&go_namespace["conformance"], "hard_negative_refs"),
        vec![
            "go-stdlib-namespace-call-missing-import-hard-negative",
            "go-stdlib-namespace-call-wrong-pack-hard-negative"
        ]
    );
    assert_eq!(
        json_array_strings(&go_namespace["conformance"], "unsupported_refs"),
        Vec::<String>::new()
    );
}

fn assert_c_language_pack(packs: &[serde_json::Value]) {
    let c_language = inventory_pack(packs, "nose.lang.c");
    assert_eq!(c_language["audit"]["exact_capable"], true);
    assert_eq!(c_language["audit"]["coverage_status"], "covered");
    assert_eq!(
        json_array_strings(&c_language["declarations"], "source_fact_producers"),
        vec!["c.source.fact", "c.source.cast.unsigned32"]
    );
    assert_eq!(
        json_array_strings(&c_language["conformance"], "positive_refs"),
        vec![
            "c-unsigned32-byte-lane-cast-positive",
            "c-unsigned32-alias-cast-positive"
        ]
    );
    assert_eq!(
        json_array_strings(&c_language["conformance"], "hard_negative_refs"),
        vec![
            "c-unsigned32-signed-cast-hard-negative",
            "c-unsigned32-non-byte-lane-hard-negative"
        ]
    );
}

fn assert_python_type_domain_pack(packs: &[serde_json::Value]) {
    let python_type_domain = inventory_pack(packs, "nose.python.stdlib.type_domain");
    let aliases = json_array_strings(&python_type_domain["declarations"], "type_domain_aliases");
    assert!(aliases.contains(&"python.stdlib.type-domain-alias.contract:typing.dict:map"));
}

fn assert_swift_collection_factory_pack(packs: &[serde_json::Value]) {
    let swift = inventory_pack(packs, "nose.swift.stdlib.collection_factories");
    assert_eq!(swift["kind"], "StdlibPack");
    assert_eq!(swift["audit"]["exact_capable"], true);
    assert_eq!(swift["audit"]["coverage_status"], "covered");
    assert_eq!(
        json_array_strings(&swift["declarations"], "contracts"),
        vec![
            "swift.collection_factory.array",
            "swift.collection_factory.set",
            "swift.map_factory.dictionary_unique_keys_with_values"
        ]
    );
    assert_eq!(
        json_array_strings(&swift["conformance"], "positive_refs"),
        vec![
            "swift-array-sequence-factory-positive",
            "swift-set-sequence-factory-positive",
            "swift-dictionary-unique-keys-with-values-positive"
        ]
    );
    assert_eq!(
        json_array_strings(&swift["conformance"], "hard_negative_refs"),
        vec![
            "swift-array-shadowed-hard-negative",
            "swift-set-shadowed-hard-negative",
            "swift-dictionary-wrong-label-hard-negative",
            "swift-dictionary-implicit-entry-shape-hard-negative"
        ]
    );
}

fn assert_guava_pack(packs: &[serde_json::Value]) {
    let guava = inventory_pack(
        packs,
        "nose.java.ecosystem.guava.immutable_collection_factories",
    );
    assert_eq!(guava["kind"], "LibraryPack");
    assert_eq!(guava["trust"], "builtin-default");
    assert_eq!(guava["enabled_by_default"], true);
    assert_eq!(guava["audit"]["exact_capable"], true);
    assert_eq!(guava["audit"]["coverage_status"], "covered");
    assert_eq!(
        json_array_strings(&guava["declarations"], "contracts"),
        vec![
            "java.collection_factory.guava_immutable_list_of",
            "java.collection_factory.guava_immutable_set_of",
            "java.map_factory.guava_immutable_map_of"
        ]
    );
    assert_eq!(
        json_array_strings(&guava["conformance"], "positive_refs"),
        vec![
            "java-guava-immutable-list-of-positive",
            "java-guava-immutable-set-of-positive",
            "java-guava-immutable-map-of-positive"
        ]
    );
    assert_eq!(
        json_array_strings(&guava["conformance"], "hard_negative_refs"),
        vec![
            "java-guava-immutable-copy-of-hard-negative",
            "java-guava-immutable-missing-import-hard-negative",
            "java-guava-immutable-wrong-package-hard-negative",
            "java-guava-immutable-shadowed-type-hard-negative"
        ]
    );
}

fn assert_compat_pack(packs: &[serde_json::Value]) {
    let compat = inventory_pack(packs, "nose.first_party");
    assert_eq!(compat["audit"]["exact_capable"], false);
    assert_eq!(compat["audit"]["coverage_status"], "tracked-no-exact-rows");
}
