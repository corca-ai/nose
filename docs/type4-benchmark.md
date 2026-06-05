# Type-4 benchmark factory

The plan and executable seed for a Type-4 benchmark that matches nose's exact semantic
goal. Back to [benchmark](benchmark.md) and [architecture](architecture.md).

## Goal

Measure whether nose finds the **provable semantic equivalence classes** it is meant to
model — not whether it can guess that two fragments are "semantically similar." The target
asset is an evidence-carrying semantic test factory:

- generate equivalent and non-equivalent program pairs from explicit semantic specs;
- attach evidence per label — same-spec construction plus oracle/property checks for
  positives, concrete counterexamples for negatives;
- run nose against those pairs and preserve the minimal failures as regressions;
- expand semantic coverage breadth-first, by meaningful equivalence classes rather than by
  incidental code complexity.

LLMs help mine candidates, but they are not gold truth: a positive needs evidence, a
negative needs a counterexample (or an equally concrete reason the exact semantic channel
must not merge it). **The verifier remains the judge.**

### Why synthetic first

The v5 labelset is a *product* benchmark — it asks whether a reported family is worth
refactoring. That is a different question from Type-4 exactness: a family can be
refactor-worthy without being semantically identical, and a true semantic clone can be too
small or local to matter as a refactor. Synthetic generation gives control over the
semantic class under test: starting from a spec such as `sum(filter(xs, x > 0))`, the
factory emits several implementations (explicit/indexed loops, comprehensions, builders,
builtins, reductions) — same-spec pairs are positive candidates, small meaning-changing
mutations are hard negatives. This is a complement to real-code evaluation, not a
replacement: synthetic gives semantic-class coverage and precise regressions; v5 (and any
future evidence-backed real subset) gives realism.

## The adversarial loop

Three roles, not two:

| actor | responsibility |
|---|---|
| Generator | create positive and negative pairs from specs and transform templates |
| Detector | run nose and report exact semantic matches or misses |
| Verifier | decide whether a generated label is admissible as gold (the judge) |

LLMs make the loop broader and faster, but their role is bounded to **proposals,
explanations, triage, and shrink suggestions — never promoting gold labels themselves.** A
proposal card is structured enough that the generator can act on it without trusting
free-form prose:

```json
{
  "proposal": "loop-filter-count vs comprehension-count",
  "why": "L2 loop/comprehension coverage for count(filter)",
  "positive_spec": "count(filter(xs, x > 0))",
  "negative_mutations": ["x > 0 -> x >= 0", "count -> sum"],
  "expected_evidence": "same-spec + property tests; negatives require counterexamples",
  "complexity_budget": { "max_lines": 12, "max_branch_count": 1 }
}
```

Any LLM-created item without verifier evidence (or a negative without a counterexample)
stays a candidate and does not count toward the exact Type-4 score.

## Semantic coverage matrix

The generator is coverage-guided: it fills a matrix of semantic categories rather than
freely increasing code size. Each item declares the cells it covers, and the scheduler
prefers empty/under-covered cells over more complex variants of already-covered ones.

| axis | examples |
|---|---|
| computation | `sum`, `count`, `min`, `max`, `any`, `all`, `map`, `filter`, `lookup`, `string-build` |
| representation | `for-loop`, `while-index-loop`, `iterator-loop`, `reduce`, `comprehension`, `builder`, `builtin`, `recursion` |
| control variation | `guard`, `ternary`, `early-return`, `continue`, `break`, `nested-if` |
| data shape | `int`, `bool`, `string`, `list`, `record`, `field-write` |
| proof fact | immutable binding, proven callee identity, table-key identity, static import/projection, nullish default, own-property guard, record-shape guard, equality-chain membership, flag+break reduction, ordered string-builder, statically-false loop guard, total-order comparator, C u16/u32 byte-pack, Java low-bit toggle, unsafe boundary |
| language relation | same-language, cross-language, embedded script |
| label status | positive, hard-negative |
| evidence | `E1` same-spec/property, `E2` counterexample, future interpreter/symbolic/proof |

