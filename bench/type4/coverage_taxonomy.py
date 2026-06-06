#!/usr/bin/env python3
"""Type-4 coverage taxonomy — the axis × language × arm control surface.

This is the *intended* coverage map the adversarial co-evolution loop expands against.
It deliberately fixes the bias the old `type4-next` dispenser produced: that dispenser's
work atom was an *axis* (closed after one language), with a static prevalence score and no
per-(axis, language) coverage state — so the evidence matrix went diagonal and go/js/rust
got zero frontier work. Here the atom is a CELL `(axis, language)`, and the dispenser
(`coverage_matrix.py next`) scores cells with a coverage-gap term + a fairness floor so the
loop expands EVENLY across languages and axes instead of greedily by prevalence.

Discipline carried over from the old pipeline (issue #50): an axis is a genuine SEMANTIC
INVARIANT, never a language-specific API spelling; controlled vocabulary only; a cell reaches
"covered" only with a detector run + dev/held-out evidence; every positive needs adjacent
hard negatives.

What is NEW here vs the old 9-candidate list: the high-value STRUCTURAL axes the old
taxonomy never had a row for (extract-method/interprocedural inlining, partial sub-DAG
overlap, extended recursion↔iteration, larger statement windows, async↔sync twins), plus
explicit OUT-OF-SCOPE rows so "we deliberately don't do this" is visible rather than an
invisible gap.

Fields per axis:
  axis_id      canonical id (matches real_frontier candidate_axis leading token)
  title        human description
  family       idiom | structural | algebraic | soundness
  arm          recall | soundness | both   (which detector property the cell exercises)
  direction    within | cross | both       (same-language and/or cross-language pairs)
  feasibility  landed | fixable | partial | research | out-of-scope
  languages    applicable language ids (subset of LANGS), or "all"
  aliases      real_frontier candidate_axis strings that count as evidence for this axis
  note         the equivalence + its soundness boundary / why-not
"""

from __future__ import annotations

# The 8 source languages the corpus patterns target (js and ts tracked separately).
LANGS = ["c", "go", "java", "javascript", "typescript", "python", "ruby", "rust"]

FEASIBILITY_ORDER = ["fixable", "partial", "landed", "research", "out-of-scope"]


def _axis(axis_id, title, family, arm, direction, feasibility, languages, note, aliases=()):
    return {
        "axis_id": axis_id,
        "title": title,
        "family": family,
        "arm": arm,
        "direction": direction,
        "feasibility": feasibility,
        "languages": list(LANGS) if languages == "all" else list(languages),
        "aliases": list(aliases),
        "note": note,
    }


