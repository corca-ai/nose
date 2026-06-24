use super::*;

pub(super) fn assert_group(json: &serde_json::Value) {
    let java_entries = semantic_pack_by_id(&json, "nose.java.stdlib.map_entries");
    assert_eq!(java_entries["hash"], "70b8bbc16bb60219");
    assert_eq!(java_entries["kind"], "StdlibPack");
    assert_eq!(
        java_entries["display_name"],
        "nose Java stdlib map entry pack"
    );
    assert_eq!(java_entries["source"], "compiled-builtin");
    assert_eq!(java_entries["influence"], "evidence-and-contracts");
    assert_eq!(java_entries["trust"], "builtin-default");
    assert_eq!(java_entries["enabled_by_default"], true);
    assert_eq!(java_entries["path"], serde_json::Value::Null);
    assert_eq!(java_entries["provider"], "Corca, Inc.");
    assert_eq!(
        java_entries["repository"],
        "https://github.com/corca-ai/nose"
    );
    assert_eq!(java_entries["license"], "MIT");
    assert_eq!(
        json_array_strings(java_entries, "supported_languages"),
        vec!["java"]
    );
    assert_eq!(java_entries["counts"]["evidence_producers"], 1);
    assert_eq!(java_entries["counts"]["contracts"], 1);
    assert_eq!(java_entries["counts"]["value_laws"], 0);
    assert_eq!(java_entries["counts"]["positive_fixtures"], 1);
    assert_eq!(java_entries["counts"]["hard_negatives"], 2);

    let java_collections = semantic_pack_by_id(&json, "nose.java.stdlib.collection_factories");
    assert_eq!(java_collections["hash"], "e784159038ce0c8d");
    assert_eq!(java_collections["kind"], "StdlibPack");
    assert_eq!(
        java_collections["display_name"],
        "nose Java stdlib collection factory pack"
    );
    assert_eq!(java_collections["source"], "compiled-builtin");
    assert_eq!(java_collections["influence"], "evidence-and-contracts");
    assert_eq!(java_collections["trust"], "builtin-default");
    assert_eq!(java_collections["enabled_by_default"], true);
    assert_eq!(java_collections["path"], serde_json::Value::Null);
    assert_eq!(java_collections["provider"], "Corca, Inc.");
    assert_eq!(
        java_collections["repository"],
        "https://github.com/corca-ai/nose"
    );
    assert_eq!(java_collections["license"], "MIT");
    assert_eq!(
        json_array_strings(java_collections, "supported_languages"),
        vec!["java"]
    );
    assert_eq!(java_collections["counts"]["evidence_producers"], 1);
    assert_eq!(java_collections["counts"]["contracts"], 3);
    assert_eq!(java_collections["counts"]["value_laws"], 0);
    assert_eq!(java_collections["counts"]["positive_fixtures"], 3);
    assert_eq!(java_collections["counts"]["hard_negatives"], 2);

    let guava_collections = semantic_pack_by_id(
        &json,
        "nose.java.ecosystem.guava.immutable_collection_factories",
    );
    assert_eq!(guava_collections["hash"], "bda36ee0af67ff2c");
    assert_eq!(guava_collections["kind"], "LibraryPack");
    assert_eq!(
        guava_collections["display_name"],
        "nose Java Guava immutable collection factory pack"
    );
    assert_eq!(guava_collections["source"], "compiled-builtin");
    assert_eq!(guava_collections["influence"], "evidence-and-contracts");
    assert_eq!(guava_collections["trust"], "builtin-default");
    assert_eq!(guava_collections["enabled_by_default"], true);
    assert_eq!(guava_collections["path"], serde_json::Value::Null);
    assert_eq!(guava_collections["provider"], "Corca, Inc.");
    assert_eq!(
        guava_collections["repository"],
        "https://github.com/corca-ai/nose"
    );
    assert_eq!(guava_collections["license"], "MIT");
    assert_eq!(
        json_array_strings(guava_collections, "supported_languages"),
        vec!["java"]
    );
    assert_eq!(guava_collections["counts"]["evidence_producers"], 1);
    assert_eq!(guava_collections["counts"]["contracts"], 3);
    assert_eq!(guava_collections["counts"]["value_laws"], 0);
    assert_eq!(guava_collections["counts"]["positive_fixtures"], 3);
    assert_eq!(guava_collections["counts"]["hard_negatives"], 4);

    let java_constructors = semantic_pack_by_id(&json, "nose.java.stdlib.collection_constructors");
    assert_eq!(java_constructors["hash"], "47217e0e2e1f8108");
    assert_eq!(java_constructors["kind"], "StdlibPack");
    assert_eq!(
        java_constructors["display_name"],
        "nose Java stdlib collection constructor pack"
    );
    assert_eq!(java_constructors["source"], "compiled-builtin");
    assert_eq!(java_constructors["influence"], "evidence-and-contracts");
    assert_eq!(java_constructors["trust"], "builtin-default");
    assert_eq!(java_constructors["enabled_by_default"], true);
    assert_eq!(java_constructors["path"], serde_json::Value::Null);
    assert_eq!(java_constructors["provider"], "Corca, Inc.");
    assert_eq!(
        java_constructors["repository"],
        "https://github.com/corca-ai/nose"
    );
    assert_eq!(java_constructors["license"], "MIT");
    assert_eq!(
        json_array_strings(java_constructors, "supported_languages"),
        vec!["java"]
    );
    assert_eq!(java_constructors["counts"]["evidence_producers"], 1);
    assert_eq!(java_constructors["counts"]["contracts"], 1);
    assert_eq!(java_constructors["counts"]["value_laws"], 0);
    assert_eq!(java_constructors["counts"]["positive_fixtures"], 2);
    assert_eq!(java_constructors["counts"]["hard_negatives"], 3);

    let java_static_adapters =
        semantic_pack_by_id(&json, "nose.java.stdlib.static_collection_adapters");
    assert_eq!(java_static_adapters["hash"], "6fe217885f0a8fe8");
    assert_eq!(java_static_adapters["kind"], "StdlibPack");
    assert_eq!(
        java_static_adapters["display_name"],
        "nose Java stdlib static collection adapter pack"
    );
    assert_eq!(java_static_adapters["source"], "compiled-builtin");
    assert_eq!(java_static_adapters["influence"], "evidence-and-contracts");
    assert_eq!(java_static_adapters["trust"], "builtin-default");
    assert_eq!(java_static_adapters["enabled_by_default"], true);
    assert_eq!(java_static_adapters["path"], serde_json::Value::Null);
    assert_eq!(java_static_adapters["provider"], "Corca, Inc.");
    assert_eq!(
        java_static_adapters["repository"],
        "https://github.com/corca-ai/nose"
    );
    assert_eq!(java_static_adapters["license"], "MIT");
    assert_eq!(
        json_array_strings(java_static_adapters, "supported_languages"),
        vec!["java"]
    );
    assert_eq!(java_static_adapters["counts"]["evidence_producers"], 1);
    assert_eq!(java_static_adapters["counts"]["contracts"], 1);
    assert_eq!(java_static_adapters["counts"]["value_laws"], 0);
    assert_eq!(java_static_adapters["counts"]["positive_fixtures"], 1);
    assert_eq!(java_static_adapters["counts"]["hard_negatives"], 2);

    let iterator_identity = semantic_pack_by_id(&json, "nose.protocols.iterator_identity_adapters");
    assert_eq!(iterator_identity["hash"], "554b807e3806a6af");
    assert_eq!(iterator_identity["kind"], "ProtocolPack");
    assert_eq!(
        iterator_identity["display_name"],
        "nose iterator identity adapter protocol pack"
    );
    assert_eq!(iterator_identity["source"], "compiled-builtin");
    assert_eq!(iterator_identity["influence"], "evidence-and-contracts");
    assert_eq!(iterator_identity["trust"], "builtin-default");
    assert_eq!(iterator_identity["enabled_by_default"], true);
    assert_eq!(iterator_identity["path"], serde_json::Value::Null);
    assert_eq!(iterator_identity["provider"], "Corca, Inc.");
    assert_eq!(
        iterator_identity["repository"],
        "https://github.com/corca-ai/nose"
    );
    assert_eq!(iterator_identity["license"], "MIT");
    assert_eq!(
        json_array_strings(iterator_identity, "supported_languages"),
        vec!["java", "rust"]
    );
    assert_eq!(iterator_identity["counts"]["evidence_producers"], 1);
    assert_eq!(iterator_identity["counts"]["contracts"], 1);
    assert_eq!(iterator_identity["counts"]["value_laws"], 0);
    assert_eq!(iterator_identity["counts"]["positive_fixtures"], 3);
    assert_eq!(iterator_identity["counts"]["hard_negatives"], 2);

    let js_promise = semantic_pack_by_id(&json, "nose.javascript.builtins.promise");
    assert_eq!(js_promise["hash"], "db20255756aa3abc");
    assert_eq!(js_promise["kind"], "StdlibPack");
    assert_eq!(
        js_promise["display_name"],
        "nose JavaScript builtins Promise pack"
    );
    assert_eq!(js_promise["source"], "compiled-builtin");
    assert_eq!(js_promise["influence"], "evidence-and-contracts");
    assert_eq!(js_promise["trust"], "builtin-default");
    assert_eq!(js_promise["enabled_by_default"], true);
    assert_eq!(js_promise["path"], serde_json::Value::Null);
    assert_eq!(js_promise["provider"], "Corca, Inc.");
    assert_eq!(js_promise["repository"], "https://github.com/corca-ai/nose");
    assert_eq!(js_promise["license"], "MIT");
    assert_eq!(
        json_array_strings(js_promise, "supported_languages"),
        vec!["javascript", "typescript"]
    );
    assert_eq!(js_promise["counts"]["evidence_producers"], 1);
    assert_eq!(js_promise["counts"]["contracts"], 2);
    assert_eq!(js_promise["counts"]["value_laws"], 0);
    assert_eq!(js_promise["counts"]["positive_fixtures"], 2);
    assert_eq!(js_promise["counts"]["hard_negatives"], 3);

    let js_array = semantic_pack_by_id(&json, "nose.javascript.builtins.array");
    assert_eq!(js_array["hash"], "ca9d1142025e589c");
    assert_eq!(js_array["kind"], "StdlibPack");
    assert_eq!(
        js_array["display_name"],
        "nose JavaScript builtins Array pack"
    );
    assert_eq!(js_array["source"], "compiled-builtin");
    assert_eq!(js_array["influence"], "evidence-and-contracts");
    assert_eq!(js_array["trust"], "builtin-default");
    assert_eq!(js_array["enabled_by_default"], true);
    assert_eq!(js_array["path"], serde_json::Value::Null);
    assert_eq!(js_array["provider"], "Corca, Inc.");
    assert_eq!(js_array["repository"], "https://github.com/corca-ai/nose");
    assert_eq!(js_array["license"], "MIT");
    assert_eq!(
        json_array_strings(js_array, "supported_languages"),
        vec!["javascript", "typescript"]
    );
    assert_eq!(js_array["counts"]["evidence_producers"], 1);
    assert_eq!(js_array["counts"]["contracts"], 2);
    assert_eq!(js_array["counts"]["value_laws"], 0);
    assert_eq!(js_array["counts"]["positive_fixtures"], 2);
    assert_eq!(js_array["counts"]["hard_negatives"], 3);

    let js_boolean = semantic_pack_by_id(&json, "nose.javascript.builtins.boolean");
    assert_eq!(js_boolean["hash"], "7548f93fba013b5d");
    assert_eq!(js_boolean["kind"], "StdlibPack");
    assert_eq!(
        js_boolean["display_name"],
        "nose JavaScript builtins Boolean pack"
    );
    assert_eq!(js_boolean["source"], "compiled-builtin");
    assert_eq!(js_boolean["influence"], "evidence-and-contracts");
    assert_eq!(js_boolean["trust"], "builtin-default");
    assert_eq!(js_boolean["enabled_by_default"], true);
    assert_eq!(js_boolean["path"], serde_json::Value::Null);
    assert_eq!(js_boolean["provider"], "Corca, Inc.");
    assert_eq!(js_boolean["repository"], "https://github.com/corca-ai/nose");
    assert_eq!(js_boolean["license"], "MIT");
    assert_eq!(
        json_array_strings(js_boolean, "supported_languages"),
        vec!["javascript", "typescript"]
    );
    assert_eq!(js_boolean["counts"]["evidence_producers"], 1);
    assert_eq!(js_boolean["counts"]["contracts"], 1);
    assert_eq!(js_boolean["counts"]["value_laws"], 0);
    assert_eq!(js_boolean["counts"]["positive_fixtures"], 1);
    assert_eq!(js_boolean["counts"]["hard_negatives"], 2);

    let js_regex = semantic_pack_by_id(&json, "nose.javascript.builtins.regex");
    assert_eq!(js_regex["hash"], "36d345c574d8763e");
    assert_eq!(js_regex["kind"], "StdlibPack");
    assert_eq!(
        js_regex["display_name"],
        "nose JavaScript builtins RegExp pack"
    );
    assert_eq!(js_regex["source"], "compiled-builtin");
    assert_eq!(js_regex["influence"], "evidence-and-contracts");
    assert_eq!(js_regex["trust"], "builtin-default");
    assert_eq!(js_regex["enabled_by_default"], true);
    assert_eq!(js_regex["path"], serde_json::Value::Null);
    assert_eq!(js_regex["provider"], "Corca, Inc.");
    assert_eq!(js_regex["repository"], "https://github.com/corca-ai/nose");
    assert_eq!(js_regex["license"], "MIT");
    assert_eq!(
        json_array_strings(js_regex, "supported_languages"),
        vec!["javascript", "typescript"]
    );
    assert_eq!(js_regex["counts"]["evidence_producers"], 1);
    assert_eq!(js_regex["counts"]["contracts"], 1);
    assert_eq!(js_regex["counts"]["value_laws"], 0);
    assert_eq!(js_regex["counts"]["positive_fixtures"], 1);
    assert_eq!(js_regex["counts"]["hard_negatives"], 2);

    let js_static_index =
        semantic_pack_by_id(&json, "nose.javascript.builtins.static_index_membership");
    assert_eq!(js_static_index["hash"], "5e37c95d706307df");
    assert_eq!(js_static_index["kind"], "StdlibPack");
    assert_eq!(
        js_static_index["display_name"],
        "nose JavaScript builtins static index membership pack"
    );
    assert_eq!(js_static_index["source"], "compiled-builtin");
    assert_eq!(js_static_index["influence"], "evidence-and-contracts");
    assert_eq!(js_static_index["trust"], "builtin-default");
    assert_eq!(js_static_index["enabled_by_default"], true);
    assert_eq!(js_static_index["path"], serde_json::Value::Null);
    assert_eq!(js_static_index["provider"], "Corca, Inc.");
    assert_eq!(
        js_static_index["repository"],
        "https://github.com/corca-ai/nose"
    );
    assert_eq!(js_static_index["license"], "MIT");
    assert_eq!(
        json_array_strings(js_static_index, "supported_languages"),
        vec!["javascript", "typescript"]
    );
    assert_eq!(js_static_index["counts"]["evidence_producers"], 1);
    assert_eq!(js_static_index["counts"]["contracts"], 2);
    assert_eq!(js_static_index["counts"]["value_laws"], 0);
    assert_eq!(js_static_index["counts"]["positive_fixtures"], 2);
    assert_eq!(js_static_index["counts"]["hard_negatives"], 2);
}
