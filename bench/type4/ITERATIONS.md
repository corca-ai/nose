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

## Loop acceleration: loops 119-122

Before widening the next semantic frontier, the loop machinery was made cheaper without
adding a new orchestration script.

| loop | bottleneck | change | measured result |
|---|---|---|---:|
| 119 | generator always emitted every semantic class | add `generate.py --axis` and `--proposal-prefix` filters | `string_prefix_suffix` focused corpus: 70 items instead of 2,006 ring items |
| 120 | smoke gate had only full/compact knobs | add `GATE=focused|core|full` to `scripts/type4-smoke.sh`, passing focused filters through to the generator | `GATE=focused AXIS=string_prefix_suffix`: 20/20 positives, 0/50 false merges, Raw 0/1,512 |
| 121 | prioritizer rescanned 59k files on every rerun | add input-fingerprint cache to `prioritize_frontier.py` | cached rerun: about 1.8s after a 61.8s cold run |
| 122 | real-repo audit target selection was manual | report top matching repos per frontier candidate | `membership_contains` audit starts from `guava`, `sympy`, and `sqlalchemy` rather than arbitrary repos |

The next detector loop should use `GATE=focused` for the new strict sub-axis, `GATE=core`
after the first detector fix, and full/dense validation only when the focused frontier is
closed.

## Literal collection membership: loops 123-129

The next `membership_contains` split deliberately avoided the overloaded broad contains
space. The closed sub-axis is only static literal collection membership: an element
coordinate checked against a fixed literal collection. Substring contains, map-key
membership, dynamic set membership, and arbitrary receiver `.contains()` remain outside
this proof fact.

| loop | pressure | change | measured result |
|---|---|---|---:|
| 123 | `membership_contains` was too broad to open soundly | select `literal_collection_membership` as a strict sub-axis and mark Java/C unsupported for this first slice | focused manifest generated 90 items: 18 positives, 72 negatives |
| 124 | existing detector did not converge literal membership APIs | baseline `target/release/nose` missed all focused positives | 0/18 positives, 0/72 false merges |
| 125 | method APIs and Python `in` needed one proof coordinate | add `Builtin::Contains`, lower literal-sequence `includes/include?/contains/__contains__` and Go `slices.Contains`, and map it to `Op::In` in the value graph | first CLI test converged Python/JS/TS/Ruby/Rust |
| 126 | Go `slices.Contains([]T{...}, x)` still stayed out of exact reports | normalize membership literal collections inside `Contains` and allow Go `composite_literal` only for this builtin's collection safety gate | CLI test converged Go plus Python/JS/TS/Ruby/Rust |
| 127 | adversarial focused gate | keep wrong-element, wrong-collection, substring, and semantic-mutation negatives | focused: 18/18 positives, 0/72 false merges |
| 128 | aggregate regression gate | run ring, same-surface, and dense all-cross compact gates | ring 823/823 and 0/1,273; same-surface 585/585 and 0/978; dense compact 260/260 and 0/361 |
| 129 | top real-repo audit | compare pre-loop `target/release/nose` and modified detector on `guava`, `sympy`, and `sqlalchemy` | visible family sets unchanged: 0 added, 0 removed in all three repos |

Final focused membership gate:

```text
items: 90
positive recall: 18/18
hard-negative false merges: 0/72
```

Final default ring smoke:

```text
items: 2096
positive recall: 823/823
hard-negative false merges: 0/1273

by semantic axis:
  literal_collection_membership: positive 18/18, false merges 0/72
```

Final dense all-cross compact smoke:

```text
GATE=core CROSS=all NOSE=target/debug/nose OUT_DIR=/tmp/nose-type4-smoke-core-allcross scripts/type4-smoke.sh
selected items: 621/4148
positive recall: 260/260
hard-negative false merges: 0/361
Raw nodes: 0/25238
```

Final prioritizer state:

```text
numeric_minmax_abs: partially-covered, score 64.36, 7,037 raw hits, 0 gaps
null_option_presence: partially-covered, score 51.52, 126,057 raw hits, 0 gaps
membership_contains: partially-covered, score 36.54, 22,979 raw hits, 0 gaps, 2,798 filtered
map_default_lookup: open, score 31.23, 4,319 raw hits, 0 gaps
```

Assessment: this loop expanded the strict semantic frontier and improved the adversarial
process. It did not add visible refactoring candidates in the top membership-heavy repos,
which is acceptable for this narrow proof fact: literal membership is often a small
predicate, and the first real value is reducing future ambiguity around membership/contains.
The next cost-effective ordinary open axis is now `map_default_lookup`; remaining
membership work should target map-key and dynamic set membership only when receiver/key
coordinates are provable.

## Literal map-default lookup: loops 130-136

The next `map_default_lookup` split was narrowed to literal Python/Ruby maps first. This
captures the highest-confidence dynamic-language part of the candidate without claiming
typed map semantics for Go/Java/Rust or JS/TS object/Map missing-key behavior.

| loop | pressure | change | measured result |
|---|---|---|---:|
| 130 | broad map-default APIs mix absent-key, typed-map, and mutation semantics | choose `literal_map_default_lookup`: static literal map, dynamic key, literal fallback | focused manifest generated 20 items: 4 positives, 16 negatives |
| 131 | existing detector had no common map-default coordinate | baseline `target/release/nose` missed all focused positives | 0/4 positives, 0/16 false merges |
| 132 | Python `.get` and Ruby `.fetch` needed one strict coordinate | add `Builtin::GetOrDefault`, lower only literal-map receivers, and normalize the map argument inside this builtin | CLI map-default test passed |
| 133 | adversarial focused gate | keep wrong-key, wrong-default, wrong-map, and semantic-mutation negatives | focused: 4/4 positives, 0/16 false merges |
| 134 | aggregate regression gate | run ring, same-surface, and dense all-cross compact gates | ring 827/827 and 0/1,289; same-surface 587/587 and 0/986; dense compact 263/263 and 0/369 |
| 135 | top real-repo audit | compare pre-loop `target/release/nose` and modified detector on `sqlalchemy`, `sympy`, and `rubocop` | visible family sets unchanged: 0 added, 0 removed in all three repos |
| 136 | reprioritize frontier | mark `map_default_lookup` partially-covered, leaving JS/TS object-or-Map and typed Go/Java/Rust maps open | next ordinary open axis: `property_type_guard` |

Final focused map-default gate:

```text
items: 20
positive recall: 4/4
hard-negative false merges: 0/16
```

Final default ring smoke:

```text
items: 2116
positive recall: 827/827
hard-negative false merges: 0/1289

by semantic axis:
  literal_map_default_lookup: positive 4/4, false merges 0/16
```

Final dense all-cross compact smoke:

```text
GATE=core CROSS=all NOSE=target/debug/nose OUT_DIR=/tmp/nose-type4-smoke-map-core-allcross scripts/type4-smoke.sh
selected items: 632/4163
positive recall: 263/263
hard-negative false merges: 0/369
Raw nodes: 0/25612
```

Final prioritizer state:

```text
numeric_minmax_abs: partially-covered, score 64.36, 7,037 raw hits, 0 gaps
null_option_presence: partially-covered, score 51.52, 126,057 raw hits, 0 gaps
membership_contains: partially-covered, score 36.54, 22,979 raw hits, 0 gaps, 2,798 filtered
map_default_lookup: partially-covered, score 20.30, 4,319 raw hits, 0 gaps
property_type_guard: open, score 5.01, 435 raw hits, 0 gaps
```

Assessment: this loop again widened the strict frontier without adding visible real-repo
families. The strict slice is intentionally small but useful: it prevents a future
map-default implementation from conflating key, fallback, or literal map differences.
The next open frontier is `property_type_guard`; however, broader partially-covered axes
(`numeric_minmax_abs`, `null_option_presence`, and the remaining membership/map slices)
may still be better if the next loop targets breadth rather than a new narrow JS-family
axis.

## Null/option presence predicates: loops 137-144

The next loop chose a broader `null_option_presence` slice instead of the narrower
JS-family `property_type_guard`. Existing comparison forms already converged in many
languages, but method-form absence/presence predicates were not a common proof fact:
Ruby `nil?`, Rust `is_none`/`is_some`, C `NULL`, and Rust `None` needed to meet the same
strict value coordinate as explicit null comparisons.

| loop | pressure | change | measured result |
|---|---|---|---:|
| 137 | avoid narrow JS-only work | choose `null_presence_predicate`: explicit null comparison plus method-form absence/presence direction | focused manifest generated 88 items: 22 positives, 66 negatives |
| 138 | validate real algorithm delta | baseline `target/release/nose` before this loop had partial recall but no false merges | 17/22 positives, 0/66 false merges |
| 139 | generator adversary check | fix identity semantic-mutation negatives so they actually flip to non-null direction | baseline stayed 17/22, 0/66 |
| 140 | detector/frontend strengthening | add `Builtin::IsNull`/`IsNotNull`, lower Ruby `nil?` and Rust `is_none`/`is_some`, map C `NULL` and Rust `None` to null literals | CLI null-presence test passed |
| 141 | focused gate | compare explicit null checks, method forms, non-null direction, and wrong-value boundaries | focused: 22/22 positives, 0/66 false merges |
| 142 | compact core gate | run coverage-preserving core on the focused axis | selected 48/88; 19/19 positives, 0/29 false merges |
| 143 | aggregate regression gate | run ring, same-surface, and dense all-cross compact gates | ring 849/849 and 0/1,355; same-surface 598/598 and 0/1,019; dense compact 280/280 and 0/398 |
| 144 | reprioritize frontier | update `null_option_presence` as partially covered: presence predicates covered, richer option unwrap/default and pointer aliases remain | next open axis still `property_type_guard`; broad high-yield axes remain `numeric_minmax_abs` and remaining null/option slices |

Final focused null-presence gate:

```text
items: 88
positive recall: 22/22
hard-negative false merges: 0/66
```

Final default ring smoke:

```text
items: 2204
positive recall: 849/849
hard-negative false merges: 0/1355

by semantic axis:
  null_presence_predicate: positive 22/22, false merges 0/66
```

Final dense all-cross compact smoke:

```text
GATE=core CROSS=all OUT_DIR=/tmp/nose-type4-smoke-all-null ./scripts/type4-smoke.sh
selected items: 678/4427
positive recall: 280/280
hard-negative false merges: 0/398
Raw nodes: 0/26487
```

Assessment: this loop is a better fit for the co-evolution goal than a narrow
`property_type_guard` loop. It improved the detector on a broadly meaningful semantic
axis, added adversarial hard negatives for direction and value-coordinate mistakes, and
kept the full corpus false-merge count at zero. The remaining weakness is that this still
does not model richer option operations such as unwrap/default, nor pointer alias facts.

## Loop preflight hardening: loop 145

Immediately after the null-presence loop, `property_type_guard` was tested as the next
open candidate. A focused corpus could be generated, but the baseline already detected all
strict positives after the generator mutation bug was fixed:

```text
items: 37
positive recall: 5/5
hard-negative false merges: 0/32
```

That made the candidate a benchmark-only expansion rather than a detector-improvement
loop, so the implementation was discarded. To prevent repeating that waste, the loop now
has an explicit preflight:

```text
python3 bench/type4/preflight_axis.py --axis <axis> --out-dir /tmp/nose-type4-preflight
```

The preflight generates a focused corpus and evaluates baseline and candidate binaries. It
fails if the candidate has false merges, if the baseline has no positive misses and no
false merges, or if the candidate does not reduce either misses or baseline false merges.
Running it against the already-covered `null_presence_predicate` correctly fails with:

```text
baseline: items=88 positive=22/22 misses=0 false_merges=0/66
candidate: items=88 positive=22/22 misses=0 false_merges=0/66
preflight failed: baseline already covers all strict positives
```

## Rust option-pattern presence: loops 146-152

The next null/option slice attacked a Rust-specific gap inside the broader
`null_presence_predicate` axis: `if let Some(_) = value { true } else { false }` should be
the same strict presence predicate as `value.is_some()`, while `if let None` and checks on
another option value must stay outside the family.

| loop | pressure | change | measured result |
|---|---|---|---:|
| 146 | generator adversary | add Rust `if let Some(_)` identity, `if let None` boundary, and wrong-value boundary proposals under `null_presence_predicate` | focused manifest generated 4 items: 1 positive, 3 negatives |
| 147 | baseline measurement | pre-loop release did not understand the pattern condition and also over-merged two hard negatives | baseline: 0/1 positives, 2/3 false merges |
| 148 | preflight policy | treat removal of baseline false merges as a valid detector-improvement loop, while still requiring candidate false merges to be zero | candidate preflight: 1/1 positives, 0/3 false merges |
| 149 | Rust frontend strengthening | lower `let Some`/`let None` conditions to `IsNotNull`/`IsNull`, and return tail expression statements without semicolons | CLI and equivalence regressions passed |
| 150 | value-graph strengthening | canonicalize `Phi(cond, true, false)` to `cond` and `Phi(cond, false, true)` to `Not(cond)` | `if let Some(_)` converges with `is_some()` and stays distinct from `if let None` |
| 151 | focused/core gates | run focused proposal and null-presence core gates | focused: 1/1 and 0/3; null core selected 49/92, 18/18 and 0/31 |
| 152 | aggregate validation | run default ring, same-surface, and dense all-cross compact gates | ring 850/850 and 0/1,358; same-surface 599/599 and 0/1,022; dense compact 280/280 and 0/400 |

Final focused if-let gate:

```text
items: 4
positive recall: 1/1
hard-negative false merges: 0/3
```

Final default ring smoke:

```text
items: 2208
positive recall: 850/850
hard-negative false merges: 0/1358

by semantic axis:
  null_presence_predicate: positive 23/23, false merges 0/69
```

Final dense all-cross compact smoke:

```text
GATE=core CROSS=all OUT_DIR=/tmp/nose-type4-smoke-all-iflet ./scripts/type4-smoke.sh
selected items: 680/4431
positive recall: 280/280
hard-negative false merges: 0/400
Raw nodes: 0/26551
```

Assessment: this was a real detector co-evolution loop, not just a benchmark expansion.
The generator found both an under-merge and a strict false-merge bug in the existing
release, the detector gained a narrow Rust frontend proof fact plus a shared boolean
select simplification, and the adversarial same-value/opposite-direction/wrong-value
boundaries now stay clean.

## Scalar absolute-value coevolution: loops 153-160

The next broad frontier used the prioritizer's highest-scoring partially-covered axis:
`numeric_minmax_abs`. The strict slice was deliberately limited to scalar absolute value,
not arbitrary min/max yet and not Ruby/Rust dynamic method forms. The target equivalence is
an explicit sign-normalizing conditional and a proven absolute-value builtin over the same
numeric coordinate.

| loop | pressure | change | measured result |
|---|---|---|---:|
| 153 | real-corpus priority | choose `numeric_minmax_abs` over the narrow `property_type_guard` because scalar numeric idioms are all-language and high-frequency | prioritizer: 7,037 raw hits across 93 repos and 8 languages |
| 154 | generator adversary | add `axis_scalar_abs_*` proposals with builtin identity, signed-identity boundary, wrong-value boundary, and shadowed-`Math` boundary | focused manifest generated 77 items: 18 positives, 59 negatives |
| 155 | baseline measurement | pre-loop release already handled Python/C but missed JS/TS/embedded, Go, Java, and most cross-surface pairs | baseline: 4/18 positives, 0/59 false merges |
| 156 | detector strengthening | lower JS/TS `Math.abs` with `Math` shadow checks, lower Java `Math.abs` in the Java frontend, and canonicalize Go `math.Abs` in idioms | focused candidate reached 18/18 before shadow hardening |
| 157 | soundness counterattack | local shadowed `Math` exposed that generic normalize-time `Math.abs` canonicalization was unsound; narrow normalize idiom to Go `math.Abs` and keep Java/JS in frontends | shadowed-`Math` false merges returned to 0 |
| 158 | focused/core gates | run focused proposal and numeric core gates | focused: 9/9 positives, 0/32 false merges, Raw 0; core selected 39/77, 15/15 and 0/24 |
| 159 | aggregate validation | run default ring, same-surface, and dense all-cross compact gates | ring 868/868 and 0/1,417; same-surface 608/608 and 0/1,054; dense compact 294/294 and 0/424 |
| 160 | scope decision | leave Ruby/Rust `.abs` method forms and scalar min/max for later slices until receiver type or builtin identity is provable | strict abs slice closed |

Final focused scalar-abs gate:

```text
items: 41
positive recall: 9/9
hard-negative false merges: 0/32
Raw nodes: 0/1405
```

Preflight comparison before rebuilding the release binary:

```text
baseline: items=77 positive=4/18 misses=14 false_merges=0/59
candidate: items=77 positive=18/18 misses=0 false_merges=0/59
preflight passed: candidate improves the frontier with zero false merges
```

Final default ring smoke:

```text
items: 2285
positive recall: 868/868
hard-negative false merges: 0/1417

by semantic axis:
  numeric_minmax_abs: positive 18/18, false merges 0/59
```

Final dense all-cross compact smoke:

```text
GATE=core CROSS=all OUT_DIR=/tmp/nose-type4-smoke-all-scalar-abs ./scripts/type4-smoke.sh
selected items: 718/4616
positive recall: 294/294
hard-negative false merges: 0/424
Raw nodes: 0/27842
```

Assessment: this loop followed the adversarial co-evolution pattern well. The generator
created a broad strict frontier, the baseline showed real under-merge, the detector gained
new language-specific proof facts, and the shadow boundary forced a soundness correction
before promotion. The remaining work on this axis should be scalar min/max and typed
Ruby/Rust abs only after their receiver/builtin identity can be proven.

## Scalar min/max coevolution: loops 161-168

This loop widened `numeric_minmax_abs` from absolute value into strict scalar two-way
selection. The target equivalence is a conditional choice over the same two numeric
coordinates and a proven builtin `min`/`max` form. It deliberately excludes dynamic
receiver methods and keeps JS/TS `Math` calls behind shadowing checks.

