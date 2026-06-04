# Type-4 benchmark factory

This page records the plan and executable seed for a Type-4 benchmark that matches nose's
exact semantic goal. Back to
[benchmark](benchmark.md) and [architecture](architecture.md).

## Goal

The benchmark should measure whether nose finds **provable semantic equivalence
classes** that it is meant to model. It should not reward a detector for guessing
that two fragments are "semantically similar".

The target asset is an evidence-carrying semantic test factory:

- generate equivalent and non-equivalent program pairs from explicit semantic specs;
- attach evidence for each label: same-spec construction plus oracle/property checks
  for positives, concrete counterexamples for negatives;
- run nose against those pairs and preserve the minimal failures as regressions;
- expand semantic coverage breadth-first, so the benchmark grows by meaningful
  equivalence classes instead of by incidental code complexity.

Human and LLM judgments may help mine candidates, but they are not gold truth for the
exact Type-4 metric. A positive gold item needs evidence. A negative gold item needs a
counterexample or an equally concrete reason the exact semantic channel must not merge it.
LLMs are useful as planners, miners, and analysts; the verifier remains the judge.

## Why synthetic first

The existing v5 labelset is a product benchmark: it asks whether a reported family is
worth refactoring. That is valuable, but it is not the same question as Type-4 exactness.
A family can be refactor-worthy without being semantically identical, and a true semantic
clone can be too small or too local to matter as a refactor.

Synthetic generation gives control over the semantic class under test. Starting from a
semantic spec such as `sum(filter(xs, x > 0))`, the factory can emit several implementations
in one or more languages: explicit loops, indexed loops, comprehensions, builders, builtins,
and reductions. Pairs emitted from the same spec are positive candidates; small meaning-
changing mutations around the same spec are hard negatives.

The intent is not to replace real-code evaluation. Synthetic cases provide semantic class
coverage and precise regressions; the v5 labelset and any future evidence-backed real-code
subset provide realism.

## Three actors

The adversarial loop has three roles, not two:

| actor | responsibility |
|---|---|
| Generator | create positive and negative pairs from specs and transform templates |
| Detector | run nose and report exact semantic matches or misses |
| Verifier | decide whether a generated label is admissible as gold |

The verifier is the judge. If it cannot provide evidence, the item stays in the candidate
pool and does not count in the exact Type-4 score.

## Semantic coverage matrix

The generator should be coverage-guided. It should fill a matrix of semantic categories
instead of freely increasing code size.

Useful axes:

| axis | examples |
|---|---|
| computation | `sum`, `count`, `min`, `max`, `any`, `all`, `map`, `filter`, `lookup`, `string-build` |
| representation | `for-loop`, `while-index-loop`, `iterator-loop`, `reduce`, `comprehension`, `builder`, `builtin`, `recursion` |
| control variation | `guard`, `ternary`, `early-return`, `continue`, `break`, `nested-if` |
| data shape | `int`, `bool`, `string`, `list`, `record`, `field-write` |
| proof fact | immutable binding, proven callee identity, table-key identity, static import/projection, nullish default, own-property guard, record-shape guard, unsafe boundary |
| language relation | same-language, cross-language, embedded script |
| label status | positive, hard-negative |
| evidence | `E1` same-spec/property evidence, `E2` counterexample evidence, future interpreter/symbolic/proof evidence |

Each item should declare the matrix cells it covers. The scheduler should prefer empty or
under-covered cells over more complex variants of already-covered cells.

The detector should not grow as a pile of language-specific exceptions. Each frontend should
emit the thinnest facts it can prove, while the common strict engine consumes those facts:
single-assignment immutable bindings, safe function-binding identity, receiver/method
identity, static import coordinates, nullish-default coordinates, static field/property
projection coordinates, own-property guard facts, record-shape guard facts, literal table
keys, and explicit unsafe boundaries.
`capabilities.v1.json` records which surfaces currently emit which facts so unsupported
cells stay visible.

## Breadth-first difficulty levels

Generation should advance by levels. A higher level is admitted only when lower-level
coverage for that semantic class is healthy and the verifier remains stable.

