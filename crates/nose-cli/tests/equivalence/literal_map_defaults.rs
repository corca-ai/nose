use super::*;

#[test]
fn literal_map_default_lookup_converges_with_js_map_construction_boundaries() {
    let i = Interner::new();
    let py_literal = "def f(key, other):\n    return {\"red\": 1, \"blue\": 2}.get(key, 0)\n";
    let ruby_literal = "def f(key, other)\n  {\"red\" => 1, \"blue\" => 2}.fetch(key, 0)\nend\n";
    let js_inline =
        "function f(key, other) { return new Map([[\"red\", 1], [\"blue\", 2]]).get(key) ?? 0; }";
    let js_call =
        "function f(key, other) { return Map([[\"red\", 1], [\"blue\", 2]]).get(key) ?? 0; }";
    let js_local = "function f(key, other) { const lookup = new Map([[\"red\", 1], [\"blue\", 2]]); return lookup.get(key) ?? 0; }";
    let js_has_get = "function f(key, other) { const lookup = new Map([[\"red\", 1], [\"blue\", 2]]); return lookup.has(key) ? lookup.get(key) : 0; }";
    let ts_inline = "function f(key: string, other: string): number { return new Map<string, number>([[\"red\", 1], [\"blue\", 2]]).get(key) ?? 0; }";
    let js_wrong_key =
        "function f(key, other) { return new Map([[\"red\", 1], [\"blue\", 2]]).get(other) ?? 0; }";
    let js_wrong_default =
        "function f(key, other) { return new Map([[\"red\", 1], [\"blue\", 2]]).get(key) ?? 9; }";
    let js_wrong_map =
        "function f(key, other) { return new Map([[\"red\", 9], [\"blue\", 2]]).get(key) ?? 0; }";
    let js_untyped = "function f(lookup, key, other) { return lookup.get(key) ?? 0; }";
    let js_shadowed_map = "function f(key, other, Map) { return new Map([[\"red\", 1], [\"blue\", 2]]).get(key) ?? 0; }";
    let js_global_shadowed_map = "function Map(entries) { return { get: function() { return 99; } }; }\nfunction f(key, other) { return new Map([[\"red\", 1], [\"blue\", 2]]).get(key) ?? 0; }";

    let fp = value_fp(&i, py_literal, Lang::Python);
    assert_eq!(fp, value_fp(&i, ruby_literal, Lang::Ruby));
    assert_eq!(fp, value_fp(&i, js_has_get, Lang::JavaScript)); // membership absence — stays in family
                                                                // `m.get(k) ?? d` is nullish COALESCE (default on absent OR present-null), NOT the absence-only
                                                                // default; it must not merge with the family — unsound for a null-valued map, and the value
                                                                // type's nullability is erased from the IL (#410, experiments §CT). The `?? ` forms converge
                                                                // with each other as their own class.
    let coalesce_fp = value_fp(&i, js_inline, Lang::JavaScript);
    assert_eq!(coalesce_fp, value_fp(&i, js_local, Lang::JavaScript));
    assert_eq!(coalesce_fp, value_fp(&i, ts_inline, Lang::TypeScript));
    assert_ne!(fp, coalesce_fp);
    assert_ne!(fp, value_fp(&i, js_call, Lang::JavaScript));
    assert_ne!(fp, value_fp(&i, js_wrong_key, Lang::JavaScript));
    assert_ne!(fp, value_fp(&i, js_wrong_default, Lang::JavaScript));
    assert_ne!(fp, value_fp(&i, js_wrong_map, Lang::JavaScript));
    assert_ne!(fp, value_fp(&i, js_untyped, Lang::JavaScript));
    assert_ne!(fp, value_fp(&i, js_shadowed_map, Lang::JavaScript));
    assert_ne!(
        fp,
        value_fp(&i, js_global_shadowed_map, Lang::JavaScript),
        "construct syntax alone must not prove a shadowed JS Map global"
    );
}

