use super::*;

fn manifest_json() -> String {
    r#"{
  "api_version":"nose.semantic-pack.v1",
  "pack":{
    "id":"example.guava.factories",
    "kind":"LibraryPack",
    "version":"1.0.0",
    "display_name":"Guava factories",
    "trust":"external-opt-in",
    "enabled_by_default":false
  },
  "provenance":{
    "provider":{"name":"Example"},
    "license":"MIT",
    "repository":"https://example.invalid/guava"
  },
  "compatibility":{"nose":">=0.19.0 <0.21.0"},
  "supported_languages":["java"],
  "packages":[{
    "ecosystem":"maven",
    "name":"com.google.guava:guava",
    "versions":">=32.0.0 <34.0.0"
  }],
  "declares":{"api_contracts":[{
    "id":"java.guava.immutable-list.of",
    "language":"java",
    "package":{"ecosystem":"maven","name":"com.google.guava:guava"},
    "anchor":"call-node",
    "matcher":"imported-api",
    "import":{
      "role":"type",
      "module":"com.google.common.collect",
      "name":"ImmutableList"
    },
    "call":{
      "shape":"static-method",
      "member":"of",
      "arity":{"kind":"range","min":0,"max":12},
      "receiver":"imported-type"
    },
    "operation":"collection-factory",
    "result_domain":"collection",
    "profiles":{
      "demand":"eager",
      "effects":"pure",
      "exceptions":"may-throw",
      "mutation":"none",
      "identity":"fresh"
    },
    "channel":"near"
  }]}
}"#
    .to_string()
}

fn parse_and_compile(json: &str) -> Result<CompiledSemanticPackV1, String> {
    let manifest =
        serde_json::from_str::<SemanticPackManifestV1>(json).map_err(|error| error.to_string())?;
    compile_manifest_v1(&manifest)
}

#[test]
fn compiles_read_only_deterministic_indexes() {
    let compiled = parse_and_compile(&manifest_json()).expect("valid v1 manifest compiles");

    assert_eq!(compiled.pack_id(), "example.guava.factories");
    assert_eq!(compiled.pack_version(), "1.0.0");
    assert_eq!(compiled.semantic_digest().len(), 71);
    assert!(compiled.semantic_digest().starts_with("sha256:"));
    assert_eq!(
        compiled
            .packages_by_coordinate()
            .values()
            .map(|package| package.versions.as_str())
            .collect::<Vec<_>>(),
        vec![">=32.0.0 <34.0.0"]
    );
    assert_eq!(
        compiled.contracts_by_id().keys().collect::<Vec<_>>(),
        vec![&"java.guava.immutable-list.of".to_string()]
    );
    assert_eq!(compiled.contract_ids_by_coordinate().len(), 1);
    assert_eq!(
        compiled
            .contract_ids_by_operation()
            .get(&SemanticPackV1ProtocolOperation::CollectionFactory),
        Some(&vec!["java.guava.immutable-list.of".to_string()])
    );
}

#[test]
fn json_key_order_does_not_change_digest_or_indexes() {
    let original = manifest_json();
    let value: serde_json::Value = serde_json::from_str(&original).unwrap();
    let reversed = render_with_reversed_object_keys(&value);

    let left = parse_and_compile(&original).unwrap();
    let right = parse_and_compile(&reversed).unwrap();
    assert_eq!(left.semantic_digest(), right.semantic_digest());
    assert_eq!(
        left.packages_by_coordinate(),
        right.packages_by_coordinate()
    );
    assert_eq!(left.contracts_by_id(), right.contracts_by_id());
    assert_eq!(
        left.contract_ids_by_coordinate(),
        right.contract_ids_by_coordinate()
    );
    assert_eq!(
        left.contract_ids_by_operation(),
        right.contract_ids_by_operation()
    );
}

#[test]
fn every_supported_semantic_change_changes_the_digest() {
    let original = manifest_json();
    let baseline = parse_and_compile(&original).unwrap();
    let changed = [
        original.replace(">=32.0.0 <34.0.0", ">=33.0.0 <34.0.0"),
        original.replace(
            "java.guava.immutable-list.of",
            "java.guava.immutable-list.copy-of",
        ),
        original.replace("com.google.guava:guava", "org.example:collections"),
        original.replace("com.google.common.collect", "org.example.collect"),
        original.replace("ImmutableList", "ImmutableCollection"),
        original.replace("\"member\":\"of\"", "\"member\":\"copyOf\""),
        original.replace("\"max\":12", "\"max\":11"),
        original
            .replace("\"role\":\"type\"", "\"role\":\"static-member\"")
            .replace("\"shape\":\"static-method\"", "\"shape\":\"free-function\"")
            .replace("\"receiver\":\"imported-type\"", "\"receiver\":\"none\""),
        original
            .replace(
                "\"kind\":\"range\",\"min\":0,\"max\":12",
                "\"kind\":\"set\",\"values\":[0,2,4,6,8,10,12]",
            )
            .replace(
                "\"operation\":\"collection-factory\"",
                "\"operation\":\"map-factory\"",
            )
            .replace(
                "\"result_domain\":\"collection\"",
                "\"result_domain\":\"map\"",
            ),
        original.replace(
            "\"exceptions\":\"may-throw\"",
            "\"exceptions\":\"no-throw\"",
        ),
    ];

    for candidate in changed {
        let compiled = parse_and_compile(&candidate).expect("changed manifest remains valid");
        assert_ne!(baseline.semantic_digest(), compiled.semantic_digest());
    }
}

