use super::*;

#[test]
fn literal_map_default_lookup_converges_with_js_ts_imported_bindings() {
    let (dir, corpus) = lower_temp_corpus(
        "nose_imported_js_ts_map_default",
        &[
            (
                "local.py",
                "def lookup(key, other):\n    return {\"red\": 1, \"blue\": 2}.get(key, 0)\n",
            ),
            (
                "js_tables.js",
                "export const LOOKUP = new Map([[\"red\", 1], [\"blue\", 2]]);\n",
            ),
            (
                "js_local.js",
                "export function lookup(key, other) {\n  return new Map([[\"red\", 1], [\"blue\", 2]]).get(key) ?? 0;\n}\n",
            ),
            (
                "js_imported.js",
                "import { LOOKUP } from './js_tables';\nexport function lookup(key, other) {\n  return LOOKUP.get(key) ?? 0;\n}\n",
            ),
            (
                "js_mutated_tables.js",
                "export const LOOKUP = new Map([[\"red\", 1], [\"blue\", 2]]);\nLOOKUP.set(\"red\", 9);\n",
            ),
            (
                "js_imported_mutated_provider.js",
                "import { LOOKUP } from './js_mutated_tables';\nexport function lookup(key, other) {\n  return LOOKUP.get(key) ?? 0;\n}\n",
            ),
            (
                "js_imported_mutated_receiver.js",
                "import { LOOKUP } from './js_tables';\nLOOKUP.set(\"red\", 9);\nexport function lookup(key, other) {\n  return LOOKUP.get(key) ?? 0;\n}\n",
            ),
            (
                "js_wrong_map.js",
                "import { LOOKUP } from './js_tables';\nexport function lookup(key, other) {\n  return new Map([[\"red\", 9], [\"blue\", 2]]).get(key) ?? 0;\n}\n",
            ),
            (
                "js_shadowed_tables.js",
                "function Map(_entries) { return { get: function() { return 9; } }; }\nexport const LOOKUP = new Map([[\"red\", 1], [\"blue\", 2]]);\n",
            ),
            (
                "js_imported_shadowed_provider.js",
                "import { LOOKUP } from './js_shadowed_tables';\nexport function lookup(key, other) {\n  return LOOKUP.get(key) ?? 0;\n}\n",
            ),
            (
                "ts_tables.ts",
                "export const LOOKUP = new Map<string, number>([[\"red\", 1], [\"blue\", 2]]);\n",
            ),
            (
                "ts_local.ts",
                "export function lookup(key: string, other: string): number {\n  return new Map<string, number>([[\"red\", 1], [\"blue\", 2]]).get(key) ?? 0;\n}\n",
            ),
            (
                "ts_imported.ts",
                "import { LOOKUP } from './ts_tables';\nexport function lookup(key: string, other: string): number {\n  return LOOKUP.get(key) ?? 0;\n}\n",
            ),
        ],
    );
    let python_absence_default = corpus_value_fp(&corpus, "local.py", "lookup");
    let coalesce = corpus_value_fp(&corpus, "js_local.js", "lookup");
    assert_eq!(
        coalesce,
        corpus_value_fp(&corpus, "ts_local.ts", "lookup"),
        "local JS and TS Map constructors should stay in the same nullish-default family"
    );
    assert_ne!(
        python_absence_default, coalesce,
        "JS nullish-default Map.get remains distinct from Python absence-default dict.get"
    );
    assert_eq!(
        coalesce,
        corpus_value_fp(&corpus, "js_imported.js", "lookup"),
        "JS imported Map binding should retain provider constructor provenance"
    );
    assert_eq!(
        coalesce,
        corpus_value_fp(&corpus, "ts_imported.ts", "lookup"),
        "TS imported Map binding should retain provider constructor provenance"
    );
    assert_ne!(
        coalesce,
        corpus_value_fp(&corpus, "js_imported_mutated_provider.js", "lookup"),
        "provider mutation must block JS imported map provenance"
    );
    assert_ne!(
        coalesce,
        corpus_value_fp(&corpus, "js_imported_mutated_receiver.js", "lookup"),
        "importer mutation must keep JS imported receiver distinct"
    );
    assert_ne!(
        coalesce,
        corpus_value_fp(&corpus, "js_wrong_map.js", "lookup"),
        "different JS map contents must stay distinct"
    );
    assert_ne!(
        coalesce,
        corpus_value_fp(&corpus, "js_imported_shadowed_provider.js", "lookup"),
        "provider-local Map shadowing must block JS imported map provenance"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
