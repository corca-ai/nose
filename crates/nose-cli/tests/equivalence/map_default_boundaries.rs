use super::*;

#[test]
fn map_default_lookup_converges_cross_language() {
    let i = Interner::new();
    let go = "package p\n\nfunc F(lookup map[string]int, otherLookup map[string]int, key string, otherKey string, fallback int, otherDefault int) int { value, ok := lookup[key]; if !ok { value = fallback }; return value }\n";
    let java_explicit = "import java.util.Map;\n\nclass C { static int f(Map<String, Integer> lookup, Map<String, Integer> other_lookup, String key, String other_key, int fallback, int other_default) { return lookup.containsKey(key) ? lookup.get(key) : fallback; } }\n";
    let java_builtin = "import java.util.Map;\n\nclass C { static int f(Map<String, Integer> lookup, Map<String, Integer> other_lookup, String key, String other_key, int fallback, int other_default) { return lookup.getOrDefault(key, fallback); } }\n";
    let java_guard_return = "import java.util.Map;\n\nclass C { static int f(Map<String, Integer> lookup, Map<String, Integer> other_lookup, String key, String other_key, int fallback, int other_default) { if (lookup.containsKey(key)) { return lookup.get(key); } return fallback; } }\n";
    let rust_explicit = "use std::collections::HashMap;\n\npub fn f(lookup: &HashMap<&str, i32>, other_lookup: &HashMap<&str, i32>, key: &str, other_key: &str, fallback: i32, other_default: i32) -> i32 { if lookup.contains_key(key) { lookup[key] } else { fallback } }\n";
    let rust_unwrap = "use std::collections::HashMap;\n\npub fn f(lookup: &HashMap<&str, i32>, other_lookup: &HashMap<&str, i32>, key: &str, other_key: &str, fallback: i32, other_default: i32) -> i32 { *lookup.get(key).unwrap_or(&fallback) }\n";
    let ts_nullish = "function f(lookup: Map<string, number>, other_lookup: Map<string, number>, key: string, other_key: string, fallback: number, other_default: number): number { return lookup.get(key) ?? fallback; }\n";
    let ts_has_get = "function f(lookup: Map<string, number>, other_lookup: Map<string, number>, key: string, other_key: string, fallback: number, other_default: number): number { return lookup.has(key) ? lookup.get(key) : fallback; }\n";
    let ts_temp_guard = "function f(lookup: Map<string, number>, other_lookup: Map<string, number>, key: string, other_key: string, fallback: number, other_default: number): number { const selected = lookup.get(key); return selected === undefined ? fallback : selected; }\n";
    let ts_guard_return = "function f(lookup: Map<string, number>, other_lookup: Map<string, number>, key: string, other_key: string, fallback: number, other_default: number): number { if (lookup.has(key)) { return lookup.get(key)!; } return fallback; }\n";
    let py_dict = "def f(lookup: dict[str, int], other_lookup: dict[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return lookup.get(key, fallback)\n";
    let py_guard_return = "def f(lookup: dict[str, int], other_lookup: dict[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    if key in lookup:\n        return lookup[key]\n    return fallback\n";
    let py_mapping = "from collections.abc import Mapping\n\ndef f(lookup: Mapping[str, int], other_lookup: Mapping[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return lookup.get(key, fallback)\n";
    let py_mutable_mapping = "from collections.abc import MutableMapping\n\ndef f(lookup: MutableMapping[str, int], other_lookup: MutableMapping[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return lookup.get(key, fallback)\n";
    let py_alias_mapping = "from collections.abc import Mapping as MapLike\n\ndef f(lookup: MapLike[str, int], other_lookup: MapLike[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return lookup.get(key, fallback)\n";
    let py_alias_mutable_mapping = "from collections.abc import MutableMapping as MapLike\n\ndef f(lookup: MapLike[str, int], other_lookup: MapLike[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return lookup.get(key, fallback)\n";
    let py_alias_dict = "from typing import Dict as MapLike\n\ndef f(lookup: MapLike[str, int], other_lookup: MapLike[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return lookup.get(key, fallback)\n";

    let fp = value_fp(&i, go, Lang::Go);
    assert_eq!(fp, value_fp(&i, java_explicit, Lang::Java));
    assert_eq!(fp, value_fp(&i, java_builtin, Lang::Java));
    assert_eq!(fp, value_fp(&i, java_guard_return, Lang::Java));
    assert_eq!(fp, value_fp(&i, rust_explicit, Lang::Rust));
    assert_eq!(fp, value_fp(&i, rust_unwrap, Lang::Rust));
    assert_eq!(fp, value_fp(&i, ts_has_get, Lang::TypeScript));
    assert_eq!(fp, value_fp(&i, ts_guard_return, Lang::TypeScript));
    // `lookup.get(key) ?? fallback` is nullish COALESCE; the strict `selected === undefined ? …`
    // guard is conflated with `== null` by the null/undefined value model. Neither merges with the
    // absence-default family — they diverge on a present null-valued key (#410, experiments §CT).
    assert_ne!(fp, value_fp(&i, ts_nullish, Lang::TypeScript));
    assert_ne!(fp, value_fp(&i, ts_temp_guard, Lang::TypeScript));
    assert_eq!(fp, value_fp(&i, py_dict, Lang::Python));
    assert_eq!(fp, value_fp(&i, py_guard_return, Lang::Python));
    assert_eq!(fp, value_fp(&i, py_mapping, Lang::Python));
    assert_eq!(fp, value_fp(&i, py_mutable_mapping, Lang::Python));
    assert_eq!(fp, value_fp(&i, py_alias_mapping, Lang::Python));
    assert_eq!(fp, value_fp(&i, py_alias_mutable_mapping, Lang::Python));
    assert_eq!(fp, value_fp(&i, py_alias_dict, Lang::Python));
}

