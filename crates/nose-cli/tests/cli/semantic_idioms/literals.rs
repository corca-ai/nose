use super::*;

#[test]
fn query_mode_semantic_converges_cross_language_list_literals() {
    assert_single_semantic_family(
        "list_cross",
        &[
            (
                "pair.js",
                "export function pair(a, b) {\n    return [a, b];\n}\n",
            ),
            ("pair.py", "def make_pair(x, y):\n    return [x, y]\n"),
            (
                "pair.rb",
                "def build_pair(first, second)\n  [first, second]\nend\n",
            ),
            (
                "tuple_negative.py",
                "def tuple_pair(a, b):\n    return (a, b)\n",
            ),
        ],
        &["pair.js", "pair.py", "pair.rb"],
        &["tuple_negative.py"],
    );
}

#[test]
fn query_mode_semantic_preserves_js_object_keys() {
    assert_single_semantic_family(
        "object_semantic",
        &[
            (
                "object_a.ts",
                "export function example(command: string, description: string) {\n    return { command, description };\n}\n",
            ),
            (
                "object_b.ts",
                "export function makeExample(cmd: string, desc: string) {\n    return { command: cmd, description: desc };\n}\n",
            ),
            (
                "object_key_negative.ts",
                "export function makeParam(name: string, description: string) {\n    return { name, description };\n}\n",
            ),
            (
                "object_computed_a.ts",
                "const KEY = \"command\";\nexport function computed(command: string, description: string) {\n    return { [KEY]: command, description };\n}\n",
            ),
            (
                "object_computed_b.ts",
                "const FIELD = \"command\";\nexport function computedOther(cmd: string, desc: string) {\n    return { [FIELD]: cmd, description: desc };\n}\n",
            ),
        ],
        &["object_a.ts", "object_b.ts"],
        &[
            "object_key_negative.ts",
            "object_computed_a.ts",
            "object_computed_b.ts",
        ],
    );
}

#[test]
fn query_mode_semantic_converges_cross_language_map_literals() {
    assert_single_semantic_family(
        "map_cross",
        &[
            (
                "map.ts",
                "export function example(command: string, description: string) {\n    return { command, description };\n}\n",
            ),
            (
                "map.py",
                "def make_example(cmd, desc):\n    return {\"command\": cmd, \"description\": desc}\n",
            ),
            (
                "map.rb",
                "def build_example(command, description)\n  { command: command, description: description }\nend\n",
            ),
            (
                "map_key_negative.ts",
                "export function makeParam(name: string, description: string) {\n    return { name, description };\n}\n",
            ),
        ],
        &["map.ts", "map.py", "map.rb"],
        &["map_key_negative.ts"],
    );
}

#[test]
fn query_mode_semantic_captures_module_literal_bindings() {
    assert_single_semantic_family(
        "module_const",
        &[
            (
                "locale_a.ts",
                "const labels = { today: \"today\", tomorrow: \"tomorrow\" };\nexport function label(token: string) {\n    return labels[token];\n}\n",
            ),
            (
                "locale_b.ts",
                "const labels = { today: \"heute\", tomorrow: \"morgen\" };\nexport function label(token: string) {\n    return labels[token];\n}\n",
            ),
            (
                "locale_a_copy.ts",
                "const labels = { today: \"today\", tomorrow: \"tomorrow\" };\nexport function relativeLabel(key: string) {\n    return labels[key];\n}\n",
            ),
            (
                "locale_mutated.ts",
                "let labels = { today: \"today\", tomorrow: \"tomorrow\" };\nlabels = { today: \"heute\", tomorrow: \"morgen\" };\nexport function mutatedLabel(key: string) {\n    return labels[key];\n}\n",
            ),
        ],
        &["locale_a.ts", "locale_a_copy.ts"],
        &["locale_b.ts", "locale_mutated.ts"],
    );
}

#[test]
fn query_mode_semantic_preserves_python_dict_keys() {
    assert_single_semantic_family(
        "dict_semantic",
        &[
            (
                "dict_a.py",
                "def example(command, description):\n    return {\"command\": command, \"description\": description}\n",
            ),
            (
                "dict_b.py",
                "def make_example(cmd, desc):\n    return {\"command\": cmd, \"description\": desc}\n",
            ),
            (
                "dict_key_negative.py",
                "def make_param(name, description):\n    return {\"name\": name, \"description\": description}\n",
            ),
            (
                "dict_spread_a.py",
                "def with_spread(base, command):\n    return {**base, \"command\": command}\n",
            ),
            (
                "dict_spread_b.py",
                "def copy_spread(other, cmd):\n    return {**other, \"command\": cmd}\n",
            ),
        ],
        &["dict_a.py", "dict_b.py"],
        &[
            "dict_key_negative.py",
            "dict_spread_a.py",
            "dict_spread_b.py",
        ],
    );
}

#[test]
fn query_mode_semantic_preserves_ruby_hash_keys() {
    assert_single_semantic_family(
        "hash_semantic",
        &[
            (
                "hash_a.rb",
                "def example(command, description)\n  { command: command, description: description }\nend\n",
            ),
            (
                "hash_b.rb",
                "def make_example(cmd, desc)\n  { command: cmd, description: desc }\nend\n",
            ),
            (
                "hash_key_negative.rb",
                "def make_param(name, description)\n  { name: name, description: description }\nend\n",
            ),
            (
                "hash_splat_a.rb",
                "def with_splat(base, command)\n  { **base, command: command }\nend\n",
            ),
            (
                "hash_splat_b.rb",
                "def copy_splat(other, cmd)\n  { **other, command: cmd }\nend\n",
            ),
        ],
        &["hash_a.rb", "hash_b.rb"],
        &[
            "hash_key_negative.rb",
            "hash_splat_a.rb",
            "hash_splat_b.rb",
        ],
    );
}
