#![allow(
    clippy::cognitive_complexity,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]

use super::*;
use std::collections::HashSet;
use std::fs;

fn unique_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nose_semantic_pack_{tag}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn assert_source_fact_language_descriptor(
    pack_id: &str,
    supported_languages: &[&str],
    langs: &[nose_il::Lang],
    file_extensions: &[&str],
    parser: &str,
    lowering_entrypoint: &str,
    core_producer_id: &str,
    source_fact_producer_id: &str,
) {
    let descriptor = builtin_pack_descriptor(pack_id).expect("language descriptor");
    assert_eq!(descriptor.kind, SemanticPackKind::LanguagePack);
    assert_eq!(descriptor.supported_languages, supported_languages);
    assert!(descriptor.supported_packages.is_empty());
    let language = descriptor
        .language
        .expect("language descriptor should expose binding metadata");
    assert_eq!(language.langs, langs);
    assert_eq!(language.file_extensions, file_extensions);
    assert_eq!(language.parser, parser);
    assert_eq!(language.lowering_entrypoint, lowering_entrypoint);
    assert_eq!(
        descriptor.evidence_producer_ids,
        &[core_producer_id, source_fact_producer_id]
    );
    assert_eq!(
        descriptor.source_fact_producer_ids,
        &[source_fact_producer_id]
    );
    assert!(descriptor.contract_ids.is_empty());
    assert_eq!(descriptor.counts().evidence_producers, 2);
    assert_eq!(descriptor.counts().contracts, 0);
    assert_eq!(descriptor.counts().positive_fixtures, 0);
    assert_eq!(descriptor.counts().hard_negatives, 0);
}

// nose-ignore: inline semantic-pack manifest fixture; keeping the JSON shape visible matters here.
fn manifest(id: &str) -> String {
    format!(
        r#"{{
  "api_version": "nose.semantic-pack.v0",
  "pack": {{
    "id": "{id}",
    "kind": "LibraryPack",
    "version": "0.1.0",
    "display_name": "Example",
    "trust": "external-opt-in",
    "enabled_by_default": false
  }},
  "provenance": {{
    "provider": {{ "name": "Example" }},
    "license": "MIT",
    "repository": "https://example.invalid"
  }},
  "compatibility": {{ "nose": ">=0.16.0 <0.17.0" }},
  "supported_languages": [{{ "id": "python" }}],
  "declares": {{
    "evidence_producers": [{{
      "id": "python.library-api.example",
      "kind": "LibraryApi.Contract",
      "anchors": ["node"],
      "channel": "exact-empirical",
      "stable_hash_inputs": ["pack.id", "producer.id", "call_span"],
      "conflict_policy": "fail-closed"
    }}],
    "contracts": [{{
      "id": "python.example.contract",
      "surface": {{ "kind": "function" }},
      "requires": [{{
        "ref": "python.library-api.example",
        "subject": "call",
        "required": true
      }}],
      "semantics": {{
        "operation": "Example",
        "demand": {{ "arguments": "eager-left-to-right" }},
        "effects": ["argument-effects-in-order"]
      }},
      "channel": "exact-empirical",
      "proof_status": "covered",
      "conformance_refs": ["positive", "negative"]
    }}],
    "value_laws": []
  }},
  "conformance": {{
    "positive_fixtures": [{{
      "id": "positive",
      "description": "positive",
      "path": "fixtures/positive.py",
      "expectation": "exact-contract-open"
    }}],
    "hard_negatives": [{{
      "id": "negative",
      "description": "negative",
      "path": "fixtures/negative.py",
      "expectation": "exact-contract-closed"
    }}],
    "known_unsupported": []
  }}
}}"#
    )
}

fn manifest_with_value_law(id: &str) -> String {
    manifest(id).replace(
        r#""value_laws": []"#,
        r#""value_laws": [{
      "id": "python.example.numeric-law",
      "requires": [{
        "ref": "Domain.Number",
        "subject": "operands",
        "required": true,
        "same_anchor_as": "value"
      }],
      "semantics": {
        "law": "x + 0 == x",
        "domain": "numeric-only",
        "demand": { "arguments": "preserve-original-expression-demand" },
        "effects": ["no-new-effects"]
      },
      "channel": "exact-proven",
      "proof_status": "proven",
      "conformance_refs": ["positive", "negative"]
    }]"#,
    )
}

fn manifest_with_fixed_result_domain(id: &str) -> String {
    manifest(id).replace(
        r#""operation": "Example",
        "demand": { "arguments": "eager-left-to-right" }"#,
        r#""operation": "Example",
        "result_domain": {
          "kind": "fixed",
          "domain": "Collection",
          "subject": "call"
        },
        "demand": { "arguments": "eager-left-to-right" }"#,
    )
}

fn manifest_with_executable_gates(id: &str) -> String {
    manifest(id).replace(
        r#""known_unsupported": []"#,
        r#""known_unsupported": [],
    "executable": [{
      "id": "python.library-api.example.gate",
      "row_ref": "python.library-api.example",
      "oracle": "fixture-expectations",
      "positive_fixtures": ["positive"],
      "hard_negatives": ["negative"],
      "expected_positive": "exact-contract-open",
      "expected_hard_negative": "exact-contract-closed"
    }, {
      "id": "python.example.contract.gate",
      "row_ref": "python.example.contract",
      "oracle": "fixture-expectations",
      "positive_fixtures": ["positive"],
      "hard_negatives": ["negative"],
      "expected_positive": "exact-contract-open",
      "expected_hard_negative": "exact-contract-closed"
    }]"#,
    )
}