# --- Landed / in-progress idiom + algebraic axes (the old taxonomy, made cell-aware) ------
_IDIOM = [
    _axis("collection_empty_check", "Collection emptiness / non-emptiness", "idiom", "both",
          "both", "landed", "all",
          "len(x)==0 / x.isEmpty() / !x — typed receiver domain kept distinct (array vs "
          "collection vs string).",
          aliases=["collection_empty_check / typed_empty_domain_soundness"]),
    _axis("string_prefix_suffix", "String prefix / suffix test", "idiom", "recall",
          "both", "landed", [l for l in LANGS if l != "c"],
          "startswith/endswith ↔ slice/compare forms."),
    _axis("membership_contains", "Membership / contains", "idiom", "both", "both", "partial",
          [l for l in LANGS if l != "c"],
          "x in xs ↔ xs.contains(x); array vs singleton-list domain kept distinct. java "
          "currently unsupported (literal_collection_membership).",
          aliases=["membership_contains / literal_collection_membership"]),
    _axis("null_option_presence", "Null / option presence", "idiom", "recall", "both",
          "landed", "all", "x is None / x==null / x.is_none() ↔ option checks."),
    _axis("map_default_lookup", "Map default lookup", "idiom", "recall", "both", "partial",
          [l for l in LANGS if l != "c"],
          "dict.get(k, d) / Hash#fetch(k, d) ↔ guarded lookup. ruby currently unsupported.",
          aliases=["map_default_lookup / literal_map_default_lookup"]),
    _axis("numeric_minmax_abs", "min/max/abs idioms", "algebraic", "recall", "both",
          "landed", "all", "ternary min/max, abs ↔ builtins; reduce-lambda min/max."),
    _axis("numeric_clamp", "Scalar clamp (bounded min/max composition)", "algebraic",
          "recall", "both", "fixable", "all",
          "min(max(x,lo),hi) ↔ clamp under lo<=hi; proof in formal/.../clamp. python is a "
          "real-miss; surface bridges + per-corpus bound proof remain.",
          aliases=["numeric_clamp / clamp_surface_bridge"]),
    _axis("total_order_compare", "Total-order comparison absorption", "algebraic", "both",
          "both", "landed", "all", "x<y ∧ x<=y → x<y for non-overloadable orders."),
    _axis("property_type_guard", "Property / type guard", "idiom", "recall", "within",
          "landed", ["javascript", "typescript"], "typeof / property guards."),
    _axis("own_property_guard", "Own-property guard", "idiom", "recall", "within", "landed",
          ["javascript", "typescript"], "hasOwnProperty guards."),
    _axis("import_identity", "Module import binding identity", "idiom", "recall", "both",
          "landed", "all", "sibling-module LOOKUP import literal binding identity."),
    _axis("filter_fusion", "Filter fusion", "algebraic", "recall", "both", "landed", "all",
          "filter q (filter p) ↔ filter (p∧q); filtered comprehension / .filter().filter()."),
    _axis("map_fusion", "Map fusion", "algebraic", "recall", "both", "landed", "all",
          "map g (map f) ↔ map (g∘f)."),
    _axis("flat_map", "Flat-map / nested comprehension", "algebraic", "recall", "both",
          "landed", "all", "one-level FlatMap[A, λa. Map[B, λb. e]] across surfaces."),
    _axis("reduce_minmax_anyall", "Reduce / any / all", "algebraic", "recall", "both",
          "landed", "all", "loop ↔ reduce/sum/any/all/count-of-filter."),
    _axis("recursion_tail_numeric", "Recursion→iteration (tail + numeric monoid)",
          "structural", "recall", "both", "landed", "all",
          "tail recursion → while; numeric structural recursion → accumulator fold."),
]

# --- NEW high-value STRUCTURAL axes the old taxonomy never tracked --------------------------
_STRUCTURAL = [
    _axis("extract_method_inline", "Extract-method / interprocedural pure inline",
          "structural", "recall", "both", "fixable", "all",
          "f with inline body ≡ f' that calls an extracted PURE helper. Calls are opaque "
          "today (ValOp::Call(0)); bounded depth/size-limited inlining of proven-pure "
          "callees before build_unit recovers it. Hard negative: impure/effectful callee."),
    _axis("partial_subdag_overlap", "Partial shared sub-computation (anchored sub-DAG)",
          "structural", "recall", "both", "partial", "all",
          "two larger, differently-shaped functions sharing a semantic core. Whole-unit "
          "multiset equality misses it; anchor on rare canonical sub-DAGs, restrict to a "
          "pure return/cond sub-DAG with identical live-outs and no effect-ordinal ancestor "
          "(effects break naive matching)."),
    _axis("recursion_iteration_extended", "Recursion↔iteration (beyond tail/numeric)",
          "structural", "recall", "both", "partial", "all",
          "tree & mutual recursion, list-tail catamorphisms over opaque slices, "
          "countdown-loop ↔ range-loop. Sound but value graph does not yet converge."),
    _axis("statement_window_fragment", "Larger exact statement-window fragments",
          "structural", "recall", "within", "partial", "all",
          "exact fragments beyond the current bounded grammar — needs free-variable / "
          "live-out / receiver-overload / effect-ordering boundary modeling."),
    _axis("async_sync_twin", "async ↔ sync twins", "structural", "recall", "within",
          "research", ["javascript", "typescript", "python", "rust"],
          "async/await version vs sync version of the same logic — the #1 real-world Type-4 "
          "gap (experiments §K). Needs an async-desugaring model; soundness boundary is "
          "scheduling/await-point observability."),
    _axis("representation_choice", "Equivalent data-structure choice", "structural",
          "recall", "both", "research", "all",
          "same logic via dict vs list-of-pairs, array vs map, etc. where behavior matches."),
]

