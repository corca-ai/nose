# Type-4 coverage iteration log

This log records the first ten coverage-expansion iterations after the initial seed
benchmark. Each iteration adds one semantic proposal card to `proposals.v1.json`; the
generator then expands it across every supported language surface plus embedded script
surfaces.

## Baseline

Initial seed proposals:

- `sum_filter_positive`
- `count_filter_positive`
- `any_predicate_positive`

Baseline smoke:

```text
items: 99
positive recall: 38/66
hard-negative false merges: 0/33
```

## Iterations

| iteration | proposal | semantic class | hard-negative sibling |
|---|---|---|---|
| 1 | `iter01_all_nonnegative` | `all(xs, x >= 0)` universal predicate | `x >= 0 -> x > 0` |
| 2 | `iter02_product_positive` | `product(filter(xs, x > 0), init=1)` | `init 1 -> 2` |
| 3 | `iter03_sum_even` | `sum(filter(xs, x % 2 == 0))` | `even -> odd` |
| 4 | `iter04_count_negative` | `count(filter(xs, x < 0))` | `x < 0 -> x <= 0` |
| 5 | `iter05_any_zero` | `any(xs, x == 0)` | `x == 0 -> x != 0` |
| 6 | `iter06_all_nonzero` | `all(xs, x != 0)` | `x != 0 -> x > 0` |
| 7 | `iter07_product_nonzero` | `product(filter(xs, x != 0), init=1)` | `x != 0 -> x > 0` |
| 8 | `iter08_sum_small` | `sum(filter(xs, x < 3))` | `x < 3 -> x <= 3` |
| 9 | `iter09_count_small` | `count(filter(xs, x < 3))` | `x < 3 -> x <= 3` |
| 10 | `iter10_any_even` | `any(xs, x % 2 == 0)` | `even -> odd` |

## Result after iteration 10

Default ring cross-surface generation:

```text
items: 429
positive recall: 156/286
hard-negative false merges: 0/143
```

Same-surface only:

```text
items: 286
positive recall: 54/143
hard-negative false merges: 0/143
```

Lowering smoke:

```text
files: 858
Raw nodes: 0 (0.000%)
```

Per-proposal recall with the default ring cross-surface set:

```text
any_predicate_positive: positive 14/22, false merges 0/11
count_filter_positive: positive 11/22, false merges 0/11
iter01_all_nonnegative: positive 14/22, false merges 0/11
iter02_product_positive: positive 10/22, false merges 0/11
iter03_sum_even: positive 11/22, false merges 0/11
iter04_count_negative: positive 11/22, false merges 0/11
iter05_any_zero: positive 14/22, false merges 0/11
iter06_all_nonzero: positive 14/22, false merges 0/11
iter07_product_nonzero: positive 10/22, false merges 0/11
iter08_sum_small: positive 11/22, false merges 0/11
iter09_count_small: positive 11/22, false merges 0/11
iter10_any_even: positive 14/22, false merges 0/11
sum_filter_positive: positive 11/22, false merges 0/11
```

The hard-negative result is the important safety signal: the new coverage increased the
frontier without introducing exact semantic false merges under the current evaluator. The
missed positives are under-merge work items for future normalization/value-graph iterations.

## Detector co-evolution loops

After the coverage-only expansion above, the benchmark was used as a detector-improvement
frontier. The starting point for the default ring cross-surface set was:

```text
items: 429
positive recall: 156/286
hard-negative false merges: 0/143
```

Ten detector/process co-evolution loops were then run. Loops 1-10 below are detector-facing
iterations against the generated semantic frontier; loops 11-17 close the original frontier
and harden the loop so later detector work cannot regress silently.

| loop | frontier target | detector change | recall after loop | false merges |
|---|---|---|---|---|
| 1 | filtered `reduce`/`fold` aggregate vs guarded loop | lower `filter(p).reduce(⊕, init)` to guarded `Reduce(⊕, init, p ? contrib : identity)` in the shared value graph | 186/286 | 0/143 |
| 2 | count-filter aggregate vs guarded count loop | lower `filter(p).length`, `filter(p).count()`, and `count { p }` to `Reduce(Add, 0, p ? 1 : 0)` | 207/286 | 0/143 |
| 3 | Java stream aggregate vs enhanced-for loop | recover single-parameter Java lambdas, peel `Arrays.stream(xs)` to `xs`, and canonicalize `anyMatch`/`allMatch` to `Any`/`All` | 220/286 | 0/143 |
| 4 | Ruby `select.reduce` vs guarded `each` loop | stop lowering Ruby `select` as a loop, canonicalize Ruby `select`/`collect` as `Filter`/`Map` HoF forms | 225/286 | 0/143 |
| 5 | Ruby `any?`/`all?` vs early-return loops | canonicalize Ruby predicate reductions and lower `return true/false` through Ruby argument lists | 235/286 | 0/143 |
| 6 | Rust early-return loops vs predicate reductions | process terminators nested under expression statements so Rust `return true;` participates in `Any`/`All` recognition | 250/286 | 0/143 |
| 7 | Python `math.prod` vs product loop | canonicalize `math.prod(iterable, start=...)` to multiplicative `Reduce` with filtered-map support | 252/286 | 0/143 |
| 8 | C pointer-length `for`/`while` aggregate forms | recognize `i < n` plus `xs[i]` as a bounded indexed loop while preserving non-contract bounds | 260/286 | 0/143 |
| 9 | C pointer-length loop vs other languages | introduce the explicit `(int *xs, int n)` full-length contract for strict `<` traversals | 276/286 | 0/143 |
| 10 | C `1/0` predicate reductions vs boolean languages | accept C `1/0` only inside guarded early-return `Any`/`All` recognition | 286/286 | 0/143 |

Same-surface only after these loops:

```text
items: 286
positive recall: 143/143
hard-negative false merges: 0/143
```

Lowering coverage after these loops:

```text
files: 858
Raw nodes: 0 (0.000%)
```

The original ring frontier is closed:

```text
default ring misses: 0/286 positives
same-surface misses: 0/143 positives
hard-negative false merges: 0/143
```

## Loop hardening after frontier closure

After the detector closed the original ring frontier, the loop was strengthened so the next
frontier expansion has better safety gates.