| loop | pressure | change | measured result |
|---|---|---|---:|
| 161 | generator adversary | add `axis_scalar_min_*` and `axis_scalar_max_*` proposals: builtin identity, wrong-value boundary, shadowed-`Math` boundary, and min/max direction mutation | focused manifest generated 118 items: 36 positives, 82 negatives |
| 162 | baseline measurement | compare installed/release baseline with the generated focused corpus | baseline: 0/36 positives, 0/82 false merges |
| 163 | value graph strengthening | treat 2-arg `Builtin::Min`/`Builtin::Max` as scalar `Bin(MIN/MAX)` choices while preserving 1-arg collection reductions | targeted cross-language min test passed |
| 164 | language proof facts | canonicalize Python/C bare `min/max` and `fmin/fmax`, Go `math.Min/Max`, JS/TS safe `Math.min/max`, and Java `Math.min/max` | candidate focused: 36/36 positives, 0/82 false merges |
| 165 | soundness counterattack | keep shadowed JS/TS `Math` as ordinary calls and preserve wrong-value plus min/max direction boundaries | shadowed/wrong/direction negatives: 0 false merges |
| 166 | focused/core gates | run proposal-focused and numeric core gates on the release binary | focused: 18/18 positives, 0/46 false merges, Raw 0; core selected 69/195, 24/24 and 0/45 |
| 167 | aggregate validation | run default ring, same-surface, and dense all-cross compact gates | ring 904/904 and 0/1,499; same-surface 626/626 and 0/1,100; dense compact 304/304 and 0/445 |
| 168 | scope decision | keep Ruby/Rust dynamic `.abs`/`.min`/`.max` method forms out until builtin/receiver identity can be proven | strict scalar min/max slice closed |

Preflight comparison before rebuilding the release binary:

```text
baseline: items=118 positive=0/36 misses=36 false_merges=0/82
candidate: items=118 positive=36/36 misses=0 false_merges=0/82
preflight passed: candidate improves the frontier with zero false merges
```

Final focused scalar-min/max gate:

```text
items: 64
positive recall: 18/18
hard-negative false merges: 0/46
Raw nodes: 0/2392
```

Final default ring smoke:

```text
items: 2403
positive recall: 904/904
hard-negative false merges: 0/1499

by semantic axis:
  numeric_minmax_abs: positive 54/54, false merges 0/141
```

Final dense all-cross compact smoke:

```text
GATE=core CROSS=all OUT_DIR=/tmp/nose-type4-smoke-all-scalar-minmax ./scripts/type4-smoke.sh
selected items: 749/4896
positive recall: 304/304
hard-negative false merges: 0/445
Raw nodes: 0/28962
```

Assessment: this was a real detector co-evolution loop, not benchmark-only expansion.
The generator exposed a broad strict under-merge class, the detector gained shared value
graph semantics plus language-specific proof facts, and the adversarial negatives
confirmed that `min`/`max` direction, wrong coordinates, and shadowed `Math` do not merge.

## Map key-membership coevolution: loops 169-176

This loop widened the membership frontier from static literal collections into dynamic map
key-presence predicates. The strict slice covers surfaces where key membership has a
direct, high-confidence proof shape without using JS/TS runtime type information: Python,
Go, Java, Ruby, and Rust.

| loop | pressure | change | measured result |
|---|---|---|---:|
| 169 | real-corpus priority | choose `membership_contains` over narrow `property_type_guard` because key-membership patterns have broader multi-language spread | prioritizer: 22,979 raw matches across 99 repos and 7 languages |
| 170 | generator adversary | add `map_key_membership` with identity, wrong-key, wrong-map, and value-membership boundaries | focused manifest generated 50 items: 10 positives, 40 negatives |
| 171 | baseline measurement | compare the release baseline with the focused map-key corpus | baseline: 0/10 positives, 0/40 false merges |
| 172 | detector strengthening | canonicalize Python `__contains__`, Java `containsKey`/`keySet().contains`, Ruby `key?`/`has_key?`, Rust `contains_key`/`get().is_some`, and Go map lookup-ok assignment to `Contains(key, map)` | candidate focused: 10/10 positives, 0/40 false merges |
| 173 | regression counterattack | full CLI tests caught that the new `contains` arm shadowed literal Rust array membership; merge keySet handling into the existing literal-membership branch | literal collection membership restored; targeted map-key test still passed |
| 174 | focused/core gates | run proposal-focused and map-key core gates on the release binary | focused: 5/5 positives, 0/20 false merges, Raw 0; core selected 26/50, 9/9 and 0/17 |
| 175 | aggregate validation | run default ring, same-surface, and dense all-cross compact gates | ring 914/914 and 0/1,539; same-surface 631/631 and 0/1,120; dense compact 312/312 and 0/462 |
| 176 | scope decision | leave JS/TS `Map.has` and ambiguous `contains/includes` until receiver type or literal construction is proven | strict map-key slice closed |

Preflight comparison before rebuilding the release binary:

```text
baseline: items=50 positive=0/10 misses=10 false_merges=0/40
candidate: items=50 positive=10/10 misses=0 false_merges=0/40
preflight passed: candidate improves the frontier with zero false merges
```

Final focused map-key gate:

```text
items: 25
positive recall: 5/5
hard-negative false merges: 0/20
Raw nodes: 0/794
```

Final default ring smoke:

```text
items: 2453
positive recall: 914/914
hard-negative false merges: 0/1539

by semantic axis:
  map_key_membership: positive 10/10, false merges 0/40
```

Final dense all-cross compact smoke:

```text
GATE=core CROSS=all OUT_DIR=/tmp/nose-type4-smoke-all-map-key ./scripts/type4-smoke.sh
selected items: 774/4971
positive recall: 312/312
hard-negative false merges: 0/462
Raw nodes: 0/29753
```

Assessment: this loop followed the intended adversarial co-evolution pattern. The
generator found a real multi-language under-merge class, the detector gained both shared
canonicalization and Go frontend lowering, and the regression suite forced a process
correction so broad `contains` support did not weaken existing literal-membership safety.

## Typed map-default coevolution: loops 177-184

This loop closes the next strict slice of the broader `map_default_lookup` frontier:
typed/dynamic maps in Go, Java, and Rust. It deliberately stays separate from the earlier
Python/Ruby `literal_map_default_lookup` slice. JS/TS object or `Map.get() ?? fallback`
forms remain open until receiver type or construction facts make the absent-value
semantics strict.

| loop | pressure | change | measured result |
|---|---|---|---:|
| 177 | scope split | choose typed map fallback over broad JS/TS defaulting because Go/Java/Rust expose map receiver, key, presence, and fallback coordinates directly | new `map_default_lookup` capability slice selected |
| 178 | generator adversary | add `axis_map_fallback_*` proposals with identity, wrong-key, wrong-default, wrong-map, and semantic-mutation siblings | focused manifest generated 15 items: 3 positives, 12 negatives |
| 179 | baseline measurement | compare the existing release binary with the focused corpus before rebuilding release | baseline: 1/3 positives, 0/12 false merges; misses were Java and Rust API/default forms |
| 180 | detector strengthening | lower Java `getOrDefault` and Rust `get(key).unwrap_or(default)` to `GetOrDefault`, and fold `Phi(key in map, map[key]/map.get(key), fallback)` to the same value-graph node | candidate focused: 3/3 positives, 0/12 false merges |
| 181 | focused ring check | run cross-surface focused ring for Go, Java, and Rust | 6/6 positives, 0/24 false merges |
| 182 | regression test | add `map_default_lookup_converges_cross_language_with_boundaries` | full equivalence suite: 114/114 passed |
| 183 | compact gate | run `GATE=core AXIS=map_default_lookup` on the release candidate | selected 17/30; 5/5 positives, 0/12 false merges, Raw 0 |
| 184 | pause point | stop after this loop per operator request, with large default-ring/dense validation deferred | ready to resume from the notes below |

Baseline comparison before rebuilding the release binary:

```text
baseline focused: items=15, positive=1/3, false_merges=0/12
candidate focused: items=15, positive=3/3, false_merges=0/12
delta: +2 positive hits, +0 false merges
```

Final release focused map-default gate:

```text
GATE=focused PROPOSAL_PREFIX=axis_map_fallback NOSE=target/release/nose ./scripts/type4-smoke.sh
items: 15
positive recall: 3/3
hard-negative false merges: 0/12
Raw nodes: 0/775
```

Final release compact map-default gate:

```text
GATE=core AXIS=map_default_lookup NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 17/30
positive recall: 5/5
hard-negative false merges: 0/12
Raw nodes: 0/873
```

Validation run before the pause:

```text
cargo test -p nose-cli --test equivalence
cargo test -p nose-cli --test cli
cargo test -p nose-frontend -p nose-normalize -p nose-detect
cargo build --release -p nose-cli
```

Pause/resume note: this is a clean stopping point. On the next session, do not restart the
same loop. First run one periodic broader validation if needed:

```sh
GATE=full AXIS=map_default_lookup NOSE=target/release/nose ./scripts/type4-smoke.sh
GATE=core CROSS=all NOSE=target/release/nose OUT_DIR=/tmp/nose-type4-smoke-resume-all ./scripts/type4-smoke.sh
```

Then refresh the prioritizer with cache and choose the next cost-effective strict axis:

```sh
python3 bench/type4/prioritize_frontier.py \
  --cache /tmp/nose-frontier-priorities.cache.json \
  --json-out /tmp/nose-frontier-priorities.json \
  --markdown-out bench/type4/FRONTIER_PRIORITIES.md
```

Assessment: this was a real detector improvement, not just benchmark growth. The baseline
already handled the Go same-shape case because both sides used the explicit lookup-ok
fallback, but it missed Java/Rust API forms. The detector now has a shared strict
`GetOrDefault(map, key, fallback)` proof fact and keeps all coordinate-changing adversaries
separate.

## Ruby numeric micro-batch coevolution: loops 185-192

This loop adopts the faster cadence: one co-evolution loop can carry about three
same-axis micro-frontiers, provided each micro-frontier has its own proposal prefix and the
batch ends with a shared compact gate. The selected batch is `numeric_minmax_abs` for Ruby:
`value.abs`, `[left, right].min`, and `[left, right].max`.

| loop | pressure | change | measured result |
|---|---|---|---:|
| 185 | acceleration rule | switch from one frontier per loop to a same-axis micro-batch of roughly three additions | batch target: Ruby scalar `abs`, `min`, `max` |
| 186 | prioritization correction | reject `property_type_guard` as detector-work for now because focused probing showed current strict positives already converge | next real miss chosen from `numeric_minmax_abs` |
| 187 | baseline measurement | generate `axis_scalar*` focused corpus after adding Ruby cases, then scan with the previous release binary | baseline: 27/30 positives, 0/85 false merges; all 3 misses were Ruby |
| 188 | generator expansion | include Ruby in scalar numeric support and emit `value.abs` plus two-element array `.min/.max` adversaries | focused corpus: 115 items, 30 positives, 85 hard negatives |
| 189 | detector strengthening | canonicalize method-form `abs` to `Abs(value)` and literal two-element `.min/.max` to scalar `Min/Max(left, right)` | candidate focused: 30/30 positives, 0/85 false merges |
| 190 | regression tests | extend scalar numeric cross-language tests with Ruby `abs`, `[left, right].min`, and `[left, right].max` | full equivalence suite: 114/114 passed |
| 191 | compact axis gate | run `GATE=core AXIS=numeric_minmax_abs` on the release candidate | selected 73/215; 25/25 positives, 0/48 false merges, Raw 0 |
| 192 | broad compact gate | run `GATE=core CROSS=all` to catch cross-axis regressions | selected 792/5101; 318/318 positives, 0/474 false merges, Raw 0 |

Focused baseline/candidate comparison:

```text
baseline focused: items=115, positive=27/30, misses=3, false_merges=0/85
candidate focused: items=115, positive=30/30, misses=0, false_merges=0/85
delta: +3 positive hits, +0 false merges
```

Final release focused numeric gate:

```text
GATE=focused PROPOSAL_PREFIX=axis_scalar NOSE=target/release/nose ./scripts/type4-smoke.sh
items: 115
positive recall: 30/30
hard-negative false merges: 0/85
Raw nodes: 0/4131
```

Final release compact all-cross gate:

```text
GATE=core CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 792/5101
positive recall: 318/318
hard-negative false merges: 0/474
Raw nodes: 0/30656
```

Assessment: this batch changed the detector, not just the benchmark. The previous release
missed exactly the three new Ruby numeric positives and already rejected the adversarial
hard negatives. The candidate closes all three without increasing false merges. The faster
cadence is viable when the batch stays inside one semantic axis and the end gate is compact
and cross-axis.

## Accelerated micro-batch loop policy

For ordinary frontier work, use batches of about three micro-frontiers instead of one full
loop per proposal. Keep the batch inside one semantic family or proof channel whenever
possible, so failures remain attributable and a shared hard-negative set can police the
whole widening. A batch should normally contain one strictness/soundness counterattack plus
two recall/frontier probes. Each candidate still gets a small focused probe when needed,
but the expensive gates are run once for the whole batch:

```sh
GATE=focused PROPOSAL_PREFIX=prefix_a,prefix_b,prefix_c NOSE=target/debug/nose ./scripts/type4-smoke.sh
GATE=core CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
```

The batch is acceptable only if every focused candidate has clear proof evidence, the
combined compact gate has zero false merges, and the change either widens the strict
semantic frontier or prevents an over-broad future widening. If a candidate fails, split
only that candidate out; keep the rest of the batch intact.

## Rust option-default micro-batch coevolution: loops 193-201

This loop continues the accelerated cadence with three same-axis micro-frontiers under
`nullish_default`: Rust `Option::unwrap_or`, capture-only `unwrap_or_else(|| fallback)`,
and identity `map_or(fallback, |inner| inner)`. The detector also preserves the existing
JS/TS nullish default guarantees by sharing one `ValueOrDefault(value, fallback)` proof
fact across expression, ternary, guard-return, and option-API forms.

| loop | pressure | change | measured result |
|---|---|---|---:|
| 193 | priority selection | choose `nullish_default` option defaulting from the null/option frontier instead of another benchmark-only slice | release probe on hand-written Rust option defaults returned no semantic clones |
| 194 | generator adversary | add `axis_option_*` proposals: `unwrap_or`, `unwrap_or_else`, `map_or`, wrong-default, and wrong-value boundaries | focused corpus: 8 items, 3 positives, 5 hard negatives |
| 195 | baseline measurement | scan the focused corpus with the previous release binary | baseline: 0/3 positives, 0/5 false merges |
| 196 | detector strengthening | add `ValueOrDefault(value, fallback)`, canonicalize Rust option APIs, and fold nullish/option `Phi` default patterns | candidate focused: 3/3 positives, 0/5 false merges |
| 197 | regression correction | prevent `ValueOrDefault` from folding path-bottom sentinels and add guarded-return/fallthrough recognition for JS nullish guards | CLI nullish/truthy regression restored |
| 198 | tests | add cross-language value-fingerprint coverage for JS `??`/guard/ternary and Rust option APIs plus wrong-coordinate boundaries | equivalence: 115/115; CLI: 52/52 |
| 199 | axis compact gate | run `GATE=core AXIS=nullish_default` on the release candidate | selected 18/33; 8/8 positives, 0/10 false merges, Raw 0 |
| 200 | broad compact gate | run `GATE=core CROSS=all` to catch cross-axis regressions | selected 800/5109; 321/321 positives, 0/479 false merges, Raw 0 |
| 201 | reprioritize | mark `null_option_presence` as covered-current for ordinary detector work; alias/effectful variants require new proof facts | next recommended frontier shifts to `membership_contains` / `map_default_lookup` |

Focused baseline/candidate comparison:

```text
baseline focused: items=8, positive=0/3, misses=3, false_merges=0/5
candidate focused: items=8, positive=3/3, misses=0, false_merges=0/5
delta: +3 positive hits, +0 false merges
```

Final release nullish-default axis gate:

```text
GATE=core AXIS=nullish_default NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 18/33
positive recall: 8/8
hard-negative false merges: 0/10
Raw nodes: 0/495
```

Final release compact all-cross gate:

```text
GATE=core CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 800/5109
positive recall: 321/321
hard-negative false merges: 0/479
Raw nodes: 0/30914
```

Assessment: this loop widened a real strict frontier. The previous detector missed all
three Rust option-default API forms, while the candidate closes them and keeps all
wrong-value/default and truthy-default adversaries separate. The process also improved the
loop itself by forcing a regression correction for guard-return nullish defaults.

## Membership strictness micro-batch: loops 202-205

This batch applies the accelerated loop policy. It does not open broad dynamic collection
membership yet; instead it fixes the soundness boundary needed before that frontier can be
expanded. The adversary found that unproven receiver-overloaded calls such as Java
`List.contains(value)` and `String.contains(value)` had the same value graph shape after
types were erased.

| loop | pressure | change | measured result |
|---|---|---|---:|
| 202 | operator cadence | adopt three-candidate micro-batches: focused attribution per candidate, expensive validation once per batch | policy recorded above |
| 203 | strictness adversary | add `axis_membership_unproven_receiver_boundary` for Java, Rust, and TypeScript | previous release false-merged 3/3 hard negatives |
| 204 | detector strengthening | make unproven membership-like field calls source-salted opaque values; proven `Builtin::Contains` facts are unchanged | candidate focused: 0/3 false merges |
| 205 | batched validation | run the new boundary together with `axis_map_fallback*` and `axis_scalar*`, then compact all-cross | batch 33/33 positives, 0/100 false merges; compact all-cross 321/321 positives, 0/480 false merges |

Focused release/candidate comparison for the new boundary:

```text
previous release: items=3, positive=0/0, false_merges=3/3
candidate:        items=3, positive=0/0, false_merges=0/3
delta:            -3 false merges
```

Final release batched focused gate:

```text
GATE=focused PROPOSAL_PREFIX=axis_membership_unproven_receiver,axis_map_fallback,axis_scalar NOSE=target/release/nose ./scripts/type4-smoke.sh
items: 133
positive recall: 33/33
hard-negative false merges: 0/100
Raw nodes: 0/4978
```

Final release compact all-cross gate:

```text
GATE=core CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 801/5112
positive recall: 321/321
hard-negative false merges: 0/480
Raw nodes: 0/30940
```

Assessment: this is a detector improvement by subtraction. Exact semantic mode is stricter
now: broad `.contains`/`.includes`/`include?`-style calls do not become Type-4 evidence
unless an earlier proof fact canonicalized them to `Builtin::Contains`. The next
membership recall step should add dynamic collection membership only with receiver/key type
facts, not by matching method names.

## Typed dynamic collection membership: loops 206-211

This loop opens the next safe `membership_contains` slice after the unproven receiver
boundary. The detector still does not trust method names alone; it now accepts dynamic
collection membership only when an explicit parameter type proves the receiver is a
collection. The first supported surfaces are Go, Java, Python, Rust, and TypeScript.