The detector should not grow as a pile of language-specific exceptions: each frontend emits
the thinnest facts it can prove (single-assignment bindings, callee/receiver identity,
static import/projection coordinates, own-property and record-shape guards, literal table
keys, equality-chain membership, flag+break reduction, ordered string-builder, explicit
unsafe boundaries), and the shared strict engine consumes them.
`capabilities.v1.json` records which surfaces emit which facts, so unsupported cells stay
visible.

### Breadth-first difficulty levels

Generation advances by level; a higher level is admitted only when lower-level coverage for
that class is healthy and the verifier is stable.

| level | scope |
|---|---|
| L0 | same language, same structure, controlled renaming or literal placement |
| L1 | loop-form variation: `for` vs `while`, indexed vs iterator |
| L2 | loop vs builtin/reduce/comprehension/builder |
| L3 | control-flow variation: guard, ternary, early return, continue, break |
| L4 | cross-language forms for the same semantic spec |
| L5 | local effects, fields, records, ordered string/list building |
| L6 | bounded recursion vs iteration (list folds, tree folds) |

This keeps the benchmark diagnostic — a failed L2 case should be understood before the
factory produces many L5/L6 combinations for the same computation.

## Generation discipline

**Complexity budget.** Every item has a small budget (max AST nodes, lines, nesting depth,
branch count, free variables, primary/secondary transforms). Most cases should have one
primary transform and at most one or two secondary transforms — a `loop` vs `reduce` case
may add a guard, but not also field mutation, recursion, exceptions, and string concat. If
too many axes move at once it stops being a useful regression. The executable generator
already gates `max_lines` and `max_branch_count`; AST-node/nesting/transform-count gates
come later.

**Minimality.** Generated failures are shrunk before promotion — simplify literals (keeping
the counterexample), drop unneeded branches and secondary transforms, minimize
language/library surface. A stored case should be explainable in one sentence, e.g.
*"Positive `loop-filter-sum` vs `reduce-filter-sum`, cross-language, currently under-merged
by exact semantic detection."*

**Usefulness score.** The factory queues and promotes by

```text
usefulness = coverage_novelty + real_corpus_prior + detector_failure_value
           + verifier_confidence + minimality
           - complexity_penalty - duplicate_pattern_penalty
```

where `real_corpus_prior` comes from the v5 labelset, under-merge diagnostics, field
evaluation, and idiom frequency — keeping synthetic work pointed at patterns that occur in
real code. The executable triage step:

```sh
python3 bench/type4/prioritize_frontier.py \
  --json-out /tmp/nose-frontier-priorities.json \
  --markdown-out bench/type4/FRONTIER_PRIORITIES.md
```

It ranks candidate axes by real-code frequency, repo/language spread, estimated cost,
soundness risk, scope, and coverage status. It chooses what to investigate next — it is not
label evidence; the generator/verifier/evaluator loop still decides which items become cases.

## Hard-negative siblings

Every positive family needs nearby negatives — exact Type-4 progress is unsafe unless the
benchmark also proves what must *not* merge. Each negative carries a concrete counterexample.