| loop | hardening target | change | smoke result |
|---|---|---|---|
| 11 | cross-language transitive coverage | run `--cross all` to verify every supported surface pair, not only the ring adjacency | 858/858 positives, 0/143 false merges |
| 12 | regression enforcement | add `frontier.py --fail-on-regression` and wire it into `scripts/type4-smoke.sh` when `BASELINE_JSON` is provided | no recall/false-merge regression against loop 10 |
| 13 | C contract adversaries | generate `c_start_one` and `c_stride_two` hard-negative siblings for every proposal | 286/286 positives, 0/169 false merges |
| 14 | evidence discipline | add `evidence.level` to the manifest schema and generator (`E1` positives, `E2` counterexample negatives) | generated manifests contain `E1,E2` |
| 15 | breadth-first complexity control | validate proposal fields and enforce `max_lines`/`max_branch_count` budgets during generation | 286/286 positives, 0/169 false merges |
| 16 | dense smoke with adversaries | run all-cross with the new C hard negatives and regression gate | 858/858 positives, 0/169 false merges |
| 17 | final documentation and gate | update docs and run the final release/test/docs smoke | 286/286 positives, 0/169 false merges |

Current default ring smoke after loop 16:

```text
items: 455
positive recall: 286/286
hard-negative false merges: 0/169
```

Current all-cross smoke after loop 16:

```text
items: 1027
positive recall: 858/858
hard-negative false merges: 0/169
```

This is the intended co-evolution shape: the synthetic factory exposed under-merge classes,
the detector gained concrete language/value-graph capabilities, and the hard negatives
remained clean. The next frontier should expand the semantic matrix rather than make these
same aggregate cases more complex; good next cells are selection reductions (`min`/`max`),
map/builders, zip/dot-product, string builders, and bounded recursion.

## Second detector/process expansion: loops 18-27

The next ten loops expanded the semantic matrix and used the generated misses to improve
the detector. This run added three new cells:

- selection reductions: clamped `min`/`max` folds with nearby `min`/`max` hard negatives;
- map-contribution reductions: `sum`/`product` over `x*x` contributions with `x*x -> x`
  hard negatives;
- aligned multi-collection reductions: dot product over `(a, b)` with `x*y -> x+y`
  hard negatives.

| loop | target | detector/process change | smoke result |
|---|---|---|---|
| 18 | selection reductions | add `iter11_max_seed_zero` and `iter12_min_seed_zero`; generator exposed 44 selection misses | 286/330 positives, 0/195 false merges |
| 19 | selection candidate reachability | add exact value-bucket candidate generation and selection-seed normalization for `Reduce` | 328/330 positives, 0/195 false merges |
| 20 | Rust fold selection | evaluate Rust expression-`if` branch blocks as values in the value graph | 330/330 positives, 0/195 false merges |
| 21 | map-contribution reductions | add `iter13_sum_positive_squares` and `iter14_product_nonzero_squares` | 374/374 positives, 0/221 false merges |
| 22 | map-contribution propagation | run dense `--cross all` for the expanded map-contribution set | 1122/1122 positives, 0/221 false merges |
| 23 | dot-product frontier | add `iter15_dot_product`; generator exposed Python zip, Go/Ruby enumerate, Rust zip-fold, and C shared-length misses | 386/396 positives, 0/234 false merges |
| 24 | dot-product detector fixes | preserve Go/Ruby index+value iteration via `Enumerate`, canonicalize Rust `.zip()` and tuple lambda params, support `Reduce` over zip element bindings, recognize Rust `0..a.len()`, and extend the C shared-length contract to `(a,b,n)` | 396/396 positives, 0/234 false merges |
| 25 | dot-product propagation | run dense `--cross all` for the dot-product set | 1188/1188 positives, 0/234 false merges |
| 26 | same-surface hard-negative gate | run `--cross none` to isolate per-surface positives plus all hard negatives | 198/198 positives, 0/234 false merges |
| 27 | final smoke/documentation gate | update the semantic scope/docs and run `scripts/type4-smoke.sh` on the release binary | 396/396 positives, 0/234 false merges |

Final current smoke numbers after loop 27:

```text
default ring: items 630, positives 396/396, false merges 0/234
same-surface: items 432, positives 198/198, false merges 0/234
all-cross: items 1422, positives 1188/1188, false merges 0/234
lowering coverage: 1260 files, 31042 IL nodes, Raw nodes 0 (0.000%)
```

The important detector changes were not language-isolated end states. Go range,
Ruby `each_with_index`, Rust iterator zip/fold, Python zip comprehensions, Java/JS indexed
loops, and C pointer-length loops now converge through the same shared value-graph shape:

```text
Reduce(Add, init=0, contrib=Elem(a) * Elem(b))
```

The hard-negative sibling `Elem(a) + Elem(b)` stayed separate across the generated corpus.

## Loop-quality strengthening: loops 28-32

After loop 27, the process was tightened to address the main critique: the previous
success could still be too generator-specific. These five loops strengthened the evaluation
loop before adding more semantic complexity.

| loop | target | change | result |
|---|---|---|---|
| 28 | split discipline and stronger hard negatives | mark cross-language positives and all negatives as `heldout`; add same-template and cross-template semantic hard negatives in addition to aggregate and C contract negatives | default ring: 396/396 positives, 0/630 false merges; heldout 198/198 positives, 0/630 false merges |
| 29 | dense language-pair heldout gate | run the stronger negative set with `--cross all` | all-cross: 1188/1188 positives, 0/1422 false merges; heldout 990/990 positives, 0/1422 false merges |
| 30 | unseen representation template | add heldout `indexed_loop` positives and `indexed-template-semantic-mutation` negatives for single-list specs | default ring: 583/583 positives, 0/817 false merges; heldout 385/385 positives, 0/817 false merges |
| 31 | negative taxonomy reporting | evaluator/frontier report false merges by `negative_tag` | all negative tags remained 0 false merges: aggregate, same-template, indexed-template, cross-template, C skipped-first, C stride-two |
| 32 | dense gate and docs | run current generator with `--cross none`, default ring, and `--cross all`; update docs/schema/logs | same-surface 385/385 and 0/619; default ring 583/583 and 0/817; all-cross 1375/1375 and 0/1609 |

