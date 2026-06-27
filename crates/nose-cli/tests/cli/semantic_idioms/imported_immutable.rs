use super::*;

#[test]
fn query_mode_semantic_proves_imported_immutable_value_provenance() {
    let project = TempProject::new("imported_immutable_567");
    write_imported_immutable_fixtures(&project);

    let semantic_json = project.query_semantic_min_json();
    assert_imported_map_default_family(&semantic_json);
    assert_imported_membership_family(&semantic_json);
    assert_imported_affix_family(&semantic_json);
    assert_imported_immutable_hard_negatives(&semantic_json);
}

#[allow(clippy::too_many_lines)]
fn write_imported_immutable_fixtures(project: &TempProject) {
    project.write(
        "inline_map_default.py",
        "def lookup(key, other):\n    return {\"red\": 1, \"blue\": 2}.get(key, 0)\n",
    );
    project.write("py_tables.py", "LOOKUP = {\"red\": 1, \"blue\": 2}\n");
    project.write(
        "py_imported_map.py",
        "from py_tables import LOOKUP\n\n\ndef lookup(key, other):\n    return LOOKUP.get(key, 0)\n",
    );
    project.write(
        "ImportedJavaTables.java",
        "import java.util.Map;\n\nclass ImportedJavaTables {\n    static final Map<String, Integer> LOOKUP = Map.of(\"red\", 1, \"blue\", 2);\n}\n",
    );
    project.write(
        "java_imported_map.java",
        "import static ImportedJavaTables.LOOKUP;\n\nclass JavaImportedMap {\n    static int lookup(String key, String other) {\n        return LOOKUP.getOrDefault(key, 0);\n    }\n}\n",
    );
    project.write(
        "gotables.go",
        "package gotables\n\nvar Lookup = map[string]int{\"red\": 1, \"blue\": 2}\n",
    );
    project.write(
        "go_imported_map.go",
        "package consumer\n\nimport \"gotables\"\n\nfunc Lookup(key string, other string) int {\n    return gotables.Lookup[key]\n}\n",
    );

    project.write(
        "inline_membership.ts",
        "function membership(value: string, other: string): boolean {\n    return [\"red\", \"blue\"].includes(value);\n}\n",
    );
    project.write("py_values.py", "VALUES = [\"red\", \"blue\"]\n");
    project.write(
        "py_imported_membership.py",
        "from py_values import VALUES\n\n\ndef membership(value, other):\n    return value in VALUES\n",
    );
    project.write(
        "ts_values.ts",
        "export const VALUES = [\"red\", \"blue\"];\n",
    );
    project.write(
        "ts_imported_membership.ts",
        "import { VALUES } from \"./ts_values\";\n\nfunction membership(value: string, other: string): boolean {\n    return VALUES.includes(value);\n}\n",
    );
    project.write(
        "JavaValues.java",
        "import java.util.List;\n\nclass JavaValues {\n    static final List<String> VALUES = List.of(\"red\", \"blue\");\n}\n",
    );
    project.write(
        "java_imported_membership.java",
        "import static JavaValues.VALUES;\n\nclass JavaImportedMembership {\n    static boolean membership(String value, String other) {\n        return VALUES.contains(value);\n    }\n}\n",
    );
    project.write(
        "rust_values.rs",
        "pub const VALUES: [&str; 2] = [\"red\", \"blue\"];\n",
    );
    project.write(
        "rust_imported_membership.rs",
        "use rust_values::VALUES;\n\npub fn membership(value: &str, other: &str) -> bool {\n    VALUES.contains(&value)\n}\n",
    );

    project.write(
        "literal_prefix.py",
        "def prefix(subject, other):\n    return subject.startswith(\"pre\")\n",
    );
    project.write("py_prefixes.py", "PREFIX = \"pre\"\n");
    project.write(
        "py_imported_prefix.py",
        "from py_prefixes import PREFIX\n\n\ndef prefix(subject, other):\n    return subject.startswith(PREFIX)\n",
    );
    project.write("ts_prefixes.ts", "export const PREFIX = \"pre\";\n");
    project.write(
        "ts_imported_prefix.ts",
        "import { PREFIX } from \"./ts_prefixes\";\n\nfunction prefix(subject: string, other: string): boolean {\n    return subject.startsWith(PREFIX);\n}\n",
    );
    project.write(
        "JavaPrefixes.java",
        "class JavaPrefixes {\n    static final String PREFIX = \"pre\";\n}\n",
    );
    project.write(
        "java_imported_prefix.java",
        "import static JavaPrefixes.PREFIX;\n\nclass JavaImportedPrefix {\n    static boolean prefix(String subject, String other) {\n        return subject.startsWith(PREFIX);\n    }\n}\n",
    );
    project.write("rust_prefixes.rs", "pub const PREFIX: &str = \"pre\";\n");
    project.write(
        "rust_imported_prefix.rs",
        "use rust_prefixes::PREFIX;\n\npub fn prefix(subject: &str, other: &str) -> bool {\n    subject.starts_with(PREFIX)\n}\n",
    );
    project.write(
        "goprefixes.go",
        "package goprefixes\n\nvar Prefix = \"pre\"\n",
    );
    project.write(
        "go_imported_prefix.go",
        "package consumer\n\nimport \"goprefixes\"\nimport \"strings\"\n\nfunc Prefix(subject string, other string) bool {\n    return strings.HasPrefix(subject, goprefixes.Prefix)\n}\n",
    );

    project.write(
        "py_mutated_tables.py",
        "LOOKUP = {\"red\": 1}\nLOOKUP[\"blue\"] = 2\n",
    );
    project.write(
        "py_mutated_imported_map.py",
        "from py_mutated_tables import LOOKUP\n\n\ndef lookup(key, other):\n    return LOOKUP.get(key, 0)\n",
    );
    project.write(
        "py_wrong_default.py",
        "from py_tables import LOOKUP\n\n\ndef lookup(key, other):\n    return LOOKUP.get(key, 9)\n",
    );
    project.write(
        "goshadow.go",
        "package goshadow\n\nvar Lookup = map[string]int{\"red\": 1}\n",
    );
    project.write(
        "go_shadowed_map.go",
        "package consumer\n\nimport \"goshadow\"\n\nfunc Shadow(goshadow any, key string) int {\n    return goshadow.Lookup[key]\n}\n",
    );
    project.write(
        "ts_reexport_values.ts",
        "export { VALUES } from \"./ts_values\";\n",
    );
    project.write(
        "ts_reexport_membership.ts",
        "import { VALUES } from \"./ts_reexport_values\";\n\nfunction membership(value: string, other: string): boolean {\n    return VALUES.includes(value);\n}\n",
    );
}