#[test]
fn nullish_coalesce_map_default_is_distinct_from_absence_default() {
    // #410 / experiments §CT: `m.get(k) ?? d` (nullish coalesce — default on absent OR present-null)
    // must NOT share a fingerprint with the absence-only default `m.has(k) ? m.get(k) : d` /
    // `dict.get(k, d)`. They diverge on a present key whose value is null:
    //   const m = new Map<string, number | null>([["x", null]]);
    //   m.get("x") ?? 0             // 0     (?? replaces present-null)
    //   m.has("x") ? m.get("x") : 0 // null  (presence keeps the stored null)
    // The value model erases the map's value-type nullability, so the merge can never be proven
    // sound; it was the LATENT false merge recorded in bench/coevo/false_merges/map_nullish_default.ts.
    let i = Interner::new();
    let coalesce = "function f(m: Map<string, number | null>, k: string): number | null { return m.get(k) ?? 0; }\n";
    let coalesce_eqnull = "function f(m: Map<string, number | null>, k: string): number | null { const g = m.get(k); return g == null ? 0 : g; }\n";
    let presence_has = "function f(m: Map<string, number | null>, k: string): number | null { if (m.has(k)) { return m.get(k); } return 0; }\n";
    let py_get_default = "def f(m, k):\n    return m.get(k, 0)\n";

    let coalesce_fp = value_fp(&i, coalesce, Lang::TypeScript);
    // the two nullish-coalesce spellings still converge as their own class
    assert_eq!(coalesce_fp, value_fp(&i, coalesce_eqnull, Lang::TypeScript));
    // …but coalesce is DISTINCT from the membership-guarded absence default (the fixed false merge)
    assert_ne!(coalesce_fp, value_fp(&i, presence_has, Lang::TypeScript));
    // …and distinct cross-language from Python's absence-only `dict.get(k, default)`
    assert_ne!(coalesce_fp, value_fp(&i, py_get_default, Lang::Python));
}