No detector patch was needed in loops 28-32: the strengthened heldout templates and stronger
hard negatives did not expose a new under-merge or false merge. That is a useful signal, but
not a proof of breadth. It means the next semantic expansion should target a genuinely new
axis rather than another loop/reduction variant.

Operational finding: the default ring gate is still practical for routine loops, but the
strong all-cross gate grew to 2984 items and should be treated as a periodic dense gate, not
the inner-loop default.

## Coverage-preserving compaction and abs axis: loops 33-37

The next five loops addressed the scale problem directly, then added a new semantic axis.
The goal was to keep verification strict while reducing routine scan cost, and to ensure the
generator still forces detector changes when it exposes a real under-merge.

| loop | target | change | result |
|---|---|---|---|
| 33 | coverage-preserving compaction | add `select_cases.py` and `SUITE=core` smoke support; copy only selected source files into the compact suite | default ring compact: 125/1400 items, 18/18 positives, 0/107 false merges; full corpus remained 583/583 and 0/817 |
| 34 | new semantic axis | add `iter16_sum_abs_all`, covering sign-normalizing `sum(abs(x))` across every supported surface | first compact selector passed 19/19 and 0/113, but full ring exposed 13 misses, all in `iter16_sum_abs_all`; this showed the selector was too weak |
| 35 | detector and selector correction | generalize loop recurrence recognition from filtered reductions to full conditional contributions, strengthen `abs` pattern operand handling, and tune core selection to preserve proposal/representation/cross-surface coverage | compact ring: 365/1479 items, 153/153 positives, 0/212 false merges; full ring: 616/616 positives, 0/863 false merges |
| 36 | dense all-cross compact gate | run the new selector on `--cross all` to check all language-surface participation without scanning the whole dense corpus | all-cross compact: 359/3151 items, 151/151 positives, 0/208 false merges |
| 37 | smoke integration | run `scripts/type4-smoke.sh` with `SUITE=core CROSS=all`, including stats and frontier output | 359/3151 selected, 151/151 positives, 0/208 false merges; lowering coverage 718 files, 18446 IL nodes, Raw 0 (0.000%) |

Additional validation after loop 37:

- same-surface full (`--cross none`): 1061 items, 407/407 positives, 0/654 false merges;
- default ring full: 1479 items, 616/616 positives, 0/863 false merges;
- focused regression tests: `abs_idiom_converges` and
  `conditional_abs_reduction_converges_with_aggregate`.

Critical finding: compaction must preserve interactions, not just individual labels. A
proposal-level representative can miss a whole representation class; the first compact core
did not include the `loop -> aggregate` abs positives that failed in the full run. The
current selector deliberately keeps proposal-by-representation and proposal-by-cross-surface
features while still reducing dense all-cross evaluation from 3151 to 359 items.

## Strict semantic frontier expansion: loops 38-47

After the strict `semantic` mode contract was tightened, exact candidates are reported only
when every participating unit is `exact_safe`: no `Raw` nodes, no abstract-only literals,
and no opaque calls. The next ten loops widened that strict frontier without reintroducing
near-mode behavior into semantic mode.

Starting point after the strict gate:

```text
all-cross compact: 359/3151 selected, 143/151 positives, 0/208 false merges
misses: Rust range/adapter forms around dot-product and aggregate equivalents
```

| loop | frontier target | detector/frontend change | compact smoke result |
|---|---|---|---|
| 38 | strict-safe Rust range and modeled adapters | allow proved `Seq` range forms and a narrow set of modeled adapter calls (`iter`, `into_iter`, `collect`, `copied`, `cloned`, etc.) while keeping opaque calls unsafe | 151/151 positives, 0/208 false merges |
| 39 | regex literal predicates | retain JS regex source as `LitStr` and allow `.test(...)` only when the receiver is a literal regex/string; same pattern matches, different patterns do not | 151/151 positives, 0/208 false merges |
| 40 | sequence kind identity | key `Seq` value-graph nodes by sequence kind; distinguish Python list vs tuple and JS/Rust array-like forms | 151/151 positives, 0/208 false merges |
| 41 | JS object keys | lower JS object literals as key/value pairs; keep spread and methods as `Raw` so exact mode does not claim unproved object semantics | 151/151 positives, 0/208 false merges |
| 42 | Python dict keys | lower Python dict literals as key/value pairs; keep `**` unpacking as `Raw` until overwrite-order semantics are modeled | 151/151 positives, 0/208 false merges |
| 43 | Ruby hash keys | lower Ruby hash literals as key/value pairs; keep `**` hash splats as `Raw` | 151/151 positives, 0/208 false merges |
| 44 | JS static builtin guard | allow `Array.isArray(x)` only for receiver `Array` and method `isArray` | 151/151 positives, 0/208 false merges |
| 45 | JS `typeof` soundness | preserve `typeof x` as a strict-safe call and make `void`/`delete` raw instead of stripping them to the operand | 151/151 positives, 0/208 false merges |
| 46 | cross-language list-like literals | normalize list-like sequence tags (`array`, `list`, `array_expression`) while keeping tuple tags distinct | 151/151 positives, 0/208 false merges |
| 47 | cross-language map-like literals | normalize map-like tags (`object`, `dictionary`, `hash`) and normalize Ruby symbol/hash keys to their atom text | 151/151 positives, 0/208 false merges |

Focused strict-mode regressions added in these loops cover:

- same-regex positive vs different-regex negative;
- list vs tuple sequence identity;
- JS object, Python dict, and Ruby hash key preservation, including spread/splat negatives;
- exact `Array.isArray` and `typeof` guards;
- cross-language list-like and map-like literal convergence.

Real-repo check on `../craken-agents`:

```text
loop 39 semantic families: 17
loop 47 semantic families: 22
```

The five new families are attributable to the new strict frontier rather than near matching:

- `isRecord` / `isPlainObject` / `isWikiFrontMatter` style guards using
  `typeof value === 'object' && value !== null && !Array.isArray(value)`;
- `record` / `recordValue` object-or-null helpers;
- `isUnknownArray` / `isD1MigrationList` exact `Array.isArray` wrappers;
- `session` / `devUserSession` same-key object construction;
- `queueBatch` same-key object construction in production-error tests.

Final verification for loop 47:

