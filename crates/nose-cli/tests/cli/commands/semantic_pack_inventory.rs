use super::*;

#[test]
fn semantic_pack_inventory_json_reports_builtin_coverage() {
    let out = run(&["semantic-pack", "inventory", "--format", "json"]);
    let json: serde_json::Value =
        serde_json::from_str(&out).expect("inventory must emit valid JSON");

    assert_eq!(json["schema_version"], 1);
    assert_eq!(json["status"], "ok");
    assert_eq!(json["totals"]["packs"], 44);
    assert_eq!(json["totals"]["builtin_packs"], 44);
    assert_eq!(json["totals"]["hard_negatives"], 89);
    assert_eq!(json["totals"]["conformance_refs"], 230);
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
    assert_guava_pack(packs);
    assert_compat_pack(packs);
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
            "go-stdlib-namespace-call-strings-has-prefix-positive",
            "go-stdlib-namespace-call-strings-has-suffix-positive",
            "go-stdlib-namespace-call-slices-contains-positive"
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
