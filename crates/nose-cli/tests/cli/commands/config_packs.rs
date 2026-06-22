#![allow(
    clippy::cognitive_complexity,
    clippy::needless_borrow,
    clippy::too_many_lines
)]

use super::*;

#[test]
fn config_file_supplies_defaults() {
    // A nose.toml in the working dir provides defaults (here: an exclude glob and
    // min-size); a CLI flag still overrides.
    let dir = make_project("cfg");
    fs::write(
        dir.join("nose.toml"),
        "[query]\nexclude = [\"a/**\"]\nmin-size = 12\n",
    )
    .unwrap();
    let out = Command::new(bin())
        .args(["query", ".", "--format", "json"])
        .current_dir(&dir)
        .output()
        .expect("run");
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        !stdout.contains("a/f.py"),
        "config exclude a/** should drop a/f.py: {stdout}"
    );
    assert!(stdout.contains("b/f.py"), "b/ remains: {stdout}");
    let _ = fs::remove_dir_all(&dir);
}

fn semantic_pack_manifest(id: &str) -> String {
    format!(
        r#"{{
  "api_version": "nose.semantic-pack.v0",
  "pack": {{
    "id": "{id}",
    "kind": "LibraryPack",
    "version": "0.1.0",
    "display_name": "Example semantic pack",
    "trust": "external-opt-in",
    "enabled_by_default": false
  }},
  "provenance": {{
    "provider": {{ "name": "Example Packs" }},
    "license": "MIT",
    "repository": "https://example.invalid/semantic-pack"
  }},
  "compatibility": {{ "nose": ">=0.15.0 <0.16.0" }},
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

fn semantic_pack_manifest_with_value_law(id: &str) -> String {
    semantic_pack_manifest(id).replace(
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

fn semantic_pack_manifest_with_executable_gates(id: &str) -> String {
    semantic_pack_manifest(id).replace(
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

fn semantic_pack_by_id<'a>(json: &'a serde_json::Value, id: &str) -> &'a serde_json::Value {
    json["semantic_packs"]
        .as_array()
        .expect("query JSON should report semantic_packs")
        .iter()
        .find(|pack| pack["id"] == id)
        .unwrap_or_else(|| panic!("semantic_packs should include {id}: {json}"))
}

fn assert_hex_hash(value: &serde_json::Value) {
    let hash = value.as_str().expect("hash should be a string");
    assert_eq!(hash.len(), 16, "hash should be 16 hex digits: {hash}");
    assert!(
        hash.chars().all(|ch| ch.is_ascii_hexdigit()),
        "hash should be hex: {hash}"
    );
}

fn assert_example_external_pack(pack: &serde_json::Value, expected_id: &str) {
    assert_eq!(pack["id"], expected_id);
    assert_eq!(pack["kind"], "LibraryPack");
    assert_eq!(pack["version"], "0.1.0");
    assert_eq!(pack["display_name"], "Example semantic pack");
    assert_eq!(pack["trust"], "external-opt-in");
    assert_eq!(pack["enabled_by_default"], false);
    assert_eq!(pack["source"], "local-manifest");
    assert_eq!(pack["influence"], "metadata-only");
    assert_eq!(pack["provider"], "Example Packs");
    assert_eq!(pack["repository"], "https://example.invalid/semantic-pack");
    assert_eq!(pack["license"], "MIT");
    assert_eq!(
        json_array_strings(pack, "supported_languages"),
        vec!["python"]
    );
    assert_eq!(pack["counts"]["evidence_producers"], 1);
    assert_eq!(pack["counts"]["contracts"], 1);
    assert_eq!(pack["counts"]["value_laws"], 0);
    assert_eq!(pack["counts"]["positive_fixtures"], 1);
    assert_eq!(pack["counts"]["hard_negatives"], 1);
}

#[path = "config_packs/builtin_report.rs"]
mod builtin_report;
#[path = "config_packs/external_cases_0.rs"]
mod external_cases_0;