#[test]
fn map_default_lookup_keeps_wrong_coordinate_boundaries() {
    let i = Interner::new();
    let go = "package p\n\nfunc F(lookup map[string]int, otherLookup map[string]int, key string, otherKey string, fallback int, otherDefault int) int { value, ok := lookup[key]; if !ok { value = fallback }; return value }\n";
    let wrong_key = "import java.util.Map;\n\nclass C { static int f(Map<String, Integer> lookup, Map<String, Integer> other_lookup, String key, String other_key, int fallback, int other_default) { return lookup.getOrDefault(other_key, fallback); } }\n";
    let wrong_default = "use std::collections::HashMap;\n\npub fn f(lookup: &HashMap<&str, i32>, other_lookup: &HashMap<&str, i32>, key: &str, other_key: &str, fallback: i32, other_default: i32) -> i32 { *lookup.get(key).unwrap_or(&other_default) }\n";
    let wrong_map = "package p\n\nfunc F(lookup map[string]int, otherLookup map[string]int, key string, otherKey string, fallback int, otherDefault int) int { value, ok := otherLookup[key]; if !ok { value = fallback }; return value }\n";
    let ts_wrong_key = "function f(lookup: Map<string, number>, other_lookup: Map<string, number>, key: string, other_key: string, fallback: number, other_default: number): number { return lookup.get(other_key) ?? fallback; }\n";
    let ts_wrong_default = "function f(lookup: Map<string, number>, other_lookup: Map<string, number>, key: string, other_key: string, fallback: number, other_default: number): number { return lookup.get(key) ?? other_default; }\n";
    let ts_wrong_map = "function f(lookup: Map<string, number>, other_lookup: Map<string, number>, key: string, other_key: string, fallback: number, other_default: number): number { return other_lookup.get(key) ?? fallback; }\n";
    let ts_untyped = "function f(lookup, other_lookup, key, other_key, fallback, other_default) { return lookup.get(key) ?? fallback; }\n";
    let ts_temp_shadowed_undefined = "function f(lookup: Map<string, number>, other_lookup: Map<string, number>, key: string, other_key: string, fallback: number, other_default: number, undefined: number): number { const selected = lookup.get(key); return selected === undefined ? fallback : selected; }\n";
    let py_wrong_key = "def f(lookup: dict[str, int], other_lookup: dict[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return lookup.get(other_key, fallback)\n";
    let py_wrong_default = "def f(lookup: dict[str, int], other_lookup: dict[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return lookup.get(key, other_default)\n";
    let py_wrong_map = "def f(lookup: dict[str, int], other_lookup: dict[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return other_lookup.get(key, fallback)\n";
    let py_untyped = "def f(lookup, other_lookup, key, other_key, fallback, other_default):\n    return lookup.get(key, fallback)\n";

    let fp = value_fp(&i, go, Lang::Go);
    assert_ne!(fp, value_fp(&i, wrong_key, Lang::Java));
    assert_ne!(fp, value_fp(&i, wrong_default, Lang::Rust));
    assert_ne!(fp, value_fp(&i, wrong_map, Lang::Go));
    assert_ne!(fp, value_fp(&i, ts_wrong_key, Lang::TypeScript));
    assert_ne!(fp, value_fp(&i, ts_wrong_default, Lang::TypeScript));
    assert_ne!(fp, value_fp(&i, ts_wrong_map, Lang::TypeScript));
    assert_ne!(fp, value_fp(&i, ts_untyped, Lang::TypeScript));
    assert_ne!(
        fp,
        value_fp(&i, ts_temp_shadowed_undefined, Lang::TypeScript)
    );
    assert_ne!(fp, value_fp(&i, py_wrong_key, Lang::Python));
    assert_ne!(fp, value_fp(&i, py_wrong_default, Lang::Python));
    assert_ne!(fp, value_fp(&i, py_wrong_map, Lang::Python));
    assert_ne!(fp, value_fp(&i, py_untyped, Lang::Python));
}

