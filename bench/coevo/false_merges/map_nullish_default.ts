// FIXED (coevo series 10, §CT, #410) — kept as a split-asserting reproducer. nose USED TO give
// `coalesce`, `presence`, and `undef_check` ONE exact-value-graph fingerprint, but they are
// behaviorally distinct on a present key whose stored value is null. After the fix the
// null-guarded coalesce folds to the faithful `ValueOrDefault` (split from the membership-guarded
// `GetOrDefault`) and `=== undefined` stays a distinct opaque, so all three get distinct
// fingerprints — `nose query <this file> top=0` now reports NO multi-member family.
//
//   const m = new Map<string, number | null>([["x", null]]);
//   coalesce(m, "x")    // 0     —  null ?? 0  →  0   (?? replaces present-null)
//   presence(m, "x")    // null  —  m.has("x") is true → returns m.get("x") = null
//   undef_check(m, "x") // null  —  null === undefined is false → returns null
//
// Root cause: `m.get(k) ?? d` (nullish-coalesce: d on absent OR present-null) is
// folded to the same node as the absence-only `GetOrDefault` (d on absent only).
// The fold is unsound whenever the map's value type admits null/undefined, and the
// value model erases that type, so it cannot be gated on nullability. The `=== undefined`
// form is additionally indistinguishable from `?? `/`== null` because the value model
// conflates null and undefined into one constant (value_graph/eval.rs §7 comment).
//
// Oracle-blind: the interpreter shares the null/undefined conflation, so
// `nose verify --max-violations 0` stays green (its calibration line even prints
// `vj[1.0]: 0/1 = 0% behavior-equal`, but the gate does not trip). Same class as
// float_assoc.py and array_element_mutation.py.
//
// Sound fix (deferred, see issue): fold `?? `/`== null`-guarded map defaults to the
// faithful coalesce (ValueOrDefault), distinct from the membership-guarded GetOrDefault,
// AND only re-converge them where the map's values are provably non-null (literal maps
// with non-null literal values — already a separate fingerprint class, see
// equivalence.rs::literal_map_default_lookup_converges_with_js_map_construction_boundaries).
// This needs a "map values provably non-null" signal and de-conflation of null/undefined.

function coalesce(lookup: Map<string, number | null>, key: string): number | null {
    return lookup.get(key) ?? 0;
}

function presence(lookup: Map<string, number | null>, key: string): number | null {
    if (lookup.has(key)) {
        return lookup.get(key);
    }
    return 0;
}

function undef_check(lookup: Map<string, number | null>, key: string): number | null {
    const g = lookup.get(key);
    return g === undefined ? 0 : g;
}