| loop | pressure | change | measured result |
|---|---|---|---:|
| 206 | proof fact gap | IL erased Java/Rust/TypeScript/Go/Python parameter type annotations, so strict membership could not distinguish collection receivers from strings | add `ParamTypeFact(span, Collection/Map/String)` metadata and preserve it across normalization rebuilds |
| 207 | generator adversary | add `axis_membership_typed_receiver_identity`, wrong-element, and typed-string hard boundaries | focused corpus: 5 positives, 13 hard negatives |
| 208 | baseline measurement | run the typed corpus with the previous release | baseline: 2/5 positives, 0/13 false merges; Java/Rust/TypeScript missed |
| 209 | detector strengthening | seed value-graph parameter semantics by alpha-renamed cid and lower only proven collection receiver calls to `Op::In`; unproven calls remain source-salted opaque | candidate focused: 5/5 positives, 0/13 false merges |
| 210 | cross-surface pressure | run focused typed membership with `CROSS=all` | 15/15 positives, 0/36 false merges |
| 211 | compact all-cross validation | run release compact all-cross after adding typed dynamic membership | 328/328 positives, 0/489 false merges |

Focused release/candidate comparison:

```text
previous release: items=18, positive=2/5, false_merges=0/13
candidate:        items=18, positive=5/5, false_merges=0/13
delta:            +3 positive hits, +0 false merges
```

Final release typed membership focused gate:

```text
GATE=focused PROPOSAL_PREFIX=axis_membership_typed CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
items: 51
positive recall: 15/15
hard-negative false merges: 0/36
Raw nodes: 0/1328
```

Final release compact all-cross gate:

```text
GATE=core CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 817/5163
positive recall: 328/328
hard-negative false merges: 0/489
Raw nodes: 0/31374
```

Assessment: this is a real strict-frontier expansion and not a broad method-name match.
The previous release already handled Python/Go shapes but missed Java, Rust, and
TypeScript after the unproven-call hardening. The new path recovers those positives only
through explicit source-level receiver facts, while typed string and wrong-element
adversaries remain separate.

## Typed TypeScript map-key membership: loops 212-216

This loop reuses the `ParamTypeFact` proof channel for the separate `map_key_membership`
axis. It opens only TypeScript `Map<K,V>.has(key)` when the receiver parameter is explicitly
typed as a `Map`; untyped JavaScript `has`, `Set.has`, value-membership, wrong-key, and
wrong-map cases remain hard boundaries.

| loop | pressure | change | measured result |
|---|---|---|---:|
| 212 | frontier reuse | `ParamSemantic::Map` existed but was not consumed by the value graph | choose TypeScript typed `Map.has` as the next low-cost strict map-key slice |
| 213 | generator adversary | add TypeScript to `axis_map_key_*` with identity, wrong-key, wrong-map, and value-membership boundaries | focused all-cross corpus: 21 positives, 84 hard negatives |
| 214 | baseline measurement | run the focused corpus with the previous release | baseline: 15/21 positives, 0/84 false merges; all misses involved TypeScript |
| 215 | detector strengthening | lower `receiver.has(key)` to `Op::In` only when the receiver parameter has `ParamSemantic::Map` | candidate focused: 21/21 positives, 0/84 false merges |
| 216 | release validation | run release focused all-cross and compact all-cross | focused: 21/21, 0/84; compact all-cross: 334/334, 0/491 |

Focused release/candidate comparison:

```text
previous release: items=105, positive=15/21, false_merges=0/84
candidate:        items=105, positive=21/21, false_merges=0/84
delta:            +6 positive hits, +0 false merges
```

Final release typed map-key focused gate:

```text
GATE=focused PROPOSAL_PREFIX=axis_map_key CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
items: 105
positive recall: 21/21
hard-negative false merges: 0/84
Raw nodes: 0/3210
```

Final release compact all-cross gate:

```text
GATE=core CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 825/5193
positive recall: 334/334
hard-negative false merges: 0/491
Raw nodes: 0/31582
```

Assessment: this is a clean proof-fact reuse. It broadens the map-key frontier into
TypeScript without trusting `has` by name. The next related frontier is not untyped
JavaScript `Map.has`; it needs construction or import/binding facts that prove the receiver
is a `Map` rather than a `Set` or arbitrary object.

## Batched typed TypeScript map-default lookup: loops 217-222

This is the first accelerated batch after adopting the "about three candidates per inner
loop" rule. The three adjacent candidates share one proof channel: explicit
`ParamSemantic::Map` receiver facts for TypeScript `Map<K,V>` parameters, lowered into the
existing `GetOrDefault(map, key, fallback)` value-graph primitive.

The batch candidates were:

- `axis_map_fallback_ts_nullish_identity`: `lookup.get(key) ?? fallback`;
- `axis_map_fallback_ts_has_get_identity`: `lookup.has(key) ? lookup.get(key) : fallback`;
- `axis_map_fallback_ts_temp_guard_identity`: temp-bound `lookup.get(key)` followed by an
  `undefined` guard.

The hard-negative siblings cover wrong key, wrong default, wrong map, and an untyped
receiver. The untyped receiver is intentionally not opened: a `.get` method by name alone
does not prove `Map` absent-key semantics.

| loop | pressure | change | measured result |
|---|---|---|---:|
| 217 | batched frontier selection | group three TypeScript map-default surfaces under `axis_map_fallback_ts_*` | focused corpus: 9 positives, 21 hard negatives |
| 218 | baseline measurement | run the focused batch with the previous release detector | baseline: 3/9 positives, 0/21 false merges; only `has/get` already converged through typed `Map.has` |
| 219 | detector strengthening | upgrade `ValueOrDefault(typed Map.get(map,key), fallback)` to `GetOrDefault(map,key,fallback)` | candidate focused: 9/9 positives, 0/21 false merges |
| 220 | loop soundness hardening | stop extracting expression ternaries as sub-function block units | untyped `lookup.get(key) ?? fallback` no longer appears through a proof-context-free block clone |
| 221 | targeted regression tests | add value-graph and CLI semantic tests for TS nullish, has/get, temp guard, and hard boundaries | CLI/equivalence targeted tests passed |
| 222 | release validation | run release focused, map-default compact, and global compact all-cross gates | focused: 9/9, 0/21; map-default core: 14/14, 0/33; all-cross core: 343/343, 0/512 |

Focused release/candidate comparison:

```text
previous release: items=30, positive=3/9, false_merges=0/21
candidate:        items=30, positive=9/9, false_merges=0/21
delta:            +6 positive hits, +0 false merges
```

Final release focused gate:

```text
GATE=focused PROPOSAL_PREFIX=axis_map_fallback_ts CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
items: 30
positive recall: 9/9
hard-negative false merges: 0/21
Raw nodes: 0/1468
```

Final release map-default compact gate:

```text
GATE=core AXIS=map_default_lookup CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 47/60
positive recall: 14/14
hard-negative false merges: 0/33
Raw nodes: 0/2326
```

Final release compact all-cross gate:

```text
GATE=core CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 855/5223
positive recall: 343/343
hard-negative false merges: 0/512
Raw nodes: 0/33055
```

Assessment: batching worked here because all three positives share the same proof
mechanism and the combined hard negatives made each boundary explicit. It also improved
the loop itself: the untyped boundary exposed that expression ternaries were being
extracted as exact block units without enough semantic context, so block extraction was
tightened to statement-level `if` units only.

## Proven Set membership micro-batch: loops 223-228

This loop applies the accelerated three-candidate cadence to the `membership_contains`
frontier. The batch opens three adjacent strict positives that share one semantic family:

- typed TypeScript `Set<T>.has(value)`;
- inline `new Set([...]).has(value)` over static literal items;
- immutable local `const values = new Set([...]); values.has(value)`.

The hard-negative siblings cover wrong element, wrong literal collection, untyped receiver,
and shadowed `Set` constructor boundaries. The central rule is unchanged: `.has` by name is
not proof. The receiver must be proven by explicit collection type or by an exact Set
construction whose constructor is not shadowed.

| loop | pressure | change | measured result |
|---|---|---|---:|
| 223 | batched frontier selection | group three Set-membership surfaces under `axis_membership_set_*` | focused corpus: 24 positives, 54 hard negatives |
| 224 | baseline measurement | scan the focused batch with the previous release detector | baseline: 0/24 positives, 0/54 false merges |
| 225 | detector strengthening | prove `Set` constructor calls over exact literal collections and route typed `Set<T>.has` through proven collection membership | candidate focused: 24/24 positives, 0/54 false merges |
| 226 | regression counterattack | full CLI tests caught `Set.has` merging with typed `Map.has` when receiver names matched; add a value-graph `CollectionParam` wrapper for proven collection receivers | map-key and Set-membership targeted tests both passed |
| 227 | release focused/axis gates | build release and run focused Set plus literal-membership core gates | focused: 24/24, 0/54; literal-membership core: 32/32, 0/63 |
| 228 | broad compact gate | run `GATE=core CROSS=all` on the release candidate | all-cross core: 354/354 positives, 0/536 false merges |

Focused release/candidate comparison:

```text
previous release: items=78, positive=0/24, false_merges=0/54
candidate:        items=78, positive=24/24, false_merges=0/54
delta:            +24 positive hits, +0 false merges
```

Final release Set-membership focused gate:

```text
GATE=focused PROPOSAL_PREFIX=axis_membership_set CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
items: 78
positive recall: 24/24
hard-negative false merges: 0/54
Raw nodes: 0/2132
```

Final release literal-membership compact gate:

```text
GATE=core AXIS=literal_collection_membership CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 95/357
positive recall: 32/32
hard-negative false merges: 0/63
Raw nodes: 0/2497
```

Final release compact all-cross gate:

```text
GATE=core CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 890/5301
positive recall: 354/354
hard-negative false merges: 0/536
Raw nodes: 0/34007
```

Assessment: the three-at-once loop is effective when the batch stays inside one operator
family and shares hard negatives. It also improved strictness: the Map/Set collision found
by the batch would have been easy to miss in a one-off focused positive test, but the
combined CLI and compact gates forced the detector to preserve the `Map.has` vs `Set.has`
semantic boundary.

## JavaScript/TypeScript Map construction defaults: loops 229-234

This loop moves from typed `Map` parameters into construction-proven literal maps for
JavaScript and TypeScript. It stays in one proof family: a `Map` receiver is strict only
when it is the built-in constructor applied to exact static entry pairs, or an immutable
local binding of that construction.

The three positive micro-frontiers were:

- inline `new Map([...]).get(key) ?? fallback`;
- local immutable `const lookup = new Map([...]); lookup.get(key) ?? fallback`;
- local immutable `Map.has(key) ? Map.get(key) : fallback`.

The hard-negative siblings cover wrong key, wrong default, wrong entry values, arbitrary
untyped `.get` receivers, and shadowed `Map` constructors.

| loop | pressure | change | measured result |
|---|---|---|---:|
| 229 | batched frontier selection | group three JS/TS constructed-`Map` default lookup surfaces under `axis_map_default_js_map_*` | focused corpus: 12 positives, 32 hard negatives |
| 230 | baseline measurement | scan the focused batch with the previous release detector | baseline: 0/12 positives, 0/32 false merges |
| 231 | detector strengthening | canonicalize exact `new Map` entry arrays to the existing literal-map `Seq(3)` value and fold `get ?? fallback` / `has-get` into `GetOrDefault` | candidate focused: 12/12 positives, 0/32 false merges |
| 232 | strictness hardening | add exact-safety gates for built-in `Map` construction and `.get/.has` only when the constructor is not shadowed | shadowed constructor and untyped receiver remain hard negatives |
| 233 | release focused/axis gates | build release and run focused JS/TS Map plus literal-map core gates | focused: 12/12, 0/32; literal-map core: 9/9, 0/24 |
| 234 | broad compact gate | run `GATE=core CROSS=all` on the release candidate | all-cross core: 360/360 positives, 0/552 false merges |

Focused release/candidate comparison:

```text
previous release: items=44, positive=0/12, false_merges=0/32
candidate:        items=44, positive=12/12, false_merges=0/32
delta:            +12 positive hits, +0 false merges
```

Final release constructed-Map focused gate:

```text
GATE=focused PROPOSAL_PREFIX=axis_map_default_js_map CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
items: 44
positive recall: 12/12
hard-negative false merges: 0/32
Raw nodes: 0/2092
```

Final release literal-map compact gate:

```text
GATE=core AXIS=literal_map_default_lookup CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 33/59
positive recall: 9/9
hard-negative false merges: 0/24
Raw nodes: 0/1420
```

Final release compact all-cross gate:

```text
GATE=core CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 912/5345
positive recall: 360/360
hard-negative false merges: 0/552
Raw nodes: 0/35053
```

Assessment: this is a real strict-frontier widening, not benchmark-only expansion. The
generator first exposed a clean 0/12 baseline gap, and the detector change reuses the
existing literal-map default primitive instead of inventing a parallel JavaScript-only
path. At this point, the remaining open JS/TS map-default work was object-literal/default
access plus imported or module-level construction where receiver identity and mutation
boundaries need stronger proof facts.

## JavaScript/TypeScript object-literal defaults: loops 235-240

This loop applies the faster micro-batch rule: add about three adjacent strict positives
inside one proof family, then validate them with the shared hard-negative envelope. The
chosen frontier was static object-literal default lookup guarded by own-property proof
facts. It is strict only when the receiver is a proven static object literal, the key and
fallback coordinates match, and the guard is a non-shadowed own-property builtin form.

The three positive micro-frontiers were:

- `Object.hasOwn(values, key) ? values[key] : fallback`;
- `Object.prototype.hasOwnProperty.call(values, key) ? values[key] : fallback`;
- `!Object.hasOwn(values, key) ? fallback : values[key]`.

The hard-negative siblings cover wrong key, wrong default, wrong entry values, unguarded
`values[key] ?? fallback`, prototype-aware `key in values`, direct
`values.hasOwnProperty(key)`, and shadowed `Object` bindings.

| loop | pressure | change | measured result |
|---|---|---|---:|
| 235 | batched frontier selection | group three JS/TS object-literal own-property default surfaces under `axis_map_default_js_object_*` | focused corpus: 12 positives, 40 hard negatives |
| 236 | baseline measurement | scan the focused batch with the previous release detector | baseline: 0/12 positives, 4/40 false merges; all false merges were `key in values` |
| 237 | detector strengthening | consume `own_property_guard` facts as literal-map default conditions only for proven static object literals | candidate focused: 12/12 positives, 0/40 false merges |
| 238 | strictness hardening | lower JS/TS `in` to a separate prototype-aware boolean value instead of map-key membership | `in` boundary false merges removed |
| 239 | release focused/axis gates | build release and run focused JS/TS object plus literal-map core gates | focused: 12/12, 0/40; literal-map core: 15/15, 0/44 |
| 240 | broad compact gate | run `GATE=core CROSS=all` on the release candidate | all-cross core: 366/366 positives, 0/572 false merges |

Focused release/candidate comparison:

```text
previous release: items=52, positive=0/12, false_merges=4/40
candidate:        items=52, positive=12/12, false_merges=0/40
delta:            +12 positive hits, -4 false merges
```

Final release object-literal focused gate:

```text
GATE=focused PROPOSAL_PREFIX=axis_map_default_js_object CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
items: 52
positive recall: 12/12
hard-negative false merges: 0/40
Raw nodes: 0/2184
```

Final release literal-map compact gate:

```text
GATE=core AXIS=literal_map_default_lookup CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 59/111
positive recall: 15/15
hard-negative false merges: 0/44
Raw nodes: 0/2512
```

Final release compact all-cross gate:

```text
GATE=core CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 938/5397
positive recall: 366/366
hard-negative false merges: 0/572
Raw nodes: 0/36145
```

Assessment: the batch-3 loop improved both recall and strictness. It opened three useful
JS/TS object-literal surfaces at once and removed an existing false merge where JS `in`
had been treated like strict map-key membership. This is the right acceleration pattern
when all positives share one proof primitive and the batch includes hard negatives that
attack the exact same primitive. The remaining map-default work is imported/module-level
construction, mutation/effect boundaries, and richer receiver proof facts beyond inline
or immutable local construction.

## Rust scalar numeric methods: loops 241-246

This loop uses the same batch-3 rule on the next compact numeric frontier: Rust scalar
methods `.abs()`, `.min()`, and `.max()`. The proof is intentionally typed. A Rust method
call is treated as a numeric intrinsic only when the receiver has an explicit numeric
parameter type fact; custom receiver methods with the same names remain hard boundaries.

The three positive micro-frontiers were:

- `value.abs()` converging with the absolute-value conditional;
- `left.min(right)` converging with the two-way minimum conditional;
- `left.max(right)` converging with the two-way maximum conditional.

The hard-negative siblings cover wrong value/right-hand coordinates, semantic mutations,
and custom Rust receiver methods named `abs`, `min`, or `max`.

| loop | pressure | change | measured result |
|---|---|---|---:|
| 241 | batched frontier selection | group three Rust numeric method surfaces under `axis_scalar_rust_*` | focused corpus: 30 positives, 90 hard negatives |
| 242 | baseline measurement | scan the focused batch with the previous release detector after marking Rust numeric as in-scope | baseline: 10/30 positives, 10/90 false merges |
| 243 | generator correction | route `axis_scalar_rust_min/max_*` proposals through min/max variants, not abs variants | true baseline established: `.abs` hit, `.min/.max` missed, custom `.abs` false-merged |
| 244 | detector strengthening | add explicit `ParamSemantic::Number` facts and fold Rust numeric `.abs/.min/.max` into existing `Abs`/`Min`/`Max` value nodes only for proven numeric receivers | candidate focused: 30/30 positives |
| 245 | strictness hardening | read Rust parameter type children directly and keep custom receiver methods outside numeric intrinsic proof; restore `&[T]` collection facts after narrowing type parsing | candidate focused: 30/30 positives, 0/90 false merges |
| 246 | release focused/core gates | build release and run focused Rust numeric, numeric core, and all-cross core gates | focused: 30/30, 0/90; numeric core: 55/55, 0/135; all-cross core: 396/396, 0/662 |

Focused release/candidate comparison:

```text
previous release: items=120, positive=10/30, false_merges=10/90
candidate:        items=120, positive=30/30, false_merges=0/90
delta:            +20 positive hits, -10 false merges
```

Final release Rust numeric focused gate:

```text
GATE=focused PROPOSAL_PREFIX=axis_scalar_rust CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
items: 120
positive recall: 30/30
hard-negative false merges: 0/90
Raw nodes: 0/4344
```

Final release numeric compact gate:

```text
GATE=core AXIS=numeric_minmax_abs CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 190/685
positive recall: 55/55
hard-negative false merges: 0/135
Raw nodes: 0/6803
```

Final release compact all-cross gate:

```text
GATE=core CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 1058/5517
positive recall: 396/396
hard-negative false merges: 0/662
Raw nodes: 0/40487
```

