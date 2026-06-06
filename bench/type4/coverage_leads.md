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

### L1b — ruby / java method recursion (deferred)

ruby `def fac` and java methods are classified `UnitKind::Method`, and `recursion::run`
filters to `UnitKind::Function` only (methods are excluded because `self.m()` self-calls lower
through a `Field` callee, and a method may carry receiver/field effects). ruby's `fac(n-1)` is
a *bare-name* self-call (not `self.fac`), so it is a false exclusion; java's may be a `Field`
call. Proper fix: admit a method to the canon when its self-call is bare-name AND the body has
no receiver/field effects (a method-purity gate) — relying on `as_self_call`'s existing
bare-name check to keep `self.m()` out. Needs the purity gate + the real-corpus `nose verify`
0-violation gate; not rushed.

## L2 — `exact_safe`: rust builder loop (`for … for … push`)

A nested builder loop converges with `.flat_map(|xs| xs.iter().map(...)).collect()` in
**javascript** (builder loop `exact_safe=True`) but NOT in **rust** (builder loop
`exact_safe=False`).

```
flat_map/rust/pos     # pos 0/1 — rust builder-loop side exact_safe=False
flat_map/{python,javascript}/pos  # covered 1/1
```

## L3 — `exact_safe`: java stream `.reduce(seed, lambda)`

A java enhanced-for sum loop (`exact_safe=True`) does NOT converge with
`Arrays.stream(xs).reduce(0, (a,x)->a+x)` (`exact_safe=False` as written). The passing test
`java_stream_aggregates_converge_with_loops` uses a `.filter(...).reduce(...)` chain with an
`import` — narrowing why the bare fully-qualified `.reduce` is excluded is the next step.

```
reduce_minmax_anyall/java/pos   # pos 0/1 — stream side exact_safe=False
reduce_minmax_anyall/{python,javascript,rust}/pos  # covered 1/1
```

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

These four are the next *real* implement targets for even cross-language coverage. L1–L3 are
one theme: the `exact_safe` static gate admits constructs unevenly across languages, so the
same modeled equivalence surfaces in some languages and not others. Resolving them widens the
matrix without lowering thresholds — and each must ship with its hard negatives (already
authored) and a clean `nose verify`.