```text
cargo test -p nose-cli --test cli          # 38 passed
cargo test -p nose-cli --test equivalence  # 100 passed
cargo test -p nose-detect                  # 19 passed
scripts/type4-smoke.sh SUITE=core CROSS=all # 151/151 positives, 0/208 false merges
```

Assessment: this was a real strict-frontier expansion, not just benchmark churn. The
aggregate/loop benchmark stayed closed while new exact semantic classes became reportable
on real code. The next frontier should add generated adversarial cases for these literal,
guard, and map/list axes so they are measured by `type4-smoke.sh`, not only by focused CLI
tests and repo dogfooding.

## Free-binding and call-identity loops: loops 48-52

The previous real-repo review found two opposed problems:

- soundness gap: functions that reference same-named module constants could merge even when
  the module constants differed (`date-fns` locale tables);
- recall gap: useful JavaScript helper/assertion clones disappeared because strict mode
  rejected every uninterpreted call (`execa` test helpers).

These five loops addressed those two axes, with a deliberate rollback of an over-broad
call opening.

| loop | target | change | result |
|---|---|---|---|
| 48 | free module binding soundness | seed the value graph with strict-safe top-level literal/sequence/map assignments, keyed by the original binding name, so free references inside functions evaluate to the captured value instead of `Input(name)` | focused module-constant regression passed |
| 49 | module capture safety | restrict captured globals to literal/sequence/map/index/bin/unop expressions; top-level calls, lambdas, functions, loops, raw nodes, and splats remain uncaptured | `date-fns` large locale-table merge split by actual table values |
| 50 | safe uninterpreted JS calls | allow JavaScript free-function calls when the callee identity and exact-safe arguments are preserved | recovered useful `execa` helper clones |
| 51 | receiver method identity | allow JavaScript method calls with explicit receiver value and method name, still relying on the pre-walk to reject raw/spread/unsafe arguments | recovered assertion-helper clones such as `t.is(...)` |
| 52 | over-broad call rollback | an experiment that allowed generic uninterpreted calls for every language exploded the 8-repo sample (`go` 8→87, `java` 13→95, `c` 13→87); the gate was narrowed back to JavaScript-only | compact smoke remained 151/151 positives, 0/208 false merges |

Language-sample comparison after loop 52, using the same fixed sample as the prior review
and excluding `retrofit/website/public/**` generated docs:

| language | repo | installed semantic families | strict loop 47 | loop 52 |
|---|---:|---:|---:|---:|
| Python | `marshmallow` | 39 | 8 | 8 |
| JavaScript | `execa` | 15 | 0 | 3 |
| TypeScript | `date-fns` | 61 | 18 | 19 |
| Go | `etcd` | 86 | 8 | 8 |
| Rust | `alacritty` | 19 | 5 | 5 |
| Java | `retrofit` | 154 | 13 | 13 |
| C | `libsodium` | 120 | 13 | 13 |
| Ruby | `thor` | 6 | 1 | 1 |

Qualitative findings:

- `execa`: recovered 3 useful JavaScript test-helper families, e.g.
  `testNoPrintCompletion`/`testNoPrintCommand`, `testScriptStdoutSync`/`testScriptStdout`,
  and generator helpers.
- `date-fns`: the previous broad locale `formatRelative` merge no longer groups unrelated
  locale tables. The main 58-member family split into smaller same-table/same-behavior
  groups; total member count dropped from 116 to 63 despite family count increasing by one.
- Other languages stayed at the loop-47 strict level after the call rollback, avoiding the
  installed-version flood of call-heavy test/framework clones.
- `craken-agents`: remained stable at 22 strict semantic families, byte-for-byte same
  family keys as loop 47.

Final verification for loop 52:

```text
cargo test -p nose-cli --test cli          # 41 passed
cargo test -p nose-cli --test equivalence  # 100 passed
cargo test -p nose-detect                  # 19 passed
scripts/type4-smoke.sh SUITE=core CROSS=all # 151/151 positives, 0/208 false merges
```

Assessment: module-constant capture is a clear strict-soundness improvement and should be
kept. JavaScript-only uninterpreted call identity is useful but still a provisional
frontier: it recovers real helper clones without widening other languages. General
cross-language call identity should not be reopened until imports/bindings are modeled
explicitly enough to prove that two same-named callees are the same function.

## Generic proof facts and semantic-axis smoke: loop 53

Loop 52 intentionally left a language-specific JS call gate. Loop 53 replaces that with a
language-neutral proof-fact model:

- strict safety now uses file-level `StrictFacts`, not `Lang::JavaScript`;
- free `Name` values are exact-safe only when proven by a single-assignment immutable
  binding or a safe same-file function binding;
- repeated top-level assignments are not captured as immutable values;
- proven helper callees are keyed by a literal-sensitive function-binding hash, so
  `helper(x + 1)` and `helper(x + 2)` do not collapse;
- method calls are allowed generically when receiver identity and method name are proven;
- unproven free globals remain `expected_exact_detect=false` unsafe-boundary cases.

The generator now records `semantic_axes` and capability states in every manifest item and
adds breadth-first axis cases for:

- `immutable_binding`;
- `proven_callee_identity`;
- `table_access`;
- `unsafe_boundary`.

The compact selector now preserves semantic-axis and capability-state coverage, not only
proposal/representation/cross-surface coverage. A capability matrix lives in
`capabilities.v1.json`; import/re-export identity and table access for several compiled
language surfaces remain explicit unsupported/partial cells instead of hidden TODOs.

Focused axis validation after the detector change:

```text
SUITE=core CROSS=none compact axis check:
items: 200
positive recall: 66/66
hard-negative false merges: 0/134

by semantic axis:
  aggregate_reduction: positive 37/37, false merges 0/94
  immutable_binding: positive 11/11, false merges 0/11
  proven_callee_identity: positive 11/11, false merges 0/11
  table_access: positive 7/7, false merges 0/7
  unsafe_boundary: positive 0/0, false merges 0/11
```

Final dense compact smoke:

```text
scripts/type4-smoke.sh SUITE=core CROSS=all
selected items: 428/3220
positive recall: 180/180
hard-negative false merges: 0/248
Raw nodes: 0/20684
```