#[test]
fn map_default_lookup_keeps_alias_and_guard_boundaries() {
    let i = Interner::new();
    let go = "package p\n\nfunc F(lookup map[string]int, otherLookup map[string]int, key string, otherKey string, fallback int, otherDefault int) int { value, ok := lookup[key]; if !ok { value = fallback }; return value }\n";
    let py_alias_wrong_key = "from collections.abc import Mapping as MapLike\n\ndef f(lookup: MapLike[str, int], other_lookup: MapLike[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return lookup.get(other_key, fallback)\n";
    let py_alias_wrong_default = "from collections.abc import Mapping as MapLike\n\ndef f(lookup: MapLike[str, int], other_lookup: MapLike[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return lookup.get(key, other_default)\n";
    let py_alias_wrong_map = "from collections.abc import Mapping as MapLike\n\ndef f(lookup: MapLike[str, int], other_lookup: MapLike[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return other_lookup.get(key, fallback)\n";
    let py_alias_unresolved = "def f(lookup: MapLike[str, int], other_lookup: MapLike[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return lookup.get(key, fallback)\n";
    let py_alias_shadowed = "from collections.abc import Mapping as MapLike\nMapLike = list\n\ndef f(lookup: MapLike[str, int], other_lookup: MapLike[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return lookup.get(key, fallback)\n";
    let guard_wrong_key = "function f(lookup: Map<string, number>, other_lookup: Map<string, number>, key: string, other_key: string, fallback: number, other_default: number): number { if (lookup.has(other_key)) { return lookup.get(other_key)!; } return fallback; }\n";
    let guard_wrong_default = "import java.util.Map;\n\nclass C { static int f(Map<String, Integer> lookup, Map<String, Integer> other_lookup, String key, String other_key, int fallback, int other_default) { if (lookup.containsKey(key)) { return lookup.get(key); } return other_default; } }\n";
    let guard_wrong_map = "def f(lookup: dict[str, int], other_lookup: dict[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    if key in other_lookup:\n        return other_lookup[key]\n    return fallback\n";

    let fp = value_fp(&i, go, Lang::Go);
    assert_ne!(fp, value_fp(&i, py_alias_wrong_key, Lang::Python));
    assert_ne!(fp, value_fp(&i, py_alias_wrong_default, Lang::Python));
    assert_ne!(fp, value_fp(&i, py_alias_wrong_map, Lang::Python));
    assert_ne!(fp, value_fp(&i, py_alias_unresolved, Lang::Python));
    assert_ne!(fp, value_fp(&i, py_alias_shadowed, Lang::Python));
    assert_ne!(fp, value_fp(&i, guard_wrong_key, Lang::TypeScript));
    assert_ne!(fp, value_fp(&i, guard_wrong_default, Lang::Java));
    assert_ne!(fp, value_fp(&i, guard_wrong_map, Lang::Python));
}

#[test]
fn swift_dictionary_default_subscript_requires_map_receiver_coordinates() {
    let i = Interner::new();
    let python = "def f(lookup: dict[str, int], key: str, fallback: int, other: str, other_default: int) -> int:\n    return lookup.get(key, fallback)\n";
    let fp = value_fp(&i, python, Lang::Python);

    assert_swift_dictionary_default_positive_cases(&i, &fp);
    assert_swift_dictionary_default_boundary_cases(&i, &fp);
    assert_swift_dictionary_default_dispatch_boundaries(&i, &fp);
    assert_swift_dictionary_default_source_syntax_boundaries(&i, &fp);
    assert_swift_dictionary_default_effect_boundary(&i);
}

fn assert_swift_dictionary_default_positive_cases(i: &Interner, fp: &[u64]) {
    let default_subscript = r#"
func f(_ dict: Dictionary<String, Int>, _ key: String, _ fallback: Int, _ other: String, _ otherDefault: Int) -> Int {
    return dict[key, default: fallback]
}
"#;
    let renamed = r#"
func g(_ lookup: Dictionary<String, Int>, _ name: String, _ missing: Int, _ otherName: String, _ otherMissing: Int) -> Int {
    return lookup[name, default: missing]
}
"#;
    let bracket_dictionary = r#"
func f(_ dict: [String: Int], _ key: String, _ fallback: Int, _ other: String, _ otherDefault: Int) -> Int {
    return dict[key, default: fallback]
}
"#;
    let qualified_dictionary = r#"
func f(_ dict: Swift.Dictionary<String, Int>, _ key: String, _ fallback: Int, _ other: String, _ otherDefault: Int) -> Int {
    return dict[key, default: fallback]
}
"#;

    assert_eq!(
        fp,
        value_fp(i, default_subscript, Lang::Swift),
        "Swift Dictionary default subscript should join the absence-default map lookup family"
    );
    assert_eq!(
        fp,
        value_fp(i, renamed, Lang::Swift),
        "Swift Dictionary default subscripts should alpha-converge through map/key/default coordinates"
    );
    assert_eq!(
        fp,
        value_fp(i, bracket_dictionary, Lang::Swift),
        "bracket Dictionary syntax should carry the same language-core receiver proof"
    );
    assert_eq!(
        fp,
        value_fp(i, qualified_dictionary, Lang::Swift),
        "Swift.Dictionary should carry the same language-core receiver proof"
    );
}