| level | scope |
|---|---|
| L0 | same language, same structure, controlled renaming or literal placement |
| L1 | loop-form variation: `for` vs `while`, indexed vs iterator |
| L2 | loop vs builtin/reduce/comprehension/builder |
| L3 | control-flow variation: guard, ternary, early return, continue, break |
| L4 | cross-language forms for the same semantic spec |
| L5 | local effects, fields, records, and ordered string/list building |
| L6 | bounded recursion vs iteration, such as list folds and tree folds |

This keeps the benchmark diagnostic. A failed L2 case should usually be understood before
the factory produces many L5 or L6 combinations for the same computation.

## Complexity budget

Every generated item needs a budget. The default should be small:

- maximum AST nodes;
- maximum source lines;
- maximum nesting depth;
- maximum branch count;
- maximum free variables;
- maximum number of primary transforms;
- maximum number of secondary transforms.

Most cases should have one primary transform and at most one or two secondary transforms.
For example, a `loop` vs `reduce` case may also include a guard, but it should not also add
field mutation, recursion, exception behavior, and string concatenation. If too many axes
move at once, the result stops being a useful regression.

The executable generator enforces the first budget gates now: proposal cards must declare
their required fields, and generated variants must stay under `max_lines` and
`max_branch_count`. Later gates should add AST-node count, nesting depth, and transform-count
validation.

## Minimality and shrinking

Generated failures should be shrunk before promotion. The shrinker should remove or simplify
anything that is not needed to preserve the semantic status and detector outcome:

- simplify literals while preserving the counterexample;
- remove branches that are not needed;
- reduce expressions to the smallest form that still exercises the transform;
- drop secondary transforms that do not affect the failure;
- minimize language/library surface when a core form is enough.

A stored benchmark case should be explainable in one sentence, such as:

> Positive `loop-filter-sum` vs `reduce-filter-sum`, cross-language, currently
> under-merged by exact semantic detection.

## Hard-negative siblings

Every positive family should have nearby negatives. Exact Type-4 progress is unsafe unless
the benchmark also proves what must not merge.

Examples:

| positive class | sibling negatives |
|---|---|
| sum loop vs sum reduce | changed initial value, `+` changed to `*` |
| filter `x > 0` | predicate changed to `x >= 0` or `x < 0` |
| `any` | changed to `all`, negated predicate |
| `min` | changed to `max`, reversed comparison with different tie behavior |
| string concat | operand order swap, separator placement change |
| field write | target field changed, overwrite order changed |
| indexed loop | skipped first or last element, wrong collection indexed |
| C pointer-length contract | skipped first element, stride greater than one, non-contract bound |
| own-property guard | prototype-including `in`, shadowable direct method call, shadowed `Object`, different static key |
| record-shape guard | missing null exclusion, missing array exclusion, unrelated property predicate |

For negatives, a concrete counterexample is part of the benchmark item.

## Usefulness score

The factory should not store every generated item. It should queue and promote items by a
usefulness score:

```text
usefulness =
  coverage_novelty
+ real_corpus_prior
+ detector_failure_value
+ verifier_confidence
+ minimality
- complexity_penalty
- duplicate_pattern_penalty
```

`real_corpus_prior` comes from the v5 labelset, under-merge diagnostics, field evaluation,
and idiom frequency in pinned repos. This keeps synthetic work pointed at patterns that
also occur in real code.

The executable real-corpus triage step is:

```sh
python3 bench/type4/prioritize_frontier.py \
  --json-out /tmp/nose-frontier-priorities.json \
  --markdown-out bench/type4/FRONTIER_PRIORITIES.md
```

The report ranks candidate axes by real-code frequency, repo and language spread, estimated
implementation cost, soundness risk, scope, and current coverage status. It is not label
evidence. It chooses what to investigate next; the generator/verifier/evaluator loop still
decides which items become benchmark cases. Broad-probe hits that are not true strict
semantic candidates should be counted as filtered overreach, not absorbed into extraction
patterns just to make coverage look higher.

## LLM-assisted workflow

LLMs can make the factory broader and faster, but only if their role is bounded. They should
produce proposals, explanations, triage, and shrink suggestions. They should not promote gold
labels by themselves.

The intended loop is:

```text
real corpus / synthetic spec
        ↓
LLM planner: choose a semantic category and propose a spec card
        ↓
generator: emit positive and hard-negative variants
        ↓
verifier: attach evidence or counterexamples
        ↓
detector: report exact matches, misses, and false merges
        ↓
LLM analyst: classify failures, suggest shrinks and new proof obligations
        ↓
human / CI gate: review frontier cases and accept only evidence-backed items
```

