use super::*;

#[allow(clippy::too_many_lines)]
pub(super) fn write_literal_membership_fixtures(dir: &Path) {
    fs::write(
        dir.join("membership.py"),
        "def membership(value, other):\n    return value in [\"red\", \"blue\"]\n",
    )
    .unwrap();
    fs::write(
        dir.join("python_set_factory.py"),
        "def python_set_factory(value, other):\n    return set([\"red\", \"blue\"]).__contains__(value)\n",
    )
    .unwrap();
    fs::write(
        dir.join("python_tuple_factory.py"),
        "def python_tuple_factory(value, other):\n    return tuple([\"red\", \"blue\"]).__contains__(value)\n",
    )
    .unwrap();
    fs::write(
        dir.join("python_frozenset_factory.py"),
        "def python_frozenset_factory(value, other):\n    return frozenset([\"red\", \"blue\"]).__contains__(value)\n",
    )
    .unwrap();
    fs::write(
        dir.join("python_deque_import.py"),
        "from collections import deque\n\n\ndef python_deque_import(value, other):\n    return deque([\"red\", \"blue\"]).__contains__(value)\n",
    )
    .unwrap();
    fs::write(
        dir.join("python_deque_alias.py"),
        "from collections import deque as Values\n\n\ndef python_deque_alias(value, other):\n    return Values([\"red\", \"blue\"]).__contains__(value)\n",
    )
    .unwrap();
    fs::write(
        dir.join("python_deque_namespace.py"),
        "import collections\n\n\ndef python_deque_namespace(value, other):\n    return collections.deque([\"red\", \"blue\"]).__contains__(value)\n",
    )
    .unwrap();
    fs::write(
        dir.join("python_module_tuple.py"),
        "VALUES = (\"red\", \"blue\")\n\n\ndef python_module_tuple(value, other):\n    return value in VALUES\n",
    )
    .unwrap();
    fs::write(
        dir.join("python_module_set.py"),
        "VALUES = {\"red\", \"blue\"}\n\n\ndef python_module_set(value, other):\n    return value in VALUES\n",
    )
    .unwrap();
    fs::write(
        dir.join("membership.js"),
        "function membership(value, other) {\n  return [\"red\", \"blue\"].includes(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("membership.ts"),
        "function membership(value: string, other: string): boolean {\n  return [\"red\", \"blue\"].includes(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("js_in_array_a.js"),
        "function jsInArrayA(value, other) {\n  return value in [\"red\", \"blue\"];\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("js_in_array_b.js"),
        "function jsInArrayB(value, other) {\n  return value in [\"red\", \"blue\"];\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("membership.go"),
        "package p\n\nimport \"slices\"\n\nfunc Membership(value string, other string) bool {\n    return slices.Contains([]string{\"red\", \"blue\"}, value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("membership.rs"),
        "pub fn membership(value: &str, other: &str) -> bool {\n    [\"red\", \"blue\"].contains(value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("membership.swift"),
        "func membership(_ value: String, _ other: String) -> Bool {\n    return [\"red\", \"blue\"].contains(value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("membership.rb"),
        "def membership(value, other)\n  [\"red\", \"blue\"].include?(value)\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("ruby_member.rb"),
        "def ruby_member(value, other)\n  [\"red\", \"blue\"].member?(value)\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("ruby_set_new_include.rb"),
        "require \"set\"\n\ndef ruby_set_new_include(value, other)\n  Set.new([\"red\", \"blue\"]).include?(value)\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("ruby_set_new_member.rb"),
        "require \"set\"\n\ndef ruby_set_new_member(value, other)\n  Set.new([\"red\", \"blue\"]).member?(value)\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("ruby_set_local.rb"),
        "require \"set\"\n\ndef ruby_set_local(value, other)\n  values = Set.new([\"red\", \"blue\"])\n  values.include?(value)\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("module_set.js"),
        "const VALUES = new Set([\"red\", \"blue\"]);\n\nfunction moduleSet(value, other) {\n  return VALUES.has(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("module_set.ts"),
        "const VALUES = new Set<string>([\"red\", \"blue\"]);\n\nfunction moduleSet(value: string, other: string): boolean {\n  return VALUES.has(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_some.js"),
        "function arraySome(value, other) {\n  return [\"red\", \"blue\"].some((item) => item === value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_some.ts"),
        "function arraySome(value: string, other: string): boolean {\n  return [\"red\", \"blue\"].some((item: string) => item === value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_indexof.js"),
        "function arrayIndexOf(value, other) {\n  return [\"red\", \"blue\"].indexOf(value) !== -1;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_indexof.ts"),
        "function arrayIndexOf(value: string, other: string): boolean {\n  return [\"red\", \"blue\"].indexOf(value) >= 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_findindex.js"),
        "function arrayFindIndex(value, other) {\n  return [\"red\", \"blue\"].findIndex((item) => item === value) !== -1;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_findindex.ts"),
        "function arrayFindIndex(value: string, other: string): boolean {\n  return [\"red\", \"blue\"].findIndex((item: string) => item === value) >= 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_some_loose.js"),
        "function arraySomeLoose(value, other) {\n  return [\"red\", \"blue\"].some((item) => item == value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_findindex_loose.js"),
        "function arrayFindIndexLoose(value, other) {\n  return [\"red\", \"blue\"].findIndex((item) => item == value) !== -1;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_filter_length.js"),
        "function arrayFilterLength(value, other) {\n  return [\"red\", \"blue\"].filter((item) => item === value).length !== 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_filter_length.ts"),
        "function arrayFilterLength(value: string, other: string): boolean {\n  return [\"red\", \"blue\"].filter((item: string) => item === value).length >= 1;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("not_membership.py"),
        "def not_membership(value, other):\n    return value not in [\"red\", \"blue\"]\n",
    )
    .unwrap();
    fs::write(
        dir.join("not_includes.js"),
        "function notIncludes(value, other) {\n  return ![\"red\", \"blue\"].includes(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_every.js"),
        "function arrayEvery(value, other) {\n  return [\"red\", \"blue\"].every((item) => item !== value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_every.ts"),
        "function arrayEvery(value: string, other: string): boolean {\n  return [\"red\", \"blue\"].every((item: string) => item !== value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_every_loose.js"),
        "function arrayEveryLoose(value, other) {\n  return [\"red\", \"blue\"].every((item) => item != value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_filter_length_absence.js"),
        "function arrayFilterLengthAbsence(value, other) {\n  return [\"red\", \"blue\"].filter((item) => item === value).length === 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_filter_length_loose.js"),
        "function arrayFilterLengthLoose(value, other) {\n  return [\"red\", \"blue\"].filter((item) => item == value).length !== 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_filter_length_absence.ts"),
        "function arrayFilterLengthAbsence(value: string, other: string): boolean {\n  return [\"red\", \"blue\"].filter((item: string) => item === value).length <= 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("module_list.java"),
        "import java.util.List;\n\nclass ModuleList {\n    static final List<String> VALUES = List.of(\"red\", \"blue\");\n\n    static boolean moduleList(String value, String other) {\n        return VALUES.contains(value);\n    }\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("go_slices_package.go"),
        "package p\n\nimport \"slices\"\n\nvar values = []string{\"red\", \"blue\"}\n\nfunc SlicesPackage(value string, other string) bool {\n    return slices.Contains(values, value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("go_slices_alias.go"),
        "package p\n\nimport sl \"slices\"\n\nvar values = []string{\"red\", \"blue\"}\n\nfunc SlicesAlias(value string, other string) bool {\n    return sl.Contains(values, value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("go_slices_const.go"),
        "package p\n\nimport \"slices\"\n\nconst first = \"red\"\nvar values = []string{first, \"blue\"}\n\nfunc SlicesConst(value string, other string) bool {\n    return slices.Contains(values, value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("go_slices_local.go"),
        "package p\n\nimport \"slices\"\n\nfunc SlicesLocal(value string, other string) bool {\n    values := []string{\"red\", \"blue\"}\n    return slices.Contains(values, value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("java_local_list.java"),
        "import java.util.List;\n\nclass JavaLocalList {\n    static boolean javaLocalList(String value, String other) {\n        var values = List.of(\"red\", \"blue\");\n        return values.contains(value);\n    }\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("rust_local_array.rs"),
        "pub fn rust_local_array(value: &str, other: &str) -> bool {\n    let values = [\"red\", \"blue\"];\n    values.contains(&value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("rust_local_typed_array.rs"),
        "pub fn rust_local_typed_array(value: &str, other: &str) -> bool {\n    let values: [&str; 2] = [\"red\", \"blue\"];\n    values.contains(&value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("rust_local_slice_ref.rs"),
        "pub fn rust_local_slice_ref(value: &str, other: &str) -> bool {\n    let values: &[&str] = &[\"red\", \"blue\"];\n    values.contains(&value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("rust_local_vec.rs"),
        "pub fn rust_local_vec(value: &str, other: &str) -> bool {\n    let values = vec![\"red\", \"blue\"];\n    values.contains(&value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("swift_local_array.swift"),
        "func swiftLocalArray(_ value: String, _ other: String) -> Bool {\n    let values = [\"red\", \"blue\"]\n    return values.contains(value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("rust_std_hashset.rs"),
        "pub fn rust_std_hashset(value: &str, other: &str) -> bool {\n    let values = std::collections::HashSet::from([\"red\", \"blue\"]);\n    values.contains(&value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("rust_std_btreeset.rs"),
        "pub fn rust_std_btreeset(value: &str, other: &str) -> bool {\n    let values = std::collections::BTreeSet::from([\"red\", \"blue\"]);\n    values.contains(&value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("rust_std_vecdeque.rs"),
        "pub fn rust_std_vecdeque(value: &str, other: &str) -> bool {\n    let values = std::collections::VecDeque::from([\"red\", \"blue\"]);\n    values.contains(&value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_element.py"),
        "def wrong_element(value, other):\n    return other in [\"red\", \"blue\"]\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_collection.js"),
        "function wrongCollection(value, other) {\n  return [\"green\", \"blue\"].includes(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("swift_wrong_element.swift"),
        "func swiftWrongElement(_ value: String, _ other: String) -> Bool {\n    let values = [\"red\", \"blue\"]\n    return values.contains(other)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("swift_wrong_collection.swift"),
        "func swiftWrongCollection(_ value: String, _ other: String) -> Bool {\n    let values = [\"green\", \"blue\"]\n    return values.contains(value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_some_wrong_element.js"),
        "function arraySomeWrongElement(value, other, third) {\n  return [\"red\", \"blue\"].some((item) => item === third);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_some_wrong_collection.ts"),
        "function arraySomeWrongCollection(value: string, other: string): boolean {\n  return [\"purple\", \"orange\"].some((item: string) => item === value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_indexof_wrong_element.js"),
        "function arrayIndexOfWrongElement(value, other, third) {\n  return [\"red\", \"blue\"].indexOf(value + third) !== -1;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_indexof_wrong_collection.ts"),
        "function arrayIndexOfWrongCollection(value: string, other: string): boolean {\n  return [\"yellow\", \"orange\"].indexOf(value) >= 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_indexof_value.js"),
        "function arrayIndexOfValue(value, other) {\n  return [\"red\", \"blue\"].indexOf(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_indexof_ne_zero.js"),
        "function arrayIndexOfNeZero(value, other) {\n  return [\"red\", \"blue\"].indexOf(value) !== 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_indexof_reversed_gt_zero.js"),
        "function arrayIndexOfReversedGtZero(value, other) {\n  return 0 < [\"red\", \"blue\"].indexOf(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_findindex_wrong_element.js"),
        "function arrayFindIndexWrongElement(value, other, third) {\n  return [\"red\", \"blue\"].findIndex((item) => item === value + third + other) !== -1;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_findindex_wrong_collection.ts"),
        "function arrayFindIndexWrongCollection(value: string, other: string): boolean {\n  return [\"cyan\", \"magenta\"].findIndex((item: string) => item === value) >= 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_findindex_value.js"),
        "function arrayFindIndexValue(value, other) {\n  return [\"red\", \"blue\"].findIndex((item) => item === value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_findindex_ne_zero.js"),
        "function arrayFindIndexNeZero(value, other) {\n  return [\"red\", \"blue\"].findIndex((item) => item === value) !== 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_filter_length_wrong_element.js"),
        "function arrayFilterLengthWrongElement(value, other, third) {\n  return [\"red\", \"blue\"].filter((item) => item === other + third).length !== 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_filter_length_wrong_collection.ts"),
        "function arrayFilterLengthWrongCollection(value: string, other: string): boolean {\n  return [\"black\", \"white\"].filter((item: string) => item === value).length >= 1;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_filter_length_value.js"),
        "function arrayFilterLengthValue(value, other) {\n  return [\"red\", \"blue\"].filter((item) => item === value).length;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_filter_length_absence_wrong_element.js"),
        "function arrayFilterLengthAbsenceWrongElement(value, other, third) {\n  return [\"red\", \"blue\"].filter((item) => item === other + third).length === 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_filter_length_absence_wrong_collection.ts"),
        "function arrayFilterLengthAbsenceWrongCollection(value: string, other: string): boolean {\n  return [\"black\", \"white\"].filter((item: string) => item === value).length <= 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_every_wrong_element.js"),
        "function arrayEveryWrongElement(value, other, third) {\n  return [\"red\", \"blue\"].every((item) => item !== third);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_every_wrong_collection.ts"),
        "function arrayEveryWrongCollection(value: string, other: string): boolean {\n  return [\"purple\", \"orange\"].every((item: string) => item !== value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("substring.rs"),
        "pub fn substring(value: &str, other: &str) -> bool {\n    value.contains(\"red\")\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("module_set_mutated.js"),
        "const VALUES = new Set([\"red\", \"blue\"]);\nVALUES.add(\"green\");\n\nfunction moduleSetMutated(value, other) {\n  return VALUES.has(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("python_module_mutated.py"),
        "VALUES = [\"red\", \"blue\"]\nVALUES.append(\"green\")\n\n\ndef python_module_mutated(value, other):\n    return value in VALUES\n",
    )
    .unwrap();
    fs::write(
        dir.join("module_set_shadowed.ts"),
        "const Set: any = function(_values: any) {\n  return { has: function() { return false; } };\n};\nconst VALUES = new Set([\"red\", \"blue\"]);\n\nfunction moduleSetShadowed(value: string, other: string): boolean {\n  return VALUES.has(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("module_list_shadowed.java"),
        "class ModuleListShadowed {\n    static final List<String> VALUES = List.of(\"red\", \"blue\");\n\n    static boolean moduleListShadowed(String value, String other) {\n        return VALUES.contains(value);\n    }\n}\n\nclass List<T> {\n    static java.util.List<String> of(String left, String right) {\n        return java.util.List.of(\"green\", right);\n    }\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("python_factory_shadowed.py"),
        "def python_factory_shadowed(value, other):\n    def set(_values):\n        class Box:\n            def __contains__(self, _value):\n                return False\n        return Box()\n    return set([\"red\", \"blue\"]).__contains__(value)\n",
    )
    .unwrap();
    fs::write(
        dir.join("python_deque_wrong_element.py"),
        "from collections import deque\n\n\ndef python_deque_wrong_element(value, other):\n    return deque([\"red\", \"blue\"]).__contains__(other)\n",
    )
    .unwrap();
    fs::write(
        dir.join("python_deque_wrong_collection.py"),
        "from collections import deque\n\n\ndef python_deque_wrong_collection(value, other):\n    return deque([\"green\", \"blue\"]).__contains__(value)\n",
    )
    .unwrap();
    fs::write(
        dir.join("python_deque_missing_import.py"),
        "def python_deque_missing_import(value, other):\n    return deque([\"red\", \"blue\"]).__contains__(value)\n",
    )
    .unwrap();
    fs::write(
        dir.join("python_deque_shadowed.py"),
        "from collections import deque\n\n\ndef deque(_values):\n    class Box:\n        def __contains__(self, _value):\n            return False\n    return Box()\n\n\ndef python_deque_shadowed(value, other):\n    return deque([\"red\", \"blue\"]).__contains__(value)\n",
    )
    .unwrap();
    fs::write(
        dir.join("python_deque_mutated.py"),
        "from collections import deque\n\n\ndef python_deque_mutated(value, other):\n    values = deque([\"red\", \"blue\"])\n    values.append(\"green\")\n    return values.__contains__(value)\n",
    )
    .unwrap();
    fs::write(
        dir.join("go_slices_mutated.go"),
        "package p\n\nimport \"slices\"\n\nvar values = append([]string{\"red\", \"blue\"}, \"green\")\n\nfunc SlicesMutated(value string, other string) bool {\n    return slices.Contains(values, value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("go_slices_local_mutated.go"),
        "package p\n\nimport \"slices\"\n\nfunc SlicesLocalMutated(value string, other string) bool {\n    values := []string{\"red\", \"blue\"}\n    values = append(values, \"green\")\n    return slices.Contains(values, value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("go_slices_unimported.go"),
        "package p\n\ntype fakeSlices struct{}\n\nfunc (fakeSlices) Contains(values []string, value string) bool {\n    return false\n}\n\nvar slices fakeSlices\nvar values = []string{\"red\", \"blue\"}\n\nfunc SlicesUnimported(value string, other string) bool {\n    return slices.Contains(values, value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("java_local_list_mutated.java"),
        "import java.util.ArrayList;\nimport java.util.List;\n\nclass JavaLocalListMutated {\n    static boolean javaLocalListMutated(String value, String other) {\n        var values = new ArrayList<String>(List.of(\"red\", \"blue\"));\n        values.add(\"green\");\n        return values.contains(value);\n    }\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("rust_local_mutated.rs"),
        "pub fn rust_local_mutated(value: &str, other: &str) -> bool {\n    let mut values = vec![\"red\", \"blue\"];\n    values.push(\"green\");\n    values.contains(&value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("swift_local_mutated.swift"),
        "func swiftLocalMutated(_ value: String, _ other: String) -> Bool {\n    var values = [\"red\", \"blue\"]\n    values.append(\"green\")\n    return values.contains(value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("swift_custom_receiver.swift"),
        "struct Values {\n    func contains(_ value: String) -> Bool {\n        return false\n    }\n}\n\nfunc swiftCustomReceiver(_ value: String, _ other: String) -> Bool {\n    let values = Values()\n    return values.contains(value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("rust_local_custom_receiver.rs"),
        "struct Values;\n\nimpl Values {\n    fn contains(&self, _value: &&str) -> bool {\n        false\n    }\n}\n\npub fn rust_local_custom_receiver(value: &str, other: &str) -> bool {\n    let values = Values;\n    values.contains(&value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("rust_std_wrong_element.rs"),
        "pub fn rust_std_wrong_element(value: &str, other: &str) -> bool {\n    let values = std::collections::HashSet::from([\"red\", \"blue\"]);\n    values.contains(&(value.to_owned() + other))\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("rust_std_wrong_collection.rs"),
        "pub fn rust_std_wrong_collection(value: &str, other: &str) -> bool {\n    let values = std::collections::BTreeSet::from([\"silver\", \"gold\"]);\n    values.contains(&value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("rust_std_mutated.rs"),
        "pub fn rust_std_mutated(value: &str, other: &str) -> bool {\n    let mut values = std::collections::HashSet::from([\"red\", \"blue\"]);\n    values.insert(\"green\");\n    values.contains(&value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("ruby_set_wrong_element.rb"),
        "require \"set\"\n\ndef ruby_set_wrong_element(value, other)\n  Set.new([\"red\", \"blue\"]).include?(other)\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("ruby_set_wrong_collection.rb"),
        "require \"set\"\n\ndef ruby_set_wrong_collection(value, other)\n  Set.new([\"green\", \"blue\"]).include?(value)\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("ruby_set_missing_require.rb"),
        "def ruby_set_missing_require(value, other)\n  Set.new([\"red\", \"blue\"]).include?(value)\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("ruby_set_shadowed.rb"),
        "require \"set\"\n\nclass Set\n  def self.new(_values)\n    Box.new\n  end\nend\n\nclass Box\n  def include?(_value)\n    false\n  end\nend\n\ndef ruby_set_shadowed(value, other)\n  Set.new([\"red\", \"blue\"]).include?(value)\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("ruby_set_mutated.rb"),
        "require \"set\"\n\ndef ruby_set_mutated(value, other)\n  values = Set.new([\"red\", \"blue\"])\n  values.add(\"green\")\n  values.include?(value)\nend\n",
    )
    .unwrap();
}