#[test]
fn literal_map_default_lookup_converges_with_java_map_factory_boundaries() {
    let i = Interner::new();
    let py_literal = "def f(key, other):\n    return {\"red\": 1, \"blue\": 2}.get(key, 0)\n";
    let ruby_literal = "def f(key, other)\n  {\"red\" => 1, \"blue\" => 2}.fetch(key, 0)\nend\n";
    let java_map_of = "import java.util.Map;\n\nclass C { static int f(String key, String other) { return Map.of(\"red\", 1, \"blue\", 2).getOrDefault(key, 0); } }\n";
    let java_map_of_entries = "import java.util.Map;\n\nclass C { static int f(String key, String other) { return Map.ofEntries(Map.entry(\"red\", 1), Map.entry(\"blue\", 2)).getOrDefault(key, 0); } }\n";
    let java_map_local = "import java.util.Map;\n\nclass C { static int f(String key, String other) { Map<String, Integer> lookup = Map.of(\"red\", 1, \"blue\", 2); return lookup.getOrDefault(key, 0); } }\n";
    let java_collections_singleton = "import java.util.Collections;\n\nclass C { static int f(String key, String other) { return Collections.singletonMap(\"red\", 1).getOrDefault(key, 0); } }\n";
    let java_collections_empty = "import java.util.Collections;\n\nclass C { static int f(String key, String other) { return Collections.emptyMap().getOrDefault(key, 0); } }\n";
    let java_wrong_key = "import java.util.Map;\n\nclass C { static int f(String key, String other) { return Map.of(\"red\", 1, \"blue\", 2).getOrDefault(other, 0); } }\n";
    let java_wrong_default = "import java.util.Map;\n\nclass C { static int f(String key, String other) { return Map.of(\"red\", 1, \"blue\", 2).getOrDefault(key, 9); } }\n";
    let java_wrong_map = "import java.util.Map;\n\nclass C { static int f(String key, String other) { return Map.of(\"red\", 9, \"blue\", 2).getOrDefault(key, 0); } }\n";
    let java_shadowed_factory = "class C { static class MapFactory { java.util.Map<String, Integer> of(Object... values) { return java.util.Map.of(); } } static int f(String key, String other, MapFactory Map) { return Map.of(\"red\", 1, \"blue\", 2).getOrDefault(key, 0); } }\n";
    let java_type_shadow = "class C { static int f(String key, String other) { return Map.of(\"red\", 1, \"blue\", 2).getOrDefault(key, 0); } }\nclass Map { static java.util.Map<String, Integer> of(Object... values) { return java.util.Map.of(); } }\n";
    let java_collections_missing_import = "class C { static int f(String key, String other) { return Collections.singletonMap(\"red\", 1).getOrDefault(key, 0); } }\nclass Collections { static java.util.Map<String, Integer> singletonMap(String key, Integer value) { return java.util.Map.of(\"green\", value); } }\n";
    let java_collections_shadowed_receiver = "import java.util.Collections;\n\nclass C { static int f(String key, String other, Object Collections) { return Collections.singletonMap(\"red\", 1).getOrDefault(key, 0); } }\n";

    let fp = value_fp(&i, py_literal, Lang::Python);
    assert_eq!(fp, value_fp(&i, ruby_literal, Lang::Ruby));
    assert_eq!(fp, value_fp(&i, java_map_of, Lang::Java));
    assert_eq!(fp, value_fp(&i, java_map_of_entries, Lang::Java));
    assert_eq!(fp, value_fp(&i, java_map_local, Lang::Java));
    let singleton_fp = value_fp(
        &i,
        "def f(key, other):\n    return {\"red\": 1}.get(key, 0)\n",
        Lang::Python,
    );
    assert_eq!(
        singleton_fp,
        value_fp(&i, java_collections_singleton, Lang::Java)
    );
    let empty_fp = value_fp(
        &i,
        "def f(key, other):\n    return {}.get(key, 0)\n",
        Lang::Python,
    );
    assert_eq!(empty_fp, value_fp(&i, java_collections_empty, Lang::Java));
    assert_ne!(fp, value_fp(&i, java_wrong_key, Lang::Java));
    assert_ne!(fp, value_fp(&i, java_wrong_default, Lang::Java));
    assert_ne!(fp, value_fp(&i, java_wrong_map, Lang::Java));
    assert_ne!(fp, value_fp(&i, java_shadowed_factory, Lang::Java));
    assert_ne!(fp, value_fp(&i, java_type_shadow, Lang::Java));
    assert_ne!(
        singleton_fp,
        value_fp_named(&i, java_collections_missing_import, Lang::Java, "f")
    );
    assert_ne!(
        singleton_fp,
        value_fp(&i, java_collections_shadowed_receiver, Lang::Java)
    );
}