Useful LLM responsibilities:

- mine real-code patterns from the v5 labelset, under-merge diagnostics, and field-evaluation
  notes, then map them to transform tags;
- recommend the next under-covered matrix cells instead of letting generation drift toward
  arbitrary complexity;
- draft semantic spec cards before any code is emitted;
- propose hard-negative siblings close to each positive class;
- classify detector failures as extraction miss, lowering miss, value-graph under-merge,
  type/effect modeling gap, library idiom gap, or out-of-scope semantics;
- suggest shrinks that the generator/verifier/detector loop must confirm mechanically;
- draft proof obligations for new canonicalizations;
- write the short human-readable explanation for promoted benchmark items.

A proposal card should be structured enough that the generator can act on it without trusting
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

Any LLM-created item that lacks verifier evidence stays a candidate. Any LLM-created negative
that lacks a counterexample stays a candidate. This keeps the exact benchmark evidence-based
while still using LLMs to explore the frontier aggressively.

## Detector co-evolution loop

The factory is useful only if it changes the detector, not just the benchmark. Each accepted
iteration should leave four artifacts:

- a proposal card or matrix cell describing the semantic class;
- generated positive and hard-negative examples with evidence;
- a detector change that makes a previously missed exact positive converge;
- a regression check proving the nearby hard negatives still do not merge.

The operating loop is:

```text
generate benchmark slice
        ↓
scan with semantic mode
        ↓
frontier summary: missed positives grouped by computation, surface, and representation
        ↓
choose one narrow under-merge class
        ↓
add a failing convergence test
        ↓
patch the language frontend, idiom lowering, or shared value graph
        ↓
rerun positives, hard negatives, docs gate, and verifier checks
        ↓
record the result and generate the next adversarial sibling
```

This is enforced as a Definition of Done in
[`bench/type4/README.md`](../bench/type4/README.md). A loop that adds only proposal cards
or generated examples is a coverage-expansion loop, not a detector co-evolution loop.
The smoke gate is `scripts/type4-smoke.sh`, and frontier deltas should be stored as JSON
so recall changes and false-merge regressions are auditable.

Language-specific work is allowed, but only as a lowering step into the shared semantic
representation. A JavaScript `.filter().reduce(...)`, a Rust iterator chain, a Java stream,
and a loop in another language should all converge by producing the same value-graph shape.
When a patch is local to one frontend, the iteration should say so; when it changes the
shared value graph, the iteration should measure which other languages benefited.

To prevent frontier work from drifting toward one language family, each candidate should be
classified before implementation:

- `all-language`: expected to apply across most supported surfaces;
- `multi-language`: applies across several unrelated language families;
- `language-family`: applies mainly to one family, such as JavaScript/TypeScript;
- `single-language`: applies to one frontend only.

After a `language-family` or `single-language` loop, the next ordinary frontier should be
`all-language` or `multi-language` unless the narrower loop fixes a demonstrated strict
soundness bug. The prioritizer should be rerun before selecting that next axis.

Some cross-language convergence requires an explicit semantic contract. The current C list
contract is narrow: generated `int f(int *xs, int n)` cases treat `n` as the exact logical
length of `xs`, and generated aligned-array cases such as `int f(int *a, int *b, int n)`
treat `n` as the shared logical length of both arrays. C predicate reductions use `1/0` as
boolean `true/false`. The detector may consume that contract only for strict full
traversals; skipped-first and stride-two C siblings are generated as hard negatives.

This loop is breadth-first. It should prefer one missing semantic class across all relevant
surfaces over many increasingly complex variants in one language. A good iteration is small
enough to explain, but strong enough that the next generator round can create a harder
neighbor around it.

## Promotion rules

An item can enter the Type-4 gold set only if it satisfies all relevant rules:

- the semantic status is evidence-backed;
- positives pass the chosen oracle/property/symbolic checks under the declared semantics;
- negatives include a counterexample or precise non-equivalence witness;
- transform tags and semantic scope are explicit;
- expected detector behavior is explicit;
- the case has been shrunk;
- the case is not redundant with an existing item unless it fills a new matrix cell;
- the dev/heldout assignment is fixed before tuning.