Assessment: this is the intended direction. A rule learned from JavaScript helper clones is
now represented as a common proof obligation, and the generator can exercise it across all
surfaces that currently emit the necessary facts. The next frontier is structured import
facts: same import coordinate should become proven callee identity, while wildcard imports,
re-exports, dynamic import, and unresolved aliases should remain unsafe-boundary cases.

## Adversarial import-identity coevolution: loops 54-63

This run followed the stricter coevolution protocol: generator first, current detector
measurement second, detector change only when a real frontier failure appeared, then an
immediate generator counterattack.

| loop | generator move | current-detector result | detector / loop change | result |
|---|---|---:|---|---:|
| 54 | add `import_identity` capability axis plus named/alias/namespace/default/unsafe import cases | import positives 0/9, false merges 0/11 | no detector change yet; this established the frontier | failure recorded |
| 55 | same corpus, focused on static named/alias imports | miss confirmed for JS-family, Python, Rust, Java, Go namespace | lower static imports to common `import_binding` / `import_namespace` facts; extend strict-safe sequence tags | import positives 9/9, false merges 0/11 |
| 56 | default import and default-vs-named boundary | passed after loop 55 | no detector change; kept as hard-negative coverage | 1/1 positive, 0/1 false merges |
| 57 | multi-specifier import counterattack | Python multi-specifier positive missed | top-level synthetic `Block` assignment lists are flattened during strict fact collection and value-graph seeding | multi-specifier positive 1/1 |
| 58 | re-export boundary | no false merge | no detector change; re-export remains unproven local binding | 0/1 false merges |
| 59 | wildcard / unresolved import boundary | no false merge | no detector change; wildcard/dot/import-star remain unsafe-boundary cases | 0/5 false merges |
| 60 | compact selector pressure | import-axis features appeared in compact suite, but only selected coverage cells | selector already preserved `semantic_axis` and `capability` features; no code change | import axis selected 21/94 |
| 61 | focused CLI regression | synthetic smoke covered it, but unit gate needed a smaller regression | add `scan_mode_semantic_allows_static_import_identity` with Python multi-specifier alias positive and different export negative | CLI test passed |
| 62 | dense all-cross validation | full compact smoke needed to ensure aggregate frontier stayed closed | no detector change | 449/3314 selected, 189/189 positives, 0/260 false merges |
| 63 | real repo audit on `../craken-agents` | strict semantic families 26→32, removed 0 | no detector change; qualitative added families looked useful | +6 families, 0 removed |

Final dense compact smoke:

```text
scripts/type4-smoke.sh SUITE=core CROSS=all
selected items: 449/3314
positive recall: 189/189
hard-negative false merges: 0/260
Raw nodes: 0/21324
```

Real-repo audit (`../craken-agents`) added six strict semantic families and removed none.
Examples of added families:

- `fileBrowserTargetHref` / `fileTargetHref`;
- `interactionCheckResult` / `visualCheckResult`;
- `agentTokenUsageCommand` / `readMessagePageFromStoreCommand`;
- `agentEffectSink` fixtures;
- `conversationKindBinding` helpers;
- `queueBatch` fixtures.

Assessment: this run matched the adversarial coevolution shape better than earlier loops.
The generator found a real under-merge first, the detector fix was a common proof-fact
extension rather than a language-name exception, and the counterattack found a second real
miss in multi-specifier imports. Unsupported forms remain explicit unsafe-boundary cases
instead of being silently accepted.

## Adversarial projection-identity coevolution: loops 64-73

This run widened the strict frontier from import coordinates to static field/property
projection coordinates. The rule is intentionally narrow: a frontend may emit ordinary
`Field(base, key)` evidence only when both the receiver expression and projected key are
static in the source form. Dynamic keys and destructuring defaults remain outside strict
exact reporting unless a future proof establishes the missing facts.

| loop | generator move | current-detector result | detector / loop change | result |
|---|---|---:|---|---:|
| 64 | add `projection_identity` capability axis with temp, destructuring, and static-key cases | projection positives 7/11, false merges 0/11 | no detector change yet; this established the frontier | JS-family destructuring, Rust destructuring, and JS static string-key misses recorded |
| 65 | focus static string-key projection | `row['today']` did not converge with `row.today` | lower JS/TS static string subscript keys to the same `Field(base, key)` coordinate as dotted access | static-key positive 1/1 |
| 66 | focus destructuring projection | object/struct patterns were not binding selected fields as values | lower JS object destructuring and Rust aliased struct patterns to projection assignments | destructuring positives closed in compact smoke |
| 67 | add shorthand/multi destructuring plus default/dynamic-key hard negatives | compact smoke passed, but selection only sampled part of the new projection surface | add full-manifest evaluation as a countercheck for newly widened axes | compact projection 11/11, false merges 0/12 |
| 68 | full-manifest countercheck | Rust shorthand struct destructuring missed outside compact selection | add conservative Rust struct-pattern text fallback for simple shorthand/alias fields | full projection 34/34, false merges 0/44 |
| 69 | default destructuring boundary | no false merge | reject JS alias defaults as projection evidence; defaults need a field-presence proof | 0/5 false merges |
| 70 | dynamic-key boundary | no false merge | no detector change; dynamic keys remain unproven projection bindings | 0/5 false merges |
| 71 | focused CLI regression | synthetic smoke covered the behavior, but unit gate needed a smaller check | add `scan_mode_semantic_allows_static_projection_identity` for JS key/destructure and Rust shorthand | CLI test passed |
| 72 | dense all-cross validation | full compact smoke needed to ensure aggregate/import/table frontiers stayed closed | no detector change | 471/3392 selected, 200/200 positives, 0/271 false merges |
| 73 | real repo audit on `../craken-agents` | strict semantic families 32→32 | no detector change; this repo did not expose new projection-result families | 0 added, 0 removed |

Final full same-surface manifest check for the projection counterattack:

```text
items: 1302
positive recall: 509/509
hard-negative false merges: 0/793

by semantic axis:
  projection_identity: positive 34/34, false merges 0/44
```

Final dense compact smoke:

```text
scripts/type4-smoke.sh SUITE=core CROSS=all
selected items: 471/3392
positive recall: 200/200
hard-negative false merges: 0/271
Raw nodes: 0/21812
```