| positive class | sibling negatives |
|---|---|
| sum loop vs sum reduce | changed initial value, `+` changed to `*` |
| filter `x > 0` | predicate changed to `x >= 0` or `x < 0` |
| `any` | changed to `all`, negated predicate |
| `min` | changed to `max`, reversed comparison with different tie behavior |
| string concat | operand order swap, separator placement change |
| field write | target field changed, overwrite order changed |
| statement effect | append/emit order changed on the same execution path |
| foreach index assignment | wrong receiver, wrong index expression, wrong assigned value, unused iteration binding |
| branch temp-consumed index assignment | wrong temp RHS, wrong index/value, temp consumed through the receiver, temp chain skips first temp, final assignment also reads prior temp |
| foreach temp-consumed effect | wrong temp RHS, temp not consumed by the effect, temp RHS independent of iteration, receiver touches iteration/temp, temp chain skips first temp, final effect also reads prior temp |
| indexed loop | skipped first or last element, wrong collection indexed |
| C pointer-length contract | skipped first element, stride greater than one, non-contract bound |
| C byte-pack | swapped byte order, overlapping shift, wrong byte coordinate, unproven byte alias, non-byte receiver, uncasted 32-bit high-lane shift, unproven unsigned cast alias |
| total-order comparator | descending sign, equality-boundary sign, wrong returned sign value, overloadable receiver comparison |
| statically-false loop guard | wrong reachable return, false initial value, positive guard, reassigned guard |
| Java low-bit toggle | reversed +/- branches, `^ 2`, `% 2 == 1` with negative odds, wrong branch delta |
| own-property guard | prototype-including `in`, shadowable method call, shadowed `Object`, different static key |
| record-shape guard | missing null exclusion, missing array exclusion, unrelated property predicate |

## Detector co-evolution loop

The factory is useful only if it changes the **detector**, not just the benchmark. Each
accepted iteration leaves four artifacts: a proposal card / matrix cell; generated positives
and hard-negatives with evidence; a detector change that makes a previously-missed exact
positive converge; and a regression check proving the nearby hard-negatives still don't
merge. The loop:

```text
generate a benchmark slice  →  scan with semantic mode
        →  frontier summary: missed positives grouped by computation/surface/representation
        →  pick one narrow under-merge class  →  add a failing convergence test
        →  patch the frontend / idiom lowering / shared value graph
        →  rerun positives + hard-negatives + docs gate + verifier checks  →  record + generate the next sibling
```

For speed the inner loop batches ~3 adjacent frontier candidates **that share a proof
mechanism** before running the expensive gates; the detector patch may be shared, but each
candidate keeps its own proposal id, focused positives, hard-negatives, and before/after
result, and the acceptance gate runs once per batch. Do **not** batch unrelated
soundness-risk changes — if one candidate regresses or needs a different semantic contract,
split it out before accepting. This Definition of Done is enforced in
[`bench/type4/README.md`](../bench/type4/README.md); the smoke gate is
`scripts/type4-smoke.sh`, and frontier deltas are stored as JSON so recall changes and
false-merge regressions are auditable. A loop that adds only proposal cards or generated
examples is a coverage-expansion loop, not a co-evolution loop.

Language-specific work is allowed **only as a lowering step into the shared semantic
representation** — a JS `.filter().reduce(...)`, a Rust iterator chain, a Java stream, and a
plain loop should all converge to the same value-graph shape. Each candidate is classified
before implementation (`all-language` / `multi-language` / `language-family` /
`single-language`); after a narrow loop, the next ordinary frontier should be
`all-/multi-language` unless the narrow loop fixed a demonstrated strict soundness bug.

Some cross-language convergence needs an explicit **semantic contract**. The current C list
contract is narrow: generated `int f(int *xs, int n)` treats `n` as the exact logical length
of `xs`, and aligned-array `int f(int *a, int *b, int n)` treats `n` as the shared length;
C predicate reductions use `1/0` as boolean `true/false`. The detector consumes that
contract only for strict full traversals — skipped-first and stride-two C siblings are
generated as hard negatives.

The current C total-order comparator contract is similarly narrow: generated comparator
callbacks dereference two scalar coordinates and return `-1`, `1`, or `0` for ascending
less, greater, and equal cases. Guard-return order and nested ternary sign forms may
converge only when the comparison operators are primitive ordered comparisons; overloadable
or effectful receiver comparisons remain hard negatives.