Assessment: this loop widened recall and strengthened strictness at the same time. The
baseline already merged Rust `.abs()`, but it missed `.min/.max()` and also accepted a
custom `.abs()` method. The final detector requires an explicit numeric receiver fact
before treating method names as numeric intrinsics, so real Rust scalar methods converge
without admitting arbitrary user-defined methods with the same spelling.

## Java literal collection factories: loops 247-252

This loop continues the batch-3 cadence on the highest-priority open membership frontier.
The chosen micro-frontiers are Java literal collection factories whose receiver identity
can be proven without trusting arbitrary `.contains` calls:

- `List.of("red", "blue").contains(value)`;
- `Set.of("red", "blue").contains(value)`;
- `Arrays.asList("red", "blue").contains(value)`.

The proof is strict only when the factory receiver is a standard Java free name and the
same file does not define a type with that name. Hard-negative siblings cover wrong
element coordinates, wrong literal item coordinates, local name shadowing, and same-file
type shadowing.

| loop | pressure | change | measured result |
|---|---|---|---:|
| 247 | batched frontier selection | group three Java literal factory membership surfaces under `axis_membership_java_*` | focused corpus: 27 positives, 108 hard negatives |
| 248 | baseline measurement | scan the focused batch with the previous release detector | baseline: 0/27 positives, 0/108 false merges |
| 249 | detector strengthening | treat Java `List.of`, `Set.of`, and `Arrays.asList` calls as proven literal collections when the receiver is an unshadowed standard free name | candidate focused: 27/27 positives |
| 250 | strict-safe gate alignment | mark the same Java factory calls and their `.contains` uses as exact-safe only under the factory proof conditions | CLI semantic scan now reports Java factories in the literal membership family |
| 251 | targeted regression tests | add value-graph and CLI tests for the three factories plus wrong-coordinate and shadow boundaries | targeted and full CLI/equivalence tests passed |
| 252 | release focused/core gates | build release and run focused Java factory, literal-membership core, and all-cross core gates | focused: 27/27, 0/108; literal core: 58/58, 0/170; all-cross core: 421/421, 0/769 |

Focused release/candidate comparison:

```text
previous release: items=135, positive=0/27, false_merges=0/108
candidate:        items=135, positive=27/27, false_merges=0/108
delta:            +27 positive hits, +0 false merges
```

Final release Java factory focused gate:

```text
GATE=focused PROPOSAL_PREFIX=axis_membership_java CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
items: 135
positive recall: 27/27
hard-negative false merges: 0/108
Raw nodes: 0/4194
```

Final release literal-membership compact gate:

```text
GATE=core AXIS=literal_collection_membership CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 228/492
positive recall: 58/58
hard-negative false merges: 0/170
Raw nodes: 0/6612
```

Final release compact all-cross gate:

```text
GATE=core CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 1190/5652
positive recall: 421/421
hard-negative false merges: 0/769
Raw nodes: 0/44585
```

Assessment: this is a real strict-frontier widening inside `membership_contains`.
The previous detector had no recall for Java literal collection factories. The final
detector maps the factories into the existing literal collection membership value only
under narrow receiver proof conditions, and exact semantic reporting uses the same proof
for its `exact_safe` gate. The shadow boundaries keep the change from becoming a broad
method-name or class-name heuristic.

## Java literal map factories: loops 253-258

This loop switches the frontier cadence to a batch-3 unit on `literal_map_default_lookup`.
The chosen micro-frontiers are Java literal map factories whose receiver identity and
entries can be proven without trusting arbitrary `.getOrDefault` receivers:

- `Map.of("red", 1, "blue", 2).getOrDefault(key, 0)`;
- `Map.ofEntries(Map.entry("red", 1), Map.entry("blue", 2)).getOrDefault(key, 0)`;
- a local immutable `Map.of(...)` binding followed by `lookup.getOrDefault(key, 0)`.

The proof is strict only when `Map` is the unshadowed Java standard free name. Hard
negatives cover wrong key, wrong fallback, wrong literal value, a local value named
`Map`, and a same-file `class Map` type shadow.

| loop | pressure | change | measured result |
|---|---|---|---:|
| 253 | batched frontier selection | group three Java literal map factory default surfaces under `axis_map_default_java_map_*` | focused corpus: 6 positives, 16 hard negatives |
| 254 | baseline measurement | scan the focused batch with the previous release detector after marking Java literal-map factories in-scope | baseline: 0/6 positives, 0/16 false merges |
| 255 | generator hardening | add wrong-coordinate, local-shadow, and same-file-type-shadow boundaries for the Java factory batch | focused generator emits 22 items with 0 Raw nodes |
| 256 | detector strengthening | canonicalize Java `Map.of` and `Map.ofEntries(Map.entry(...))` into the existing literal-map value coordinate, and pass `GetOrDefault` map operands through proven map canonicalization | targeted value-graph test passes |
| 257 | strict-safe gate alignment | mark the same Java factory calls and their `Map.entry` children as exact-safe only under the unshadowed standard-name proof | CLI semantic scan reports Java factory positives in the literal map-default family |
| 258 | release focused/core gates | build release and run focused Java factory, literal-map-default core, and all-cross core gates | focused: 6/6, 0/16; literal core: 21/21, 0/60; all-cross core: 427/427, 0/785 |

Focused release/candidate comparison:

```text
previous release:  items=22, positive=0/6, false_merges=0/16
candidate release: items=22, positive=6/6, false_merges=0/16
delta:             +6 positive hits, +0 false merges
```

Final release focused gate:

```text
GATE=focused PROPOSAL_PREFIX=axis_map_default_java_map CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
items: 22
positive recall: 6/6
hard-negative false merges: 0/16
Raw nodes: 0/920
```

Final release literal-map-default compact gate:

```text
GATE=core AXIS=literal_map_default_lookup CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 81/133
positive recall: 21/21
hard-negative false merges: 0/60
Raw nodes: 0/3432
```

Final release compact all-cross gate:

```text
GATE=core CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 1212/5674
positive recall: 427/427
hard-negative false merges: 0/785
Raw nodes: 0/45505
```

Assessment: this is a real strict-frontier widening inside `map_default_lookup`, not
only a new benchmark slice. The previous detector already handled Java typed map
parameters but had no proof for literal Java map factory receivers. The final path maps
Java factories into the same canonical literal-map coordinate used by Python/Ruby and
JS/TS, while the exact-safe gate repeats the same standard-name and shadow checks before
semantic mode can report a clone.

## Module-level map default bindings: loops 259-264

This loop keeps the batch-3 cadence on `literal_map_default_lookup`, but moves from
inline/local construction to module-level immutable bindings. The chosen micro-frontiers
are:

- JavaScript `const LOOKUP = new Map([...])` followed by `LOOKUP.get(key) ?? 0`;
- TypeScript `const LOOKUP = new Map<string, number>(...)` followed by `LOOKUP.get(key) ?? 0`;
- Java `static final Map<String, Integer> LOOKUP = Map.of(...)` followed by
  `LOOKUP.getOrDefault(key, 0)`.

The proof is strict only when the module binding is assigned once, its initializer is a
proven map construction/factory, and the same file does not use that binding as the
receiver of mutating map operations such as `set`, `delete`, `clear`, `put`, `remove`,
`compute`, or `merge`. Hard negatives cover wrong key, wrong fallback, wrong map value,
post-construction mutation, and shadowed `Map` constructor/type boundaries.

| loop | pressure | change | measured result |
|---|---|---|---:|
| 259 | batched frontier selection | group JS module `Map`, TS module `Map`, and Java static-final `Map.of` under `axis_map_default_module_*` | focused corpus: 6 positives, 34 hard negatives |
| 260 | baseline measurement | scan the focused batch with the previous release detector | baseline: 0/6 positives, 0/34 false merges, 0 Raw nodes |
| 261 | generator adversary | add wrong-coordinate, mutation, and shadow boundaries for module-level map bindings | focused generator emits 40 items across Python/Ruby refs and JS/TS/Java targets |
| 262 | detector strengthening | seed module/global bindings with canonical proven map values when the initializer is a proven map constructor/factory and the binding is not mutated | candidate focused: 6/6 positives |
| 263 | strict-safe gate alignment | mark the same module map bindings as exact-safe only under the same initializer and mutation-exclusion proof | targeted value-graph and CLI semantic tests passed |
| 264 | release focused/core gates | build release and run focused module-map, literal-map-default core, and all-cross core gates | focused: 6/6, 0/34; literal core: 27/27, 0/80; all-cross core: 433/433, 0/805 |

Focused release/candidate comparison:

```text
previous release:  items=40, positive=0/6, false_merges=0/34
candidate release: items=40, positive=6/6, false_merges=0/34
delta:             +6 positive hits, +0 false merges
```

Final release focused gate:

```text
GATE=focused PROPOSAL_PREFIX=axis_map_default_module CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
items: 40
positive recall: 6/6
hard-negative false merges: 0/34
Raw nodes: 0/1874
```

Final release literal-map-default compact gate:

```text
GATE=core AXIS=literal_map_default_lookup CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 107/173
positive recall: 27/27
hard-negative false merges: 0/80
Raw nodes: 0/4637
```

Final release compact all-cross gate:

```text
GATE=core CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 1238/5714
positive recall: 433/433
hard-negative false merges: 0/805
Raw nodes: 0/46710
```

Assessment: this widens the strict frontier while also strengthening the loop's
adversary. Opening module-level `Map` construction would be unsound if `const` were
mistaken for deep immutability, so the detector now requires both a proven map
initializer and a whole-file exclusion of mutating receiver calls before exposing the
binding through `global_env` or `exact_safe` facts.

## Module-level collection membership bindings: loops 265-270

This loop applies the accelerated batch-3 cadence to `literal_collection_membership`.
Instead of opening one surface at a time, the batch opens three adjacent strict positives
that share the same proof channel:

- JavaScript `const VALUES = new Set([...])` followed by `VALUES.has(value)`;
- TypeScript `const VALUES = new Set<string>(...)` followed by `VALUES.has(value)`;
- Java `static final List<String> VALUES = List.of(...)` followed by
  `VALUES.contains(value)`.

The strict proof is intentionally the same as the module-map proof shape from loops
259-264: the module/static binding must be assigned once, its initializer must be a
proven standard collection construction/factory, and the same file must not mutate the
binding through collection mutators such as `add`, `delete`, `clear`, `put`, `remove`,
`push`, `sort`, or `splice`. Hard negatives cover wrong element, wrong collection,
post-construction mutation, and shadowed `Set`/Java `List` boundaries.

| loop | pressure | change | measured result |
|---|---|---|---:|
| 265 | accelerated frontier selection | group JS module `Set`, TS module `Set`, and Java static-final `List.of` under `axis_membership_module_*` | focused corpus: 6 positives, 28 hard negatives |
| 266 | baseline measurement | scan the focused batch with the previous release detector | baseline: 0/6 positives, 0/28 false merges, 0 Raw nodes |
| 267 | generator adversary | add wrong-coordinate, mutation, and shadow boundaries for module-level collection membership | focused generator emits 34 items across Python/Ruby refs and JS/TS/Java targets |
| 268 | detector strengthening | seed module/global bindings with canonical proven collection values when the initializer is a proven collection constructor/factory and the binding is not mutated | candidate focused: 6/6 positives |
| 269 | strict-safe gate alignment | mark the same module collection bindings as exact-safe only under the same initializer and mutation-exclusion proof | targeted value-graph and CLI semantic tests passed |
| 270 | release focused/core gates | build release and run focused module-membership, literal-membership core, and all-cross core gates | focused: 6/6, 0/28; literal core: 64/64, 0/187; all-cross core: 439/439, 0/822 |

Focused release/candidate comparison:

```text
previous release:  items=34, positive=0/6, false_merges=0/28
candidate release: items=34, positive=6/6, false_merges=0/28
delta:             +6 positive hits, +0 false merges
```

Final release focused gate:

```text
GATE=focused PROPOSAL_PREFIX=axis_membership_module CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
items: 34
positive recall: 6/6
hard-negative false merges: 0/28
Raw nodes: 0/1109
```

Final release literal-membership compact gate:

```text
GATE=core AXIS=literal_collection_membership CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 251/526
positive recall: 64/64
hard-negative false merges: 0/187
Raw nodes: 0/7346
```

Final release compact all-cross gate:

```text
GATE=core CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 1261/5748
positive recall: 439/439
hard-negative false merges: 0/822
Raw nodes: 0/47444
```

Assessment: the batch-3 loop materially widened the strict detector, not only the
benchmark. The previous release missed every module-level collection membership
positive, while the candidate proves all six focused cross-language pairs and preserves
all mutation, coordinate, and shadowing boundaries. This also validates the faster loop
cadence: three adjacent positives can be opened together when they share one proof rule
and the generator attacks that rule with batch-level hard negatives before release gates
are run.

## Go package slice membership bindings: loops 271-276

This loop keeps the accelerated batch-3 cadence and applies it to Go
`slices.Contains` over package-level slice bindings. The batch opens three adjacent
strict positives that share one proof channel:

- default imported `slices.Contains(values, value)` where `values` is a package-level
  literal slice;
- aliased imported `sl "slices"` followed by `sl.Contains(values, value)`;
- a package-level literal slice whose first element is derived from an immutable
  `const`.

The strict proof is narrower than a name check. The `Contains` receiver must resolve to
the static import namespace for Go's `slices` package, and the collection argument must
evaluate to a proven package-level literal/composite collection. Hard negatives cover
wrong element, wrong collection, append-expanded construction, and a local value named
`slices` with a `Contains` method. These boundaries are the reason the batch can move
faster without weakening strict Type-4 semantics.

| loop | pressure | change | measured result |
|---|---|---|---:|
| 271 | accelerated frontier selection | group default import, aliased import, and const-derived package slice under `axis_membership_go_slices_*` | focused corpus: 6 positives, 14 hard negatives |
| 272 | baseline measurement | scan the focused batch with the previous release detector | baseline: 0/6 positives, 0/14 false merges |
| 273 | generator adversary | add wrong-element, wrong-collection, append-expanded, and unimported-receiver boundaries | focused generator emits 20 items across Python/Ruby references and Go targets |
| 274 | detector strengthening | canonicalize Go composite literals as proven collections and allow package-level proven slice values in `slices.Contains` only when the receiver has an import-namespace proof | candidate focused: 6/6 positives, 0/14 false merges |
| 275 | strict-safe gate alignment | keep the exact-safe fact tied to the same proven collection value and import-coordinate receiver proof | CLI semantic and equivalence tests passed |
| 276 | release focused/core gates | build release and run focused Go-slices, literal-membership core, and all-cross core gates | focused: 6/6, 0/14; literal core: 70/70, 0/201; all-cross core: 445/445, 0/836 |

Focused release/candidate comparison:

```text
previous release:  items=20, positive=0/6, false_merges=0/14
candidate release: items=20, positive=6/6, false_merges=0/14
delta:             +6 positive hits, +0 false merges
```

Final release focused gate:

```text
GATE=focused PROPOSAL_PREFIX=axis_membership_go_slices CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
items: 20
positive recall: 6/6
hard-negative false merges: 0/14
Raw nodes: 0/698
```

Final release literal-membership compact gate:

```text
GATE=core AXIS=literal_collection_membership CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 271/546
positive recall: 70/70
hard-negative false merges: 0/201
Raw nodes: 0/8048
```

Final release compact all-cross gate:

```text
GATE=core CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 1281/5768
positive recall: 445/445
hard-negative false merges: 0/836
Raw nodes: 0/48142
```

Assessment: this is another real detector widening rather than benchmark-only growth.
The previous detector handled inline Go slice literals and typed slice parameters, but it
missed package-level proven slice bindings and aliased import coordinates. The new proof
uses the same strict shape across all three positives while preserving the adversarial
boundaries. The batch-3 cadence should remain the default for future loops, with one
constraint: batch together only positives that share a single proof invariant, then add
shared hard negatives that attack that invariant before running focused and compact core
gates.

Open next frontier: post-construction Go package mutation such as `values =
append(values, "green")` inside `init` is still a better adversary than the current
append-expanded initializer boundary. Opening or rejecting that safely needs a
scope-aware global mutation fact before alpha-normalized names lose the original package
binding coordinate.

## Rust local literal collection membership bindings: loops 277-282

This loop keeps the batch-3 cadence on `literal_collection_membership` and targets a
real Rust miss from the `rust_contains_ambiguous` frontier. The value graph already knew
that a local literal collection binding could be inlined into the same membership value,
but `semantic` scan did not report it because strict exact-safe rejected the normalized
`seq.contains(value)` receiver.

The batch opens three adjacent strict positives:

- `let values = ["red", "blue"]; values.contains(&value)`;
- `let values: [&str; 2] = ["red", "blue"]; values.contains(&value)`;
- `let values: &[&str] = &["red", "blue"]; values.contains(&value)`.

The proof is intentionally narrow: after normalization, the method receiver must be a
literal collection sequence, not an arbitrary value with a custom `contains` method.
Hard negatives cover wrong element, wrong collection, a locally mutated `Vec`, and a
custom receiver implementing `contains`.

| loop | pressure | change | measured result |
|---|---|---|---:|
| 277 | accelerated frontier selection | group Rust local array, typed local array, and local slice-reference membership under `axis_membership_rust_local_*` | focused corpus: 6 positives, 14 hard negatives |
| 278 | baseline measurement | scan the focused batch with the previous release detector | baseline: 0/6 positives, 0/14 false merges |
| 279 | generator adversary | add wrong-coordinate, mutated-vector, and custom-receiver boundaries | focused generator emits 20 items across Python/Ruby references and Rust targets |
| 280 | detector strengthening | allow strict exact-safe `contains` calls only when the normalized receiver is a literal membership collection sequence | candidate focused: 6/6 positives, 0/14 false merges |
| 281 | strict-safe tests | extend semantic CLI and value-graph equivalence tests with Rust local positives and mutation/custom boundaries | targeted tests passed |
| 282 | release focused/core gates | build release and run focused Rust-local, literal-membership core, and all-cross core gates | focused: 6/6, 0/14; literal core: 76/76, 0/215; all-cross core: 451/451, 0/850 |

Focused release/candidate comparison:

```text
previous release:  items=20, positive=0/6, false_merges=0/14
candidate release: items=20, positive=6/6, false_merges=0/14
delta:             +6 positive hits, +0 false merges
```

Final release focused gate:

```text
GATE=focused PROPOSAL_PREFIX=axis_membership_rust_local CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
items: 20
positive recall: 6/6
hard-negative false merges: 0/14
Raw nodes: 0/554
```