# --- Soundness arm: the false-merge families to keep closed (cells = adjacent hard negs) ---
_SOUNDNESS = [
    _axis("loop_extent_soundness", "Loop iteration-extent must stay distinct", "soundness",
          "soundness", "both", "landed", "all",
          "§AS Family A: range-start, while-stride, early-break must NOT collapse to a full "
          "Elem(C). Reproducers in tests/equivalence.rs; per-language cells still to map.",
          aliases=["collection_size_bound / verifier-lead-rejection"]),
    _axis("identity_value_soundness", "Identities/values must stay distinct", "soundness",
          "soundness", "both", "landed", "all",
          "§AS Family B: slice bounds, free-variable identity, boolean literal values, "
          "membership non-commutativity (§AT) must NOT collapse."),
    _axis("typed_concat_soundness", "Ordered concatenation must not commute", "soundness",
          "soundness", "both", "partial", "all",
          "string/list + is ordered; commutes only when an operand is proven non-string/"
          "non-list (types.rs). Unknown-typed concat is the boundary."),
]

# --- Explicit OUT-OF-SCOPE rows (visible non-goals, not invisible gaps) --------------------
_OUT_OF_SCOPE = [
    _axis("different_algorithm_same_result", "Different algorithm, same result",
          "structural", "recall", "both", "out-of-scope", "all",
          "bubble vs quick sort; iterative sum vs n*(n+1)/2. Arbitrary I/O equivalence is "
          "undecidable and explicitly NOT a goal."),
    _axis("value_domain_algebra", "Value/domain-dependent algebra", "algebraic", "recall",
          "both", "out-of-scope", "all",
          "x*2≡x+x, s[-1]≡s[len(s)-1]. Rejected as cross-language-unsound without domain "
          "proofs a type-free tool lacks (§BA)."),
]

# --- Language-specific landed axes (a genuine invariant that only one surface expresses) ---
_LANG_SPECIFIC = [
    _axis("java_integer_low_bit_toggle", "Java int low-bit toggle (x^1)", "algebraic",
          "recall", "within", "landed", ["java"],
          "x%2==0 ? x+1 : x-1 ≡ x^1 on Java primitive int (overflow-safe at extremes)."),
    _axis("java_statically_false_loop", "Java statically-false loop entry guard", "structural",
          "recall", "within", "landed", ["java"],
          "proven-true local makes `while(!local && rhs)` body unreachable."),
    _axis("class_literal_equality", "Class-literal equality", "idiom", "recall", "within",
          "research", ["java"],
          "Class<?> token equality over reflection chains (getClass()/getType()). Semantic "
          "mode CORRECTLY ABSTAINS today (opaque library receiver chain); the real duplicate "
          "is a Type-1/2 clone syntax/near already reports. Needs shared proof facts for Java "
          "reflection/JUnit APIs before it is a sound semantic target — not a detector bug."),
    _axis("c_u16_be_byte_pack", "C u16 big-endian byte pack", "idiom", "recall", "within",
          "landed", ["c"], "(a[0]<<8)+a[1] ≡ (a[0]<<8)|a[1] under a byte-array proof."),
    _axis("c_u32_be_byte_pack", "C u32 big-endian byte pack", "idiom", "recall", "within",
          "landed", ["c"], "u32 lane pack; requires an explicit unsigned 32-bit cast proof."),
    _axis("python_docstring_noop", "Python docstring is a no-op", "idiom", "recall", "within",
          "landed", ["python"], "leading static docstring is metadata, not behavior."),
    _axis("immutable_binding", "Immutable binding identity", "idiom", "recall", "both",
          "landed", "all", "single immutable binding folds to its value."),
    _axis("proven_callee_identity", "Proven callee identity", "idiom", "recall", "both",
          "landed", "all", "a unique/immutable/unambiguous callee binding resolves to its "
          "definition (module-binding folding)."),
]

AXES = _IDIOM + _LANG_SPECIFIC + _STRUCTURAL + _SOUNDNESS + _OUT_OF_SCOPE


def axis_index():
    return {a["axis_id"]: a for a in AXES}


if __name__ == "__main__":
    import json
    print(json.dumps({"languages": LANGS, "axis_count": len(AXES), "axes": AXES}, indent=2))