Real-repo audit (`../craken-agents`) did not change the visible strict semantic family
set: 32 before, 32 after, with no added or removed families. Assessment: the synthetic
frontier expanded in a strict and useful way, but this particular repo does not yet
validate projection identity as a high-yield real-world source of new refactoring
candidates. The loop itself did improve: compact smoke missed one Rust shorthand case, so
full-manifest evaluation should be used as a periodic adversarial countercheck whenever a
new semantic axis is introduced or substantially widened.

## Adversarial nullish-default coevolution: loops 74-83

This run targeted a soundness-critical JavaScript-family frontier. Before this loop, `??`
was lowered to the same value-`Or` operator as `||`. That recovered some superficial
similarity, but it was not strict Type-4: `value ?? fallback` differs from
`value || fallback` for falsy non-null values such as `0`.

| loop | generator move | current-detector result | detector / loop change | result |
|---|---|---:|---|---:|
| 74 | add `nullish_default` capability axis with `??`, explicit `== null` ternary, guard return, and truthy-or boundary | nullish positives 0/5, false merges 3/5 | no detector change yet; this established both under-merge and over-merge failures | failure recorded |
| 75 | focus `??` vs explicit nullish ternary | coalesce positives missed | lower JS/TS `??` to `If(value == null, fallback, value)` instead of `Or(value, fallback)` | coalesce positives 4/4 in compact smoke |
| 76 | focus guard-return equivalence | guard positive now converged through existing guarded-return/Phi machinery | no value-graph change needed | guard positive 1/1 |
| 77 | truthy-or counterattack | previous false merges disappeared after loop 75 | no detector change; `||` remains value-or, distinct from nullish default | truthy boundary 0/3 false merges |
| 78 | generator audit | an automatic hard-negative for guard identity was accidentally identical to the positive | mutate the guard fallback in negative variants | generator bug fixed |
| 79 | focused CLI regression | synthetic smoke covered it, but a smaller invariant was needed | add `scan_mode_semantic_distinguishes_nullish_from_truthy_defaults` | CLI test passed |
| 80 | full same-surface manifest | compact selector was not enough for a new soundness axis | no detector change | 519/519 positives, 0/808 false merges |
| 81 | dense all-cross validation | aggregate/import/projection frontiers stayed closed | no detector change | 481/3417 selected, 205/205 positives, 0/276 false merges |
| 82 | real repo audit on `../craken-agents` | strict semantic families 32→32 | no detector change; this repo did not expose new nullish-result families | 0 added, 0 removed |
| 83 | process assessment | this loop found a real over-merge, not only missed positives | keep truthy-vs-nullish hard negatives as a standing soundness gate | gate retained |

Final full same-surface manifest check:

```text
items: 1327
positive recall: 519/519
hard-negative false merges: 0/808

by semantic axis:
  nullish_default: positive 10/10, false merges 0/15
```

Final dense compact smoke:

```text
scripts/type4-smoke.sh SUITE=core CROSS=all
selected items: 481/3417
positive recall: 205/205
hard-negative false merges: 0/276
Raw nodes: 0/22082
```

Real-repo audit (`../craken-agents`) again left the visible strict semantic family set
unchanged: 32 before, 32 after. Assessment: this was still a worthwhile frontier expansion
because it closed a strict-mode soundness bug (`??` vs `||`) and added an adversarial
boundary that should prevent future regressions.

## Adversarial record-shape-guard coevolution: loops 84-93

This run targeted a narrow but common JavaScript-family proof obligation: a value is a
plain record only when the source proves all three facts at once:

- `typeof value === "object"`;
- `value` is not null, either by explicit null comparison or a conservative truthy check;
- `!Array.isArray(value)`.

The detector must not treat partial object checks as exact Type-4 clones of full
record-shape guards.

| loop | generator move | current-detector result | detector / loop change | result |
|---|---|---:|---|---:|
| 84 | add `record_shape_guard` capability axis with clause-order and truthy-null-check positives plus missing-null/missing-array boundaries | record positives 0/5, false merges 0/5 | no detector change yet; this established the under-merge frontier | failure recorded |
| 85 | focus reordered three-clause guard | equivalent guards lowered as ordinary boolean chains | add conservative JS/TS recognition for exactly three same-identifier clauses and emit `record_guard` | single-case IL converged |
| 86 | exact-value size counterattack | `record_guard(value)` was too small for the strict value-family floor | include fact literals (`object`, `non_null`, `not_array`) and mark `record_guard` strict-safe in the value graph | single-case scan reported the family |
| 87 | generator hard-negative audit | three reported false merges were caused by generated negatives that were accidentally equivalent | mutate identity-proposal negatives to an unrelated property predicate | focused compact: 96/96 positives, 0/167 false merges |
| 88 | focused CLI regression | synthetic smoke covered it, but a smaller invariant was needed | add `scan_mode_semantic_proves_js_record_shape_guards` with missing-null and missing-array negatives | CLI test passed |
| 89 | full same-surface manifest | compact selector was not enough for a new proof axis | no detector change | 529/529 positives, 0/828 false merges |
| 90 | dense all-cross validation | aggregate/import/projection/nullish frontiers stayed closed | no detector change | 491/3447 selected, 210/210 positives, 0/281 false merges |
| 91 | default ring validation | full ring corpus needed current README numbers | no detector change | 1775 items, 738/738 positives, 0/1037 false merges |
| 92 | real repo audit on `../craken-agents` | strict semantic families 32→32 | no detector change; this repo already had same-form `isRecord` helpers but no newly exposed reordered/truthy variants | 0 added, 0 removed |
| 93 | process assessment | synthetic frontier expanded but real-repo family yield was neutral | keep full-manifest counterchecks for new proof axes and mine real corpora for the next guard-family variants | gate retained |

Final full same-surface manifest check:

```text
items: 1357
positive recall: 529/529
hard-negative false merges: 0/828

by semantic axis:
  record_shape_guard: positive 10/10, false merges 0/20
```

Final dense compact smoke:

```text
scripts/type4-smoke.sh SUITE=core CROSS=all
selected items: 491/3447
positive recall: 210/210
hard-negative false merges: 0/281
Raw nodes: 0/22287
```