Final release literal-membership compact gate:

```text
GATE=core AXIS=literal_collection_membership CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 291/566
positive recall: 76/76
hard-negative false merges: 0/215
Raw nodes: 0/8602
```

Final release compact all-cross gate:

```text
GATE=core CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 1301/5788
positive recall: 451/451
hard-negative false merges: 0/850
Raw nodes: 0/48696
```

Assessment: this widens the strict detector in the scan path, not only in the value
graph. The previous release already had the semantic value available but refused to
promote the local Rust form into exact semantic reports. The new exact-safe rule is
minimal: it opens method `contains` only when the normalized receiver is a literal
collection sequence, so custom receiver and mutated-vector boundaries remain outside
strict Type-4.

## Rust std map factory default lookups: loops 283-288

This loop shifts from `membership_contains` to the second-ranked frontier,
`map_default_lookup`, while preserving the batch-3 cadence. The target is the literal
map-default sub-axis for Rust std map factories. The previous detector handled typed
Rust map fallback APIs, but it did not prove inline/local map literals built through
fully-qualified std factories.

The batch opens three adjacent strict positives:

- `std::collections::HashMap::from([("red", 1), ("blue", 2)]).get(key).unwrap_or(&0)`;
- `std::collections::BTreeMap::from([("red", 1), ("blue", 2)]).get(key).unwrap_or(&0)`;
- a local binding initialized from `std::collections::HashMap::from([...])` followed by
  the same `get(...).unwrap_or(...)` default.

The proof is intentionally tied to a fully-qualified standard-library factory and a
literal array of tuple entries. Hard negatives cover wrong key, wrong fallback, wrong
map value, and a local map mutated after construction.

| loop | pressure | change | measured result |
|---|---|---|---:|
| 283 | frontier selection | move to `literal_map_default_lookup` and group Rust `HashMap::from`, `BTreeMap::from`, and local `HashMap::from` binding under `axis_map_default_rust_*` | focused corpus: 6 positives, 14 hard negatives |
| 284 | baseline measurement | scan the focused batch with the previous release detector | baseline: 0/6 positives, 0/14 false merges |
| 285 | generator/capability adversary | mark Rust literal-map-default as partial and add wrong-key/default/map plus mutation boundaries | focused generator emits 20 items across Python/Ruby references and Rust targets |
| 286 | detector strengthening | canonicalize Rust std map factory calls into literal map entries and mark the same factories strict exact-safe | candidate focused: 6/6 positives, 0/14 false merges |
| 287 | strict-safe tests | extend value-graph equivalence and CLI semantic tests with Rust std map factories and mutation boundaries | targeted tests passed |
| 288 | release focused/core gates | build release and run focused Rust-map, literal-map-default core, and all-cross core gates | focused: 6/6, 0/14; literal-map core: 33/33, 0/94; all-cross core: 457/457, 0/864 |

Focused release/candidate comparison:

```text
previous release:  items=20, positive=0/6, false_merges=0/14
candidate release: items=20, positive=6/6, false_merges=0/14
delta:             +6 positive hits, +0 false merges
```

Final release focused gate:

```text
GATE=focused PROPOSAL_PREFIX=axis_map_default_rust CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
items: 20
positive recall: 6/6
hard-negative false merges: 0/14
Raw nodes: 0/790
```

Final release literal-map-default compact gate:

```text
GATE=core AXIS=literal_map_default_lookup CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 127/193
positive recall: 33/33
hard-negative false merges: 0/94
Raw nodes: 0/5427
```

Final release compact all-cross gate:

```text
GATE=core CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 1321/5808
positive recall: 457/457
hard-negative false merges: 0/864
Raw nodes: 0/49488
```

Assessment: this is a real strict frontier expansion on a different high-priority axis.
The new proof shares the existing literal-map coordinate model instead of adding a Rust
special case at the report layer: factory entries become canonical map entries in the
value graph, and exact-safe accepts only the same fully-qualified std factories over
literal tuple arrays. Mutation remains outside the exact clone family because the
mutating call contributes a distinct effect/value footprint.

## Named-vs-namespace import coordinates: loops 289-294

This loop deliberately switches away from the tempting but unsound imported-map-default
shortcut. A static import proves an exported coordinate, but it does not by itself prove
that an imported object has ordinary `Map` semantics. The strict frontier opened here is
smaller and sound: a named import and a namespace member import of the same module/export
coordinate denote the same imported value.

The batch opens three adjacent same-invariant positives through generated surfaces:

- JavaScript named import `helper` versus namespace member `mathOps.helper`;
- TypeScript named import versus namespace member over the same export coordinate;
- Python `from shared_math import helper` versus `import shared_math as math_ops` followed
  by `math_ops.helper`.

The hard boundary is a neighboring namespace member such as `mathOps.otherHelper` or
`math_ops.other_helper`. Same module is not enough; the exported member coordinate must
also match.

| loop | pressure | change | measured result |
|---|---|---|---:|
| 289 | frontier selection | select named-vs-namespace import member identity under `axis_import_namespace_member_*` | focused corpus: 6 positives, 12 hard negatives |
| 290 | baseline measurement | scan the focused batch with the previous release detector | baseline: 0/6 positives, 0/12 false merges |
| 291 | generator adversary | add `axis_import_namespace_member_identity` and `axis_import_namespace_member_wrong_boundary` for JS-like/Python surfaces | wrong member is held out as an import-member boundary |
| 292 | detector strengthening | canonicalize `namespace.member` over a proven static namespace import to the same `import_binding(module, member)` coordinate used by named imports | candidate focused: 6/6 positives, 0/12 false merges |
| 293 | strict regression tests | add value-graph and CLI semantic tests for JS/TS/Python named-vs-namespace import coordinates | targeted tests passed |
| 294 | release focused/core gates | build release and run focused import, import core, and all-cross core gates | focused 6/6, 0/12; import core 9/9, 0/12; all-cross 457/457, 0/866 |

Focused release/candidate comparison:

```text
previous release:  items=18, positive=0/6, false_merges=0/12
candidate release: items=18, positive=6/6, false_merges=0/12
delta:             +6 positive hits, +0 false merges
```

Final release focused gate:

```text
GATE=focused PROPOSAL_PREFIX=axis_import_namespace_member CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
items: 18
positive recall: 6/6
hard-negative false merges: 0/12
Raw nodes: 0/540
```

Final release import core gate:

```text
GATE=core AXIS=import_identity CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 21/112
positive recall: 9/9
hard-negative false merges: 0/12
Raw nodes: 0/646
```

Final release compact all-cross gate:

```text
GATE=core CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 1323/5826
positive recall: 457/457
hard-negative false merges: 0/866
Raw nodes: 0/49548
```

Assessment: this is a smaller change than cross-file value resolution, but it improves
the proof system in the right place. Namespace imports already prove a module coordinate;
the member access now contributes the exported member coordinate, making it equivalent to
the corresponding named import. This helps later import-aware detector work without
guessing behavior of arbitrary imported objects.

## Static array existential membership: loops 295-300

This is the first accelerated batch-3 macro-loop. Instead of opening one surface at a
time, the loop adds one strict positive proposal and two hard boundaries together, then
validates the batch as a single focused frontier. The target remains the highest-ranked
`membership_contains` frontier, but the proof is deliberately narrow: a static literal
JS-like array existential predicate
`["red", "blue"].some(item => item === value)` denotes the same element-in-literal-set
coordinate as `includes`, Python `in`, Ruby `include?`, and the existing strict
collection-membership family.

The batch opens three adjacent proposal IDs:

- `axis_membership_array_some_identity`;
- `axis_membership_array_some_wrong_element_boundary`;
- `axis_membership_array_some_wrong_collection_boundary`.

The detector rule is source-gated. It only rewrites `Any(Elem(collection) == value)` to
`value in collection` when the original collection expression is a direct non-float
static literal sequence. This keeps dynamic receivers, non-literal collections, and the
`NaN`/SameValueZero edge outside the strict frontier.

| loop | pressure | change | measured result |
|---|---|---|---:|
| 295 | batch frontier selection | group static JS-like array `.some` membership plus wrong-element/wrong-collection boundaries under `axis_membership_array_some_*` | focused corpus: 18 positives, 54 hard negatives |
| 296 | baseline measurement | scan the focused batch with the previous release detector | baseline: 0/18 positives, 0/54 false merges |
| 297 | generator adversary | add cross-surface Python/Ruby/JS/TS references against JS/TS/Vue/Svelte/HTML `.some` forms | semantic mutation and hard boundaries both held out |
| 298 | detector strengthening | canonicalize source-gated `Any(Elem(static literal collection) == value)` to `In(value, collection)` | candidate focused: 18/18 positives, 0/54 false merges |
| 299 | strict regression tests | extend value-graph and CLI membership tests, including a `NaN`/free-name boundary | targeted tests passed |
| 300 | release focused/core gates | build release and run focused array-some, membership core, and all-cross core gates | focused 18/18, 0/54; membership core 80/80, 0/228; all-cross 461/461, 0/879 |

Focused release/candidate comparison:

```text
previous release:  items=72, positive=0/18, false_merges=0/54
candidate release: items=72, positive=18/18, false_merges=0/54
delta:             +18 positive hits, +0 false merges
```

Final release focused gate:

```text
GATE=focused PROPOSAL_PREFIX=axis_membership_array_some CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
items: 72
positive recall: 18/18
hard-negative false merges: 0/54
Raw nodes: 0/2140
```

Final release literal-membership core gate:

```text
GATE=core AXIS=literal_collection_membership CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 308/638
positive recall: 80/80
hard-negative false merges: 0/228
Raw nodes: 0/9111
```

Final release compact all-cross gate:

```text
GATE=core CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 1340/5898
positive recall: 461/461
hard-negative false merges: 0/879
Raw nodes: 0/50054
```

Assessment: the batch-3 cadence worked for this frontier. The three generated pressures
shared one proof kernel, so the implementation remained small while the focused corpus
grew enough to catch over-generalization. The key guard is the source-level literal
check: without it, JavaScript `includes`/`some` can diverge on `NaN`; with it, static
string/int/bool/null literal membership remains an exact Type-4 claim.

## Static array absence membership: loops 301-306

This macro-loop keeps the batch-3 cadence on the same high-frequency membership frontier,
but opens the negated coordinate rather than another positive membership spelling. A
static literal JS-like array predicate
`["red", "blue"].every(item => item !== value)` is exact-equivalent to `value not in
["red", "blue"]` and `!["red", "blue"].includes(value)` when the collection is a direct
non-float literal sequence.

The batch opens three adjacent proposal IDs:

- `axis_membership_array_every_absence_identity`;
- `axis_membership_array_every_wrong_element_boundary`;
- `axis_membership_array_every_wrong_collection_boundary`.

The proof is the dual of the previous `some` rule: source-gated
`All(Elem(collection) != value)` canonicalizes to `!In(value, collection)`. It is not a
general `.every` theorem. Dynamic receivers and the JavaScript `NaN` edge stay outside
strict Type-4 because `includes` uses SameValueZero while `!==` does not.

| loop | pressure | change | measured result |
|---|---|---|---:|
| 301 | batch frontier selection | group static JS-like array `.every` absence plus wrong-element/wrong-collection boundaries under `axis_membership_array_every_*` | focused corpus: 18 positives, 54 hard negatives |
| 302 | baseline measurement | scan the focused batch with the previous release detector | baseline: 0/18 positives, 0/54 false merges |
| 303 | generator adversary | add cross-surface Python/Ruby/JS/TS negated-membership references against JS/TS/Vue/Svelte/HTML `.every` forms | semantic mutation and hard boundaries both held out |
| 304 | detector strengthening | canonicalize source-gated `All(Elem(static literal collection) != value)` to `!In(value, collection)` | candidate focused: 18/18 positives, 0/54 false merges |
| 305 | strict regression tests | extend value-graph and CLI membership tests with negated-membership family and `NaN` boundary | targeted tests passed |
| 306 | release focused/core gates | build release and run focused array-every, membership core, and all-cross core gates | focused 18/18, 0/54; membership core 83/83, 0/240; all-cross 466/466, 0/891 |

Focused release/candidate comparison:

```text
previous release:  items=72, positive=0/18, false_merges=0/54
candidate release: items=72, positive=18/18, false_merges=0/54
delta:             +18 positive hits, +0 false merges
```

Final release focused gate:

```text
GATE=focused PROPOSAL_PREFIX=axis_membership_array_every CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
items: 72
positive recall: 18/18
hard-negative false merges: 0/54
Raw nodes: 0/2212
```

Final release literal-membership core gate:

```text
GATE=core AXIS=literal_collection_membership CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 323/710
positive recall: 83/83
hard-negative false merges: 0/240
Raw nodes: 0/9565
```

Final release compact all-cross gate:

```text
GATE=core CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 1357/5970
positive recall: 466/466
hard-negative false merges: 0/891
Raw nodes: 0/50572
```

Assessment: this confirms the batch-3 loop can open a semantic dual without loosening
strictness. The generator made the wrong-element and wrong-collection attacks explicit,
and the detector rule stayed anchored to the same literal-source guard as `array.some`.
The result extends the exact Type-4 frontier for absence predicates while keeping
membership, non-membership, and JavaScript `NaN` behavior separated.

## Static array indexOf membership: loops 307-312

This macro-loop keeps the accelerated batch-3 cadence on the same membership frontier,
but targets index-producing APIs only when their result is consumed as a membership
predicate. A static literal JS-like array comparison such as
`["red", "blue"].indexOf(value) !== -1`, `>= 0`, or `> -1` is exact-equivalent to
`value in ["red", "blue"]` only when the receiver is a direct non-float literal sequence.

The batch opens three adjacent proposal IDs:

- `axis_membership_array_indexof_identity`;
- `axis_membership_array_indexof_wrong_element_boundary`;
- `axis_membership_array_indexof_wrong_collection_boundary`.

The proof is deliberately at the comparison level, not the call level. The detector does
not rewrite `indexOf(value)` itself; it rewrites only proven membership comparisons over
the call result to `In(value, collection)`. The strict safety gate mirrors the same source
proof, so a generic `indexOf` call remains outside exact semantic mode. This preserves
dynamic receivers, raw index-valued uses, and JavaScript `NaN` behavior.

| loop | pressure | change | measured result |
|---|---|---|---:|
| 307 | batch frontier selection | group static JS-like `.indexOf(...)` membership comparisons plus wrong-element/wrong-collection boundaries under `axis_membership_array_indexof_*` | focused corpus: 18 positives, 54 hard negatives |
| 308 | baseline measurement | scan the focused batch with the previous release detector | baseline: 0/18 positives, 0/54 false merges |
| 309 | generator adversary | add cross-surface Python/Ruby/JS/TS references against JS/TS/Vue/Svelte/HTML `indexOf` comparison forms | `!== -1`, `>= 0`, and `> -1` spellings represented |
| 310 | detector strengthening | canonicalize source-gated static `indexOf` membership comparisons to `In(element, collection)` and mark the same proof exact-safe | candidate focused: 18/18 positives, 0/54 false merges |
| 311 | strict regression tests | extend value-graph and CLI membership tests with spelling, reversed-comparison, raw-index, wrong-coordinate, and `NaN` boundaries | targeted tests passed |
| 312 | release focused/core gates | build release and run focused indexOf, membership core, and all-cross core gates | focused 18/18, 0/54; membership core 87/87, 0/252; all-cross 469/469, 0/903 |

Focused release/candidate comparison:

```text
previous release:  items=72, positive=0/18, false_merges=0/54
candidate release: items=72, positive=18/18, false_merges=0/54
delta:             +18 positive hits, +0 false merges
```

Final release focused gate:

```text
GATE=focused PROPOSAL_PREFIX=axis_membership_array_indexof CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
items: 72
positive recall: 18/18
hard-negative false merges: 0/54
Raw nodes: 0/1880
```

Final release literal-membership core gate:

```text
GATE=core AXIS=literal_collection_membership CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 339/782
positive recall: 87/87
hard-negative false merges: 0/252
Raw nodes: 0/9983
```

Final release compact all-cross gate:

```text
GATE=core CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 1372/6042
positive recall: 469/469
hard-negative false merges: 0/903
Raw nodes: 0/50964
```

Assessment: this batch widened the strict frontier without turning `indexOf` into a loose
membership-like name heuristic. The important refinement was updating both value
canonicalization and exact-safety gating: the return value fingerprint converged first,
but semantic mode still correctly rejected the unit until the same proof was reflected in
the safety walker. That coupling should remain a checklist item for future strict
frontier loops.

## Static array findIndex membership: loops 313-318

This macro-loop keeps the same index-producing membership frontier, but adds a lambda
predicate step. A static literal JS-like array comparison such as
`["red", "blue"].findIndex(item => item === value) !== -1`, `>= 0`, or `> -1` is
exact-equivalent to `value in ["red", "blue"]` only when the receiver is a direct
non-float literal sequence and the lambda proves equality between the iterated element
and the searched coordinate.

The batch opens three adjacent proposal IDs:

- `axis_membership_array_findindex_identity`;
- `axis_membership_array_findindex_wrong_element_boundary`;
- `axis_membership_array_findindex_wrong_collection_boundary`.

The proof extends the previous `indexOf` comparison rule, but does not treat arbitrary
`findIndex` calls as membership. The detector first checks the membership threshold
comparison, then evaluates the lambda over `Elem(static literal collection)`, and only
canonicalizes when the predicate is a literal-membership equality. The exact-safety walker
mirrors this with a structural lambda check: first return must compare the lambda
parameter to a safe searched element with `Eq`.

| loop | pressure | change | measured result |
|---|---|---|---:|
| 313 | batch frontier selection | group static JS-like `.findIndex(...)` membership comparisons plus wrong-element/wrong-collection boundaries under `axis_membership_array_findindex_*` | focused corpus: 18 positives, 54 hard negatives |
| 314 | baseline measurement | scan the focused batch with the previous release detector | baseline: 0/18 positives, 0/54 false merges |
| 315 | generator adversary | add cross-surface Python/Ruby/JS/TS references against JS/TS/Vue/Svelte/HTML `findIndex` comparison forms | `!== -1`, `>= 0`, and `> -1` spellings represented |
| 316 | detector strengthening | canonicalize source-gated static `findIndex` lambda membership comparisons to `In(element, collection)` and mark the same proof exact-safe | candidate focused: 18/18 positives, 0/54 false merges |
| 317 | strict regression tests | extend value-graph and CLI membership tests with spelling, reversed-comparison, raw-index, wrong-coordinate, and `NaN` boundaries | targeted tests passed |
| 318 | release focused/core gates | build release and run focused findIndex, membership core, and all-cross core gates | focused 18/18, 0/54; membership core 91/91, 0/264; all-cross 473/473, 0/915 |

