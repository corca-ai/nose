# Coverage leads — gaps surfaced by the adversarial probe battery

Found while filling the coverage matrix with `coverage_probe.py`. Each is a real, reproduced
finding, not a fixture bug (root cause verified via `nose features … exact_safe`). They follow
the frontier discipline: a documented lead with a reproducer, to be promoted to a target
packet + sound implementation (with adjacent hard negatives + oracle gate) — not patched
blind.

## L1 — recursion→iteration not firing for return-wrapping languages — ✅ RESOLVED (rust); ruby/java methods → L1b

A numeric structural recursion (`fac(n) = n*fac(n-1)`, base 1) converged with its accumulator
loop in **python/js/go** but not **rust** (and not ruby/java).

**True root cause** (the earlier `proven_name` hypothesis was a red herring): the
recursion→iteration canon `recursion::recognize` matches a *bare* `NodeKind::Return` for the
function's last statement and guard arms. Languages whose `return`/`throw` are expressions
(Rust) lower them wrapped as `ExprStmt(Return)`, so `recognize` returned `None` and the canon
never fired — leaving the self-call opaque (rust fac value graph was vlen 15 vs python's 7).
The value graph already treats `ExprStmt(Return) ≡ Return` (a simple `return x+1` converges
rust↔python), so only the *syntactic* recognizer was affected.

**Fix (fundamental, not a workaround):** `desugar::emit_stmt` now unwraps
`ExprStmt(Return|Throw)` to the bare statement, making return/throw representation
language-uniform at the IL source for *every* syntactic pass. Validated: rust fac now
`exact_safe=True`, vlen 7; converges with the rust loop AND cross-language with the python
loop; sum-monoid hard negative stays separate; full suite + clippy green; corpus
behavior-invariance diff (only new recursion convergences, nothing else changed). Test:
`rust_recursion_converges_with_iteration_via_return_unwrap`.

### L1b — ruby / java method recursion — ✅ RESOLVED

ruby `def fac` and java methods are `UnitKind::Method`; `recursion::run` filtered to
`UnitKind::Function` only. Both languages' self-call `fac(n-1)` is in fact a *bare-name* call
(`(call (var "fac"))`), so the exclusion was too broad.

**Fix:** admit a `Method` unit to the recursion canon when `method_recursion_safe` holds — its
body has NO `Field` node (no `self.x`, no `.method()` — so no receiver/field state the fold
rewrite could drop). A `self.m()` self-call lowers through a `Field` callee and is excluded by
both the no-field gate and `as_self_call`'s bare-name test. Pure numeric recursion qualifies.
Conservative: false negatives only, never an unsound rewrite; the recursion interpreter oracle
also validates each rewrite.

Validated: `recursion_tail_numeric` now covered in ALL 7 languages (java/ruby flipped); test
`pure_method_recursion_converges_with_iteration`; full suite + clippy green; real-corpus
`nose verify` on the affected repos (netty/antlr4/commons-lang) **byte-identical to baseline**
(0 false merges, same 20 canon-changed). The full-corpus scan diff's family reshuffling is the
Lean-proven recursion canon correctly remapping coincidental matches — 0 detection genuinely
lost (members stay detected), 0 new false merges.

## L2 — `exact_safe`: rust builder loop (`for … for … push`) — ✅ RESOLVED

A nested builder loop did not converge with `.flat_map(...).collect()` in **rust** (builder
loop `exact_safe=False`) though it did in python/js. Root cause: the loop is seeded with
`out = Vec::new()` (python/js use `out = []`). `Vec::new()` is a non-builtin call the
exact-safe gate didn't recognize, even though the value graph already models it as an empty
`Seq` (identical to `[]`, via `value_graph::is_rust_vec_new_call`).

**Fix:** `strict_exact_rust_vec_new_safe` (units.rs) admits `Vec::new()` (no args) to the
exact channel — mirroring the value graph (a constant empty collection, no inputs/effects).
rust builder-loop now `exact_safe=True`; rust flat_map converges. Validated: test
`rust_vec_new_builder_loop_converges_with_flat_map`, full suite + clippy, real-corpus scan
behavior-invariance (0 detection lost; only new convergences) + `nose verify` 0-violation gate.

## L3 — java stream `.reduce(seed, lambda)` — ✅ RESOLVED (fixture, not a detector gap)

NOT a detector gap. The idiomatic `import java.util.Arrays; Arrays.stream(xs).reduce(0, (a,x)
-> a+x)` already converges with the loop (exact_safe=True, gate handles the reduce lambda). My
probe fixture used the non-idiomatic fully-qualified `java.util.Arrays.stream(...)`, whose
longer field chain (`java.util.Arrays`) the gate doesn't recognize as the stream source.
Fixed the fixture to the import form; `reduce_minmax_anyall` now covered in all applicable
languages. (A separate minor lead: recognize the fully-qualified `java.util.Arrays` receiver
chain — low value, rare in idiomatic code.)

## L4 — recall extension: `.flatMap(x => x)` (identity) ≡ flatten — ✅ RESOLVED

`xss.flatMap(xs => xs)` is behaviorally `flatten`, equal to the nested builder loop, but was
not the modeled `FlatMap[A, λa. Map[B, λb. e]]` shape (no inner map), so it did not converge.

**Implemented** (value_graph.rs `HoFKind::FlatMap` arm): when the inner lambda is identity
(`inner == outer_elem`), canonicalize it to the modeled element-stream inner `Map[Elem(x)]` —
the monad law `flatMap id = join`. Proven in
`formal/obligations/normalize/value_graph/flatmap_identity/` (`flatMap_id`, `lmap_id`,
`flatMap_inner_mapId_eq`); reproducer test `flatmap_identity_converges_with_inner_map_and_flatten_loop`
in `crates/nose-cli/tests/equivalence.rs` (positive + changed-element hard negative). Validated:
oracle SOUND on the case, broad cross=all oracle violation count unchanged (delta 0), full
test suite + clippy + Lean gate green. js/ts/python flat-map identity now converge.

---

**Status: L1, L1b, L2, L3, L4 all RESOLVED** (4 structural axes implemented + 1 fixture
artifact). The recurring theme was that the *value graph* already modeled an equivalence but a
*syntactic recognizer/gate* hadn't caught up (recursion recognizer's bare-Return match; the
exact-safe gate's `Vec::new`; the recursion canon's Function-only filter) — each fix aligned
the gate/recognizer with the established value-graph semantics, never a blind loosen, and was
validated by full suite + clippy + real-corpus scan behavior-invariance (0 detection lost) +
`nose verify` delta (0 new false merges) + Lean where a new equivalence (flatMap-identity).