Real-repo audit (`../craken-agents`) left the visible strict semantic family set unchanged:
32 before, 32 after. The current result already contains a useful seven-member
`isRecord`/`isPlainObject` family, so this loop did not create a new refactoring candidate
there. Assessment: this was still a strict-frontier expansion because reordered full
guards and conservative truthy-null checks now converge, while missing-null, missing-array,
and property-predicate siblings remain outside exact semantic mode. The next similar loop
should start from real-corpus guard variants, especially property-presence and typed-field
checks, before adding another synthetic guard axis.

## Real-corpus-guided own-property-guard coevolution: loops 94-103

This run followed the real-corpus-guided variant of the loop. Before adding a new synthetic
axis, the 105 pinned benchmark repos were mined for JavaScript-family guard idioms:

```text
quoted `key in obj`: 599 matches in 21 repos
typeof property === function: 425 matches in 13 repos
direct .hasOwnProperty(...): 224 matches in 11 repos
typeof property === string: 199 matches in 12 repos
Object.hasOwn(...): 116 matches in 6 repos
Object.prototype.hasOwnProperty.call(...): 109 matches in 11 repos
```

The chosen strict frontier was `own_property_guard`: `Object.hasOwn(obj, key)` and
`Object.prototype.hasOwnProperty.call(obj, key)` prove the same own-property presence check
when the receiver and key coordinates are fixed. The prototype-including `key in obj`,
shadowable direct `obj.hasOwnProperty(key)`, locally shadowed `Object`, and different
static keys remain hard boundaries.

| loop | generator move | current-detector result | detector / loop change | result |
|---|---|---:|---|---:|
| 94 | mine guard idioms in the 105 pinned benchmark repos | `Object.hasOwn` and `hasOwnProperty.call` were both frequent enough to justify an axis | choose `own_property_guard` instead of inventing an arbitrary synthetic-only guard | frontier selected |
| 95 | add `own_property_guard` capability axis with `Object.hasOwn` vs `Object.prototype.hasOwnProperty.call`, `in`, direct-method, shadowed-`Object`, and different-key siblings | own-property positives 0/5, false merges 0/5 | no detector change yet; this established the under-merge frontier | failure recorded |
| 96 | focus static own-property call identity | two equivalent builtins lowered as unrelated calls | lower both call forms to `own_property_guard(receiver, key, own, present)` | focused compact closed |
| 97 | hard-negative counterattack | `in`, direct method, shadowed `Object`, and different-key siblings stayed separate | reject `Object.hasOwn` special lowering when `Object` is locally bound before the call | 0/5 false merges |
| 98 | focused CLI regression | synthetic smoke covered it, but a smaller invariant was needed | add `scan_mode_semantic_proves_js_own_property_guards` with direct/in/different-key/shadowed-Object negatives | CLI test passed |
| 99 | full same-surface manifest | compact selector was not enough for a new proof axis | no detector change | 534/534 positives, 0/848 false merges |
| 100 | dense all-cross validation | aggregate/import/projection/nullish/record frontiers stayed closed | no detector change | 501/3472 selected, 215/215 positives, 0/286 false merges |
| 101 | default ring validation | README current smoke numbers needed a full ring run | no detector change | 1800 items, 743/743 positives, 0/1057 false merges |
| 102 | real repo audit on `../craken-agents`, `axios`, and `drizzle-orm` | visible strict semantic family counts unchanged | no detector change; these repos did not contain same-key mixed-form families that surfaced as new refactoring candidates | 0 added, 0 removed |
| 103 | process assessment plus prioritizer | real-corpus mining improved axis choice, but visible refactoring yield was neutral and recent loops were JS-family heavy | add `prioritize_frontier.py` and require the next ordinary frontier to be all-language or multi-language | next target: `collection_empty_check` |

Final full same-surface manifest check:

```text
items: 1382
positive recall: 534/534
hard-negative false merges: 0/848

by semantic axis:
  own_property_guard: positive 5/5, false merges 0/20
```

Final dense compact smoke:

```text
scripts/type4-smoke.sh SUITE=core CROSS=all
selected items: 501/3472
positive recall: 215/215
hard-negative false merges: 0/286
Raw nodes: 0/22507
```

Real-repo audits were neutral:

```text
../craken-agents: 32 -> 32, added 0, removed 0
bench/repos/axios: 7 -> 7, added 0, removed 0
bench/repos/drizzle-orm: 56 -> 56, added 0, removed 0
```

Assessment: this loop did widen the strict frontier from real-code evidence, not from
synthetic convenience. It is a modest detector improvement because the exact semantic
channel now understands a common own-property idiom pair and preserves the important
prototype/shadowing boundaries. The next loop should probably target property type guards
only after a broader axis. The newly added prioritizer ranks `collection_empty_check` first:
49,377 matches across 92 repos and seven languages, with low estimated implementation cost
and moderate soundness risk. That should be the next ordinary frontier.

## Pattern-guided collection-empty coevolution: loops 104-110

This run used the repo-wide pattern prioritizer instead of choosing a hand-picked language
feature. The selected all-language axis was `collection_empty_check`: zero-length
comparisons and named empty/non-empty predicates should converge only when the receiver
coordinate and threshold are fixed.

| loop | generator / audit move | current-detector result | detector / loop change | result |
|---|---|---:|---|---:|
| 104 | use pattern-frequency prioritization across the 105 pinned repos | `collection_empty_check` ranked first: 21,562 raw / 18,145 weighted hits across 98 repos and 8 languages | choose it as the next ordinary frontier and keep the broad-probe gap as diagnostic-only evidence | frontier selected |
| 105 | add same-surface positives for `len/length/size == 0` vs named empty predicates, plus nonzero-threshold and wrong-receiver hard negatives | release detector hit 16/22 collection positives and 0/44 false merges | misses were Rust `.is_empty()`, Java `.isEmpty()`, and Ruby `.empty?` named forms | failure recorded |
| 106 | focus named empty/non-empty predicates | named forms did not share the length-zero proof fact | add `Builtin::IsEmpty`, lower Rust/Java/Ruby named predicates, and canonicalize `Len(x) == 0` / `Len(x) != 0` in the value graph | focused collection check 22/22, 0/44 false merges |
| 107 | full same-surface countercheck | all previous axes needed a regression check after adding a new builtin | no extra detector change | 1,448 items, 556/556 positives, 0/892 false merges |
| 108 | real-repo delta audit on Rust/Ruby/Java repos | fastlane exposed a misleading long-span family after else-after-return flattening: a large `if/else` was reported as the guard-only value | shrink the flattened guard `if` span to condition+then branch and add `scan_mode_semantic_reports_flattened_guard_span_only` | the candidate became a short guard-clause family, not a whole-branch refactor |
| 109 | dense compact all-cross validation | old axes and the new collection axis needed a combined smoke | no extra detector change | 523/3,538 selected, 226/226 positives, 0/297 false merges, Raw 0/22,976 |
| 110 | default ring validation and priority refresh | collection frontier was closed but still ranked first until status changed | mark `collection_empty_check` as `covered-current` in the prioritizer | 1,866 items, 765/765 positives, 0/1,101 false merges; next recommended axis: `string_prefix_suffix` |