Focused release/candidate comparison:

```text
previous release:  items=72, positive=0/18, false_merges=0/54
candidate release: items=72, positive=18/18, false_merges=0/54
delta:             +18 positive hits, +0 false merges
```

Final release focused gate:

```text
GATE=focused PROPOSAL_PREFIX=axis_membership_array_findindex CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
items: 72
positive recall: 18/18
hard-negative false merges: 0/54
Raw nodes: 0/2312
```

Final release literal-membership core gate:

```text
GATE=core AXIS=literal_collection_membership CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 355/854
positive recall: 91/91
hard-negative false merges: 0/264
Raw nodes: 0/10496
```

Final release compact all-cross gate:

```text
GATE=core CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 1388/6114
positive recall: 473/473
hard-negative false merges: 0/915
Raw nodes: 0/51477
```

Assessment: this is a real frontier expansion beyond method-name normalization because
the proof crosses into a callback. The useful pattern is still narrow: source literal
receiver, non-float elements, proven equality predicate, and membership-threshold
comparison. That keeps raw index results, non-membership predicates, dynamic receivers,
and `NaN`/SameValueZero differences outside strict Type-4.

## Static array filter-length membership: loops 319-324

This macro-loop uses a three-variant batch instead of one spelling at a time. A static
literal JS-like array filter count such as
`["red", "blue"].filter(item => item === value).length !== 0`, `.length > 0`,
`0 < .length`, or `.length >= 1` is exact-equivalent to `value in ["red", "blue"]`
only when the receiver is a direct non-float literal sequence, the callback proves
equality between the iterated element and the searched coordinate, and the length
comparison is a non-empty threshold.

The batch opens three adjacent proposal IDs:

- `axis_membership_array_filter_length_identity`;
- `axis_membership_array_filter_length_wrong_element_boundary`;
- `axis_membership_array_filter_length_wrong_collection_boundary`.

The detector canonicalizes only `Len(Filter(static literal collection, equality
lambda))` non-empty comparisons to `In(element, collection)`. Raw filtered lengths,
zero-length absence checks, wrong element/collection boundaries, and float-sensitive
literal collections stay distinct. A tempting adjacent family,
`array.find(...) !== undefined`, is deliberately deferred because the current JS-like
frontend lowers `undefined`, `null`, and loose null checks to the same literal-null
shape; strict Type-4 should not prove that family until those values are distinguished.

| loop | pressure | change | measured result |
|---|---|---|---:|
| 319 | batch frontier selection | group static JS-like `.filter(...).length` non-empty membership comparisons plus wrong-element/wrong-collection boundaries under `axis_membership_array_filter_length_*` | focused corpus: 18 positives, 54 hard negatives |
| 320 | baseline measurement | scan the focused batch with the previous release detector | baseline: 0/18 positives, 0/54 false merges |
| 321 | generator adversary | add cross-surface Python/Ruby/JS/TS references against JS/TS/Vue/Svelte/HTML filter-length forms | `!== 0`, `> 0`, reversed `0 <`, and `>= 1` spellings represented |
| 322 | detector strengthening | canonicalize source-gated static filter-length equality callbacks to `In(element, collection)` | first candidate exposed rule-order pressure: `!== 0` was swallowed by length-zero canonicalization |
| 323 | strict regression tests | add value-graph and CLI positives plus wrong-coordinate, raw-length, zero-length, and `NaN` boundaries | targeted tests passed |
| 324 | release focused/core gates | run focused filter-length, membership core, and all-cross core gates | focused 18/18, 0/54; membership core 95/95, 0/276; all-cross 477/477, 0/927 |

Focused release/candidate comparison:

```text
previous release:  items=72, positive=0/18, false_merges=0/54
candidate release: items=72, positive=18/18, false_merges=0/54
delta:             +18 positive hits, +0 false merges
```

Final release focused gate:

```text
GATE=focused PROPOSAL_PREFIX=axis_membership_array_filter_length CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
items: 72
positive recall: 18/18
hard-negative false merges: 0/54
Raw nodes: 0/2356
```

Final release literal-membership core gate:

```text
GATE=core AXIS=literal_collection_membership CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 371/926
positive recall: 95/95
hard-negative false merges: 0/276
Raw nodes: 0/11020
```

Final release compact all-cross gate:

```text
GATE=core CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 1404/6186
positive recall: 477/477
hard-negative false merges: 0/927
Raw nodes: 0/52001
```

Assessment: batching roughly three tightly adjacent variants is a better default for
the coevolution loop. It amortizes build/gate cost while still forcing the generator to
add identity plus boundary pressure for the whole small family. The detector rule order
is now part of the loop checklist: semantic membership rewrites that depend on
`Len(Filter(...))` must run before the more general length-zero canonicalizer, or strict
positive forms like `!== 0` can remain count-shaped and miss the exact clone.

## Static array filter-length absence: loops 325-330

This macro-loop takes the dual of the previous filter-length membership frontier. A
static literal JS-like array filter count such as
`["red", "blue"].filter(item => item === value).length === 0`, `.length <= 0`,
`.length < 1`, or `1 > .length` is exact-equivalent to
`value not in ["red", "blue"]` only when the receiver is a direct non-float literal
sequence, the callback proves equality between the iterated element and the searched
coordinate, and the comparison is a zero-count threshold. The detector keeps this proof
specific to `Len(Filter(...))`, where the count is known to be non-negative; it does not
turn arbitrary numeric comparisons into emptiness or absence facts.

The batch opens three adjacent proposal IDs:

- `axis_membership_array_filter_length_absence_identity`;
- `axis_membership_array_filter_length_absence_wrong_element_boundary`;
- `axis_membership_array_filter_length_absence_wrong_collection_boundary`.

The detector now canonicalizes `Len(Filter(static literal collection, equality lambda))`
non-empty comparisons to `In(element, collection)` and zero-count comparisons to
`Not(In(element, collection))`. Raw filtered lengths, wrong element/collection
coordinates, non-empty checks, and float-sensitive literal collections stay distinct.

| loop | pressure | change | measured result |
|---|---|---|---:|
| 325 | batch frontier selection | group static JS-like `.filter(...).length` zero-count absence comparisons plus wrong-element/wrong-collection boundaries under `axis_membership_array_filter_length_absence_*` | focused corpus: 18 positives, 54 hard negatives |
| 326 | baseline measurement | scan the focused batch with the previous release detector | baseline: 0/18 positives, 0/54 false merges |
| 327 | generator adversary | add cross-surface Python/Ruby/JS/TS absence references against JS/TS/Vue/Svelte/HTML zero-count filter-length forms | `=== 0`, `<= 0`, `< 1`, and reversed threshold spellings represented |
| 328 | detector strengthening | canonicalize source-gated static filter-length zero-count equality callbacks to `Not(In(element, collection))` | candidate focused: 18/18 positives, 0/54 false merges |
| 329 | strict regression tests | extend value-graph and CLI membership tests with absence spellings, wrong-coordinate boundaries, and `NaN` SameValueZero boundaries | targeted tests passed |
| 330 | release focused/core gates | build release and run focused absence, membership core, and all-cross core gates | focused 18/18, 0/54; membership core 99/99, 0/288; all-cross 481/481, 0/939 |

Focused release/candidate comparison:

```text
previous release:  items=72, positive=0/18, false_merges=0/54
candidate release: items=72, positive=18/18, false_merges=0/54
delta:             +18 positive hits, +0 false merges
```

Final release focused gate:

```text
GATE=focused PROPOSAL_PREFIX=axis_membership_array_filter_length_absence CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
items: 72
positive recall: 18/18
hard-negative false merges: 0/54
Raw nodes: 0/2428
```

Final release literal-membership core gate:

```text
GATE=core AXIS=literal_collection_membership CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 387/998
positive recall: 99/99
hard-negative false merges: 0/288
Raw nodes: 0/11560
```

Final release compact all-cross gate:

```text
GATE=core CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 1420/6258
positive recall: 481/481
hard-negative false merges: 0/939
Raw nodes: 0/52541
```

Assessment: this confirms that the three-variant batch style can widen both sides of a
predicate family quickly without loosening strict Type-4. The useful implementation
pattern is a polarity-aware canonicalizer: keep the source/lambda proof shared, but
separate non-empty thresholds from zero-count thresholds so the value graph emits either
`In` or `Not(In)` explicitly. The `NaN` boundary remains important because JS
`includes` and callback equality do not have identical SameValueZero behavior.

## Go literal map index default lookup: loops 331-336

This macro-loop moves back to the broader multi-language `map_default_lookup` frontier.
Go exposes a strict zero-value map lookup idiom: for `map[string]int`, `lookup[key]`
returns the stored integer when the key is present and `0` when absent. That is
exact-equivalent to Python/Ruby literal map default lookup with fallback `0`, but only
for proven integer-valued Go map literals. Non-int value maps and keyed slice literals
must stay outside the proof.

The batch opens three adjacent positive proposal IDs plus two boundaries:

- `axis_map_default_go_map_inline_identity`;
- `axis_map_default_go_map_local_identity`;
- `axis_map_default_go_map_var_identity`;
- `axis_map_default_go_map_wrong_key_boundary`;
- `axis_map_default_go_map_wrong_map_boundary`.

The generator marks Go `literal_map_default_lookup` as `partial` in the capability
matrix, then creates Python/Ruby reference pairs against Go inline, local short-binding,
and local `var` map-index forms. The detector canonicalizes only Go keyed composite
literals whose entries are string-literal keys with integer literal values, turning
`Index(map, key)` into `GetOrDefault(map, key, 0)`. This deliberately excludes keyed
slice literals such as `[]int{0: 1}[i]` and string-valued maps such as
`map[string]string{...}[key]`.

| loop | pressure | change | measured result |
|---|---|---|---:|
| 331 | batch frontier selection | group Go inline/local/var `map[string]int{...}[key]` default lookups plus wrong-key/wrong-map boundaries under `axis_map_default_go_map_*` | focused corpus: 6 positives, 10 hard negatives |
| 332 | baseline measurement | scan the focused batch with the previous release detector after opening Go capability to `partial` | baseline: 0/6 positives, 0/10 false merges |
| 333 | generator adversary | add Python/Ruby references against Go right-surface variants and include semantic mutation negatives | focused generator emits inline, short local binding, and local `var` forms |
| 334 | detector strengthening | canonicalize proven Go literal int-map index to `GetOrDefault(map, key, 0)` and add exact-safety for inline keyed map literals | candidate focused: 6/6 positives, 0/10 false merges |
| 335 | strict regression tests | add value-graph and CLI positives plus wrong-key, wrong-map, keyed-slice, and string-value boundaries | targeted tests passed |
| 336 | release focused/core gates | build release and run focused Go map, literal-map core, and all-cross core gates | focused 6/6, 0/10; literal-map core 39/39, 0/104; all-cross 487/487, 0/949 |

Focused release/candidate comparison:

```text
previous release:  items=16, positive=0/6, false_merges=0/10
candidate release: items=16, positive=6/6, false_merges=0/10
delta:             +6 positive hits, +0 false merges
```

Final release focused gate:

```text
GATE=focused PROPOSAL_PREFIX=axis_map_default_go_map CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
items: 16
positive recall: 6/6
hard-negative false merges: 0/10
Raw nodes: 0/568
```

Final release literal-map core gate:

```text
GATE=core AXIS=literal_map_default_lookup CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 143/209
positive recall: 39/39
hard-negative false merges: 0/104
Raw nodes: 0/5995
```

Final release compact all-cross gate:

```text
GATE=core CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 1436/6274
positive recall: 487/487
hard-negative false merges: 0/949
Raw nodes: 0/53109
```

Assessment: this is a useful cross-language frontier expansion because it adds a Go
literal-map proof to an axis previously dominated by Python/Ruby, JS/TS, Java, and Rust
literal factories. The important strictness constraint is not to mistake all Go
`composite_literal` index expressions for maps: the proof currently requires keyed
entries, string-literal keys, integer literal values, and the implicit fallback
`0`.

## Go non-int literal map zero defaults: loops 337-342

This macro-loop keeps the accelerated batch-3 cadence: choose about three adjacent
frontier candidates with one shared proof invariant, generate them together, and run the
expensive gates once for the batch. The selected frontier is the next Go zero-value map
lookup slice: `map[string]string{...}[key]` has implicit fallback `""`, and
`map[string]bool{...}[key]` has implicit fallback `false`. These are strict Type-4 only
when the literal map has one homogeneous value literal kind and a string-keyed map
shape; mixed-value maps and keyed slice literals must stay outside the proof.

The batch opens three adjacent positive proposal IDs plus three boundaries:

- `axis_map_default_go_zero_string_inline_identity`;
- `axis_map_default_go_zero_string_local_identity`;
- `axis_map_default_go_zero_bool_inline_identity`;
- `axis_map_default_go_zero_wrong_key_boundary`;
- `axis_map_default_go_zero_wrong_map_boundary`;
- `axis_map_default_go_zero_mixed_value_boundary`.

The detector now evaluates Go composite literals that satisfy the zero-map proof into a
small `go_literal_zero_map(default, map)` value wrapper. Indexing that wrapper emits the
same `GetOrDefault(map, key, default)` primitive used by Python/Ruby literal default
lookups. This wrapper is deliberately built from the source IL payload kind rather than
from evaluated string constant key ranges: string hashes are folded into 32-bit constant
keys, so high-bit prefix checks are not reliable after wrapping.

| loop | pressure | change | measured result |
|---|---|---|---:|
| 337 | batch frontier selection | group Go string inline, string local, and bool inline zero-value map defaults under `axis_map_default_go_zero_*` | focused corpus: 6 positives, 12 hard negatives |
| 338 | baseline measurement | scan the focused batch with the previous release detector and current generator | baseline: 0/6 positives, 0/12 false merges |
| 339 | generator adversary | add Python/Ruby references against Go string/bool right surfaces plus wrong-key, wrong-map, and mixed-value boundaries | focused generator emits 18 items |
| 340 | detector strengthening | canonicalize homogeneous Go literal int/string/bool map values to a zero-default wrapper and unwrap it at index lookup | candidate focused: 6/6 positives, 0/12 false merges |
| 341 | strict regression tests | add value-graph and CLI positives plus keyed-slice and mixed-value boundaries across string/bool cases | targeted and full tests passed |
| 342 | release focused/core gates | build release and run focused Go zero, literal-map core, and all-cross core gates | focused 6/6, 0/12; literal-map core 45/45, 0/116; all-cross 493/493, 0/961 |

Focused release/candidate comparison:

```text
previous release:  items=18, positive=0/6, false_merges=0/12
candidate release: items=18, positive=6/6, false_merges=0/12
delta:             +6 positive hits, +0 false merges
```

Final release focused gate:

```text
GATE=focused PROPOSAL_PREFIX=axis_map_default_go_zero CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
items: 18
positive recall: 6/6
hard-negative false merges: 0/12
Raw nodes: 0/624
```

Final release literal-map core gate:

```text
GATE=core AXIS=literal_map_default_lookup CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 161/227
positive recall: 45/45
hard-negative false merges: 0/116
Raw nodes: 0/6619
```

Final release compact all-cross gate:

```text
GATE=core CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 1454/6292
positive recall: 493/493
hard-negative false merges: 0/961
Raw nodes: 0/53733
```

Assessment: the batch-3 cadence worked here: one detector proof opened three positive
proposal IDs, while the hard negatives kept each proof coordinate explicit. The useful
generalization is not "all Go map indexes"; it is "homogeneous literal map values whose
language zero value can be represented exactly." Composite, pointer, slice, struct, and
interface-typed zero values remain open until they have their own source-level proof and
boundaries.

## Typed Python dynamic map defaults: loops 343-348

This batch moves from static literal-map defaults back to the dynamic `map_default_lookup`
axis. The prioritizer shows Python `.get(default)` as the largest high-precision slice in
this family, but untyped Python `.get` is not strict evidence that the receiver is a map.
The strict frontier is therefore typed Python receiver annotations: `dict[str, int]`,
`Mapping[str, int]`, and `MutableMapping[str, int]`. All three share one proof invariant:
the receiver parameter carries `ParamSemantic::Map`, and `.get(key, fallback)` has exactly
the same map/key/default coordinates as Go/Java/Rust/TypeScript dynamic map-default forms.

The batch opens three adjacent positive proposal IDs plus four boundaries:

- `axis_map_fallback_python_dict_get_identity`;
- `axis_map_fallback_python_mapping_get_identity`;
- `axis_map_fallback_python_mutable_mapping_get_identity`;
- `axis_map_fallback_python_wrong_key_boundary`;
- `axis_map_fallback_python_wrong_default_boundary`;
- `axis_map_fallback_python_wrong_map_boundary`;
- `axis_map_fallback_python_untyped_boundary`.

The detector now treats Python `Mapping`/`MutableMapping` annotations as map semantics and
canonicalizes proven `.get(key, fallback)` calls to `GetOrDefault(map, key, fallback)`.
The exact-safety gate uses the same annotation-derived `ParamSemantic::Map` fact; it does
not trust the method name alone. Untyped Python `.get` remains outside strict semantic
mode.

| loop | pressure | change | measured result |
|---|---|---|---:|
| 343 | batch frontier selection | group typed Python `dict`, `Mapping`, and `MutableMapping` dynamic map defaults under `axis_map_fallback_python_*` | focused corpus: 9 positives, 21 hard negatives |
| 344 | baseline measurement | open Python `map_default_lookup` to `partial` and scan the focused batch with the previous release detector | baseline: 0/9 positives, 0/21 false merges |
| 345 | generator adversary | add Go/Java/Rust references against Python right-surface variants plus wrong-key/default/map and untyped boundaries | focused generator emits 30 items |
| 346 | detector strengthening | recognize `mapping[...]` annotations as map facts and canonicalize proven `.get(key, fallback)` to `GetOrDefault` | candidate focused: 9/9 positives, 0/21 false merges |
| 347 | strict regression tests | add value-graph and CLI positives plus untyped and wrong-coordinate Python boundaries | targeted and full tests passed |
| 348 | release focused/core gates | build release and run focused Python map-default, map-default core, and all-cross core gates | focused 9/9, 0/21; map-default core 23/23, 0/54; all-cross 502/502, 0/982 |

Focused release/candidate comparison:

```text
previous release:  items=30, positive=0/9, false_merges=0/21
candidate release: items=30, positive=9/9, false_merges=0/21
delta:             +9 positive hits, +0 false merges
```

Final release focused gate:

```text
GATE=focused PROPOSAL_PREFIX=axis_map_fallback_python CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
items: 30
positive recall: 9/9
hard-negative false merges: 0/21
Raw nodes: 0/1330
```

Final release map-default core gate:

```text
GATE=core AXIS=map_default_lookup CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 77/90
positive recall: 23/23
hard-negative false merges: 0/54
Raw nodes: 0/3656
```

Final release compact all-cross gate:

```text
GATE=core CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 1484/6322
positive recall: 502/502
hard-negative false merges: 0/982
Raw nodes: 0/55061
```

Assessment: this is a useful general-language expansion because it closes a high-volume
Python slice without broadening untyped receiver assumptions. The proof is deliberately
annotation-gated. Ruby `fetch` and JavaScript `Map.get` defaults remain open unless a
comparable receiver type fact or construction fact is available.

## Batch-3 typed collection type facts: loops 349-352

This loop adopts the faster cadence: add roughly three positive proposals that share one
proof invariant, then verify the batch together. The invariant is annotation-derived
dynamic collection membership. If a receiver parameter is proven to be a collection, a
membership call/operator can be lowered to the same `element in collection` value graph
coordinate as other typed dynamic collection surfaces. The batch opens:

- `axis_membership_typefact_python_tuple_identity`;
- `axis_membership_typefact_java_queue_identity`;
- `axis_membership_typefact_rust_vecdeque_identity`.

The detector change is deliberately small: `param_semantic_from_text` recognizes additional
collection type tokens (`tuple[...]`, `Container[...]`, Java `Queue`/`Deque`, and Rust
`VecDeque`) and the existing strict membership lowering consumes the resulting
`ParamSemantic::Collection` fact. Untyped receivers, substring/string `contains`, map-key
membership, and wrong-element coordinates remain separate.

| loop | pressure | change | measured result |
|---|---|---|---:|
| 349 | batch frontier selection | group Python tuple, Java Queue, and Rust VecDeque under the new `axis_membership_typefact_*` prefix | focused corpus: 15 positives, 15 hard negatives |
| 350 | baseline measurement | scan the focused batch with the previous release detector before rebuilding | baseline: 0/15 positives, 0/15 false merges |
| 351 | detector strengthening | extend annotation-to-collection facts and add value/CLI tests for the three surfaces | targeted tests passed |
| 352 | release focused/core gates | build release and run focused, membership core, and all-cross core gates | focused 15/15, 0/15; membership core 112/112, 0/302; all-cross 515/515, 0/996 |

Focused release/candidate comparison:

```text
previous release:  items=30, positive=0/15, false_merges=0/15
candidate release: items=30, positive=15/15, false_merges=0/15
delta:             +15 positive hits, +0 false merges
```

Final release focused gate:

```text
GATE=focused PROPOSAL_PREFIX=axis_membership_typefact_ CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
items: 30
positive recall: 15/15
hard-negative false merges: 0/15
Raw nodes: 0/816
```

Final release membership core gate:

```text
GATE=core AXIS=literal_collection_membership CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 414/1028
positive recall: 112/112
hard-negative false merges: 0/302
Raw nodes: 0/12313
```

Final release compact all-cross gate:

```text
GATE=core CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 1511/6352
positive recall: 515/515
hard-negative false merges: 0/996
Raw nodes: 0/55806
```

Assessment: the batch-3 cadence is a good default. It cut orchestration overhead without
weakening the oracle: the new proposals share a single proof rule, focused smoke captured
the baseline miss, and core smoke proved no boundary regression. The next loop should keep
the same shape: three adjacent positives opened by one strict invariant, plus focused
negative mutations in the same prefix.

## Python builtin collection factories: loops 353-356

This loop continues the accelerated batch-3 cadence on `literal_collection_membership`.
The shared invariant is a Python construction fact: an unshadowed builtin
`set(...)`, `tuple(...)`, or `frozenset(...)` call over a proven static collection
denotes the same membership collection coordinate. The batch opens:

- `axis_membership_python_set_factory_identity`;
- `axis_membership_python_tuple_factory_identity`;
- `axis_membership_python_frozenset_factory_identity`.

The strict boundary is just as important as the positives. The generator includes
wrong-element and wrong-collection mutations, plus a local `set = ...` shadowing case.
The detector only trusts the factory when the callee is a Python free builtin name and
the same file does not define that name.

| loop | pressure | change | measured result |
|---|---|---|---:|
| 353 | batch frontier selection | group Python `set`, `tuple`, and `frozenset` builtin factories under `axis_membership_python_*` | focused corpus: 15 positives, 18 hard negatives |
| 354 | baseline measurement | scan the focused batch with the previous release detector before rebuilding | baseline: 0/15 positives, 0/18 false merges |
| 355 | detector strengthening | normalize unshadowed Python collection factories and mark matching `.__contains__` calls exact-safe | targeted tests passed |
| 356 | release focused/core gates | build release and run focused, membership core, and all-cross core gates | focused 15/15, 0/18; membership core 128/128, 0/320; all-cross 530/530, 0/1014 |

Focused release/candidate comparison:

```text
previous release:  items=33, positive=0/15, false_merges=0/18
candidate release: items=33, positive=15/15, false_merges=0/18
delta:             +15 positive hits, +0 false merges
```

Final release focused gate:

```text
GATE=focused PROPOSAL_PREFIX=axis_membership_python_ CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
items: 33
positive recall: 15/15
hard-negative false merges: 0/18
Raw nodes: 0/946
```

Final release membership core gate:

```text
GATE=core AXIS=literal_collection_membership CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 448/1061
positive recall: 128/128
hard-negative false merges: 0/320
Raw nodes: 0/13298
```

Final release compact all-cross gate:

```text
GATE=core CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 1544/6385
positive recall: 530/530
hard-negative false merges: 0/1014
Raw nodes: 0/56752
```

Assessment: this is the right acceleration pattern. Three related positives moved in
one implementation batch, but the detector still gained only one narrow proof fact:
unshadowed Python builtin collection factories over already-proven static collections.
The batch added coverage without trusting arbitrary dynamic receivers or shadowed names.

## Function-local constructed collection membership: loops 357-361

This loop keeps the batch-3 cadence on `literal_collection_membership`, but moves from
module/package bindings to function-local constructed bindings. The shared invariant is:
a local collection binding can be used as the membership collection only when the
initializer is a proven static collection construction and the binding is not reassigned
or mutated before the membership predicate. The batch opens:

- `axis_membership_local_go_slice_identity`;
- `axis_membership_local_java_list_identity`;
- `axis_membership_local_rust_vec_identity`.

The generator also adds wrong-element, wrong-collection, and local-mutation hard
boundaries. This matters because the local binding proof is intentionally narrower than
"any variable used as a receiver": appending/pushing/adding to the local collection
invalidates the original static coordinate.

| loop | pressure | change | measured result |
|---|---|---|---:|
| 357 | batch frontier selection | group Go local slice, Java local `List.of`, and Rust local `vec!` membership under `axis_membership_local_*` | focused corpus: 12 positives, 21 hard negatives |
| 358 | baseline measurement | scan the focused batch with the previous release detector | baseline: 4/12 positives, 0/21 false merges; Java local already converged, Go/Rust missed |
| 359 | detector strengthening | canonicalize import-proven Go `slices.Contains` over local collection args, prove Rust `vec![...]` as a collection construction, and add local single-assignment collection fallback with mutation rejection | focused: 12/12 positives, 0/21 false merges |
| 360 | strict regression tests | add value-graph and CLI positives plus Go/Java local mutation boundaries | targeted tests passed |
| 361 | release focused/core gates | build release and run focused, membership core, and all-cross core gates | focused 12/12, 0/21; membership core 140/140, 0/341; all-cross 542/542, 0/1035 |

Focused release/candidate comparison:

```text
previous release:  items=33, positive=4/12, false_merges=0/21
candidate release: items=33, positive=12/12, false_merges=0/21
delta:             +8 positive hits, +0 false merges
```

Final release focused gate:

```text
GATE=focused PROPOSAL_PREFIX=axis_membership_local_ CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
items: 33
positive recall: 12/12
hard-negative false merges: 0/21
Raw nodes: 0/1062
```

Final release membership core gate:

```text
GATE=core AXIS=literal_collection_membership CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 481/1094
positive recall: 140/140
hard-negative false merges: 0/341
Raw nodes: 0/14355
```

Final release compact all-cross gate:

```text
GATE=core CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 1577/6418
positive recall: 542/542
hard-negative false merges: 0/1035
Raw nodes: 0/57814
```

Assessment: this is a real detector expansion, not just benchmark growth. Java local
`List.of` was already handled by existing temporary inlining, but the same focused batch
exposed two true misses: Go `slices.Contains` over a local slice binding and Rust
`vec![...]` local membership. The final detector now carries an import-aware Go
canonicalization and a Rust `vec!` construction fact while keeping local mutation,
wrong-element, and wrong-collection boundaries closed.

## Batch-3 Go zero-value map defaults: loops 362-366

This loop switches the cadence from one frontier at a time to a batch of three related
frontiers under one proof rule. The batch stays within `literal_map_default_lookup` and
extends Go literal map index proofs from `int|string|bool` zero values to:

- `axis_map_default_go_zero_float_inline_identity`;
- `axis_map_default_go_zero_float_local_identity`;
- `axis_map_default_go_zero_nil_pointer_identity`.

The invariant is still narrow: a Go `map[string]T{...}[key]` can be lowered to the same
strict `GetOrDefault(map, key, zero(T))` coordinate only when every keyed entry is a
literal of one supported zero-value family. Mixed value kinds remain a hard boundary.
The nil-pointer case also avoids the degenerate "wrong key" mutation because all-nil
maps have the same result for present and missing keys; its hard negative changes the
right-side map value family instead.

| loop | pressure | change | measured result |
|---|---|---|---:|
| 362 | batch frontier selection | group Go `float64` inline, Go `float64` local, and Go nil-pointer literal map index lookups | focused corpus: 6 positives, 6 hard negatives |
| 363 | baseline measurement | scan the focused batch with the previous release detector | baseline: 0/6 positives, 0/6 false merges |
| 364 | detector strengthening | add float-zero and null-zero default values to Go literal-zero map canonicalization and strict exact-safety checks | focused: 6/6 positives, 0/6 false merges |
| 365 | strict regression tests | add value-graph and CLI semantic positives for float/nil plus wrong-key/wrong-map boundaries | targeted and full CLI/equivalence tests passed |
| 366 | release focused/core gates | build release and run focused, map-default core, and all-cross core gates | focused 6/6, 0/6; map-default core 51/51, 0/122; all-cross 548/548, 0/1041 |

Focused release/candidate comparison:

```text
previous release:  items=12, positive=0/6, false_merges=0/6
candidate release: items=12, positive=6/6, false_merges=0/6
delta:             +6 positive hits, +0 false merges
```

Final release focused gate:

```text
GATE=focused PROPOSAL_PREFIX=axis_map_default_go_zero_float,axis_map_default_go_zero_nil CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
items: 12
positive recall: 6/6
hard-negative false merges: 0/6
Raw nodes: 0/420
```

Final release map-default core gate:

```text
GATE=core AXIS=literal_map_default_lookup CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 173/239
positive recall: 51/51
hard-negative false merges: 0/122
Raw nodes: 0/7039
```

Final release compact all-cross gate:

```text
GATE=core CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 1589/6430
positive recall: 548/548
hard-negative false merges: 0/1041
Raw nodes: 0/58234
```

Assessment: batch-3 is a better loop unit when the items share one semantic invariant.
This batch produced a real detector expansion, not just new benchmark rows: the previous
release missed all six new cross-surface positives, while the candidate closes them with
no added false merges in focused, axis-core, or compact all-cross gates. Keep future
batches similarly scoped: three adjacent frontiers, one proof rule, one focused baseline,
one detector change, and one combined focused/core verification pass.

## Batch-3 Rust std collection factory membership: loops 367-371

This loop keeps the batch-3 cadence and applies it to `literal_collection_membership`.
The selected frontier is a tight Rust std construction family:

- `axis_membership_rust_std_hashset_identity`;
- `axis_membership_rust_std_btreeset_identity`;
- `axis_membership_rust_std_vecdeque_identity`.

All three variants share the same proof invariant: `Collection::from([literal...])`
creates a strict immutable collection coordinate for `.contains(&value)` when the
receiver binding is local and unmutated. Wrong element, wrong collection, and post-
construction mutation remain hard boundaries. This is the intended acceleration shape
for future loops: add about three adjacent frontier cases together, but only when they
share one detector rule and one negative boundary model.

| loop | pressure | change | measured result |
|---|---|---|---:|
| 367 | batch frontier selection | group Rust std `HashSet::from`, `BTreeSet::from`, and `VecDeque::from` membership under `axis_membership_rust_std_*` | focused corpus: 6 positives, 12 hard negatives |
| 368 | baseline measurement | scan the focused batch with the previous release detector | baseline: 0/6 positives, 0/12 false merges |
| 369 | detector strengthening | canonicalize Rust std collection factory calls into proven collection values and mark the same factories strict exact-safe | focused: 6/6 positives, 0/12 false merges |
| 370 | strict regression tests | add value-graph and CLI semantic positives plus wrong-element, wrong-collection, and mutation boundaries | targeted and full CLI/equivalence tests passed |
| 371 | release focused/core gates | build release and run focused, membership core, and all-cross core gates | focused 6/6, 0/12; membership core 146/146, 0/353; all-cross 554/554, 0/1053 |

Focused release/candidate comparison:

```text
previous release:  items=18, positive=0/6, false_merges=0/12
candidate release: items=18, positive=6/6, false_merges=0/12
delta:             +6 positive hits, +0 false merges
```

Final release focused gate:

```text
GATE=focused PROPOSAL_PREFIX=axis_membership_rust_std_ CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
items: 18
positive recall: 6/6
hard-negative false merges: 0/12
Raw nodes: 0/523
```

Final release membership core gate:

```text
GATE=core AXIS=literal_collection_membership CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 499/1112
positive recall: 146/146
hard-negative false merges: 0/353
Raw nodes: 0/14878
```

Final release compact all-cross gate:

```text
GATE=core CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 1607/6448
positive recall: 554/554
hard-negative false merges: 0/1053
Raw nodes: 0/58757
```

Assessment: the batch-3 cadence is now justified for adjacent strict frontiers. This
batch expands the detector, not just the benchmark: the previous release missed every
new Rust std factory positive, while the candidate proves all six and preserves all
focused, membership-core, and all-cross hard negatives. The limit is also clear:
batching should be breadth-first across related surface families, not a license to mix
unrelated semantics into one opaque detector change.

## Batch-3 Python stdlib map type aliases: loops 372-376

This loop keeps the batch-3 cadence but moves from collection construction to typed
dynamic map-default lookup. The selected frontier is Python annotation provenance for
stdlib map aliases:

- `axis_map_fallback_python_alias_mapping_identity`;
- `axis_map_fallback_python_alias_mutable_mapping_identity`;
- `axis_map_fallback_python_alias_dict_identity`.

The proof invariant is not the alias spelling itself. It is the static import fact:
`from typing|collections.abc import Dict|Mapping|MutableMapping as Alias` gives the
parameter annotation `Alias[...]` the same coarse `ParamSemantic::Map` fact as a direct
`dict[...]`, `Mapping[...]`, or `MutableMapping[...]` annotation. Unresolved aliases,
shadowed aliases, wrong keys, wrong defaults, and wrong receivers stay hard boundaries.

| loop | pressure | change | measured result |
|---|---|---|---:|
| 372 | batch frontier selection | group Python `Mapping as Alias`, `MutableMapping as Alias`, and `Dict as Alias` map-default lookups under `axis_map_fallback_python_alias_*` | focused corpus: 9 positives, 24 hard negatives |
| 373 | baseline measurement | scan the focused batch with the previous release detector | baseline: 0/9 positives, 0/24 false merges |
| 374 | detector strengthening | record stdlib type-import aliases during Python lowering and seed `ParamSemantic::Map` from alias-backed annotations, clearing aliases on shadowing | focused: 9/9 positives, 0/24 false merges |
| 375 | strict regression tests | add value-graph and CLI semantic positives plus wrong-key, wrong-default, wrong-map, unresolved-alias, and shadowed-alias boundaries | targeted and full CLI/equivalence tests passed |
| 376 | release focused/core gates | build release and run focused, map-default core, and all-cross core gates | focused 9/9, 0/24; map-default core 32/32, 0/78; all-cross 563/563, 0/1077 |

Focused release/candidate comparison:

```text
previous release:  items=33, positive=0/9, false_merges=0/24
candidate release: items=33, positive=9/9, false_merges=0/24
delta:             +9 positive hits, +0 false merges
```

Final release focused gate:

```text
GATE=focused PROPOSAL_PREFIX=axis_map_fallback_python_alias_ CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
items: 33
positive recall: 9/9
hard-negative false merges: 0/24
Raw nodes: 0/1556
```

Final release map-default core gate:

```text
GATE=core AXIS=map_default_lookup CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 110/123
positive recall: 32/32
hard-negative false merges: 0/78
Raw nodes: 0/5212
```

Final release compact all-cross gate:

```text
GATE=core CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 1640/6481
positive recall: 563/563
hard-negative false merges: 0/1077
Raw nodes: 0/60313
```

Assessment: this is a useful coevolution step because the generator found a strict
frontier the detector genuinely missed, and the detector expansion is proof-bearing
rather than name-based. The unresolved-alias and shadowed-alias boundaries are important:
`Alias[...]` alone is still not evidence, and a once-valid alias stops being evidence
after rebinding. Only an active earlier static stdlib type import makes the alias safe
enough for exact Type-4 reporting.

## Batch-3 Python stdlib collection type aliases: loops 377-381

This loop keeps the accelerated batch-3 cadence on `literal_collection_membership`.
It targets Python annotation provenance for stdlib collection aliases:

- `axis_membership_python_alias_sequence_identity`;
- `axis_membership_python_alias_container_identity`;
- `axis_membership_python_alias_set_identity`.

The shared proof invariant is the active static type-import fact:
`from typing|collections.abc import Sequence|Container|Set as Values` gives a
parameter annotation `Values[...]` the same coarse `ParamSemantic::Collection` fact
as a direct collection annotation. The alias spelling alone is not enough evidence.
Unresolved aliases, shadowed aliases, wrong elements, and wrong receivers remain hard
boundaries.

This is the intended faster loop shape going forward: add about three adjacent positive
frontiers in one generator batch, but only when they share one detector invariant and
one boundary model. The focused gate still reports each proposal separately, so a bad
case can be split back out without losing strictness.