Items that fail these rules may still be useful as generator seeds or triage notes, but they
must not affect the exact Type-4 score.

## Splits and anti-overfit controls

The benchmark needs more than a repo split. It should also hold out semantic structure:

- seen vs unseen semantic specs;
- seen vs unseen transform classes;
- seen vs unseen generator templates;
- seen vs unseen language pairs;
- seen vs unseen negative mutations.

This prevents the detector from merely learning the generator's surface grammar. A useful
held-out result should show generalization to a semantic class or representation form that
was not used for tuning.

The executable generator now marks same-surface `loop -> aggregate` positives as `dev` and
places cross-language positives, indexed-loop template positives, and all hard negatives in
`heldout`. The evaluator reports split-level recall and false merges. It also groups hard
negatives by `negative_tag`, currently including aggregate semantic mutations,
same-template semantic mutations, indexed-template semantic mutations, cross-template
semantic mutations, and C skipped/strided traversal contract negatives.

The default ring gate is the normal per-loop gate. The dense `--cross all` gate is a
periodic or pre-merge gate because the stronger cross-template negative set is much more
expensive.

### Coverage-preserving compaction

The full generated corpus should remain the source of truth, but the inner detector loop
should not scan every generated pair on every iteration. Routine gates should first select a
compact manifest that preserves the coverage axes that matter for exact Type-4 behavior:

- proposal and semantic computation;
- positive vs hard-negative status;
- dev vs heldout split;
- representation pair, especially `loop -> aggregate`, `loop -> indexed_loop`, and
  cross-surface `loop -> loop`;
- transform tags and hard-negative tags;
- language surface and cross-surface participation.
- semantic-axis and capability-state coverage, so proof-fact regressions are not compacted
  away.

This is not random sampling. A compact suite is acceptable only if it is selected from the
full manifest by explicit feature coverage, keeps the same evidence/counterexample records,
and copies only the selected source files into its own `sources/` tree so scan time actually
drops. Compact gates are allowed to find detector failures; full ring and dense all-cross
runs remain periodic validation that the selector did not hide an interaction.

## Relationship to existing data

The v5 refactoring-family labelset remains the product-quality benchmark. The Type-4 factory
adds a different measurement:

| asset | primary question |
|---|---|
| v5 refactoring-family labelset | Does `nose scan` surface useful refactoring candidates first? |
| Type-4 synthetic factory | Does exact semantic detection cover the intended equivalence classes without false merges? |
| future evidence-backed real subset | Do proven Type-4 cases occur and get detected in real repos? |

The bridge is candidate mining. Existing worthy and not-worthy v5 families can seed generator
templates and hard negatives, but only evidence-backed pairs should enter the Type-4 gold set.

## Initial implementation shape

The seed implementation lives in `bench/type4/`:

- `proposals.v1.json` — LLM-planned semantic proposal cards;
- `capabilities.v1.json` — current surface-by-surface proof-fact capability matrix;
- `ITERATIONS.md` — the first ten coverage-expansion iterations and smoke results;
- `generate.py` — deterministic source/manifest generator for all supported language
  surfaces;
- `select_cases.py` — coverage-preserving compaction for routine gates;
- `eval_manifest.py` — compares `nose scan --mode semantic` output against the manifest;
- `schema.json` — pair-level manifest schema;
- `README.md` — commands and current smoke numbers.

The initial implementation deliberately starts with a small semantic DSL and broad language
surface coverage:

1. Define the benchmark schema for pair-level items: locations or generated source, status,
   semantic scope, transform tags, evidence, expected detector behavior, split.
2. Implement a tiny semantic-spec DSL for pure list/int computations.
3. Build emitters and verifier support across all supported languages, including embedded
   script surfaces, and use the coverage matrix to decide which language/transform cells are
   filled first.
4. Generate positives for `sum`, `count`, `any`, `all`, `min`, `max`, `map`,
   `filter`, `abs`/sign-normalization, and aligned `zip`/dot-product reductions.
5. Generate hard-negative siblings for every positive template.
6. Run `nose scan --mode semantic` and record under-merges and false merges.
7. Promote only shrunk, evidence-backed cases.

The long-term direction is an adversarial semantic test factory: generator creates frontier
cases, verifier proves or refutes them, detector failures become minimal regressions, and the
coverage matrix decides where the next breadth-first expansion goes.
