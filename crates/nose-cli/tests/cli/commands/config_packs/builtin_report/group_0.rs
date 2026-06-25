use super::*;

pub(super) fn assert_group(json: &serde_json::Value) {
    let first_party = semantic_pack_by_id(&json, "nose.first_party");
    assert_eq!(first_party["source"], "compiled-builtin");
    assert_eq!(first_party["influence"], "evidence-and-contracts");
    assert_eq!(first_party["trust"], "builtin-default");
    assert_eq!(first_party["enabled_by_default"], true);
    assert!(first_party["path"].is_null());

    let python_lang = semantic_pack_by_id(&json, "nose.lang.python");
    assert_eq!(python_lang["kind"], "LanguagePack");
    assert_eq!(python_lang["source"], "compiled-builtin");
    assert_eq!(
        json_array_strings(python_lang, "supported_languages"),
        vec!["python"]
    );
    assert_eq!(python_lang["counts"]["evidence_producers"], 2);
    assert_eq!(python_lang["counts"]["contracts"], 0);

    let js_ts_lang = semantic_pack_by_id(&json, "nose.lang.javascript-typescript");
    assert_eq!(js_ts_lang["kind"], "LanguagePack");
    assert_eq!(
        json_array_strings(js_ts_lang, "supported_languages"),
        vec!["javascript", "typescript"]
    );
    assert_eq!(js_ts_lang["counts"]["evidence_producers"], 2);
    assert_eq!(js_ts_lang["counts"]["contracts"], 0);

    let html_lang = semantic_pack_by_id(&json, "nose.lang.html");
    assert_eq!(html_lang["kind"], "LanguagePack");
    assert_eq!(
        json_array_strings(html_lang, "supported_languages"),
        vec!["html", "vue", "svelte"]
    );
    assert_eq!(html_lang["counts"]["evidence_producers"], 2);
    assert_eq!(html_lang["counts"]["contracts"], 0);

    let c = semantic_pack_by_id(&json, "nose.lang.c");
    assert_eq!(c["kind"], "LanguagePack");
    assert_eq!(c["source"], "compiled-builtin");
    assert_eq!(json_array_strings(c, "supported_languages"), vec!["c"]);
    assert_eq!(c["counts"]["evidence_producers"], 3);
    assert_eq!(c["counts"]["contracts"], 0);
    assert_eq!(c["counts"]["positive_fixtures"], 2);
    assert_eq!(c["counts"]["hard_negatives"], 2);

    let builtins = semantic_pack_by_id(&json, "nose.python.builtins.collection_factories");
    assert_eq!(builtins["kind"], "StdlibPack");
    assert_eq!(
        json_array_strings(builtins, "supported_languages"),
        vec!["python"]
    );
    assert_eq!(builtins["counts"]["evidence_producers"], 1);
    assert_eq!(builtins["counts"]["contracts"], 1);
    assert_eq!(builtins["counts"]["positive_fixtures"], 4);
    assert_eq!(builtins["counts"]["hard_negatives"], 2);

    let collections = semantic_pack_by_id(&json, "nose.python.stdlib.collection_factories");
    assert_eq!(collections["kind"], "StdlibPack");
    assert_eq!(
        json_array_strings(collections, "supported_languages"),
        vec!["python"]
    );
    assert_eq!(collections["counts"]["evidence_producers"], 1);
    assert_eq!(collections["counts"]["contracts"], 1);
    assert_eq!(collections["counts"]["positive_fixtures"], 3);
    assert_eq!(collections["counts"]["hard_negatives"], 2);

    let math = semantic_pack_by_id(&json, "nose.python.stdlib.math");
    assert_eq!(math["hash"], "9abb9da5e7aa81e0");
    assert_eq!(math["kind"], "StdlibPack");
    assert_eq!(math["display_name"], "nose Python stdlib math pack");
    assert_eq!(math["source"], "compiled-builtin");
    assert_eq!(math["influence"], "evidence-and-contracts");
    assert_eq!(math["trust"], "builtin-default");
    assert_eq!(math["enabled_by_default"], true);
    assert_eq!(math["path"], serde_json::Value::Null);
    assert_eq!(math["provider"], "Corca, Inc.");
    assert_eq!(math["repository"], "https://github.com/corca-ai/nose");
    assert_eq!(math["license"], "MIT");
    assert_eq!(
        json_array_strings(math, "supported_languages"),
        vec!["python"]
    );
    assert_eq!(math["counts"]["evidence_producers"], 1);
    assert_eq!(math["counts"]["contracts"], 1);
    assert_eq!(math["counts"]["value_laws"], 0);
    assert_eq!(math["counts"]["positive_fixtures"], 1);
    assert_eq!(math["counts"]["hard_negatives"], 2);

    let ruby_set = semantic_pack_by_id(&json, "nose.ruby.stdlib.set");
    assert_eq!(ruby_set["kind"], "StdlibPack");
    assert_eq!(
        json_array_strings(ruby_set, "supported_languages"),
        vec!["ruby"]
    );
    assert_eq!(ruby_set["counts"]["evidence_producers"], 1);
    assert_eq!(ruby_set["counts"]["contracts"], 1);
    assert_eq!(ruby_set["counts"]["positive_fixtures"], 3);
    assert_eq!(ruby_set["counts"]["hard_negatives"], 3);

    let rust_vec = semantic_pack_by_id(&json, "nose.rust.stdlib.vec");
    assert_eq!(rust_vec["hash"], "cc787cbb5aa0a87c");
    assert_eq!(rust_vec["kind"], "StdlibPack");
    assert_eq!(rust_vec["display_name"], "nose Rust stdlib Vec pack");
    assert_eq!(rust_vec["source"], "compiled-builtin");
    assert_eq!(rust_vec["influence"], "evidence-and-contracts");
    assert_eq!(rust_vec["trust"], "builtin-default");
    assert_eq!(rust_vec["enabled_by_default"], true);
    assert_eq!(rust_vec["path"], serde_json::Value::Null);
    assert_eq!(rust_vec["provider"], "Corca, Inc.");
    assert_eq!(rust_vec["repository"], "https://github.com/corca-ai/nose");
    assert_eq!(rust_vec["license"], "MIT");
    assert_eq!(
        json_array_strings(rust_vec, "supported_languages"),
        vec!["rust"]
    );
    assert_eq!(rust_vec["counts"]["evidence_producers"], 1);
    assert_eq!(rust_vec["counts"]["contracts"], 2);
    assert_eq!(rust_vec["counts"]["value_laws"], 0);
    assert_eq!(rust_vec["counts"]["positive_fixtures"], 2);
    assert_eq!(rust_vec["counts"]["hard_negatives"], 2);

    let rust_option = semantic_pack_by_id(&json, "nose.rust.stdlib.option");
    assert_eq!(rust_option["hash"], "8ffb410363be1b73");
    assert_eq!(rust_option["kind"], "StdlibPack");
    assert_eq!(rust_option["display_name"], "nose Rust stdlib Option pack");
    assert_eq!(rust_option["source"], "compiled-builtin");
    assert_eq!(rust_option["influence"], "evidence-and-contracts");
    assert_eq!(rust_option["trust"], "builtin-default");
    assert_eq!(rust_option["enabled_by_default"], true);
    assert_eq!(rust_option["path"], serde_json::Value::Null);
    assert_eq!(rust_option["provider"], "Corca, Inc.");
    assert_eq!(
        rust_option["repository"],
        "https://github.com/corca-ai/nose"
    );
    assert_eq!(rust_option["license"], "MIT");
    assert_eq!(
        json_array_strings(rust_option, "supported_languages"),
        vec!["rust"]
    );
    assert_eq!(rust_option["counts"]["evidence_producers"], 1);
    assert_eq!(rust_option["counts"]["contracts"], 3);
    assert_eq!(rust_option["counts"]["value_laws"], 0);
    assert_eq!(rust_option["counts"]["positive_fixtures"], 3);
    assert_eq!(rust_option["counts"]["hard_negatives"], 3);

    let rust_result = semantic_pack_by_id(&json, "nose.rust.stdlib.result");
    assert_eq!(rust_result["hash"], "d078e92695934687");
    assert_eq!(rust_result["kind"], "StdlibPack");
    assert_eq!(rust_result["display_name"], "nose Rust stdlib Result pack");
    assert_eq!(rust_result["source"], "compiled-builtin");
    assert_eq!(rust_result["influence"], "evidence-and-contracts");
    assert_eq!(rust_result["trust"], "builtin-default");
    assert_eq!(rust_result["enabled_by_default"], true);
    assert_eq!(rust_result["path"], serde_json::Value::Null);
    assert_eq!(rust_result["provider"], "Corca, Inc.");
    assert_eq!(
        rust_result["repository"],
        "https://github.com/corca-ai/nose"
    );
    assert_eq!(rust_result["license"], "MIT");
    assert_eq!(
        json_array_strings(rust_result, "supported_languages"),
        vec!["rust"]
    );
    assert_eq!(rust_result["counts"]["evidence_producers"], 1);
    assert_eq!(rust_result["counts"]["contracts"], 4);
    assert_eq!(rust_result["counts"]["value_laws"], 0);
    assert_eq!(rust_result["counts"]["positive_fixtures"], 4);
    assert_eq!(rust_result["counts"]["hard_negatives"], 5);

    let rust_integer_methods = semantic_pack_by_id(&json, "nose.rust.stdlib.integer_methods");
    assert_eq!(rust_integer_methods["hash"], "ce3664f7abe81ee9");
    assert_eq!(rust_integer_methods["kind"], "StdlibPack");
    assert_eq!(
        rust_integer_methods["display_name"],
        "nose Rust stdlib integer method pack"
    );
    assert_eq!(rust_integer_methods["source"], "compiled-builtin");
    assert_eq!(rust_integer_methods["influence"], "evidence-and-contracts");
    assert_eq!(rust_integer_methods["trust"], "builtin-default");
    assert_eq!(rust_integer_methods["enabled_by_default"], true);
    assert_eq!(rust_integer_methods["path"], serde_json::Value::Null);
    assert_eq!(rust_integer_methods["provider"], "Corca, Inc.");
    assert_eq!(
        rust_integer_methods["repository"],
        "https://github.com/corca-ai/nose"
    );
    assert_eq!(rust_integer_methods["license"], "MIT");
    assert_eq!(
        json_array_strings(rust_integer_methods, "supported_languages"),
        vec!["rust"]
    );
    assert_eq!(rust_integer_methods["counts"]["evidence_producers"], 1);
    assert_eq!(rust_integer_methods["counts"]["contracts"], 4);
    assert_eq!(rust_integer_methods["counts"]["value_laws"], 0);
    assert_eq!(rust_integer_methods["counts"]["positive_fixtures"], 4);
    assert_eq!(rust_integer_methods["counts"]["hard_negatives"], 2);

    let rust_collections = semantic_pack_by_id(&json, "nose.rust.stdlib.collection_factories");
    assert_eq!(rust_collections["hash"], "c0913f2d5652c20f");
    assert_eq!(rust_collections["kind"], "StdlibPack");
    assert_eq!(
        rust_collections["display_name"],
        "nose Rust stdlib collection factory pack"
    );
    assert_eq!(rust_collections["source"], "compiled-builtin");
    assert_eq!(rust_collections["influence"], "evidence-and-contracts");
    assert_eq!(rust_collections["trust"], "builtin-default");
    assert_eq!(rust_collections["enabled_by_default"], true);
    assert_eq!(rust_collections["path"], serde_json::Value::Null);
    assert_eq!(rust_collections["provider"], "Corca, Inc.");
    assert_eq!(
        rust_collections["repository"],
        "https://github.com/corca-ai/nose"
    );
    assert_eq!(rust_collections["license"], "MIT");
    assert_eq!(
        json_array_strings(rust_collections, "supported_languages"),
        vec!["rust"]
    );
    assert_eq!(rust_collections["counts"]["evidence_producers"], 1);
    assert_eq!(rust_collections["counts"]["contracts"], 1);
    assert_eq!(rust_collections["counts"]["value_laws"], 0);
    assert_eq!(rust_collections["counts"]["positive_fixtures"], 3);
    assert_eq!(rust_collections["counts"]["hard_negatives"], 2);

    let rust_maps = semantic_pack_by_id(&json, "nose.rust.stdlib.map_factories");
    assert_eq!(rust_maps["hash"], "418077a33dc67531");
    assert_eq!(rust_maps["kind"], "StdlibPack");
    assert_eq!(
        rust_maps["display_name"],
        "nose Rust stdlib map factory pack"
    );
    assert_eq!(rust_maps["source"], "compiled-builtin");
    assert_eq!(rust_maps["influence"], "evidence-and-contracts");
    assert_eq!(rust_maps["trust"], "builtin-default");
    assert_eq!(rust_maps["enabled_by_default"], true);
    assert_eq!(rust_maps["path"], serde_json::Value::Null);
    assert_eq!(rust_maps["provider"], "Corca, Inc.");
    assert_eq!(rust_maps["repository"], "https://github.com/corca-ai/nose");
    assert_eq!(rust_maps["license"], "MIT");
    assert_eq!(
        json_array_strings(rust_maps, "supported_languages"),
        vec!["rust"]
    );
    assert_eq!(rust_maps["counts"]["evidence_producers"], 1);
    assert_eq!(rust_maps["counts"]["contracts"], 1);
    assert_eq!(rust_maps["counts"]["value_laws"], 0);
    assert_eq!(rust_maps["counts"]["positive_fixtures"], 2);
    assert_eq!(rust_maps["counts"]["hard_negatives"], 2);

    let swift_collections = semantic_pack_by_id(&json, "nose.swift.stdlib.collection_factories");
    assert_eq!(swift_collections["hash"], "d560c62d16075bfa");
    assert_eq!(swift_collections["kind"], "StdlibPack");
    assert_eq!(
        swift_collections["display_name"],
        "nose Swift stdlib collection factory pack"
    );
    assert_eq!(swift_collections["source"], "compiled-builtin");
    assert_eq!(swift_collections["influence"], "evidence-and-contracts");
    assert_eq!(swift_collections["trust"], "builtin-default");
    assert_eq!(swift_collections["enabled_by_default"], true);
    assert_eq!(swift_collections["path"], serde_json::Value::Null);
    assert_eq!(swift_collections["provider"], "Corca, Inc.");
    assert_eq!(
        swift_collections["repository"],
        "https://github.com/corca-ai/nose"
    );
    assert_eq!(swift_collections["license"], "MIT");
    assert_eq!(
        json_array_strings(swift_collections, "supported_languages"),
        vec!["swift"]
    );
    assert_eq!(swift_collections["counts"]["evidence_producers"], 1);
    assert_eq!(swift_collections["counts"]["contracts"], 3);
    assert_eq!(swift_collections["counts"]["value_laws"], 0);
    assert_eq!(swift_collections["counts"]["positive_fixtures"], 3);
    assert_eq!(swift_collections["counts"]["hard_negatives"], 4);

    let java_maps = semantic_pack_by_id(&json, "nose.java.stdlib.map_factories");
    assert_eq!(java_maps["hash"], "1eecb2960193782f");
    assert_eq!(java_maps["kind"], "StdlibPack");
    assert_eq!(
        java_maps["display_name"],
        "nose Java stdlib map factory pack"
    );
    assert_eq!(java_maps["source"], "compiled-builtin");
    assert_eq!(java_maps["influence"], "evidence-and-contracts");
    assert_eq!(java_maps["trust"], "builtin-default");
    assert_eq!(java_maps["enabled_by_default"], true);
    assert_eq!(java_maps["path"], serde_json::Value::Null);
    assert_eq!(java_maps["provider"], "Corca, Inc.");
    assert_eq!(java_maps["repository"], "https://github.com/corca-ai/nose");
    assert_eq!(java_maps["license"], "MIT");
    assert_eq!(
        json_array_strings(java_maps, "supported_languages"),
        vec!["java"]
    );
    assert_eq!(java_maps["counts"]["evidence_producers"], 1);
    assert_eq!(java_maps["counts"]["contracts"], 4);
    assert_eq!(java_maps["counts"]["value_laws"], 0);
    assert_eq!(java_maps["counts"]["positive_fixtures"], 4);
    assert_eq!(java_maps["counts"]["hard_negatives"], 4);
}