| loop | pressure | change | measured result |
|---|---|---|---:|
| 377 | batch frontier selection | group Python `Sequence as Values`, `Container as Values`, and `Set as Values` typed dynamic collection membership under `axis_membership_python_alias_*` | focused corpus: 12 positives, 28 hard negatives |
| 378 | baseline measurement | scan the focused batch with the previous release detector | baseline: 0/12 positives, 0/28 false merges |
| 379 | detector strengthening | extend stdlib type-alias semantic resolution from map aliases to collection aliases, while keeping alias clearing on shadowing | focused: 12/12 positives, 0/28 false merges |
| 380 | strict regression tests | add value-graph and CLI semantic positives plus wrong-element, wrong-receiver, unresolved-alias, and shadowed-alias boundaries | targeted and full CLI/equivalence tests passed |
| 381 | release focused/core gates | build release and run focused, membership core, and all-cross core gates | focused 12/12, 0/28; membership core 158/158, 0/381; all-cross 575/575, 0/1105 |

Focused release/candidate comparison:

```text
previous release:  items=40, positive=0/12, false_merges=0/28
candidate release: items=40, positive=12/12, false_merges=0/28
delta:             +12 positive hits, +0 false merges
Raw nodes:         0/1192 in both runs
```

Final release focused gate:

```text
GATE=focused PROPOSAL_PREFIX=axis_membership_python_alias_ CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
items: 40
positive recall: 12/12
hard-negative false merges: 0/28
Raw nodes: 0/1192
```

Final release membership core gate:

```text
GATE=core AXIS=literal_collection_membership CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 539/1152
positive recall: 158/158
hard-negative false merges: 0/381
Raw nodes: 0/16070
```

Final release compact all-cross gate:

```text
GATE=core CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 1680/6521
positive recall: 575/575
hard-negative false merges: 0/1105
Raw nodes: 0/61505
```

Assessment: this is a real strict-frontier widening. The previous detector missed every
new alias-backed collection positive, while the candidate proves all twelve and keeps
focused, membership-core, and all-cross hard negatives at zero false merges. Two
process fixes matter for future accelerated loops: keep auxiliary boundary parameters
after the original coordinate parameters so wrong-coordinate tests stay meaningful, and
assert CLI negatives against the positive family rather than against every unrelated
opaque negative family that may exact-clone with another negative.

## Batch-3 Ruby stdlib Set membership: loops 382-386

This loop continues the batch-3 cadence on `literal_collection_membership`, now on the
Ruby side of the broad membership frontier. The batch opens:

- `axis_membership_ruby_set_new_include_identity`;
- `axis_membership_ruby_set_new_member_identity`;
- `axis_membership_ruby_set_local_identity`.

The shared proof invariant is explicit stdlib provenance plus immutable construction:
a top-level `require "set"` proves the standard Ruby `Set` constant, provided the file
does not define or shadow `Set`; then `Set.new([literal...])` denotes the same static
collection coordinate as literal collection membership. `member?` is accepted only as
a membership alias under the same proven-collection receiver rules. Missing `require`,
shadowed `Set`, mutated local Set bindings, wrong elements, and wrong collections stay
hard boundaries.

| loop | pressure | change | measured result |
|---|---|---|---:|
| 382 | batch frontier selection | group Ruby `Set.new(...).include?`, `Set.new(...).member?`, and local `Set.new` binding membership under `axis_membership_ruby_set_*` | focused corpus: 6 positives, 16 hard negatives |
| 383 | baseline measurement | scan the focused batch with the previous release detector | baseline: 0/6 positives, 0/16 false merges |
| 384 | detector strengthening | prove Ruby `Set.new([literal...])` only with top-level `require "set"` and no local `Set` definition; add `member?` as a proven collection membership alias | focused: 6/6 positives, 0/16 false merges |
| 385 | strict regression tests | add value-graph and CLI positives plus missing-require, shadowed-Set, mutation, wrong-element, and wrong-collection boundaries | targeted and full CLI/equivalence tests passed |
| 386 | release focused/core gates | build release and run focused, membership core, and all-cross core gates | focused 6/6, 0/16; membership core 161/161, 0/389; all-cross 578/578, 0/1113 |

Focused release/candidate comparison:

```text
previous release:  items=22, positive=0/6, false_merges=0/16
candidate release: items=22, positive=6/6, false_merges=0/16
delta:             +6 positive hits, +0 false merges
Raw nodes:         0/719 in both runs
```

Final release focused gate:

```text
GATE=focused PROPOSAL_PREFIX=axis_membership_ruby_set_ CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
items: 22
positive recall: 6/6
hard-negative false merges: 0/16
Raw nodes: 0/719
```

Final release membership core gate:

```text
GATE=core AXIS=literal_collection_membership CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 550/1174
positive recall: 161/161
hard-negative false merges: 0/389
Raw nodes: 0/16424
```

Final release compact all-cross gate:

```text
GATE=core CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 1691/6543
positive recall: 578/578
hard-negative false merges: 0/1113
Raw nodes: 0/61859
```

Assessment: this is a useful strict-frontier widening because the proof is not just a
method-name heuristic. The detector now requires an explicit stdlib import fact, rejects
local `Set` definitions, rejects mutated bindings, and still keeps wrong-coordinate
boundaries separate. The CLI test was also tightened to check negative boundaries
against the positive family, because independently valid negative families can exact-
clone each other without being false positives against the target frontier.

## Batch-3 Python stdlib deque membership: loops 387-391

This loop applies the accelerated batch-3 cadence to `literal_collection_membership`.
The batch opens three surface forms under one proof invariant:

- `axis_membership_python_deque_import_identity`;
- `axis_membership_python_deque_alias_identity`;
- `axis_membership_python_deque_namespace_identity`.

The shared invariant is explicit stdlib provenance: `collections.deque([...])` is a
strict static collection factory only when the callee is proven by a top-level
`from collections import deque`, an alias import, or a `collections` namespace import.
Missing imports, top-level shadowing/rebinding, local mutation after construction,
wrong elements, and wrong collections remain hard boundaries.

| loop | pressure | change | measured result |
|---|---|---|---:|
| 387 | batch frontier selection | group direct import, alias import, and namespace-qualified `collections.deque` membership under `axis_membership_python_deque_*` | focused corpus: 12 positives, 32 hard negatives |
| 388 | baseline measurement | scan the focused batch with the previous release detector | baseline: 0/12 positives, 0/32 false merges |
| 389 | detector strengthening | prove imported Python `collections.deque([literal...])` through `import_binding` / `import_namespace` value provenance | focused: 12/12 positives, 0/32 false merges |
| 390 | strict boundary repair | reject top-level unit shadowing in value-graph import evidence, and require membership collection calls to be proven collection factories rather than arbitrary safe calls | targeted CLI/equivalence tests passed; shadowed deque no longer joins the positive family |
| 391 | release focused/core gates | build release and run focused, membership core, and all-cross core gates | focused 12/12, 0/32; membership core 173/173, 0/421; all-cross 590/590, 0/1145 |

Focused release/candidate comparison:

```text
previous release:  items=44, positive=0/12, false_merges=0/32
candidate release: items=44, positive=12/12, false_merges=0/32
delta:             +12 positive hits, +0 false merges
Raw nodes:         0/1582 in both runs
```

Final release focused gate:

```text
GATE=focused PROPOSAL_PREFIX=axis_membership_python_deque_ CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
items: 44
positive recall: 12/12
hard-negative false merges: 0/32
Raw nodes: 0/1582
```

Final release membership core gate:

```text
GATE=core AXIS=literal_collection_membership CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 594/1218
positive recall: 173/173
hard-negative false merges: 0/421
Raw nodes: 0/18006
```

Final release compact all-cross gate:

```text
GATE=core CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 1735/6587
positive recall: 590/590
hard-negative false merges: 0/1145
Raw nodes: 0/63441
```

Assessment: this batch confirms the accelerated cadence is viable when the three
items share one proof invariant. The first CLI run exposed a real strictness gap:
generic safe calls were accepted as membership collections, and top-level function
shadowing could leave stale import evidence in the value graph. Fixing that gap
improved the loop itself, not just the deque case, while preserving all existing core
frontiers at zero false merges.

## Batch-3 map key-view membership: loops 392-396

This loop moves from collection membership back to the separate `map_key_membership`
frontier. The batch opens three key-view surfaces under one proof invariant:

- `axis_map_key_python_keys_in_identity`;
- `axis_map_key_python_keys_contains_identity`;
- `axis_map_key_ts_array_from_keys_identity`.

The invariant is typed/proven map receiver provenance: `lookup.keys()` is a key-view
only when `lookup` is a typed map parameter or a proven map value. Membership over
that key-view is lowered to the existing map-key predicate `key in lookup`. Value-view
membership (`lookup.values()`), wrong-key, and wrong-map variants remain hard
boundaries. TypeScript spread keys (`[...lookup.keys()]`) was deliberately excluded
because the current IL cannot distinguish it from `[lookup.keys()]`.

| loop | pressure | change | measured result |
|---|---|---|---:|
| 392 | batch frontier selection | group Python `key in lookup.keys()`, Python `lookup.keys().__contains__(key)`, and TypeScript `Array.from(lookup.keys()).includes(key)` | focused corpus: 15 positives, 45 hard negatives |
| 393 | baseline measurement | scan the focused batch with the previous release detector | baseline: 0/15 positives, 0/45 false merges |
| 394 | detector strengthening | recognize typed/proven map key-views in the value graph and strict gate; lower key-view membership to `In(key, map)` | focused: 15/15 positives |
| 395 | generator counterattack | focused gate exposed that special identity semantic-mutation negatives did not actually mutate the key; fix generator mutation for the new proposals | focused repaired: 15/15 positives, 0/45 false merges |
| 396 | release focused/core gates | build release and run focused, map-key core, and all-cross core gates | focused 15/15, 0/45; map-key core 27/27, 0/63; all-cross 603/603, 0/1190 |

Focused release/candidate comparison:

```text
previous release:  items=60, positive=0/15, false_merges=0/45
candidate release: items=60, positive=15/15, false_merges=0/45
delta:             +15 positive hits, +0 false merges
Raw nodes:         0/1809 in both runs
```

Final release focused gate:

```text
GATE=focused PROPOSAL_PREFIX=axis_map_key_python_keys_,axis_map_key_ts_array_from_keys_ CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
items: 60
positive recall: 15/15
hard-negative false merges: 0/45
Raw nodes: 0/1809
```

Final release map-key core gate:

```text
GATE=core AXIS=map_key_membership CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 90/165
positive recall: 27/27
hard-negative false merges: 0/63
Raw nodes: 0/2718
```

Final release compact all-cross gate:

```text
GATE=core CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 1793/6647
positive recall: 603/603
hard-negative false merges: 0/1190
Raw nodes: 0/65191
```

Assessment: this is a real frontier widening and a loop-quality improvement. The
detector now proves key-view membership without treating value-view membership as
equivalent, and the generator now mutates special map-key identity proposals correctly
instead of accidentally producing duplicate positives as hard negatives.

## Batch-3 map default guard-return: loops 397-401

This loop adopts the accelerated cadence: add about three closely-related frontier
items in one generator batch, then verify the whole batch with the same focused,
axis-core, and compact all-cross gates. The batch opens early-return guard forms
for dynamic map-default lookup:

- `axis_map_fallback_python_guard_return_identity`;
- `axis_map_fallback_ts_guard_return_identity`;
- `axis_map_fallback_java_guard_return_identity`.

The invariant is presence-guarded map defaulting: a guard over the same map/key
followed by a present-key return and an absent-key fallback is the same behavior as
`GetOrDefault(map, key, fallback)`. The detector now collapses the partial
`GetOrDefault(map, key, bottom)` produced inside a guarded return with the
fallthrough fallback return. Map-specific Java key predicates (`containsKey`,
`contains_key`, `key?`, `has_key?`) are also lowered to map-key membership; ambiguous
`has` remains typed/proven-map gated.

| loop | pressure | change | measured result |
|---|---|---|---:|
| 397 | batch frontier selection | group Python, TypeScript, and Java early-return guard forms under one map-default proof invariant | focused corpus: 9 positives, 9 hard negatives |
| 398 | baseline measurement | scan the focused batch with the previous release detector | baseline: 0/9 positives, 0/9 false merges |
| 399 | detector strengthening | collapse guarded/partial map-default return sinks to `GetOrDefault(map,key,fallback)` and widen Java map-key predicate lowering | targeted equivalence/CLI tests passed |
| 400 | release focused/core gates | build release and run focused and map-default core gates | focused 9/9, 0/9; map-default core 40/40, 0/86 |
| 401 | compact all-cross gate | run the full compact all-cross core suite to check cross-frontier regressions | all-cross 611/611, 0/1198 |

Focused release/candidate comparison:

```text
previous release:  items=18, positive=0/9, false_merges=0/9
candidate release: items=18, positive=9/9, false_merges=0/9
delta:             +9 positive hits, +0 false merges
Raw nodes:         0/906 in both runs
```

Final release focused gate:

```text
GATE=focused PROPOSAL_PREFIX=axis_map_fallback_python_guard_return_identity,axis_map_fallback_ts_guard_return_identity,axis_map_fallback_java_guard_return_identity CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
items: 18
positive recall: 9/9
hard-negative false merges: 0/9
Raw nodes: 0/906
```

Final release map-default core gate:

```text
GATE=core AXIS=map_default_lookup CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 126/141
positive recall: 40/40
hard-negative false merges: 0/86
Raw nodes: 0/6009
```

Final release compact all-cross gate:

```text
GATE=core CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 1809/6665
positive recall: 611/611
hard-negative false merges: 0/1198
Raw nodes: 0/65988
```

Assessment: the batch-3 cadence worked here because all three items shared one proof
invariant. It reduced loop overhead without reducing strictness: the focused batch
added 9 exact positives, the map-default core kept every existing hard boundary at
zero false merges, and all-cross core still passed at zero false merges.

## Batch-3 Python module collection membership: loops 402-406

This loop follows the accelerated cadence requested after loop 401: add about three
closely-related frontier items, then validate them together with one focused gate,
one axis-core gate, and one compact all-cross gate. The batch stays within
`literal_collection_membership` and opens:

- `axis_membership_module_python_tuple_identity`;
- `axis_membership_module_python_set_identity`;
- `axis_membership_module_python_mutated_boundary`.

The proof invariant is module-level collection membership through a stable binding.
A Python module tuple literal and a module set literal are exact membership collections
when the binding is assigned once and never mutated. A module list that is appended
after initialization is not the same strict proof, even if its original literal items
match the reference collection.

The detector now canonicalizes tuple and Python set literals to the same strict
membership collection value as list literals only after the binding has passed the
module-immutability proof. The mutation scanner was also strengthened to treat the
normalized `@Append(receiver, value)` builtin as a binding mutation; previously it
only saw field-method calls, so `VALUES.append(...)` after idiom normalization could
slip past the strict gate.

| loop | pressure | change | measured result |
|---|---|---|---:|
| 402 | batch frontier selection | group Python module tuple, module set, and mutated module list membership under one stable-binding invariant | focused corpus: 4 positives, 6 hard negatives |
| 403 | baseline measurement | scan the focused batch with the previous release detector | baseline: 0/4 positives, 2/6 false merges |
| 404 | detector strengthening | canonicalize tuple/Python set collection values and reject normalized `@Append` mutations for module/local bindings | targeted equivalence/CLI tests passed |
| 405 | release focused/core gates | build release and run focused and literal-membership core gates | focused 4/4, 0/6; membership core 175/175, 0/424 |
| 406 | compact all-cross gate | run the full compact all-cross core suite to check cross-frontier regressions | all-cross 613/613, 0/1201 |

Focused release/candidate comparison:

```text
previous release:  items=10, positive=0/4, false_merges=2/6
candidate release: items=10, positive=4/4, false_merges=0/6
delta:             +4 positive hits, -2 false merges
```

Final release focused gate:

```text
GATE=focused PROPOSAL_PREFIX=axis_membership_module_python_ CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
items: 10
positive recall: 4/4
hard-negative false merges: 0/6
Raw nodes: 0/265
```

Final release literal-membership core gate:

```text
GATE=core AXIS=literal_collection_membership CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 599/1228
positive recall: 175/175
hard-negative false merges: 0/424
Raw nodes: 0/18141
```

Final release compact all-cross gate:

```text
GATE=core CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
selected items: 1814/6675
positive recall: 613/613
hard-negative false merges: 0/1201
Raw nodes: 0/66123
```

Assessment: this batch validates the faster loop shape. It expanded strict positives
and simultaneously removed a previous false merge, so batching did not weaken the
proof bar. The important constraint is that a batch should still share one invariant:
here, all three items are about the same stable module binding proof and its mutation
boundary.

## Extreme exact Type-4 idiom slice: loops 407-411

This pass implements the four highest-priority Type-4 improvements selected from the
current frontier proposal, but keeps each one in a strict first slice rather than opening
the full fuzzy space:

- JS/TS record-shape guards now accept `Array.isArray(value) === false` and its symmetric
  false comparison as the same not-array proof as `!Array.isArray(value)`, while preserving
  the existing shadowed-`Array` boundary.
- Simple boolean flag loops with `assign; break` branches now converge with `any`/`all`
  reductions when the predicate is over the loop element.
- Equality OR-chains over the same candidate and static literals now converge with literal
  collection membership, with collection item order ignored.
- Python ordered string builders of the form `out = ""; for x in xs: out += f(x)` now
  converge with literal-separator `sep.join(xs)` when the contribution is genuinely
  per-element; prepend and non-element builders remain outside the canon.

| loop | pressure | change | measured result |
|---|---|---|---:|
| 407 | priority batch selection | implement record not-array comparisons, flag+break reductions, equality-chain membership, and ordered string builder joins as four strict slices | focused hand corpus covers 4 positives and 4 hard negatives |
| 408 | detector strengthening | add common value-graph canons plus the JS/TS record guard and Python literal-separator `join` idiom | targeted CLI regression passed |
| 409 | cache boundary | bump the scan cache schema because semantic feature extraction changed | stale v4 cache entries ignored |
| 410 | focused regression gate | run the new `scan_mode_semantic_proves_extreme_type4_idioms` test | CLI test passed |
| 411 | local semantic probe | scan a hand-written four-file corpus in semantic JSON mode | 4/4 expected families, 0/4 hard-negative inclusions |

Assessment: this is an exactness-first widening. The useful new merges are deliberately
small and high-confidence, and every slice has an adjacent hard negative in the CLI gate:
missing not-array proof, wrong early-exit predicate, wrong literal set, and ordered prepend.