#[test]
fn external_exact_requires_closed_profiles_and_source_fixture_coverage() {
    let baseline = parse_and_compile(&manifest_json()).unwrap();
    let mut value: serde_json::Value = serde_json::from_str(&manifest_json()).unwrap();
    value["declares"]["api_contracts"][0]["channel"] = serde_json::json!("external-exact");
    value["declares"]["api_contracts"][0]["profiles"]["exceptions"] = serde_json::json!("no-throw");
    value["conformance"] = serde_json::json!({
        "fixtures": [{
            "id": "positive",
            "row_id": "java.guava.immutable-list.of",
            "kind": "positive",
            "path": "fixtures/positive",
            "dependency": "pom.xml",
            "expectation": "external-exact-match"
        }, {
            "id": "negative",
            "row_id": "java.guava.immutable-list.of",
            "kind": "hard-negative",
            "path": "fixtures/negative",
            "dependency": "pom.xml",
            "expectation": "no-external-exact-match"
        }]
    });
    let exact = parse_and_compile(&serde_json::to_string(&value).unwrap()).unwrap();
    assert_ne!(baseline.semantic_digest(), exact.semantic_digest());
    assert_eq!(exact.conformance_fixtures().len(), 2);

    value["declares"]["api_contracts"][0]["profiles"]["exceptions"] =
        serde_json::json!("may-throw");
    assert!(parse_and_compile(&serde_json::to_string(&value).unwrap()).is_err());
}

#[test]
fn unknown_vocabulary_is_rejected_during_deserialization() {
    let original = manifest_json();
    for invalid in [
        original.replace("\"anchor\":\"call-node\"", "\"anchor\":\"package\""),
        original.replace("\"matcher\":\"imported-api\"", "\"matcher\":\"regex\""),
        original.replace(
            "\"operation\":\"collection-factory\"",
            "\"operation\":\"provider-rewrite\"",
        ),
        original.replace(
            "\"result_domain\":\"collection\"",
            "\"result_domain\":\"tensor\"",
        ),
        original.replace("\"demand\":\"eager\"", "\"demand\":\"lazy\""),
        original.replace("\"effects\":\"pure\"", "\"effects\":\"provider-defined\""),
        original.replace("\"identity\":\"fresh\"", "\"identity\":\"unknown\""),
    ] {
        assert!(parse_and_compile(&invalid).is_err());
    }
}

#[test]
fn provider_matcher_languages_cannot_be_encoded() {
    let original = manifest_json();
    for field in ["regex", "expression", "callback", "selector"] {
        let invalid = original.replace(
            "\"matcher\":\"imported-api\"",
            &format!("\"matcher\":\"imported-api\",\"{field}\":\".*\""),
        );
        assert!(
            parse_and_compile(&invalid).is_err(),
            "field {field} must fail"
        );
    }
}

#[test]
fn invalid_typed_combinations_fail_before_compilation() {
    let original = manifest_json();
    for invalid in [
        original.replace("\"max\":12", "\"max\":33"),
        original.replace("\"min\":0,\"max\":12", "\"min\":12,\"max\":0"),
        original.replace(
            "\"result_domain\":\"collection\"",
            "\"result_domain\":\"map\"",
        ),
        original.replace("\"receiver\":\"imported-type\"", "\"receiver\":\"none\""),
        original.replace("com.google.guava:guava", "not-a-maven-coordinate"),
    ] {
        assert!(parse_and_compile(&invalid).is_err());
    }
}

#[test]
fn local_set_keeps_v1_compilation_separate_from_v0_external_rows() {
    let dir =
        std::env::temp_dir().join(format!("nose_semantic_pack_v1_set_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("pack.json");
    std::fs::write(&path, manifest_json()).unwrap();

    let set = SemanticPackSet::new_local(&[path]).expect("v1 local pack loads");
    let summary = set
        .packs()
        .iter()
        .find(|pack| pack.id == "example.guava.factories")
        .unwrap();
    assert_eq!(summary.api_version, Some(SEMANTIC_PACK_API_VERSION_V1));
    assert!(summary
        .semantic_digest
        .as_deref()
        .is_some_and(|digest| digest.starts_with("sha256:")));
    assert_eq!(summary.influence, SemanticPackInfluence::MetadataOnly);
    assert!(set.external_evidence_producer_rows().is_empty());
    assert!(set.external_contract_rows().is_empty());
    assert!(set.external_value_law_rows().is_empty());
    assert_eq!(set.compiled_external_v1_packs().len(), 1);

    let _ = std::fs::remove_dir_all(&dir);
}

fn render_with_reversed_object_keys(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Object(object) => {
            let entries = object
                .iter()
                .rev()
                .map(|(key, value)| {
                    format!(
                        "{}:{}",
                        serde_json::to_string(key).unwrap(),
                        render_with_reversed_object_keys(value)
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            format!("{{{entries}}}")
        }
        serde_json::Value::Array(array) => format!(
            "[{}]",
            array
                .iter()
                .map(render_with_reversed_object_keys)
                .collect::<Vec<_>>()
                .join(",")
        ),
        scalar => serde_json::to_string(scalar).unwrap(),
    }
}