fn assert_swift_dictionary_default_boundary_cases(i: &Interner, fp: &[u64]) {
    let wrong_key = r#"
func f(_ dict: Dictionary<String, Int>, _ key: String, _ fallback: Int, _ other: String, _ otherDefault: Int) -> Int {
    return dict[other, default: fallback]
}
"#;
    let wrong_default = r#"
func f(_ dict: Dictionary<String, Int>, _ key: String, _ fallback: Int, _ other: String, _ otherDefault: Int) -> Int {
    return dict[key, default: otherDefault]
}
"#;
    let untyped_receiver = r#"
func f(_ dict: Any, _ key: String, _ fallback: Int, _ other: String, _ otherDefault: Int) -> Int {
    return dict[key, default: fallback]
}
"#;
    let nullish_default = r#"
func f(_ dict: Dictionary<String, Int>, _ key: String, _ fallback: Int, _ other: String, _ otherDefault: Int) -> Int {
    return dict[key] ?? fallback
}
"#;
    let mutated_inout_receiver = r#"
func f(_ dict: inout Dictionary<String, Int>, _ key: String, _ fallback: Int, _ other: String, _ otherDefault: Int) -> Int {
    dict[key] = otherDefault
    return dict[key, default: fallback]
}
"#;
    assert_ne!(
        fp,
        value_fp(i, wrong_key, Lang::Swift),
        "a different key coordinate changes the lookup"
    );
    assert_ne!(
        fp,
        value_fp(i, wrong_default, Lang::Swift),
        "a different fallback coordinate changes the lookup"
    );
    assert_ne!(
        fp,
        value_fp(i, untyped_receiver, Lang::Swift),
        "subscript syntax alone must not prove a map receiver"
    );
    assert_ne!(
        fp,
        value_fp(i, nullish_default, Lang::Swift),
        "Swift optional defaulting is not an absence-only Dictionary default subscript"
    );
    assert_ne!(
        fp,
        value_fp(i, mutated_inout_receiver, Lang::Swift),
        "an inout receiver with an intervening write must stay outside the stable-map perimeter"
    );
}

fn assert_swift_dictionary_default_dispatch_boundaries(i: &Interner, fp: &[u64]) {
    let custom_dictionary = r#"
struct Dictionary<Key: Hashable, Value> {
    subscript(key: Key, default fallback: Value) -> Value { fallback }
}
func f(_ dict: Dictionary<String, Int>, _ key: String, _ fallback: Int, _ other: String, _ otherDefault: Int) -> Int {
    return dict[key, default: fallback]
}
"#;
    let shadowed_dictionary_alias = r#"
struct Lookup<Key: Hashable, Value> {
    subscript(key: Key, default fallback: Value) -> Value { fallback }
}
typealias Dictionary = Lookup
func f(_ dict: Dictionary<String, Int>, _ key: String, _ fallback: Int, _ other: String, _ otherDefault: Int) -> Int {
    return dict[key, default: fallback]
}
"#;
    let imported_dispatch = r#"
import Foundation
func f(_ dict: Swift.Dictionary<String, Int>, _ key: String, _ fallback: Int, _ other: String, _ otherDefault: Int) -> Int {
    return dict[key, default: fallback]
}
"#;
    let custom_dictionary_extension = r#"
extension Dictionary where Key == String, Value == Int {
    subscript(key: String, default fallback: Int) -> Int { return fallback + 1 }
}
func f(_ dict: Dictionary<String, Int>, _ key: String, _ fallback: Int, _ other: String, _ otherDefault: Int) -> Int {
    return dict[key, default: fallback]
}
"#;

    assert_ne!(
        fp,
        value_fp(i, custom_dictionary, Lang::Swift),
        "a user-defined Dictionary and subscript must not borrow stdlib absence-default semantics"
    );
    assert_ne!(
        fp,
        value_fp(i, shadowed_dictionary_alias, Lang::Swift),
        "a Dictionary typealias must not borrow stdlib absence-default semantics"
    );
    assert_ne!(
        fp,
        value_fp(i, imported_dispatch, Lang::Swift),
        "imports keep Dictionary default-subscript overload resolution closed"
    );
    assert_ne!(
        fp,
        value_fp(i, custom_dictionary_extension, Lang::Swift),
        "a visible Dictionary default-subscript overload must close stdlib dispatch"
    );
}