#[test]
fn literal_map_default_lookup_converges_with_rust_std_map_factory_boundaries() {
    let i = Interner::new();
    let py_literal = "def f(key, other):\n    return {\"red\": 1, \"blue\": 2}.get(key, 0)\n";
    let rust_hashmap_inline = "pub fn f(key: &str, other: &str) -> i32 {\n    *std::collections::HashMap::from([(\"red\", 1), (\"blue\", 2)]).get(key).unwrap_or(&0)\n}\n";
    let rust_btreemap_inline = "pub fn f(key: &str, other: &str) -> i32 {\n    *std::collections::BTreeMap::from([(\"red\", 1), (\"blue\", 2)]).get(key).unwrap_or(&0)\n}\n";
    let rust_hashmap_local = "pub fn f(key: &str, other: &str) -> i32 {\n    let lookup = std::collections::HashMap::from([(\"red\", 1), (\"blue\", 2)]);\n    *lookup.get(key).unwrap_or(&0)\n}\n";
    let rust_wrong_key = "pub fn f(key: &str, other: &str) -> i32 {\n    *std::collections::HashMap::from([(\"red\", 1), (\"blue\", 2)]).get(other).unwrap_or(&0)\n}\n";
    let rust_wrong_default = "pub fn f(key: &str, other: &str) -> i32 {\n    *std::collections::HashMap::from([(\"red\", 1), (\"blue\", 2)]).get(key).unwrap_or(&9)\n}\n";
    let rust_wrong_map = "pub fn f(key: &str, other: &str) -> i32 {\n    *std::collections::HashMap::from([(\"red\", 9), (\"blue\", 2)]).get(key).unwrap_or(&0)\n}\n";
    let rust_mutated = "pub fn f(key: &str, other: &str) -> i32 {\n    let mut lookup = std::collections::HashMap::from([(\"red\", 1), (\"blue\", 2)]);\n    lookup.insert(\"red\", 9);\n    *lookup.get(key).unwrap_or(&0)\n}\n";

    let fp = value_fp(&i, py_literal, Lang::Python);
    assert_eq!(fp, value_fp(&i, rust_hashmap_inline, Lang::Rust));
    assert_eq!(fp, value_fp(&i, rust_btreemap_inline, Lang::Rust));
    assert_eq!(fp, value_fp(&i, rust_hashmap_local, Lang::Rust));
    assert_ne!(fp, value_fp(&i, rust_wrong_key, Lang::Rust));
    assert_ne!(fp, value_fp(&i, rust_wrong_default, Lang::Rust));
    assert_ne!(fp, value_fp(&i, rust_wrong_map, Lang::Rust));
    assert_ne!(fp, value_fp(&i, rust_mutated, Lang::Rust));
}

#[test]
fn literal_map_default_lookup_converges_with_go_literal_map_index_boundaries() {
    let i = Interner::new();
    let py_literal = "def f(key, other):\n    return {\"red\": 1, \"blue\": 2}.get(key, 0)\n";
    let ruby_literal = "def f(key, other)\n  {\"red\" => 1, \"blue\" => 2}.fetch(key, 0)\nend\n";
    let go_inline = "package p\n\nfunc F(key string, other string) int { return map[string]int{\"red\": 1, \"blue\": 2}[key] }\n";
    let go_local = "package p\n\nfunc F(key string, other string) int { lookup := map[string]int{\"red\": 1, \"blue\": 2}; return lookup[key] }\n";
    let go_var = "package p\n\nfunc F(key string, other string) int { var lookup = map[string]int{\"red\": 1, \"blue\": 2}; return lookup[key] }\n";
    let go_wrong_key =
        "package p\n\nfunc F(key string, other string) int { return map[string]int{\"red\": 1, \"blue\": 2}[other] }\n";
    let go_wrong_map =
        "package p\n\nfunc F(key string, other string) int { return map[string]int{\"red\": 9, \"blue\": 2}[key] }\n";
    let py_int_key_literal = "def f(key, other):\n    return {0: 1, 1: 2}.get(key, 0)\n";
    let go_keyed_slice =
        "package p\n\nfunc F(key int, other int) int { return []int{0: 1, 1: 2}[key] }\n";
    let go_string_inline =
        "package p\n\nfunc F(key string, other string) string { return map[string]string{\"red\": \"apple\", \"blue\": \"berry\"}[key] }\n";

    let fp = value_fp(&i, py_literal, Lang::Python);
    assert_eq!(fp, value_fp(&i, ruby_literal, Lang::Ruby));
    assert_eq!(fp, value_fp(&i, go_inline, Lang::Go));
    assert_eq!(fp, value_fp(&i, go_local, Lang::Go));
    assert_eq!(fp, value_fp(&i, go_var, Lang::Go));
    assert_ne!(fp, value_fp(&i, go_wrong_key, Lang::Go));
    assert_ne!(fp, value_fp(&i, go_wrong_map, Lang::Go));
    assert_ne!(
        value_fp(&i, py_int_key_literal, Lang::Python),
        value_fp(&i, go_keyed_slice, Lang::Go)
    );
    assert_ne!(fp, value_fp(&i, go_string_inline, Lang::Go));
}

