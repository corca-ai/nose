# Coverage leads — gaps surfaced by the adversarial probe battery

Found while filling the coverage matrix with `coverage_probe.py`. Each is a real, reproduced
finding, not a fixture bug (root cause verified via `nose features … exact_safe`). They follow
the frontier discipline: a documented lead with a reproducer, to be promoted to a target
packet + sound implementation (with adjacent hard negatives + oracle gate) — not patched
blind.

## L1 — `exact_safe` language asymmetry: recursive functions (rust, java)

A numeric structural recursion (`fac(n) = n*fac(n-1)`, base 1) converges with its accumulator
loop in **python and javascript** (both sides `exact_safe=True`), but NOT in **rust or java**:
the recursive function is `exact_safe=False` there, so it never enters the exact channel.

```
recursion_tail_numeric/{rust,java}/pos   # pos 0/1 — recursive side exact_safe=False
recursion_tail_numeric/{python,javascript}/pos  # covered 1/1
```
Root cause (narrowed): NOT the self-call IL (identical `(call (var "fac") …)` in python and
rust) and NOT the return style (explicit `return n*fac(n-1);` is still `exact_safe=False` in
rust). The whole recursive *function* is `exact_safe=False` in rust/java but `True` in
python/js — the divergence lives in `strict_exact_safe_var` / `StrictFacts` handling of the
self-reference per language, in `crates/nose-detect/src/units.rs`. This is a static-gate
change (a soundness boundary), so it must be done with the real-corpus `nose verify` gate, not
loosened blind. The recursion→iteration canon (recursion.rs) is language-general, so once the
recursive fn is admitted to the exact channel the convergence should follow. Hard negatives
(sum vs product monoid) already stay un-merged — the guard is in place.

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

## L4 — recall extension: `.flatMap(x => x)` (identity) ≡ flatten

`xss.flatMap(xs => xs)` is behaviorally `flatten`, equal to the nested builder loop, but is
not the modeled `FlatMap[A, λa. Map[B, λb. e]]` shape (no inner map), so it does not converge.
A sound recall extension would model identity flat-map as flatten. (Not a bug; the modeled
inner-map shape converges, incl. cross-language JS↔Python.)

---

These four are the next *real* implement targets for even cross-language coverage. L1–L3 are
one theme: the `exact_safe` static gate admits constructs unevenly across languages, so the
same modeled equivalence surfaces in some languages and not others. Resolving them widens the
matrix without lowering thresholds — and each must ship with its hard negatives (already
authored) and a clean `nose verify`.