Remaining structural axes are larger NEW MECHANISMS (not gate alignments), tracked in
`coverage_taxonomy.py`: **anchored sub-DAG matching** (partial overlap of larger functions) and
**extract-method / interprocedural pure inlining**. These need their own design + the same
validation discipline.

## L5 — ruby `arr << x` lowered as `Shl` (shift), not append (deferred, frontend)

ruby flat_map builder loop stays a gap: `out << y` lowers to `(binop Shl out y)` (ruby `<<` is
overloaded shift/append). The builder loop's value graph is a shift recurrence, not an append,
so it never matches the `flat_map`. Fix is in the ruby frontend (lower `<<` as append when the
receiver is array-like) and is inherently ambiguous without type/context — a separate frontend
lead, not a value-graph/gate alignment. Low-to-medium value; deferred.

## L6 — go composite-literal disambiguation — ✅ RESOLVED (slice); builder = next

Cross-language builder loops (`out := []T{}; for … out = append(out, e)` in Go; `new ArrayList`
+ `.add` in Java) do not converge with the comprehension/`.map` form. Investigation:
- The VALUE GRAPH *can* be made to converge them. Go's functional append `r = append(r, e)`
  (an `Assign` whose RHS is `Append(r, …)`) is recognizable as the same per-element `Map` build
  as the effect-form `r.append(e)`; with that recognition the Go builder's value fingerprint
  becomes byte-identical to the Python comprehension (verified end-to-end in a spike).
- But the EXACT channel is then blocked at the gate: the Go seed `[]int{}` lowers to a
  `Seq("composite_literal")`, which `strict_exact_safe_seq` rejects. Admitting it is NOT a clean
  L2-style alignment: `composite_literal` is also Go's MAP and STRUCT literal syntax, and the
  value graph already tags all three `Seq(1)` — so opening the gate would let `Point{1,2}`
  merge with `[]int{1,2}` / `[1,2]` (a struct ≡ slice false merge). Sound admission needs Go
  type/context info (slice vs map vs struct). Java `.add` (List vs Set) and Ruby `<<`
  (shift vs append, L5) have the same shape of ambiguity.
- These are therefore NOT quick gate-alignments like L1/L2; they need a typed/contextual
  collection-kind proof to stay sound. Deferred (the spike value-graph change was reverted to
  avoid shipping a fingerprint change with no sound exact-channel payoff).