The current C byte-pack contract is narrower still. Generated `u16` big-endian decoders may
converge only when the receiver is proven to be a byte buffer (`unsigned char *`, `uint8_t *`,
or a local `typedef unsigned char u8` from the same file or a same-directory direct quote
include) and the expression combines lane 0 shifted by exactly 8 with lane 1 from the same
base via `+` or `|`. Generated `u32` big-endian decoders additionally require an explicit
unsigned 32-bit cast proof on the high byte lane before shifting by 24: direct `unsigned`,
`unsigned int`, `uint32_t`, or a proven `typedef unsigned int u32` alias from the same file
or direct local include. Swapped lanes, overlapping shifts, wrong byte coordinates, missing
or non-byte aliases, non-byte receivers, and uncasted 32-bit high-lane shifts remain hard
negatives.

## Promotion rules

An item enters the Type-4 gold set only if **all** relevant rules hold: the semantic status
is evidence-backed; positives pass the chosen oracle/property/symbolic checks under the
declared semantics; negatives include a counterexample or precise non-equivalence witness;
transform tags and semantic scope are explicit; expected detector behavior is explicit; the
case has been shrunk; it is not redundant with an existing item unless it fills a new matrix
cell; and the dev/heldout assignment is fixed before tuning. Items failing these may still
seed the generator or triage, but must not affect the score.

## Splits, anti-overfit, and gate tiers

The benchmark holds out semantic *structure*, not just repos — seen vs unseen specs,
transform classes, generator templates, language pairs, and negative mutations — so the
detector can't merely learn the generator's surface grammar. The executable generator marks
same-surface `loop -> aggregate` positives as `dev` and places cross-language positives,
indexed-loop positives, and all hard-negatives in `heldout`; the evaluator reports
split-level recall and false merges, grouping negatives by `negative_tag`.

Routine work uses three gate tiers so the inner loop doesn't scan the whole corpus every
iteration:

- **`focused`** — generate only the selected axis/proposal prefix (usually `CROSS=none`):
  the fast inner loop for lowering / value-graph changes.
- **`core`** — generate the selected scope, then apply *coverage-preserving compaction*:
  a compact manifest selected from the full corpus by explicit feature coverage
  (proposal/computation, status, split, representation pair, transform and hard-negative
  tags, language surface, capability state — so proof-fact regressions aren't compacted
  away). Not random sampling; it copies only the selected sources so scan time actually
  drops.
- **`full`** — the full manifest; dense all-cross and broad real-repo audits are reserved
  for milestone validation, which catch any interaction the selector hid.

Real-repo audits start with the prioritizer's top repos for the active axis, expanding only
when the focused/compact gates have closed.

## Relationship to existing data

| asset | primary question |
|---|---|
| v5 refactoring-family labelset | Does `nose scan` surface useful refactoring candidates first? |
| Type-4 synthetic factory | Does exact semantic detection cover the intended equivalence classes without false merges? |
| future evidence-backed real subset | Do proven Type-4 cases occur and get detected in real repos? |

The bridge is candidate mining: v5 families seed generator templates and hard-negatives, but
only evidence-backed pairs enter the Type-4 gold set.

## Initial implementation shape

The seed lives in `bench/type4/`:

- `proposals.v1.json` — LLM-planned semantic proposal cards;
- `capabilities.v1.json` — current surface-by-surface proof-fact capability matrix;
- `ITERATIONS.md` — the first ten coverage-expansion iterations and smoke results;
- `generate.py` — deterministic source/manifest generator for all supported surfaces;
- `select_cases.py` — coverage-preserving compaction for routine gates;
- `eval_manifest.py` — compares `nose scan --mode semantic` output against the manifest;
- `schema.json` — pair-level manifest schema;
- `README.md` — commands and current smoke numbers.

The long-term direction is an adversarial semantic test factory: the generator creates
frontier cases, the verifier proves or refutes them, detector failures become minimal
regressions, and the coverage matrix decides where the next breadth-first expansion goes.