fn assert_swift_dictionary_default_effect_boundary(i: &Interner) {
    let python_effectful_default = r#"
def observe() -> int:
    return 7

def f(lookup: dict[str, int], key: str, fallback: int, other: str, other_default: int) -> int:
    return lookup.get(key, observe())
"#;
    let swift_effectful_default = r#"
func observe() -> Int { return 7 }
func f(_ dict: Dictionary<String, Int>, _ key: String, _ fallback: Int, _ other: String, _ otherDefault: Int) -> Int {
    return dict[key, default: observe()]
}
"#;
    let swift_eager_default = r#"
func observe() -> Int { return 7 }
func f(_ dict: Dictionary<String, Int>, _ key: String, _ fallback: Int, _ other: String, _ otherDefault: Int) -> Int {
    let observed = observe()
    return dict[key, default: observed]
}
"#;

    assert_ne!(
        value_fp_named(i, python_effectful_default, Lang::Python, "f"),
        value_fp_named(i, swift_effectful_default, Lang::Swift, "f"),
        "a lazy Swift default expression must not merge with an eager Python default expression"
    );
    assert_ne!(
        value_fp_named(i, swift_effectful_default, Lang::Swift, "f"),
        value_fp_named(i, swift_eager_default, Lang::Swift, "f"),
        "a lazy Swift autoclosure default must not merge with an eagerly hoisted call"
    );
}

fn assert_swift_dictionary_default_source_syntax_boundaries(i: &Interner, fp: &[u64]) {
    let wrapped_fallback = r#"
@propertyWrapper
struct Shifted {
    private var value: Int
    var wrappedValue: Int { value + 1 }
    init(wrappedValue: Int) { self.value = wrappedValue }
}
func f(_ dict: Dictionary<String, Int>, _ key: String, @Shifted _ fallback: Int, _ other: String, _ otherDefault: Int) -> Int {
    return dict[key, default: fallback]
}
"#;
    let alias_extension = r#"
typealias StringIntDictionary = Dictionary<String, Int>
extension StringIntDictionary {
    subscript(key: String, default fallback: Int) -> Int { fallback + 1 }
}
func f(_ dict: Swift.Dictionary<String, Int>, _ key: String, _ fallback: Int, _ other: String, _ otherDefault: Int) -> Int {
    return dict[key, default: fallback]
}
"#;
    let escaped_extension = r#"
extension `Dictionary` where Key == String, Value == Int {
    subscript(key: String, default fallback: Int) -> Int { fallback + 3 }
}
func f(_ dict: Dictionary<String, Int>, _ key: String, _ fallback: Int, _ other: String, _ otherDefault: Int) -> Int {
    return dict[key, default: fallback]
}
"#;
    let shadowed_swift_namespace = r#"
enum Swift {
    struct Dictionary<Key: Hashable, Value> {
        subscript(key: Key, default fallback: Int) -> Int where Value == Int { fallback + 6 }
    }
}
func f(_ dict: Swift.Dictionary<String, Int>, _ key: String, _ fallback: Int, _ other: String, _ otherDefault: Int) -> Int {
    return dict[key, default: fallback]
}
"#;
    let comment_qualified_extension = r#"
extension Swift /* gap */ . Dictionary where Key == String, Value == Int {
    subscript(key: String, default fallback: Int) -> Int { fallback + 7 }
}
func f(_ dict: Swift.Dictionary<String, Int>, _ key: String, _ fallback: Int, _ other: String, _ otherDefault: Int) -> Int {
    return dict[key, default: fallback]
}
"#;

    for (source, reason) in [
        (
            wrapped_fallback,
            "a property-wrapped fallback must not borrow a plain coordinate proof",
        ),
        (
            alias_extension,
            "a Dictionary alias extension must close stdlib dispatch",
        ),
        (
            escaped_extension,
            "an escaped Dictionary extension must close stdlib dispatch",
        ),
        (
            shadowed_swift_namespace,
            "a local Swift namespace must not prove the stdlib module qualifier",
        ),
        (
            comment_qualified_extension,
            "a comment-qualified Dictionary extension must close stdlib dispatch",
        ),
    ] {
        assert_ne!(fp, value_fp_named(i, source, Lang::Swift, "f"), "{reason}");
    }
}
