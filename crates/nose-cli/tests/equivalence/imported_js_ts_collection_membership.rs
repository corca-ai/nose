use super::*;

#[test]
fn collection_membership_converges_with_js_ts_imported_set_bindings() {
    let (dir, corpus) = lower_temp_corpus(
        "nose_imported_js_ts_set_membership",
        &[
            (
                "literal.py",
                "def member(value, other):\n    return value in [\"red\", \"blue\"]\n",
            ),
            (
                "js_local.js",
                "export function member(value, other) {\n  return new Set([\"red\", \"blue\"]).has(value);\n}\n",
            ),
            (
                "js_values.js",
                "export const VALUES = new Set([\"red\", \"blue\"]);\n",
            ),
            (
                "js_imported.js",
                "import { VALUES } from './js_values';\nexport function member(value, other) {\n  return VALUES.has(value);\n}\n",
            ),
            (
                "js_mutated_values.js",
                "export const VALUES = new Set([\"red\", \"blue\"]);\nVALUES.add(\"green\");\n",
            ),
            (
                "js_imported_mutated_provider.js",
                "import { VALUES } from './js_mutated_values';\nexport function member(value, other) {\n  return VALUES.has(value);\n}\n",
            ),
            (
                "js_imported_mutated_receiver.js",
                "import { VALUES } from './js_values';\nVALUES.add(\"green\");\nexport function member(value, other) {\n  return VALUES.has(value);\n}\n",
            ),
            (
                "js_wrong_collection.js",
                "import { VALUES } from './js_values';\nexport function member(value, other) {\n  return new Set([\"green\", \"blue\"]).has(value);\n}\n",
            ),
            (
                "js_shadowed_values.js",
                "function Set(_values) { return { has: function() { return false; } }; }\nexport const VALUES = new Set([\"red\", \"blue\"]);\n",
            ),
            (
                "js_imported_shadowed_provider.js",
                "import { VALUES } from './js_shadowed_values';\nexport function member(value, other) {\n  return VALUES.has(value);\n}\n",
            ),
            (
                "ts_values.ts",
                "export const VALUES = new Set<string>([\"red\", \"blue\"]);\n",
            ),
            (
                "ts_imported.ts",
                "import { VALUES } from './ts_values';\nexport function member(value: string, other: string): boolean {\n  return VALUES.has(value);\n}\n",
            ),
        ],
    );
    let literal = corpus_value_fp(&corpus, "literal.py", "member");
    assert_eq!(
        literal,
        corpus_value_fp(&corpus, "js_local.js", "member"),
        "local JS Set constructor should converge with literal membership"
    );
    assert_eq!(
        literal,
        corpus_value_fp(&corpus, "js_imported.js", "member"),
        "JS imported Set binding should retain provider constructor provenance"
    );
    assert_eq!(
        literal,
        corpus_value_fp(&corpus, "ts_imported.ts", "member"),
        "TS imported Set binding should retain provider constructor provenance"
    );
    assert_ne!(
        literal,
        corpus_value_fp(&corpus, "js_imported_mutated_provider.js", "member"),
        "provider mutation must block JS imported Set provenance"
    );
    assert_ne!(
        literal,
        corpus_value_fp(&corpus, "js_imported_mutated_receiver.js", "member"),
        "importer mutation must keep JS imported Set receiver distinct"
    );
    assert_ne!(
        literal,
        corpus_value_fp(&corpus, "js_wrong_collection.js", "member"),
        "different JS Set contents must stay distinct"
    );
    assert_ne!(
        literal,
        corpus_value_fp(&corpus, "js_imported_shadowed_provider.js", "member"),
        "provider-local Set shadowing must block JS imported Set provenance"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
