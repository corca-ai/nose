use super::*;

pub(super) fn assert_group(json: &serde_json::Value) {
    let java_math = semantic_pack_by_id(&json, "nose.java.stdlib.math");
    assert_eq!(java_math["hash"], "55fabb9892e17a4e");
    assert_eq!(java_math["kind"], "StdlibPack");
    assert_eq!(java_math["display_name"], "nose Java stdlib Math pack");
    assert_eq!(java_math["source"], "compiled-builtin");
    assert_eq!(java_math["influence"], "evidence-and-contracts");
    assert_eq!(java_math["trust"], "builtin-default");
    assert_eq!(java_math["enabled_by_default"], true);
    assert_eq!(java_math["path"], serde_json::Value::Null);
    assert_eq!(java_math["provider"], "Corca, Inc.");
    assert_eq!(java_math["repository"], "https://github.com/corca-ai/nose");
    assert_eq!(java_math["license"], "MIT");
    assert_eq!(
        json_array_strings(java_math, "supported_languages"),
        vec!["java"]
    );
    assert_eq!(java_math["counts"]["evidence_producers"], 1);
    assert_eq!(java_math["counts"]["contracts"], 3);
    assert_eq!(java_math["counts"]["value_laws"], 0);
    assert_eq!(java_math["counts"]["positive_fixtures"], 3);
    assert_eq!(java_math["counts"]["hard_negatives"], 3);

    let map_get = semantic_pack_by_id(&json, "nose.protocols.map_get");
    assert_eq!(map_get["hash"], "4f21cd668f95363e");
    assert_eq!(map_get["kind"], "ProtocolPack");
    assert_eq!(map_get["display_name"], "nose map-get protocol pack");
    assert_eq!(map_get["source"], "compiled-builtin");
    assert_eq!(map_get["influence"], "evidence-and-contracts");
    assert_eq!(map_get["trust"], "builtin-default");
    assert_eq!(map_get["enabled_by_default"], true);
    assert_eq!(map_get["path"], serde_json::Value::Null);
    assert_eq!(map_get["provider"], "Corca, Inc.");
    assert_eq!(map_get["repository"], "https://github.com/corca-ai/nose");
    assert_eq!(map_get["license"], "MIT");
    assert_eq!(
        json_array_strings(map_get, "supported_languages"),
        vec![
            "java",
            "rust",
            "javascript",
            "typescript",
            "vue",
            "svelte",
            "html"
        ]
    );
    assert_eq!(map_get["counts"]["evidence_producers"], 1);
    assert_eq!(map_get["counts"]["contracts"], 1);
    assert_eq!(map_get["counts"]["value_laws"], 0);
    assert_eq!(map_get["counts"]["positive_fixtures"], 3);
    assert_eq!(map_get["counts"]["hard_negatives"], 2);

    let map_get_default = semantic_pack_by_id(&json, "nose.protocols.map_get_default");
    assert_eq!(map_get_default["hash"], "9e15f7b838928d64");
    assert_eq!(map_get_default["kind"], "ProtocolPack");
    assert_eq!(
        map_get_default["display_name"],
        "nose map-get-default protocol pack"
    );
    assert_eq!(map_get_default["source"], "compiled-builtin");
    assert_eq!(map_get_default["influence"], "evidence-and-contracts");
    assert_eq!(map_get_default["trust"], "builtin-default");
    assert_eq!(map_get_default["enabled_by_default"], true);
    assert_eq!(map_get_default["path"], serde_json::Value::Null);
    assert_eq!(map_get_default["provider"], "Corca, Inc.");
    assert_eq!(
        map_get_default["repository"],
        "https://github.com/corca-ai/nose"
    );
    assert_eq!(map_get_default["license"], "MIT");
    assert_eq!(
        json_array_strings(map_get_default, "supported_languages"),
        vec!["python", "ruby", "java"]
    );
    assert_eq!(map_get_default["counts"]["evidence_producers"], 1);
    assert_eq!(map_get_default["counts"]["contracts"], 1);
    assert_eq!(map_get_default["counts"]["value_laws"], 0);
    assert_eq!(map_get_default["counts"]["positive_fixtures"], 3);
    assert_eq!(map_get_default["counts"]["hard_negatives"], 2);

    let free_function_builtin = semantic_pack_by_id(&json, "nose.protocols.free_function_builtins");
    assert_eq!(free_function_builtin["hash"], "b57ad1f1019fcdfd");
    assert_eq!(free_function_builtin["kind"], "ProtocolPack");
    assert_eq!(
        free_function_builtin["display_name"],
        "nose free-function builtin protocol pack"
    );
    assert_eq!(free_function_builtin["source"], "compiled-builtin");
    assert_eq!(free_function_builtin["influence"], "evidence-and-contracts");
    assert_eq!(free_function_builtin["trust"], "builtin-default");
    assert_eq!(free_function_builtin["enabled_by_default"], true);
    assert_eq!(free_function_builtin["path"], serde_json::Value::Null);
    assert_eq!(free_function_builtin["provider"], "Corca, Inc.");
    assert_eq!(
        free_function_builtin["repository"],
        "https://github.com/corca-ai/nose"
    );
    assert_eq!(free_function_builtin["license"], "MIT");
    assert_eq!(
        json_array_strings(free_function_builtin, "supported_languages"),
        vec!["python", "go", "swift"]
    );
    assert_eq!(free_function_builtin["counts"]["evidence_producers"], 1);
    assert_eq!(free_function_builtin["counts"]["contracts"], 1);
    assert_eq!(free_function_builtin["counts"]["value_laws"], 0);
    assert_eq!(free_function_builtin["counts"]["positive_fixtures"], 6);
    assert_eq!(free_function_builtin["counts"]["hard_negatives"], 4);

    let python_iterator_builtin = semantic_pack_by_id(&json, "nose.protocols.iterator_builtins");
    assert_eq!(python_iterator_builtin["hash"], "d48fa65341352c6c");
    assert_eq!(python_iterator_builtin["kind"], "ProtocolPack");
    assert_eq!(
        python_iterator_builtin["display_name"],
        "nose Python iterator builtin protocol pack"
    );
    assert_eq!(python_iterator_builtin["source"], "compiled-builtin");
    assert_eq!(
        python_iterator_builtin["influence"],
        "evidence-and-contracts"
    );
    assert_eq!(python_iterator_builtin["trust"], "builtin-default");
    assert_eq!(python_iterator_builtin["enabled_by_default"], true);
    assert_eq!(python_iterator_builtin["path"], serde_json::Value::Null);
    assert_eq!(python_iterator_builtin["provider"], "Corca, Inc.");
    assert_eq!(
        python_iterator_builtin["repository"],
        "https://github.com/corca-ai/nose"
    );
    assert_eq!(python_iterator_builtin["license"], "MIT");
    assert_eq!(
        json_array_strings(python_iterator_builtin, "supported_languages"),
        vec!["python"]
    );
    assert_eq!(python_iterator_builtin["counts"]["evidence_producers"], 1);
    assert_eq!(python_iterator_builtin["counts"]["contracts"], 2);
    assert_eq!(python_iterator_builtin["counts"]["value_laws"], 0);
    assert_eq!(python_iterator_builtin["counts"]["positive_fixtures"], 7);
    assert_eq!(python_iterator_builtin["counts"]["hard_negatives"], 7);

    let receiver_membership = semantic_pack_by_id(&json, "nose.protocols.receiver_membership");
    assert_eq!(receiver_membership["hash"], "b01cdfb3d7ec79c9");
    assert_eq!(receiver_membership["kind"], "ProtocolPack");
    assert_eq!(
        receiver_membership["display_name"],
        "nose receiver-membership protocol pack"
    );
    assert_eq!(receiver_membership["source"], "compiled-builtin");
    assert_eq!(receiver_membership["influence"], "evidence-and-contracts");
    assert_eq!(receiver_membership["trust"], "builtin-default");
    assert_eq!(receiver_membership["enabled_by_default"], true);
    assert_eq!(receiver_membership["path"], serde_json::Value::Null);
    assert_eq!(receiver_membership["provider"], "Corca, Inc.");
    assert_eq!(
        receiver_membership["repository"],
        "https://github.com/corca-ai/nose"
    );
    assert_eq!(receiver_membership["license"], "MIT");
    assert_eq!(
        json_array_strings(receiver_membership, "supported_languages"),
        vec![
            "python",
            "ruby",
            "java",
            "rust",
            "swift",
            "javascript",
            "typescript",
            "vue",
            "svelte",
            "html"
        ]
    );
    assert_eq!(receiver_membership["counts"]["evidence_producers"], 1);
    assert_eq!(receiver_membership["counts"]["contracts"], 1);
    assert_eq!(receiver_membership["counts"]["value_laws"], 0);
    assert_eq!(receiver_membership["counts"]["positive_fixtures"], 10);
    assert_eq!(receiver_membership["counts"]["hard_negatives"], 3);

    let map_key_view = semantic_pack_by_id(&json, "nose.protocols.map_key_views");
    assert_eq!(map_key_view["hash"], "fc74f28c4e454838");
    assert_eq!(map_key_view["kind"], "ProtocolPack");
    assert_eq!(
        map_key_view["display_name"],
        "nose map-key-view protocol pack"
    );
    assert_eq!(map_key_view["source"], "compiled-builtin");
    assert_eq!(map_key_view["influence"], "evidence-and-contracts");
    assert_eq!(map_key_view["trust"], "builtin-default");
    assert_eq!(map_key_view["enabled_by_default"], true);
    assert_eq!(map_key_view["path"], serde_json::Value::Null);
    assert_eq!(map_key_view["provider"], "Corca, Inc.");
    assert_eq!(
        map_key_view["repository"],
        "https://github.com/corca-ai/nose"
    );
    assert_eq!(map_key_view["license"], "MIT");
    assert_eq!(
        json_array_strings(map_key_view, "supported_languages"),
        vec![
            "python",
            "ruby",
            "java",
            "javascript",
            "typescript",
            "vue",
            "svelte",
            "html"
        ]
    );
    assert_eq!(map_key_view["counts"]["evidence_producers"], 1);
    assert_eq!(map_key_view["counts"]["contracts"], 2);
    assert_eq!(map_key_view["counts"]["value_laws"], 0);
    assert_eq!(map_key_view["counts"]["positive_fixtures"], 5);
    assert_eq!(map_key_view["counts"]["hard_negatives"], 6);

    let property_builtin = semantic_pack_by_id(&json, "nose.protocols.property_builtins");
    assert_eq!(property_builtin["hash"], "0bb1fdeb809a7e81");
    assert_eq!(property_builtin["kind"], "ProtocolPack");
    assert_eq!(
        property_builtin["display_name"],
        "nose property builtin protocol pack"
    );
    assert_eq!(property_builtin["source"], "compiled-builtin");
    assert_eq!(property_builtin["influence"], "evidence-and-contracts");
    assert_eq!(property_builtin["trust"], "builtin-default");
    assert_eq!(property_builtin["enabled_by_default"], true);
    assert_eq!(property_builtin["path"], serde_json::Value::Null);
    assert_eq!(property_builtin["provider"], "Corca, Inc.");
    assert_eq!(
        property_builtin["repository"],
        "https://github.com/corca-ai/nose"
    );
    assert_eq!(property_builtin["license"], "MIT");
    assert_eq!(
        json_array_strings(property_builtin, "supported_languages"),
        vec![
            "javascript",
            "typescript",
            "vue",
            "svelte",
            "html",
            "java",
            "swift"
        ]
    );
    assert_eq!(property_builtin["counts"]["evidence_producers"], 1);
    assert_eq!(property_builtin["counts"]["contracts"], 2);
    assert_eq!(property_builtin["counts"]["value_laws"], 0);
    assert_eq!(property_builtin["counts"]["positive_fixtures"], 4);
    assert_eq!(property_builtin["counts"]["hard_negatives"], 3);

    let builtin_method_call = semantic_pack_by_id(&json, "nose.protocols.builtin_method_calls");
    assert_eq!(builtin_method_call["hash"], "2b97688a4e1cf076");
    assert_eq!(builtin_method_call["kind"], "ProtocolPack");
    assert_eq!(
        builtin_method_call["display_name"],
        "nose builtin method-call protocol pack"
    );
    assert_eq!(builtin_method_call["source"], "compiled-builtin");
    assert_eq!(builtin_method_call["influence"], "evidence-and-contracts");
    assert_eq!(builtin_method_call["trust"], "builtin-default");
    assert_eq!(builtin_method_call["enabled_by_default"], true);
    assert_eq!(builtin_method_call["path"], serde_json::Value::Null);
    assert_eq!(builtin_method_call["provider"], "Corca, Inc.");
    assert_eq!(
        builtin_method_call["repository"],
        "https://github.com/corca-ai/nose"
    );
    assert_eq!(builtin_method_call["license"], "MIT");
    assert_eq!(
        json_array_strings(builtin_method_call, "supported_languages"),
        vec![
            "python",
            "javascript",
            "typescript",
            "vue",
            "svelte",
            "html",
            "rust",
            "java",
            "ruby",
            "swift"
        ]
    );
    assert_eq!(builtin_method_call["counts"]["evidence_producers"], 1);
    assert_eq!(builtin_method_call["counts"]["contracts"], 1);
    assert_eq!(builtin_method_call["counts"]["value_laws"], 0);
    assert_eq!(builtin_method_call["counts"]["positive_fixtures"], 7);
    assert_eq!(builtin_method_call["counts"]["hard_negatives"], 3);

    let string_affix = semantic_pack_by_id(&json, "nose.protocols.string_affix_predicates");
    assert_eq!(string_affix["hash"], "c5150f9f4b3559b4");
    assert_eq!(string_affix["kind"], "ProtocolPack");
    assert_eq!(
        string_affix["display_name"],
        "nose string affix predicate protocol pack"
    );
    assert_eq!(string_affix["source"], "compiled-builtin");
    assert_eq!(string_affix["influence"], "evidence-and-contracts");
    assert_eq!(string_affix["trust"], "builtin-default");
    assert_eq!(string_affix["enabled_by_default"], true);
    assert_eq!(string_affix["path"], serde_json::Value::Null);
    assert_eq!(string_affix["provider"], "Corca, Inc.");
    assert_eq!(
        string_affix["repository"],
        "https://github.com/corca-ai/nose"
    );
    assert_eq!(string_affix["license"], "MIT");
    assert_eq!(
        json_array_strings(string_affix, "supported_languages"),
        vec![
            "python",
            "javascript",
            "typescript",
            "vue",
            "svelte",
            "html",
            "go",
            "rust",
            "java",
            "ruby",
            "swift"
        ]
    );
    assert_eq!(string_affix["counts"]["evidence_producers"], 1);
    assert_eq!(string_affix["counts"]["contracts"], 1);
    assert_eq!(string_affix["counts"]["value_laws"], 0);
    assert_eq!(string_affix["counts"]["positive_fixtures"], 14);
    assert_eq!(string_affix["counts"]["hard_negatives"], 22);

    let sequence_hof_adapter = semantic_pack_by_id(&json, "nose.protocols.sequence_hof_adapters");
    assert_eq!(sequence_hof_adapter["hash"], "2c6344624cc74477");
    assert_eq!(sequence_hof_adapter["kind"], "ProtocolPack");
    assert_eq!(
        sequence_hof_adapter["display_name"],
        "nose sequence HOF adapter protocol pack"
    );
    assert_eq!(sequence_hof_adapter["source"], "compiled-builtin");
    assert_eq!(sequence_hof_adapter["influence"], "evidence-and-contracts");
    assert_eq!(sequence_hof_adapter["trust"], "builtin-default");
    assert_eq!(sequence_hof_adapter["enabled_by_default"], true);
    assert_eq!(sequence_hof_adapter["path"], serde_json::Value::Null);
    assert_eq!(sequence_hof_adapter["provider"], "Corca, Inc.");
    assert_eq!(
        sequence_hof_adapter["repository"],
        "https://github.com/corca-ai/nose"
    );
    assert_eq!(sequence_hof_adapter["license"], "MIT");
    assert_eq!(
        json_array_strings(sequence_hof_adapter, "supported_languages"),
        vec!["rust", "swift", "ruby"]
    );
    assert_eq!(sequence_hof_adapter["counts"]["evidence_producers"], 1);
    assert_eq!(sequence_hof_adapter["counts"]["contracts"], 1);
    assert_eq!(sequence_hof_adapter["counts"]["value_laws"], 0);
    assert_eq!(sequence_hof_adapter["counts"]["positive_fixtures"], 15);
    assert_eq!(sequence_hof_adapter["counts"]["hard_negatives"], 22);

    let go_namespace_call = semantic_pack_by_id(&json, "nose.go.stdlib.namespace_calls");
    assert_eq!(go_namespace_call["hash"], "d3dfae6db995411b");
    assert_eq!(go_namespace_call["kind"], "StdlibPack");
    assert_eq!(
        go_namespace_call["display_name"],
        "nose Go stdlib namespace-call pack"
    );
    assert_eq!(go_namespace_call["source"], "compiled-builtin");
    assert_eq!(go_namespace_call["influence"], "evidence-and-contracts");
    assert_eq!(go_namespace_call["trust"], "builtin-default");
    assert_eq!(go_namespace_call["enabled_by_default"], true);
    assert_eq!(go_namespace_call["path"], serde_json::Value::Null);
    assert_eq!(go_namespace_call["provider"], "Corca, Inc.");
    assert_eq!(
        go_namespace_call["repository"],
        "https://github.com/corca-ai/nose"
    );
    assert_eq!(go_namespace_call["license"], "MIT");
    assert_eq!(
        json_array_strings(go_namespace_call, "supported_languages"),
        vec!["go"]
    );
    assert_eq!(go_namespace_call["counts"]["evidence_producers"], 1);
    assert_eq!(go_namespace_call["counts"]["contracts"], 1);
    assert_eq!(go_namespace_call["counts"]["value_laws"], 0);
    assert_eq!(go_namespace_call["counts"]["positive_fixtures"], 3);
    assert_eq!(go_namespace_call["counts"]["hard_negatives"], 2);
}