Final same-surface manifest check:

```text
items: 1448
positive recall: 556/556
hard-negative false merges: 0/892

by semantic axis:
  collection_empty_check: positive 22/22, false merges 0/44
```

Final dense compact smoke:

```text
scripts/type4-smoke.sh SUITE=core CROSS=all
selected items: 523/3538
positive recall: 226/226
hard-negative false merges: 0/297
Raw nodes: 0/22976
```

Final default ring smoke:

```text
items: 1866
positive recall: 765/765
hard-negative false merges: 0/1101
positive misses: 0/765
```

Real-repo delta audits:

```text
bench/repos/alacritty: 14 -> 11 families, added 2, removed 5
bench/repos/fastlane: 89 -> 90 families, added 10, removed 9
bench/repos/jsoup: 58 -> 59 families, added 4, removed 3
```

Assessment: the quantitative pattern loop was useful, but only because it included a
real-delta audit. Synthetic hard negatives proved the strict collection axis, while the
real audit found a reporting/span bug that synthetic collection cases alone would not have
caught. This suggests the right cadence for future axes: run one complete pattern loop
per semantic axis, then repeat three to five times only while the prioritizer still
surfaces uncovered high-yield axes or real-delta audits reveal strict families that the
synthetic generator does not yet model.

## Pattern-filtered string prefix/suffix coevolution: loops 111-118

This run repeated the quantitative loop, but tightened the interpretation of extraction
gaps. Broad-probe hits are now split into true uncovered gaps and filtered overreach, so
the loop does not inflate strict Type-4 coverage by absorbing non-strict patterns.

The selected all-language axis was `string_prefix_suffix`: case-sensitive starts-with and
ends-with predicates should converge when receiver, direction, and literal affix are fixed,
and must not merge different affixes, opposite direction, or wrong receivers.

| loop | generator / audit move | current-detector result | detector / loop change | result |
|---|---|---:|---|---:|
| 111 | repeat repo-wide pattern prioritization after collection closure | `string_prefix_suffix` ranked first among open axes: 6,174 raw hits across 97 repos and 7 languages | choose it as the next ordinary frontier | frontier selected |
| 112 | add same-surface and ring cross-surface prefix/suffix positives plus affix, direction, and receiver hard negatives | release detector hit 24/40 focused positives and 0/100 false merges | misses were Go static `strings.HasPrefix/HasSuffix` and cross-language API-name convergence | failure recorded |
| 113 | focus prefix/suffix proof facts | method names did not share a cross-language proof coordinate | add `Builtin::StartsWith` and `Builtin::EndsWith`, lower Go/Java/JS/Python/Ruby/Rust/TS forms, and preserve builtin identity in the value graph | focused check 40/40 positives, 0/100 false merges |
| 114 | focused CLI regression | synthetic smoke covered it, but CLI semantic mode needed a small guard | add `scan_mode_semantic_proves_string_prefix_checks` with affix/direction/receiver negatives | CLI test passed |
| 115 | repeat pattern-gap audit | two apparent gaps were not true strict candidates | filter Python `for ... in ...` iteration from membership probes and compound `len(a)+len(b)-len(c)>0` arithmetic from collection-empty probes | membership raw 25,776→22,979 with 2,798 filtered; collection gap 1→0 with 1 filtered |
| 116 | full-manifest evaluator cost audit | full ring scan was fast, but manifest matching took more than 4 minutes | index family locations by file before checking left/right overlaps | full ring evaluation became practical without changing detector behavior |
| 117 | full and dense validation | old axes and the new prefix/suffix axis needed combined smoke | no extra detector change | full ring: 2,006 items, 805/805 positives, 0/1,201 false merges; dense compact all-cross: 578/3,923 selected, 246/246 positives, 0/332 false merges |
| 118 | real-repo delta audit on Rust/Java/JS repos | visible family set was unchanged | no detector change; this axis mostly adds primitive proof facts rather than immediate refactoring-visible families | alacritty 1→1 / 11→11 low-floor, antlr4 62→62 / 241→241, axios 7→7 / 33→33 |

Final same-surface manifest check:

```text
items: 1518
positive recall: 576/576
hard-negative false merges: 0/942

by semantic axis:
  string_prefix_suffix: positive 20/20, false merges 0/50
```

Final default ring smoke:

```text
items: 2006
positive recall: 805/805
hard-negative false merges: 0/1201

by semantic axis:
  string_prefix_suffix: positive 40/40, false merges 0/100
```

Final dense compact smoke:

```text
NOSE=target/debug/nose SUITE=core CROSS=all OUT_DIR=/tmp/nose-type4-prefix-all scripts/type4-smoke.sh
selected items: 578/3923
positive recall: 246/246
hard-negative false merges: 0/332
Raw nodes: 0/24164
```

Final prioritizer state:

```text
numeric_minmax_abs: partially-covered, score 64.36, 7,037 raw hits, 0 gaps
membership_contains: open, score 56.22, 22,979 raw hits, 0 gaps, 2,798 filtered
string_prefix_suffix: covered-current, score 7.20, 6,174 raw hits, 0 gaps
```

Assessment: this loop did expand the strict semantic frontier, but the real-repo audit did
not yet show new refactoring-visible families. That is acceptable for this axis because
prefix/suffix checks are usually small proof facts that make larger future equivalences
possible. The more important process improvement was the filtered-probe accounting: repeat
the quantitative pattern loop, but only promote gaps that remain strict after overreach
filtering. The next ordinary open axis should likely be `membership_contains`, with a
careful first split between substring contains, list/set membership, map-key membership,
and Python iteration syntax.