#[test]
fn literal_map_default_lookup_converges_with_go_literal_string_map_boundaries() {
    let i = Interner::new();
    let py_string_literal =
        "def f(key, other):\n    return {\"red\": \"apple\", \"blue\": \"berry\"}.get(key, \"\")\n";
    let ruby_string_literal =
        "def f(key, other)\n  {\"red\" => \"apple\", \"blue\" => \"berry\"}.fetch(key, \"\")\nend\n";
    let go_string_inline =
        "package p\n\nfunc F(key string, other string) string { return map[string]string{\"red\": \"apple\", \"blue\": \"berry\"}[key] }\n";
    let go_string_local =
        "package p\n\nfunc F(key string, other string) string { lookup := map[string]string{\"red\": \"apple\", \"blue\": \"berry\"}; return lookup[key] }\n";
    let go_string_wrong_key =
        "package p\n\nfunc F(key string, other string) string { return map[string]string{\"red\": \"apple\", \"blue\": \"berry\"}[other] }\n";
    let py_string_int_key_literal =
        "def f(key, other):\n    return {0: \"apple\", 1: \"berry\"}.get(key, \"\")\n";
    let go_string_keyed_slice =
        "package p\n\nfunc F(key int, other int) string { return []string{0: \"apple\", 1: \"berry\"}[key] }\n";
    let go_mixed_value =
        "package p\n\nfunc F(key string, other string) interface{} { return map[string]interface{}{\"red\": \"apple\", \"blue\": false}[key] }\n";

    let string_fp = value_fp(&i, py_string_literal, Lang::Python);
    assert_eq!(string_fp, value_fp(&i, ruby_string_literal, Lang::Ruby));
    assert_eq!(string_fp, value_fp(&i, go_string_inline, Lang::Go));
    assert_eq!(string_fp, value_fp(&i, go_string_local, Lang::Go));
    assert_ne!(string_fp, value_fp(&i, go_string_wrong_key, Lang::Go));
    assert_ne!(string_fp, value_fp(&i, go_mixed_value, Lang::Go));
    assert_ne!(
        value_fp(&i, py_string_int_key_literal, Lang::Python),
        value_fp(&i, go_string_keyed_slice, Lang::Go)
    );
}