fn manifest_with_value_law_executable_gate(id: &str) -> String {
    manifest_with_value_law(id).replace(
        r#""known_unsupported": []"#,
        r#""known_unsupported": [],
    "executable": [{
      "id": "python.example.numeric-law.gate",
      "row_ref": "python.example.numeric-law",
      "oracle": "fixture-expectations",
      "positive_fixtures": ["positive"],
      "hard_negatives": ["negative"],
      "expected_positive": "exact-contract-open",
      "expected_hard_negative": "exact-contract-closed"
    }]"#,
    )
}

#[test]
fn builtin_pack_descriptor_registry_names_current_compiled_packs() {
    let descriptors = builtin_pack_descriptors();
    assert_eq!(descriptors.len(), 49);
    let ids = descriptors
        .iter()
        .map(|descriptor| descriptor.id)
        .collect::<Vec<_>>();
    assert_eq!(
        ids,
        vec![
            BUILTIN_COMPAT_PACK_ID,
            PYTHON_LANGUAGE_PACK_ID,
            JS_TS_LANGUAGE_PACK_ID,
            GO_LANGUAGE_PACK_ID,
            RUST_LANGUAGE_PACK_ID,
            JAVA_LANGUAGE_PACK_ID,
            C_LANGUAGE_PACK_ID,
            RUBY_LANGUAGE_PACK_ID,
            SWIFT_LANGUAGE_PACK_ID,
            CSS_LANGUAGE_PACK_ID,
            HTML_EMBEDDED_LANGUAGE_PACK_ID,
            PYTHON_BUILTIN_COLLECTION_FACTORY_PACK_ID,
            PYTHON_STDLIB_COLLECTION_FACTORY_PACK_ID,
            PYTHON_STDLIB_MATH_PACK_ID,
            RUBY_STDLIB_SET_PACK_ID,
            RUST_STDLIB_VEC_PACK_ID,
            RUST_STDLIB_OPTION_PACK_ID,
            RUST_STDLIB_RESULT_PACK_ID,
            RUST_STDLIB_INTEGER_METHOD_PACK_ID,
            RUST_STDLIB_COLLECTION_FACTORY_PACK_ID,
            RUST_STDLIB_MAP_FACTORY_PACK_ID,
            SWIFT_STDLIB_COLLECTION_FACTORY_PACK_ID,
            JAVA_STDLIB_MATH_PACK_ID,
            JAVA_STDLIB_MAP_FACTORY_PACK_ID,
            JAVA_STDLIB_MAP_ENTRY_PACK_ID,
            JAVA_STDLIB_COLLECTION_FACTORY_PACK_ID,
            JAVA_GUAVA_IMMUTABLE_COLLECTION_FACTORY_PACK_ID,
            JAVA_STDLIB_COLLECTION_CONSTRUCTOR_PACK_ID,
            JAVA_STDLIB_STATIC_COLLECTION_ADAPTER_PACK_ID,
            MAP_GET_PROTOCOL_PACK_ID,
            MAP_GET_DEFAULT_PROTOCOL_PACK_ID,
            FREE_FUNCTION_BUILTIN_PROTOCOL_PACK_ID,
            PYTHON_ITERATOR_BUILTIN_PROTOCOL_PACK_ID,
            RECEIVER_MEMBERSHIP_PROTOCOL_PACK_ID,
            MAP_KEY_VIEW_PROTOCOL_PACK_ID,
            PROPERTY_BUILTIN_PROTOCOL_PACK_ID,
            BUILTIN_METHOD_CALL_PROTOCOL_PACK_ID,
            STRING_AFFIX_PREDICATE_PROTOCOL_PACK_ID,
            SEQUENCE_HOF_ADAPTER_PROTOCOL_PACK_ID,
            GO_STDLIB_NAMESPACE_CALL_PACK_ID,
            ITERATOR_IDENTITY_ADAPTER_PACK_ID,
            JS_LIKE_BUILTIN_PROMISE_PACK_ID,
            JS_LIKE_BUILTIN_ARRAY_PACK_ID,
            JS_LIKE_BUILTIN_BOOLEAN_PACK_ID,
            JS_LIKE_BUILTIN_REGEX_PACK_ID,
            JS_LIKE_BUILTIN_STATIC_INDEX_MEMBERSHIP_PACK_ID,
            JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PACK_ID,
            PYTHON_STDLIB_TYPE_DOMAIN_PACK_ID,
            VALUE_GRAPH_LAW_PACK_ID
        ]
    );
    assert_eq!(ids.iter().copied().collect::<HashSet<_>>().len(), ids.len());
    assert!(descriptors
        .iter()
        .all(|descriptor| descriptor.trust == PackTrust::BuiltinDefault));
    assert!(descriptors
        .iter()
        .all(|descriptor| descriptor.enabled_by_default));
}

mod descriptor_enumeration;
mod manifest_cases_0;
mod manifest_cases_1;
mod manifest_cases_2;
mod manifest_cases_3;
