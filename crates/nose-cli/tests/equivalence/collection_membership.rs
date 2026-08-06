use super::*;

// This is one behavior matrix so every case uses the same interner and explicit
// reference fingerprints. The case DSL keeps the breadth visible and gives each
// failure a source identifier and language without hiding the source snippets.
#[allow(clippy::too_many_lines)]
#[test]
fn collection_membership_set_construction_converges_with_boundaries() {
    let i = Interner::new();
    let py_literal = "def f(value, other):\n    return value in [\"red\", \"blue\"]\n";
    let py_set_factory =
        "def f(value, other):\n    return set([\"red\", \"blue\"]).__contains__(value)\n";
    let py_tuple_factory =
        "def f(value, other):\n    return tuple([\"red\", \"blue\"]).__contains__(value)\n";
    let py_frozenset_factory =
        "def f(value, other):\n    return frozenset([\"red\", \"blue\"]).__contains__(value)\n";
    let py_deque_import = "from collections import deque\n\ndef f(value, other):\n    return deque([\"red\", \"blue\"]).__contains__(value)\n";
    let py_deque_alias = "from collections import deque as Values\n\ndef f(value, other):\n    return Values([\"red\", \"blue\"]).__contains__(value)\n";
    let py_deque_namespace = "import collections\n\ndef f(value, other):\n    return collections.deque([\"red\", \"blue\"]).__contains__(value)\n";
    let py_module_tuple =
        "VALUES = (\"red\", \"blue\")\n\ndef f(value, other):\n    return value in VALUES\n";
    let py_module_set =
        "VALUES = {\"red\", \"blue\"}\n\ndef f(value, other):\n    return value in VALUES\n";
    let js_set_inline =
        "function f(value, other) { return new Set([\"red\", \"blue\"]).has(value); }";
    let js_set_local = "function f(value, other) { const values = new Set([\"red\", \"blue\"]); return values.has(value); }";
    let js_set_call = "function f(value, other) { return Set([\"red\", \"blue\"]).has(value); }";
    let js_module_set =
        "const VALUES = new Set([\"red\", \"blue\"]);\nfunction f(value, other) { return VALUES.has(value); }";
    let ts_module_set = "const VALUES = new Set<string>([\"red\", \"blue\"]);\nfunction f(value: string, other: string): boolean { return VALUES.has(value); }";
    let js_array_contains =
        "function f(value, other) { return [\"red\", \"blue\"].contains(value); }";
    let js_array_some =
        "function f(value, other) { return [\"red\", \"blue\"].some((item) => item === value); }";
    let ts_array_some = "function f(value: string, other: string): boolean { return [\"red\", \"blue\"].some((item: string) => item === value); }";
    let js_array_indexof_ne =
        "function f(value, other) { return [\"red\", \"blue\"].indexOf(value) !== -1; }";
    let js_sequence_indexof_ne =
        "function f(value, other) { return (\"red\", \"blue\").indexOf(value) !== -1; }";
    let ts_array_indexof_ge = "function f(value: string, other: string): boolean { return [\"red\", \"blue\"].indexOf(value) >= 0; }";
    let js_array_indexof_gt =
        "function f(value, other) { return [\"red\", \"blue\"].indexOf(value) > -1; }";
    let js_array_indexof_reversed =
        "function f(value, other) { return -1 < [\"red\", \"blue\"].indexOf(value); }";
    let js_array_findindex_ne = "function f(value, other) { return [\"red\", \"blue\"].findIndex((item) => item === value) !== -1; }";
    let ts_array_findindex_ge = "function f(value: string, other: string): boolean { return [\"red\", \"blue\"].findIndex((item: string) => item === value) >= 0; }";
    let js_array_findindex_gt = "function f(value, other) { return [\"red\", \"blue\"].findIndex((item) => item === value) > -1; }";
    let js_array_findindex_reversed =
        "function f(value, other) { return -1 < [\"red\", \"blue\"].findIndex((item) => item === value); }";
    let js_array_filter_length_ne = "function f(value, other) { return [\"red\", \"blue\"].filter((item) => item === value).length !== 0; }";
    let ts_array_filter_length_ge = "function f(value: string, other: string): boolean { return [\"red\", \"blue\"].filter((item: string) => item === value).length >= 1; }";
    let js_array_filter_length_gt = "function f(value, other) { return [\"red\", \"blue\"].filter((item) => item === value).length > 0; }";
    let js_array_filter_length_reversed = "function f(value, other) { return 0 < [\"red\", \"blue\"].filter((item) => item === value).length; }";
    let js_array_filter_length_absence_eq = "function f(value, other) { return [\"red\", \"blue\"].filter((item) => item === value).length === 0; }";
    let ts_array_filter_length_absence_le = "function f(value: string, other: string): boolean { return [\"red\", \"blue\"].filter((item: string) => item === value).length <= 0; }";
    let js_array_filter_length_absence_lt = "function f(value, other) { return [\"red\", \"blue\"].filter((item) => item === value).length < 1; }";
    let js_array_filter_length_absence_reversed = "function f(value, other) { return 1 > [\"red\", \"blue\"].filter((item) => item === value).length; }";
    let java_module_list = "import java.util.List;\n\nclass C { static final List<String> VALUES = List.of(\"red\", \"blue\"); static boolean f(String value, String other) { return VALUES.contains(value); } }";
    let ruby_member = "def f(value, other)\n  [\"red\", \"blue\"].member?(value)\nend\n";
    let ruby_set_new_include =
        "require \"set\"\n\ndef f(value, other)\n  Set.new([\"red\", \"blue\"]).include?(value)\nend\n";
    let ruby_set_new_member =
        "require \"set\"\n\ndef f(value, other)\n  Set.new([\"red\", \"blue\"]).member?(value)\nend\n";
    let ruby_set_local = "require \"set\"\n\ndef f(value, other)\n  values = Set.new([\"red\", \"blue\"])\n  values.include?(value)\nend\n";
    let js_wrong_element =
        "function f(value, other) { return new Set([\"red\", \"blue\"]).has(other); }";
    let js_wrong_collection =
        "function f(value, other) { return new Set([\"green\", \"blue\"]).has(value); }";
    let js_array_some_wrong_element =
        "function f(value, other) { return [\"red\", \"blue\"].some((item) => item === other); }";
    let js_array_some_wrong_collection =
        "function f(value, other) { return [\"green\", \"blue\"].some((item) => item === value); }";
    let js_array_indexof_wrong_element =
        "function f(value, other) { return [\"red\", \"blue\"].indexOf(other) !== -1; }";
    let js_array_indexof_wrong_collection =
        "function f(value, other) { return [\"green\", \"blue\"].indexOf(value) >= 0; }";
    let js_array_indexof_value =
        "function f(value, other) { return [\"red\", \"blue\"].indexOf(value); }";
    let js_array_findindex_wrong_element = "function f(value, other) { return [\"red\", \"blue\"].findIndex((item) => item === other) !== -1; }";
    let js_array_findindex_wrong_collection = "function f(value, other) { return [\"green\", \"blue\"].findIndex((item) => item === value) >= 0; }";
    let js_array_findindex_value =
        "function f(value, other) { return [\"red\", \"blue\"].findIndex((item) => item === value); }";
    let js_array_filter_length_wrong_element = "function f(value, other) { return [\"red\", \"blue\"].filter((item) => item === other).length !== 0; }";
    let js_array_filter_length_wrong_collection = "function f(value, other) { return [\"green\", \"blue\"].filter((item) => item === value).length >= 1; }";
    let js_array_filter_length_value =
        "function f(value, other) { return [\"red\", \"blue\"].filter((item) => item === value).length; }";
    let js_array_filter_length_zero = "function f(value, other) { return [\"red\", \"blue\"].filter((item) => item === value).length === 0; }";
    let js_array_filter_length_absence_wrong_element = "function f(value, other) { return [\"red\", \"blue\"].filter((item) => item === other).length === 0; }";
    let js_array_filter_length_absence_wrong_collection = "function f(value, other) { return [\"green\", \"blue\"].filter((item) => item === value).length <= 0; }";
    let js_nan_includes = "function f(value, other) { return [NaN].includes(value); }";
    let js_nan_some = "function f(value, other) { return [NaN].some((item) => item === value); }";
    let js_nan_indexof = "function f(value, other) { return [NaN].indexOf(value) !== -1; }";
    let js_nan_findindex =
        "function f(value, other) { return [NaN].findIndex((item) => item === value) !== -1; }";
    let js_nan_filter_length =
        "function f(value, other) { return [NaN].filter((item) => item === value).length > 0; }";
    let js_nan_filter_length_absence =
        "function f(value, other) { return [NaN].filter((item) => item === value).length === 0; }";
    let py_absence = "def f(value, other):\n    return value not in [\"red\", \"blue\"]\n";
    let js_not_includes =
        "function f(value, other) { return ![\"red\", \"blue\"].includes(value); }";
    let js_array_every_absence =
        "function f(value, other) { return [\"red\", \"blue\"].every((item) => item !== value); }";
    let ts_array_every_absence = "function f(value: string, other: string): boolean { return [\"red\", \"blue\"].every((item: string) => item !== value); }";
    let js_array_every_wrong_element =
        "function f(value, other) { return [\"red\", \"blue\"].every((item) => item !== other); }";
    let js_array_every_wrong_collection =
        "function f(value, other) { return [\"green\", \"blue\"].every((item) => item !== value); }";
    let js_nan_not_includes = "function f(value, other) { return ![NaN].includes(value); }";
    let js_nan_every = "function f(value, other) { return [NaN].every((item) => item !== value); }";
    let js_shadowed_set =
        "function f(Set, value, other) { return new Set([\"red\", \"blue\"]).has(value); }";
    let js_global_shadowed_set = "function Set(values) { return { has: function() { return false; } }; }\nfunction f(value, other) { return new Set([\"red\", \"blue\"]).has(value); }";
    let js_module_set_mutated = "const VALUES = new Set([\"red\", \"blue\"]);\nVALUES.add(\"green\");\nfunction f(value, other) { return VALUES.has(value); }";
    let js_module_array_fill_mutated = "const VALUES = [\"red\", \"blue\"];\nVALUES.fill(\"green\");\nfunction f(value, other) { return VALUES.includes(value); }";
    let js_local_array_copywithin_mutated = "function f(value, other) { const values = [\"red\", \"blue\"]; values.copyWithin(0, 1); return values.includes(value); }";
    let ts_module_set_shadowed = "const Set: any = function(_values: any) { return { has: function() { return false; } }; };\nconst VALUES = new Set([\"red\", \"blue\"]);\nfunction f(value: string, other: string): boolean { return VALUES.has(value); }";
    let java_list_of = "import java.util.List;\n\nclass C { static boolean f(String value, String other) { return List.of(\"red\", \"blue\").contains(value); } }";
    let java_set_of = "import java.util.Set;\n\nclass C { static boolean f(String value, String other) { return Set.of(\"red\", \"blue\").contains(value); } }";
    let java_arrays_aslist = "import java.util.Arrays;\n\nclass C { static boolean f(String value, String other) { return Arrays.asList(\"red\", \"blue\").contains(value); } }";
    let java_collections_singleton = "import java.util.Collections;\n\nclass C { static boolean f(String value, String other) { return Collections.singleton(\"red\").contains(value); } }";
    let java_collections_singleton_list = "import java.util.Collections;\n\nclass C { static boolean f(String value, String other) { return Collections.singletonList(\"red\").contains(value); } }";
    let java_collections_empty_list = "import java.util.Collections;\n\nclass C { static boolean f(String value, String other) { return Collections.emptyList().contains(value); } }";
    let java_collections_empty_set = "import java.util.Collections;\n\nclass C { static boolean f(String value, String other) { return Collections.emptySet().contains(value); } }";
    let go_slices_package = "package p\n\nimport \"slices\"\n\nvar values = []string{\"red\", \"blue\"}\n\nfunc F(value string, other string) bool { return slices.Contains(values, value) }\n";
    let go_slices_alias = "package p\n\nimport sl \"slices\"\n\nvar values = []string{\"red\", \"blue\"}\n\nfunc F(value string, other string) bool { return sl.Contains(values, value) }\n";
    let go_slices_const = "package p\n\nimport \"slices\"\n\nconst first = \"red\"\nvar values = []string{first, \"blue\"}\n\nfunc F(value string, other string) bool { return slices.Contains(values, value) }\n";
    let go_slices_local = "package p\n\nimport \"slices\"\n\nfunc F(value string, other string) bool {\n    values := []string{\"red\", \"blue\"}\n    return slices.Contains(values, value)\n}\n";
    let java_local_list = "import java.util.List;\n\nclass C { static boolean f(String value, String other) { var values = List.of(\"red\", \"blue\"); return values.contains(value); } }";
    let rust_local_array = "pub fn f(value: &str, other: &str) -> bool {\n    let values = [\"red\", \"blue\"];\n    values.contains(&value)\n}\n";
    let rust_local_typed_array = "pub fn f(value: &str, other: &str) -> bool {\n    let values: [&str; 2] = [\"red\", \"blue\"];\n    values.contains(&value)\n}\n";
    let rust_local_slice_ref = "pub fn f(value: &str, other: &str) -> bool {\n    let values: &[&str] = &[\"red\", \"blue\"];\n    values.contains(&value)\n}\n";
    let rust_local_vec = "pub fn f(value: &str, other: &str) -> bool {\n    let values = vec![\"red\", \"blue\"];\n    values.contains(&value)\n}\n";
    let rust_std_hashset = "pub fn f(value: &str, other: &str) -> bool {\n    let values = std::collections::HashSet::from([\"red\", \"blue\"]);\n    values.contains(&value)\n}\n";
    let rust_std_btreeset = "pub fn f(value: &str, other: &str) -> bool {\n    let values = std::collections::BTreeSet::from([\"red\", \"blue\"]);\n    values.contains(&value)\n}\n";
    let rust_std_vecdeque = "pub fn f(value: &str, other: &str) -> bool {\n    let values = std::collections::VecDeque::from([\"red\", \"blue\"]);\n    values.contains(&value)\n}\n";
    let swift_array_literal = "func f(_ value: String, _ other: String) -> Bool {\n    return [\"red\", \"blue\"].contains(value)\n}\n";
    let swift_local_array = "func f(_ value: String, _ other: String) -> Bool {\n    let values = [\"red\", \"blue\"]\n    return values.contains(value)\n}\n";
    let java_wrong_element = "import java.util.List;\n\nclass C { static boolean f(String value, String other) { return List.of(\"red\", \"blue\").contains(other); } }";
    let java_wrong_collection = "import java.util.Set;\n\nclass C { static boolean f(String value, String other) { return Set.of(\"green\", \"blue\").contains(value); } }";
    let java_shadowed_list = "class C { static boolean f(Object List, String value, String other) { return List.of(\"red\", \"blue\").contains(value); } }";
    let java_local_list_class = "class C { static boolean f(String value, String other) { return List.of(\"red\", \"blue\").contains(value); } }\nclass List { static Box of(String a, String b) { return new Box(); } }\nclass Box { boolean contains(String value) { return false; } }";
    let java_module_list_shadowed = "class C { static final List<String> VALUES = List.of(\"red\", \"blue\"); static boolean f(String value, String other) { return VALUES.contains(value); } }\nclass List<T> { static java.util.List<String> of(String left, String right) { return java.util.List.of(\"green\", right); } }";
    let java_collections_missing_import = "class C { static boolean f(String value, String other) { return Collections.singletonList(\"red\").contains(value); } }\nclass Collections { static Box singletonList(String value) { return new Box(); } }\nclass Box { boolean contains(String value) { return false; } }";
    let java_collections_shadowed_receiver = "import java.util.Collections;\n\nclass C { static boolean f(Object Collections, String value, String other) { return Collections.singletonList(\"red\").contains(value); } }";
    let py_factory_wrong_element =
        "def f(value, other):\n    return set([\"red\", \"blue\"]).__contains__(other)\n";
    let py_factory_wrong_collection =
        "def f(value, other):\n    return set([\"green\", \"blue\"]).__contains__(value)\n";
    let py_factory_shadowed = "def f(value, other):\n    def set(_values):\n        class Box:\n            def __contains__(self, _value):\n                return False\n        return Box()\n    return set([\"red\", \"blue\"]).__contains__(value)\n";
    let py_deque_wrong_element = "from collections import deque\n\ndef f(value, other):\n    return deque([\"red\", \"blue\"]).__contains__(other)\n";
    let py_deque_wrong_collection = "from collections import deque\n\ndef f(value, other):\n    return deque([\"green\", \"blue\"]).__contains__(value)\n";
    let py_deque_missing_import =
        "def f(value, other):\n    return deque([\"red\", \"blue\"]).__contains__(value)\n";
    let py_deque_shadowed = "from collections import deque\n\ndef deque(_values):\n    class Box:\n        def __contains__(self, _value):\n            return False\n    return Box()\n\ndef f(value, other):\n    return deque([\"red\", \"blue\"]).__contains__(value)\n";
    let py_deque_mutated = "from collections import deque\n\ndef f(value, other):\n    values = deque([\"red\", \"blue\"])\n    values.append(\"green\")\n    return values.__contains__(value)\n";
    let py_module_mutated = "VALUES = [\"red\", \"blue\"]\nVALUES.append(\"green\")\n\ndef f(value, other):\n    return value in VALUES\n";
    let go_slices_wrong_element = "package p\n\nimport \"slices\"\n\nvar values = []string{\"red\", \"blue\"}\n\nfunc F(value string, other string) bool { return slices.Contains(values, other) }\n";
    let go_slices_wrong_collection = "package p\n\nimport \"slices\"\n\nvar values = []string{\"green\", \"blue\"}\n\nfunc F(value string, other string) bool { return slices.Contains(values, value) }\n";
    let go_slices_mutated = "package p\n\nimport \"slices\"\n\nvar values = append([]string{\"red\", \"blue\"}, \"green\")\n\nfunc F(value string, other string) bool { return slices.Contains(values, value) }\n";
    let go_slices_local_mutated = "package p\n\nimport \"slices\"\n\nfunc F(value string, other string) bool {\n    values := []string{\"red\", \"blue\"}\n    values = append(values, \"green\")\n    return slices.Contains(values, value)\n}\n";
    let go_slices_unimported = "package p\n\ntype fakeSlices struct{}\nfunc (fakeSlices) Contains(values []string, value string) bool { return false }\nvar slices fakeSlices\nvar values = []string{\"red\", \"blue\"}\n\nfunc F(value string, other string) bool { return slices.Contains(values, value) }\n";
    let java_local_list_mutated = "import java.util.ArrayList;\nimport java.util.List;\n\nclass C { static boolean f(String value, String other) { var values = new ArrayList<String>(List.of(\"red\", \"blue\")); values.add(\"green\"); return values.contains(value); } }";
    let rust_local_wrong_element = "pub fn f(value: &str, other: &str) -> bool {\n    let values = [\"red\", \"blue\"];\n    values.contains(&other)\n}\n";
    let rust_local_wrong_collection = "pub fn f(value: &str, other: &str) -> bool {\n    let values = [\"green\", \"blue\"];\n    values.contains(&value)\n}\n";
    let rust_local_mutated = "pub fn f(value: &str, other: &str) -> bool {\n    let mut values = vec![\"red\", \"blue\"];\n    values.push(\"green\");\n    values.contains(&value)\n}\n";
    let rust_local_custom_receiver = "struct Values;\nimpl Values { fn contains(&self, _value: &&str) -> bool { false } }\npub fn f(value: &str, other: &str) -> bool {\n    let values = Values;\n    values.contains(&value)\n}\n";
    let rust_std_wrong_element = "pub fn f(value: &str, other: &str) -> bool {\n    let values = std::collections::HashSet::from([\"red\", \"blue\"]);\n    values.contains(&other)\n}\n";
    let rust_std_wrong_collection = "pub fn f(value: &str, other: &str) -> bool {\n    let values = std::collections::BTreeSet::from([\"green\", \"blue\"]);\n    values.contains(&value)\n}\n";
    let rust_std_mutated = "pub fn f(value: &str, other: &str) -> bool {\n    let mut values = std::collections::HashSet::from([\"red\", \"blue\"]);\n    values.insert(\"green\");\n    values.contains(&value)\n}\n";
    let rust_std_shadowed = "mod std { pub mod collections { pub struct HashSet; } }\npub fn f(value: &str, other: &str) -> bool {\n    let values = std::collections::HashSet::from([\"red\", \"blue\"]);\n    values.contains(&value)\n}\n";
    let swift_wrong_element = "func f(_ value: String, _ other: String) -> Bool {\n    let values = [\"red\", \"blue\"]\n    return values.contains(other)\n}\n";
    let swift_wrong_collection = "func f(_ value: String, _ other: String) -> Bool {\n    let values = [\"green\", \"blue\"]\n    return values.contains(value)\n}\n";
    let swift_mutated = "func f(_ value: String, _ other: String) -> Bool {\n    var values = [\"red\", \"blue\"]\n    values.append(\"green\")\n    return values.contains(value)\n}\n";
    let swift_custom_receiver = "struct Values { func contains(_ value: String) -> Bool { return false } }\nfunc f(_ value: String, _ other: String) -> Bool {\n    let values = Values()\n    return values.contains(value)\n}\n";
    let ruby_set_wrong_element =
        "require \"set\"\n\ndef f(value, other)\n  Set.new([\"red\", \"blue\"]).include?(other)\nend\n";
    let ruby_set_wrong_collection =
        "require \"set\"\n\ndef f(value, other)\n  Set.new([\"green\", \"blue\"]).include?(value)\nend\n";
    let ruby_set_missing_require =
        "def f(value, other)\n  Set.new([\"red\", \"blue\"]).include?(value)\nend\n";
    let ruby_set_shadowed = "require \"set\"\n\nclass Set\n  def self.new(_values)\n    Box.new\n  end\nend\n\nclass Box\n  def include?(_value)\n    false\n  end\nend\n\ndef f(value, other)\n  Set.new([\"red\", \"blue\"]).include?(value)\nend\n";
    let ruby_set_mutated = "require \"set\"\n\ndef f(value, other)\n  values = Set.new([\"red\", \"blue\"])\n  values.add(\"green\")\n  values.include?(value)\nend\n";

    let literal_fp = value_fp(&i, py_literal, Lang::Python);
    assert_fingerprint_cases_converge(
        &i,
        &literal_fp,
        [
            fp_case!(py_set_factory, Python),
            fp_case!(py_tuple_factory, Python),
            fp_case!(py_frozenset_factory, Python),
            fp_case!(py_deque_import, Python),
            fp_case!(py_deque_alias, Python),
            fp_case!(py_deque_namespace, Python),
            fp_case!(py_module_set, Python),
            fp_case!(js_set_inline, JavaScript),
            fp_case!(js_set_local, JavaScript),
            fp_case!(js_module_set, JavaScript),
            fp_case!(ts_module_set, TypeScript),
            fp_case!(js_array_some, JavaScript),
            fp_case!(ts_array_some, TypeScript),
            fp_case!(js_array_indexof_ne, JavaScript),
            fp_case!(ts_array_indexof_ge, TypeScript),
            fp_case!(js_array_indexof_gt, JavaScript),
            fp_case!(js_array_indexof_reversed, JavaScript),
            fp_case!(js_array_findindex_ne, JavaScript),
            fp_case!(ts_array_findindex_ge, TypeScript),
            fp_case!(js_array_findindex_gt, JavaScript),
            fp_case!(js_array_findindex_reversed, JavaScript),
            fp_case!(js_array_filter_length_ne, JavaScript),
            fp_case!(ts_array_filter_length_ge, TypeScript),
            fp_case!(js_array_filter_length_gt, JavaScript),
            fp_case!(js_array_filter_length_reversed, JavaScript),
            fp_case!(java_list_of, Java),
            fp_case!(java_set_of, Java),
            fp_case!(java_arrays_aslist, Java),
            fp_case!(java_module_list, Java),
            fp_case!(go_slices_package, Go),
            fp_case!(go_slices_alias, Go),
            fp_case!(go_slices_const, Go),
            fp_case!(go_slices_local, Go),
            fp_case!(java_local_list, Java),
            fp_case!(rust_local_array, Rust),
            fp_case!(rust_local_typed_array, Rust),
            fp_case!(rust_local_slice_ref, Rust),
            fp_case!(rust_local_vec, Rust),
            fp_case!(rust_std_hashset, Rust),
            fp_case!(rust_std_btreeset, Rust),
            fp_case!(rust_std_vecdeque, Rust),
            fp_case!(swift_array_literal, Swift),
            fp_case!(swift_local_array, Swift),
            fp_case!(ruby_member, Ruby),
            fp_case!(ruby_set_new_include, Ruby),
            fp_case!(ruby_set_new_member, Ruby),
            fp_case!(ruby_set_local, Ruby),
        ],
    );
    assert_fingerprint_cases_stay_split(
        &i,
        &literal_fp,
        [
            fingerprint_case(
                "module tuple lacks surface/domain evidence",
                py_module_tuple,
                Lang::Python,
            ),
            fp_case!(js_set_call, JavaScript),
            fingerprint_case(
                "Array.contains is not a standard JavaScript contract",
                js_array_contains,
                Lang::JavaScript,
            ),
            fingerprint_case(
                "sequence expression is not a static JavaScript array",
                js_sequence_indexof_ne,
                Lang::JavaScript,
            ),
            named_fingerprint_case(
                "shadowed Rust std module",
                rust_std_shadowed,
                Lang::Rust,
                "f",
            ),
            fp_case!(js_wrong_element, JavaScript),
            fp_case!(js_wrong_collection, JavaScript),
            fingerprint_case(
                "construct syntax does not prove a shadowed JavaScript Set",
                js_global_shadowed_set,
                Lang::JavaScript,
            ),
            fp_case!(js_array_some_wrong_element, JavaScript),
            fp_case!(js_array_some_wrong_collection, JavaScript),
            fp_case!(js_array_indexof_wrong_element, JavaScript),
            fp_case!(js_array_indexof_wrong_collection, JavaScript),
            fp_case!(js_array_indexof_value, JavaScript),
            fp_case!(js_array_findindex_wrong_element, JavaScript),
            fp_case!(js_array_findindex_wrong_collection, JavaScript),
            fp_case!(js_array_findindex_value, JavaScript),
            fp_case!(js_array_filter_length_wrong_element, JavaScript),
            fp_case!(js_array_filter_length_wrong_collection, JavaScript),
            fp_case!(js_array_filter_length_value, JavaScript),
            fp_case!(js_array_filter_length_zero, JavaScript),
        ],
    );
    let nan_membership_fp = value_fp(&i, js_nan_includes, Lang::JavaScript);
    assert_fingerprint_cases_stay_split(
        &i,
        &nan_membership_fp,
        [
            fp_case!(js_nan_some, JavaScript),
            fp_case!(js_nan_indexof, JavaScript),
            fp_case!(js_nan_findindex, JavaScript),
            fp_case!(js_nan_filter_length, JavaScript),
        ],
    );
    let nan_absence_fp = value_fp(&i, js_nan_not_includes, Lang::JavaScript);
    assert_fingerprint_cases_stay_split(
        &i,
        &nan_absence_fp,
        [
            fp_case!(js_nan_filter_length_absence, JavaScript),
            fp_case!(js_nan_every, JavaScript),
        ],
    );
    let absence_fp = value_fp(&i, py_absence, Lang::Python);
    assert_ne!(
        literal_fp, absence_fp,
        "membership and absence must stay split"
    );
    assert_fingerprint_cases_converge(
        &i,
        &absence_fp,
        [
            fp_case!(js_not_includes, JavaScript),
            fp_case!(js_array_every_absence, JavaScript),
            fp_case!(ts_array_every_absence, TypeScript),
            fp_case!(js_array_filter_length_absence_eq, JavaScript),
            fp_case!(ts_array_filter_length_absence_le, TypeScript),
            fp_case!(js_array_filter_length_absence_lt, JavaScript),
            fp_case!(js_array_filter_length_absence_reversed, JavaScript),
        ],
    );
    assert_fingerprint_cases_stay_split(
        &i,
        &absence_fp,
        [
            fp_case!(js_array_every_wrong_element, JavaScript),
            fp_case!(js_array_every_wrong_collection, JavaScript),
            fp_case!(js_array_filter_length_absence_wrong_element, JavaScript),
            fp_case!(js_array_filter_length_absence_wrong_collection, JavaScript),
        ],
    );
    assert_fingerprint_cases_stay_split(
        &i,
        &literal_fp,
        [
            fp_case!(js_shadowed_set, JavaScript),
            fp_case!(js_module_set_mutated, JavaScript),
            fingerprint_case(
                "Array.fill invalidates collection evidence",
                js_module_array_fill_mutated,
                Lang::JavaScript,
            ),
            fingerprint_case(
                "Array.copyWithin invalidates collection evidence",
                js_local_array_copywithin_mutated,
                Lang::JavaScript,
            ),
            fp_case!(ts_module_set_shadowed, TypeScript),
            fp_case!(java_wrong_element, Java),
            fp_case!(java_wrong_collection, Java),
            fp_case!(java_shadowed_list, Java),
            fp_case!(java_local_list_class, Java),
            fp_case!(java_module_list_shadowed, Java),
        ],
    );
    let singleton_literal = "def f(value, other):\n    return value in [\"red\"]\n";
    let singleton_fp = value_fp(&i, singleton_literal, Lang::Python);
    assert_fingerprint_cases_converge(
        &i,
        &singleton_fp,
        [
            fp_case!(java_collections_singleton, Java),
            fp_case!(java_collections_singleton_list, Java),
        ],
    );
    assert_fingerprint_cases_stay_split(
        &i,
        &singleton_fp,
        [
            named_fingerprint_case(
                "unimported Java Collections",
                java_collections_missing_import,
                Lang::Java,
                "f",
            ),
            fp_case!(java_collections_shadowed_receiver, Java),
        ],
    );
    let empty_literal = "def f(value, other):\n    return value in []\n";
    let empty_fp = value_fp(&i, empty_literal, Lang::Python);
    assert_fingerprint_cases_converge(
        &i,
        &empty_fp,
        [
            fp_case!(java_collections_empty_list, Java),
            fp_case!(java_collections_empty_set, Java),
        ],
    );
    assert_fingerprint_cases_stay_split(
        &i,
        &literal_fp,
        [
            fp_case!(py_factory_wrong_element, Python),
            fp_case!(py_factory_wrong_collection, Python),
            fp_case!(py_factory_shadowed, Python),
            fp_case!(py_deque_wrong_element, Python),
            fp_case!(py_deque_wrong_collection, Python),
            fp_case!(py_deque_missing_import, Python),
            named_fingerprint_case(
                "shadowed Python deque",
                py_deque_shadowed,
                Lang::Python,
                "f",
            ),
            fp_case!(py_deque_mutated, Python),
            fp_case!(py_module_mutated, Python),
            fp_case!(go_slices_wrong_element, Go),
            fp_case!(go_slices_wrong_collection, Go),
            fp_case!(go_slices_mutated, Go),
            fp_case!(go_slices_local_mutated, Go),
            fp_case!(go_slices_unimported, Go),
            fp_case!(java_local_list_mutated, Java),
            fp_case!(rust_local_wrong_element, Rust),
            fp_case!(rust_local_wrong_collection, Rust),
            fp_case!(rust_local_mutated, Rust),
            fp_case!(rust_local_custom_receiver, Rust),
            fp_case!(rust_std_wrong_element, Rust),
            fp_case!(rust_std_wrong_collection, Rust),
            fp_case!(rust_std_mutated, Rust),
            fp_case!(swift_wrong_element, Swift),
            fp_case!(swift_wrong_collection, Swift),
            fp_case!(swift_mutated, Swift),
            named_fingerprint_case(
                "custom Swift receiver",
                swift_custom_receiver,
                Lang::Swift,
                "f",
            ),
            fp_case!(ruby_set_wrong_element, Ruby),
            fp_case!(ruby_set_wrong_collection, Ruby),
            fp_case!(ruby_set_missing_require, Ruby),
            fp_case!(ruby_set_shadowed, Ruby),
            fp_case!(ruby_set_mutated, Ruby),
        ],
    );

    let ts_array = "function f(values: string[], value: string, other: string): boolean { return values.includes(value); }";
    let ts_set = "function f(values: Set<string>, value: string, other: string): boolean { return values.has(value); }";
    let py_tuple =
        "def f(values: tuple[str, ...], value: str, other: str) -> bool:\n    return value in values\n";
    let py_alias_sequence = "from typing import Sequence as Values\n\ndef f(values: Values[str], value: str, other: str, other_values: Values[str]) -> bool:\n    return value in values\n";
    let py_alias_container = "from collections.abc import Container as Values\n\ndef f(values: Values[str], value: str, other: str, other_values: Values[str]) -> bool:\n    return value in values\n";
    let py_alias_set = "from typing import Set as Values\n\ndef f(values: Values[str], value: str, other: str, other_values: Values[str]) -> bool:\n    return value in values\n";
    let java_queue = "import java.util.Queue;\n\nclass C { static boolean f(Queue<String> values, String value, String other) { return values.contains(value); } }\n";
    let rust_vecdeque = "use std::collections::VecDeque;\n\npub fn f(values: &VecDeque<&str>, value: &str, other: &str) -> bool { values.contains(&value) }\n";
    let ts_untyped = "function f(values, value, other) { return values.has(value); }";
    let py_alias_wrong_element = "from typing import Sequence as Values\n\ndef f(values: Values[str], value: str, other: str, other_values: Values[str]) -> bool:\n    return other in values\n";
    let py_alias_wrong_receiver = "from typing import Sequence as Values\n\ndef f(values: Values[str], value: str, other: str, other_values: Values[str]) -> bool:\n    return value in other_values\n";
    let py_alias_unresolved = "def f(values: Values[str], value: str, other: str, other_values: Values[str]) -> bool:\n    return value in values\n";
    let py_alias_shadowed = "from typing import Sequence as Values\nValues = str\n\ndef f(values: Values[str], value: str, other: str, other_values: Values[str]) -> bool:\n    return value in values\n";
    let typed_fp = value_fp(&i, ts_array, Lang::TypeScript);
    assert_fingerprint_cases_converge(
        &i,
        &typed_fp,
        [
            fp_case!(ts_set, TypeScript),
            fp_case!(py_tuple, Python),
            fp_case!(py_alias_sequence, Python),
            fp_case!(py_alias_container, Python),
            fp_case!(py_alias_set, Python),
            fp_case!(java_queue, Java),
            fp_case!(rust_vecdeque, Rust),
        ],
    );
    assert_fingerprint_cases_stay_split(
        &i,
        &typed_fp,
        [
            fp_case!(ts_untyped, TypeScript),
            fp_case!(py_alias_wrong_element, Python),
            fp_case!(py_alias_wrong_receiver, Python),
            fp_case!(py_alias_unresolved, Python),
            fp_case!(py_alias_shadowed, Python),
        ],
    );
}