#[test]
fn literal_map_default_lookup_converges_with_go_literal_scalar_map_boundaries() {
    let i = Interner::new();
    let py_bool_literal =
        "def f(key, other):\n    return {\"red\": True, \"blue\": False}.get(key, False)\n";
    let ruby_bool_literal =
        "def f(key, other)\n  {\"red\" => true, \"blue\" => false}.fetch(key, false)\nend\n";
    let go_bool_inline =
        "package p\n\nfunc F(key string, other string) bool { return map[string]bool{\"red\": true, \"blue\": false}[key] }\n";
    let go_bool_wrong_map =
        "package p\n\nfunc F(key string, other string) bool { return map[string]bool{\"red\": false, \"blue\": false}[key] }\n";
    let py_float_literal =
        "def f(key, other):\n    return {\"red\": 1.5, \"blue\": 2.5}.get(key, 0.0)\n";
    let ruby_float_literal =
        "def f(key, other)\n  {\"red\" => 1.5, \"blue\" => 2.5}.fetch(key, 0.0)\nend\n";
    let go_float_inline =
        "package p\n\nfunc F(key string, other string) float64 { return map[string]float64{\"red\": 1.5, \"blue\": 2.5}[key] }\n";
    let go_float_local =
        "package p\n\nfunc F(key string, other string) float64 { lookup := map[string]float64{\"red\": 1.5, \"blue\": 2.5}; return lookup[key] }\n";
    let go_float_wrong_key =
        "package p\n\nfunc F(key string, other string) float64 { return map[string]float64{\"red\": 1.5, \"blue\": 2.5}[other] }\n";
    let py_nil_literal =
        "def f(key, other):\n    return {\"red\": None, \"blue\": None}.get(key, None)\n";
    let ruby_nil_literal =
        "def f(key, other)\n  {\"red\" => nil, \"blue\" => nil}.fetch(key, nil)\nend\n";
    let go_nil_inline =
        "package p\n\ntype Item struct{}\n\nfunc F(key string, other string) *Item { return map[string]*Item{\"red\": nil, \"blue\": nil}[key] }\n";
    let go_nil_wrong_map =
        "package p\n\nfunc F(key string, other string) string { return map[string]string{\"red\": \"apple\", \"blue\": \"berry\"}[key] }\n";

    let bool_fp = value_fp(&i, py_bool_literal, Lang::Python);
    assert_eq!(bool_fp, value_fp(&i, ruby_bool_literal, Lang::Ruby));
    assert_eq!(bool_fp, value_fp(&i, go_bool_inline, Lang::Go));
    assert_ne!(bool_fp, value_fp(&i, go_bool_wrong_map, Lang::Go));

    let float_fp = value_fp(&i, py_float_literal, Lang::Python);
    assert_eq!(float_fp, value_fp(&i, ruby_float_literal, Lang::Ruby));
    assert_eq!(float_fp, value_fp(&i, go_float_inline, Lang::Go));
    assert_eq!(float_fp, value_fp(&i, go_float_local, Lang::Go));
    assert_ne!(float_fp, value_fp(&i, go_float_wrong_key, Lang::Go));

    let nil_fp = value_fp(&i, py_nil_literal, Lang::Python);
    assert_eq!(nil_fp, value_fp(&i, ruby_nil_literal, Lang::Ruby));
    assert_eq!(nil_fp, value_fp(&i, go_nil_inline, Lang::Go));
    assert_ne!(nil_fp, value_fp(&i, go_nil_wrong_map, Lang::Go));
}

#[test]
fn literal_map_default_lookup_converges_with_module_map_bindings() {
    let i = Interner::new();
    let py_literal = "def f(key, other):\n    return {\"red\": 1, \"blue\": 2}.get(key, 0)\n";
    let js_module = "const LOOKUP = new Map([[\"red\", 1], [\"blue\", 2]]);\nfunction f(key, other) { return LOOKUP.get(key) ?? 0; }\n";
    let ts_module = "const LOOKUP = new Map<string, number>([[\"red\", 1], [\"blue\", 2]]);\nfunction f(key: string, other: string): number { return LOOKUP.get(key) ?? 0; }\n";
    let java_static = "import java.util.Map;\n\nclass C { static final Map<String, Integer> LOOKUP = Map.of(\"red\", 1, \"blue\", 2); static int f(String key, String other) { return LOOKUP.getOrDefault(key, 0); } }\n";
    let js_wrong_key = "const LOOKUP = new Map([[\"red\", 1], [\"blue\", 2]]);\nfunction f(key, other) { return LOOKUP.get(other) ?? 0; }\n";
    let ts_wrong_default = "const LOOKUP = new Map<string, number>([[\"red\", 1], [\"blue\", 2]]);\nfunction f(key: string, other: string): number { return LOOKUP.get(key) ?? 9; }\n";
    let java_wrong_map = "import java.util.Map;\n\nclass C { static final Map<String, Integer> LOOKUP = Map.of(\"red\", 9, \"blue\", 2); static int f(String key, String other) { return LOOKUP.getOrDefault(key, 0); } }\n";
    let js_mutated = "const LOOKUP = new Map([[\"red\", 1], [\"blue\", 2]]);\nLOOKUP.set(\"red\", 9);\nfunction f(key, other) { return LOOKUP.get(key) ?? 0; }\n";
    let ts_shadowed = "const Map: any = function(_entries: any) { return { get: function() { return 9; } }; };\nconst LOOKUP = new Map([[\"red\", 1], [\"blue\", 2]]);\nfunction f(key: string, other: string): number { return LOOKUP.get(key) ?? 0; }\n";
    let java_shadowed = "class C { static final Map<String, Integer> LOOKUP = Map.of(\"red\", 1, \"blue\", 2); static int f(String key, String other) { return LOOKUP.getOrDefault(key, 0); } }\nclass Map { static java.util.Map<String, Integer> of(Object... values) { return java.util.Map.of(); } }\n";

    let fp = value_fp(&i, py_literal, Lang::Python);
    assert_eq!(fp, value_fp(&i, java_static, Lang::Java)); // getOrDefault absence — stays in family
                                                           // `?? ` is nullish coalesce, distinct from the absence-default family (#410, experiments §CT):
    let coalesce_fp = value_fp(&i, js_module, Lang::JavaScript);
    assert_eq!(coalesce_fp, value_fp(&i, ts_module, Lang::TypeScript));
    assert_ne!(fp, coalesce_fp);
    assert_ne!(fp, value_fp(&i, js_wrong_key, Lang::JavaScript));
    assert_ne!(fp, value_fp(&i, ts_wrong_default, Lang::TypeScript));
    assert_ne!(fp, value_fp(&i, java_wrong_map, Lang::Java));
    assert_ne!(fp, value_fp(&i, js_mutated, Lang::JavaScript));
    assert_ne!(fp, value_fp(&i, ts_shadowed, Lang::TypeScript));
    assert_ne!(fp, value_fp(&i, java_shadowed, Lang::Java));
}