fn assert_imported_map_default_family(json: &serde_json::Value) {
    assert!(
        family_contains_all(
            json,
            &[
                "inline_map_default.py",
                "py_imported_map.py",
                "java_imported_map.java",
                "go_imported_map.go",
            ],
        ),
        "semantic mode should admit imported immutable map-default values across Python/Java/Go: {json}"
    );
}

fn assert_imported_membership_family(json: &serde_json::Value) {
    assert!(
        family_contains_all(
            json,
            &[
                "inline_membership.ts",
                "ts_imported_membership.ts",
                "rust_imported_membership.rs",
            ],
        ),
        "semantic mode should admit imported immutable membership collections: {json}"
    );
}

fn assert_imported_affix_family(json: &serde_json::Value) {
    assert!(
        family_contains_all(
            json,
            &[
                "ts_imported_prefix.ts",
                "java_imported_prefix.java",
                "rust_imported_prefix.rs",
                "go_imported_prefix.go",
            ],
        ),
        "semantic mode should admit imported immutable string-affix coordinates: {json}"
    );
}

fn assert_imported_immutable_hard_negatives(json: &serde_json::Value) {
    for unexpected in [
        "py_mutated_imported_map.py",
        "py_wrong_default.py",
        "go_shadowed_map.go",
    ] {
        assert!(
            !family_contains_all(json, &["inline_map_default.py", unexpected]),
            "semantic mode must keep {unexpected} out of the imported map-default family: {json}"
        );
    }
    assert!(
        !family_contains_all(json, &["inline_membership.ts", "ts_reexport_membership.ts"]),
        "dynamic/re-exported imported values must stay closed without direct provenance: {json}"
    );
}