#[test]
fn literal_map_default_lookup_converges_with_imported_python_literal_binding() {
    let (dir, corpus) = lower_temp_corpus(
        "nose_imported_map_default",
        &[
            (
                "local.py",
                "def lookup(key, other):\n    return {\"red\": 1, \"blue\": 2}.get(key, 0)\n",
            ),
            ("tables.py", "LOOKUP = {\"red\": 1, \"blue\": 2}\n"),
            (
                "imported.py",
                "from tables import LOOKUP\n\ndef lookup(key, other):\n    return LOOKUP.get(key, 0)\n",
            ),
            (
                "wrong_map.py",
                "from tables import LOOKUP\n\ndef lookup(key, other):\n    return {\"red\": 9, \"blue\": 2}.get(key, 0)\n",
            ),
            (
                "mutated_tables.py",
                "LOOKUP = {\"red\": 1, \"blue\": 2}\nLOOKUP.clear()\n",
            ),
            (
                "mutated_index_tables.py",
                "LOOKUP = {\"red\": 1, \"blue\": 2}\nLOOKUP[\"red\"] = 9\n",
            ),
            (
                "escaped_tables.py",
                "LOOKUP = {\"red\": 1, \"blue\": 2}\nmutate(LOOKUP)\n",
            ),
            (
                "imported_mutated_provider.py",
                "from mutated_tables import LOOKUP\n\ndef lookup(key, other):\n    return LOOKUP.get(key, 0)\n",
            ),
            (
                "imported_mutated_index_provider.py",
                "from mutated_index_tables import LOOKUP\n\ndef lookup(key, other):\n    return LOOKUP.get(key, 0)\n",
            ),
            (
                "imported_escaped_provider.py",
                "from escaped_tables import LOOKUP\n\ndef lookup(key, other):\n    return LOOKUP.get(key, 0)\n",
            ),
            (
                "imported_mutated_receiver.py",
                "from tables import LOOKUP\nLOOKUP.clear()\n\ndef lookup(key, other):\n    return LOOKUP.get(key, 0)\n",
            ),
            (
                "imported_mutated_index_receiver.py",
                "from tables import LOOKUP\nLOOKUP[\"red\"] = 9\n\ndef lookup(key, other):\n    return LOOKUP.get(key, 0)\n",
            ),
        ],
    );
    let local = corpus_value_fp(&corpus, "local.py", "lookup");
    assert_eq!(
        local,
        corpus_value_fp(&corpus, "imported.py", "lookup"),
        "imported immutable literal map binding should prove the same lookup/default coordinates"
    );
    assert_ne!(
        local,
        corpus_value_fp(&corpus, "wrong_map.py", "lookup"),
        "different literal map contents must stay distinct"
    );
    assert_ne!(
        local,
        corpus_value_fp(&corpus, "imported_mutated_provider.py", "lookup"),
        "provider mutation must block imported literal provenance"
    );
    assert_ne!(
        local,
        corpus_value_fp(&corpus, "imported_mutated_index_provider.py", "lookup"),
        "provider index write must block imported literal provenance"
    );
    assert_ne!(
        local,
        corpus_value_fp(&corpus, "imported_escaped_provider.py", "lookup"),
        "provider opaque argument escape must block imported literal provenance"
    );
    assert_ne!(
        local,
        corpus_value_fp(&corpus, "imported_mutated_receiver.py", "lookup"),
        "importer mutation must block imported literal provenance"
    );
    assert_ne!(
        local,
        corpus_value_fp(&corpus, "imported_mutated_index_receiver.py", "lookup"),
        "importer index write must block imported literal provenance"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn literal_map_default_lookup_converges_with_java_imported_bindings() {
    let (dir, corpus) = lower_temp_corpus(
        "nose_imported_java_map_default",
        &[
            (
                "local.py",
                "def lookup(key, other):\n    return {\"red\": 1, \"blue\": 2}.get(key, 0)\n",
            ),
            (
                "Tables.java",
                "import java.util.Map;\n\nclass Tables {\n  static final Map<String, Integer> LOOKUP = Map.of(\"red\", 1, \"blue\", 2);\n}\n",
            ),
            (
                "JavaImported.java",
                "import static Tables.LOOKUP;\n\nclass JavaImported {\n  static int lookup(String key, String other) {\n    return LOOKUP.getOrDefault(key, 0);\n  }\n}\n",
            ),
            (
                "JavaImportedWithLocalMapShadow.java",
                "import static Tables.LOOKUP;\n\nclass JavaImportedWithLocalMapShadow {\n  static int lookup(String key, String other) {\n    return LOOKUP.getOrDefault(key, 0);\n  }\n}\n\nclass Map {}\n",
            ),
            (
                "WrongTables.java",
                "import java.util.Map;\n\nclass WrongTables {\n  static final Map<String, Integer> LOOKUP = Map.of(\"red\", 9, \"blue\", 2);\n}\n",
            ),
            (
                "JavaImportedWrongMap.java",
                "import static WrongTables.LOOKUP;\n\nclass JavaImportedWrongMap {\n  static int lookup(String key, String other) {\n    return LOOKUP.getOrDefault(key, 0);\n  }\n}\n",
            ),
            (
                "MissingMapImportTables.java",
                "class MissingMapImportTables {\n  static final Map<String, Integer> LOOKUP = Map.of(\"red\", 1, \"blue\", 2);\n}\n",
            ),
            (
                "JavaImportedMissingMapImport.java",
                "import static MissingMapImportTables.LOOKUP;\n\nclass JavaImportedMissingMapImport {\n  static int lookup(String key, String other) {\n    return LOOKUP.getOrDefault(key, 0);\n  }\n}\n",
            ),
        ],
    );
    let local = corpus_value_fp(&corpus, "local.py", "lookup");
    assert_eq!(
        local,
        corpus_value_fp(&corpus, "JavaImported.java", "lookup"),
        "Java static import should prove the same literal map/default coordinates"
    );
    assert_eq!(
        local,
        corpus_value_fp(&corpus, "JavaImportedWithLocalMapShadow.java", "lookup"),
        "provider-proven Java static import should not be invalidated by importer-local Map shadowing"
    );
    assert_ne!(
        local,
        corpus_value_fp(&corpus, "JavaImportedWrongMap.java", "lookup"),
        "different Java imported map contents must stay distinct"
    );
    assert_ne!(
        local,
        corpus_value_fp(&corpus, "JavaImportedMissingMapImport.java", "lookup"),
        "Java imported Map.of provider must require java.util.Map proof"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn literal_map_default_lookup_converges_with_rust_imported_bindings() {
    let (dir, corpus) = lower_temp_corpus(
        "nose_imported_rust_map_default",
        &[
            (
                "local.py",
                "def lookup(key, other):\n    return {\"red\": 1, \"blue\": 2}.get(key, 0)\n",
            ),
            (
                "tables.rs",
                "pub const LOOKUP: [(&str, i32); 2] = [(\"red\", 1), (\"blue\", 2)];\n",
            ),
            (
                "rust_imported.rs",
                "use tables::LOOKUP;\n\npub fn lookup(key: &str, other: &str) -> i32 {\n    *std::collections::HashMap::from(LOOKUP).get(key).unwrap_or(&0)\n}\n",
            ),
            (
                "rust_imported_shadowed_std.rs",
                "use tables::LOOKUP;\n\nmod std { pub mod collections { pub struct HashMap; } }\n\npub fn lookup(key: &str, other: &str) -> i32 {\n    *std::collections::HashMap::from(LOOKUP).get(key).unwrap_or(&0)\n}\n",
            ),
            (
                "wrong_tables.rs",
                "pub const LOOKUP: [(&str, i32); 2] = [(\"red\", 9), (\"blue\", 2)];\n",
            ),
            (
                "rust_imported_wrong_map.rs",
                "use wrong_tables::LOOKUP;\n\npub fn lookup(key: &str, other: &str) -> i32 {\n    *std::collections::HashMap::from(LOOKUP).get(key).unwrap_or(&0)\n}\n",
            ),
        ],
    );
    let local = corpus_value_fp(&corpus, "local.py", "lookup");
    assert_eq!(
        local,
        corpus_value_fp(&corpus, "rust_imported.rs", "lookup"),
        "Rust use-imported const entries should prove the same map/default coordinates"
    );
    assert_ne!(
        local,
        corpus_value_fp(&corpus, "rust_imported_shadowed_std.rs", "lookup"),
        "a local Rust std module must block imported std map factory provenance"
    );
    assert_ne!(
        local,
        corpus_value_fp(&corpus, "rust_imported_wrong_map.rs", "lookup"),
        "different Rust imported map contents must stay distinct"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn literal_map_default_lookup_converges_with_js_object_own_property_boundaries() {
    let i = Interner::new();
    let py_literal = "def f(key, other):\n    return {\"red\": 1, \"blue\": 2}.get(key, 0)\n";
    let ruby_literal = "def f(key, other)\n  {\"red\" => 1, \"blue\" => 2}.fetch(key, 0)\nend\n";
    let js_hasown = "function f(key, other) { const values = { \"red\": 1, \"blue\": 2 }; return Object.hasOwn(values, key) ? values[key] : 0; }";
    let js_call = "function f(key, other) { const values = { \"red\": 1, \"blue\": 2 }; return Object.prototype.hasOwnProperty.call(values, key) ? values[key] : 0; }";
    let ts_negated = "function f(key: string, other: string): number { const values: Record<string, number> = { \"red\": 1, \"blue\": 2 }; return !Object.hasOwn(values, key) ? 0 : values[key]; }";
    let js_wrong_key = "function f(key, other) { const values = { \"red\": 1, \"blue\": 2 }; return Object.hasOwn(values, other) ? values[other] : 0; }";
    let js_wrong_default = "function f(key, other) { const values = { \"red\": 1, \"blue\": 2 }; return Object.hasOwn(values, key) ? values[key] : 9; }";
    let js_wrong_map = "function f(key, other) { const values = { \"red\": 9, \"blue\": 2 }; return Object.hasOwn(values, key) ? values[key] : 0; }";
    let js_unguarded = "function f(key, other) { const values = { \"red\": 1, \"blue\": 2 }; return values[key] ?? 0; }";
    let js_in = "function f(key, other) { const values = { \"red\": 1, \"blue\": 2 }; return key in values ? values[key] : 0; }";
    let js_method = "function f(key, other) { const values = { \"red\": 1, \"blue\": 2 }; return values.hasOwnProperty(key) ? values[key] : 0; }";
    let js_shadowed_object = "function f(key, other, Object) { const values = { \"red\": 1, \"blue\": 2 }; return Object.hasOwn(values, key) ? values[key] : 0; }";

    let fp = value_fp(&i, py_literal, Lang::Python);
    assert_eq!(fp, value_fp(&i, ruby_literal, Lang::Ruby));
    assert_eq!(fp, value_fp(&i, js_hasown, Lang::JavaScript));
    assert_eq!(fp, value_fp(&i, js_call, Lang::JavaScript));
    assert_eq!(fp, value_fp(&i, ts_negated, Lang::TypeScript));
    assert_ne!(fp, value_fp(&i, js_wrong_key, Lang::JavaScript));
    assert_ne!(fp, value_fp(&i, js_wrong_default, Lang::JavaScript));
    assert_ne!(fp, value_fp(&i, js_wrong_map, Lang::JavaScript));
    assert_ne!(fp, value_fp(&i, js_unguarded, Lang::JavaScript));
    assert_ne!(fp, value_fp(&i, js_in, Lang::JavaScript));
    assert_ne!(fp, value_fp(&i, js_method, Lang::JavaScript));
    assert_ne!(fp, value_fp(&i, js_shadowed_object, Lang::JavaScript));
}
