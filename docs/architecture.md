# Architecture

nose lowers every language into one normalized intermediate language (IL),
designed so that semantically-equivalent code converges toward identical
structure, then finds and ranks duplication on top of it. The IL is **not** the
deliverable — it's the substrate.

The long-term boundary for language and library meaning is the
[semantic-kernel](semantic-kernel.md): a pack-based semantic contract layer that
makes evaluation strategy, effects, library APIs, laws, and proof status explicit
instead of scattering semantic assumptions across the engine. The first internal
facade is in `nose-semantics`; the pipeline below describes the current mixed
state while migration continues.

## North star

nose's exact Type-4 goal is **not** to guess arbitrary semantic similarity. It is
to become the strongest detector for the semantic equivalence classes it explicitly
models: broad cross-language coverage, exact fingerprint equality, and a defensible
soundness contract for every accepted semantic match.

That means recall work should raise more real equivalences to exact convergence
rather than lowering thresholds around partial similarity. A new semantic match is
only a durable win when it can be backed by the independent interpreter oracle,
counterexamples for rejected rewrites, and, for core canonicalizations, machine-checked
proofs. False merges are product bugs; fuzziness belongs in candidate generation and
review-oriented `near` scoring, not in the exact `semantic` channel.

## The pipeline

```
source ──tree-sitter──▶ raw IL ──normalize──▶ canonical IL ──▶ units + features
                                                                      │
                                       MinHash + LSH candidate gen ◀──┘
                                                  │
                          structural + value-graph scoring ──▶ clusters ──▶ ranked families
```

1. **Lower** ([languages](languages.md)): tree-sitter parses each file; a per-language pass
   walks the CST and emits raw IL using a small, desugared core node set. Every
   node copies its source span, so every match traces back to exact lines.
   Frontends also emit semantic evidence records when that core IL would
   otherwise erase exact source, domain, import, symbol, type, guard, place/effect,
   library API, or sequence-surface distinctions needed by semantic contracts,
   then tag syntactic unit boundaries (function/method/class/block), which gives
   detection accurate boundaries for free.
   Rust runtime-type lookup first checks for a matching asserted import before
   traversing enclosing scopes. This negative prescreen grants no type evidence;
   candidates still require the existing visibility, shadowing and dependency checks.
2. **Normalize** ([normalization](normalization.md)): a fixed sequence of passes canonicalizes
   the IL — desugaring (with idiom canonicalization), alpha-renaming, an oracle cutoff,
   recursion-to-iteration normalization, dataflow propagation, control-flow normalization,
   algebraic and operator canonicalization, and a hash-consed **value graph** (GVN) that
   captures *what the code computes*, invariant to temporaries, statement order, and common
   subexpressions. See [normalization](normalization.md) for the exact pass order.
   **Declarative languages** ([CSS and HTML markup](languages.md)) branch off after
   desugar and skip the imperative phase entirely: their exact fingerprint is a
   domain-specific *computed-style* / *rendered-DOM* canonicalization, dispatched by the
   unit-root kind rather than the GVN — see [normalization › declarative (CSS/HTML)](normalization.md).
3. **Extract units & features**: frontend units are augmented with bounded
   sub-function units: control-flow blocks (`loop` / statement `if` / `try`) and
   exact-safe statement fragments whose whole value subtree stays inside the reported
   source span. Exact fragments carry a first-class classification, contract, and
   behavior oracle — see [fragment-contracts](fragment-contracts.md). Each unit becomes a
   multiset of subtree-shape hashes, a value-graph fingerprint, a pre-order linearization
   for alignment, a MinHash signature, plus literal- and return-value multisets used by
   the strict precision gates.
   Corpus extraction defers MinHash until the owned feature arrays are available, then
   signs each equal multiset once in parallel. Full slice equality resolves hash collisions.
   Signatures are copied for repeated users and moved into their last user, keeping the
   signature-buffer count within the final output's requirements. Every public corpus or
   per-file extraction result is fully signed; serialized feature layouts stay the same.
4. **Candidate generation**: the selected detection channels decide which candidates exist.
   `semantic` uses value-fingerprint MinHash signatures plus exact-value buckets, `near`
   uses shape MinHash signatures, experimental `abstraction` reuses the near candidate
   stream, and `syntax` bypasses unit LSH with a Rabin-Karp token-stream pass.
   Every reportable pair in an LSH bucket reaches scoring, including buckets above 48 units.
   A chain/star before scoring is insufficient: rejected hub edges can disconnect
   real clones. Identical memberships and overlapping pairs are deduplicated across
   value, shape and exact routes before allocation and budget accounting. Dense buckets
   still have quadratic cost. Per-worker timestamp arrays deduplicate neighbors without
   allocating a hash table per left endpoint. Equal line spans in the same file are excluded:
   ordinary scoring rejects nesting, and connected descendant scoring requires strict
   containment. Cross-file pairs and strictly nested seeds remain eligible. The same rule
   applies to budget preflight, clean detection and incremental state; anchors keep their
   existing frequency and per-bucket caps. No bucket is reduced to a connectivity skeleton.
   Product analyses above one million pairs automatically stream stable candidate batches.
   Every eligible ordinary pair is still covered by scoring, retaining accepted edges and
   only the existing top/first connected seeds globally and per file. Distinct-input
   analyses score bounded batches in parallel. Repeated-input analyses can instead
   traverse the exact candidate relation in compressed rows, as described below.
   Ordered compaction preserves score ties and nested-seed order. This bounds temporary candidate/rejected-score storage,
   not source features or accepted output. Smaller analyses retain the indexed path; raw
   diagnostic dumps still materialize the requested candidates. There is no implicit
   user-facing pair ceiling; explicit CLI/environment ceilings remain fail-before-results.
5. **Accept / score**: `semantic` accepts only exact-safe value-fingerprint equality, `near`
   scores candidates with structural alignment (RANSAC) plus weighted shape/value
   Jaccard and accepts above the inline `near:T` threshold (default `near:0.70`), and
   may separately admit a bounded pair-local connected block/statement-window witness
   without changing the ordinary scorer ([connected witnesses](connected-witness-821.md));
   a separately priced same-unit route may report two disjoint concrete subtree windows under
   one enclosing unit ([bounded same-unit windows](bounded-same-unit-windows-832.md)). It is a
   `near` refactoring witness, not an exact-fragment proof, and bare scope-container blocks are
   ineligible;
   `syntax` emits duplicated runs above the line/token floors. Conservative suffix-block
   bounds skip token extension when the remaining stream cannot meet the line/token or
   operation floor. Such streams still record first k-gram occurrences, preserving later
   matches against differently formatted copies. Bounds include the whole current block,
   so nonmonotonic parent/child source spans cannot exclude a qualifying suffix. Experimental
   `abstraction` then checks same-language near-style families for one shared
   supported literal-leaf hole position and attaches a weak witness instead of an
   exact claim. Same-language `near` and shared-core families can be additionally graded
   by anti-unifying representative copies' value graphs — "equal except *k* holes", each
   a candidate parameter or named transformation such as `async-mirror`, with a
   soundness-relevant referent check ([graded-witness](graded-witness.md)).
   Large batched analyses may reuse a score for exactly equal scoring inputs. Builtin
   scorers expose analysis-local input classes; structural scoring reads only the same
   complete input view whose equality defines those classes. Hashes accelerate lookup,
   while full equality checks all score fields. Channel composition intersects class
   partitions, and custom scorers opt out by default. Reuse activates when at least half
   the units repeat an input class. A row further requires identical membership in
   every candidate bucket and equal eligibility for connected-seed pricing. That
   refinement makes its candidate relation complete: an outside endpoint reaches all
   row members or none. Row sizes give the exact unordered pair count; sparse
   shared-span counts subtract only excluded pairs. Rejected ordinary pairs that
   cannot seed a connected witness need no location-pair expansion. Structural
   scorers can prepare inverted multiset indexes: each feature contributes the
   minimum of its two multiplicities to the intersection. Value and shape classes
   share these integer counts across a row, retaining the original floating-point
   formula, every accepted and rejected score, and all subsequent scoring gates.
   Common features record absent classes instead of repeatedly visiting present
   classes. Anchor postings preserve duplicate-hash occurrence order and the exact
   maximum shared weight; source-line metadata remains outside that calculation.
   Alignment is evaluated lazily once per identical right-hand sequence in a row.
   Structural input and alignment classes use precisely the existing 600-entry
   alignment prefix; differences beyond that already unread suffix do not trigger
   another identical score calculation. The stored source features remain complete.
   Sparse rows compare estimated feature-merge work with posting traversal and
   class-array work before choosing direct scoring. A low representative count
   alone cannot make long repeated fingerprint scans cheap. Prepared exact scores
   and the structural exact shortcut reuse full-equality-checked value classes,
   retaining each unit's exact-safety and minimum-size gates. Custom scorers can
   decline preparation. Accepted pairs and
   potentially eligible connected seeds still undergo each original location check.
   Bounded streaming selectors retain the original global and per-file seed
   priorities using source-pair order for ties. A rejected row product can be skipped
   only when its best possible pair cannot improve any eligible reservation,
   including nested and per-file seeds. Accepted edges restore source-pair
   order before floating-point group aggregation. Thus this is an exact quotient of candidate work, not a
   pre-scoring connectivity skeleton. Custom scorers retain the ordinary path unless
   they explicitly declare interchangeable input classes. Neither classes nor score
   maps persist beyond the analysis.
6. **Cluster & rank**: union-find over accepted pairs/runs forms clone groups, which
   are grouped into **families** and sorted by refactoring value (removable lines
   × similarity × cross-directory/-file/-language spread). See [usage](usage.md) for how the
   ranked report reads — and [`nose query`](usage.md#nose-query) for exploring the same
   families dataset interactively.

## Crates

`Il` separates serialized arena contents from derived indexes. Mutable field access
and `Il::edit` invalidate those indexes before granting an exclusive borrow.
`push_evidence` preserves incremental append indexing; `evidence_mut` invalidates
only evidence lookups. For repeated record updates, `evidence_record_mut` retains
the index when id and anchor stay unchanged, avoiding a whole-evidence rebuild
per metadata update. Serialization keeps the existing flat arena representation.
The lazy unique-parent index covers the whole arena, including unreachable nodes.
Repeated edges from one parent remain unique; multiple distinct parents return no
answer. Object-key-view evidence uses this index after checking the call's API shape,
preserving its ambiguity and `with`-scope guards without repeated arena scans.

`DetectOptions::validate` produces an immutable execution plan and rejects invalid
thresholds, MinHash layouts, and incompatible channel prerequisites before detection.
The CLI returns configuration errors; infallible library detection entry points panic
on invalid programmer-supplied options. Group evidence is a tagged `WitnessEvidence`
enum, with required measurements carried by the corresponding variant. Its JSON
retains the existing `kind` and measurement field names.

A Cargo workspace; data flows left-to-right through them.

| crate | role |
|---|---|
| `nose-il` | arena IL model (`Vec<Node>`, `NodeId(u32)`, out-of-line edges), unit facets, node/evidence catalog, provenance spans, semantic evidence records, interner, serialization, IR verifier |
| `nose-semantics` | builtin semantic facade: language profiles, evidence/source-fact helpers, type-domain contracts, effect/operator/module/stdlib predicates, API contracts, and exact-channel proof obligations |
| `nose-frontend` | tree-sitter parse + per-language CST→IL lowering and builtin evidence emission (one module tree per language, incl. declarative CSS/HTML; `<script>`/`<style>`/markup region extraction for Vue/Svelte/HTML) |
| `nose-normalize` | the normalization passes, inferred immutable binding-domain evidence, and the value graph (GVN) |
| `nose-detect` | unit/feature extraction, exact-fragment extraction, strict exact-safety gates, MinHash/LSH, scoring, clustering, test/generated scope tagging, refactor ranking, and query-surface policy, including the divergent-edit decision policy |
| `nose-eval` | benchmark scoring (precision/recall, pooled, stratified) — see [benchmark](benchmark.md) |
| `nose-markdown` | self-contained same-language Markdown prose near-duplicate domain (char-n-gram MinHash-LSH + winnowing + containment → TF-IDF rank → span witness), distinct from the value-graph code engine — see [markdown-duplication](markdown-duplication.md) |
| `nose-cli` | the `nose` binary and process boundary: argument models, command dispatch, config/cache/baseline plumbing, query dashboard/JSON/open views, verify/oracle reporting, recall-loss reports, and local diagnostics |

The current semantic assumptions these crates share are maintained in
[semantic kernel](semantic-kernel.md) and
[semantic-pack architecture](semantic-pack-architecture.md) as current contracts.
The incremental crate/module split is ratcheted in [refactoring-ratchets](refactoring-ratchets.md);
new code should follow the focused owners there rather than growing dispatcher or facade roots.

## Design choices worth knowing

- **Arena, not boxed trees.** The IL is a flat `Vec<Node>` with `NodeId(u32)`
  indices and out-of-line child edges — cache-friendly and cheap to serialize,
  which is what makes per-file feature caching ([continuous-integration](continuous-integration.md))
  possible.
- **Index-backed lookups on the arena.** Nodes are immutable once an `Il` is
  built (passes rebuild the arena), so `Il` carries lazy indexes — nearest
  enclosing scope, span → nodes, scope → assignments, and the evidence anchor
  index (span buckets, binding-hash buckets, id resolution). Per-node helpers
  must query these instead of scanning `il.nodes`/`il.evidence`; the raw scans
  were the dominant query runtime cost until they were indexed ([experiments §BQ](experiments.md)).
- **Interner-independent features.** A unit's features are content-derived
  hashes, not interner ids, so they're portable across runs — the basis for the
  content-hash cache.
- **Delete-capable incremental detection.** Cached unit-artifact identities feed persistent LSH
  bucket membership and pair contribution counts. Additions update their buckets; removed scores
  dirty and rebuild the old connected components instead of relying on append-only union-find.
  Connected/same-unit evidence includes its file-context digest, syntax runs are partitioned by
  shared k-grams, and line-IDF/family weights use source-frequency deltas. The full storage and
  invalidation contract is in [portable cache artifacts](portable-cache-artifacts.md).
- **Transactional, bounded cache state.** Immutable, checksummed records are published before one
  complete generation manifest; its `CURRENT` pointer is replaced last. Concurrent query processes
  share the store while prune/clear wait for writers. Schema-scoped garbage collection and a 5 GiB
  default budget make eviction a performance event rather than a correctness event.
- **Determinism is a hard invariant.** Output is byte-identical across runs *and*
  thread counts. File ids come from a sorted path list; nothing iterates a
  `HashMap` into the output. There are tests for both.
- **Parallel by default.** Discovery, lowering, and the detection stages run
  under rayon; the LSH stage is sort-based so it parallelizes cleanly. See
  [experiments](experiments.md) §T for the throughput work.
- **The behavioral fingerprint is sound by intent.** The value graph's contract
  (§AJ) is *fingerprint-equal ⟹ behavior-equal* — two units sharing a value-graph
  fingerprint must compute the same thing; a *false merge* is a bug, not an accepted
  approximation. Two mechanisms enforce it. (1) A tree-walking **interpreter oracle**
  (`nose verify`) runs every interpretable unit on a battery of inputs and flags any
  fingerprint-equal pair whose behavior differs. Crucially it interprets the
  *pre-canonicalization* core IL, not the fully-normalized IL it fingerprints, so a
  behavior-changing canonicalization cannot mask itself (§AX). (2) A **canon-preservation**
  check requires each unit's core-IL behavior to equal its full-IL behavior — catching a
  bad canon even with no colliding twin. The core algebraic/control canonicalizations,
  recursion templates, IL arena invariants, fragment contracts, and oracle cutoff are
  additionally **machine-checked in Lean** ([formal-soundness](formal-soundness.md)).
  Both checks currently report zero violations; the fuzziness a clone detector needs lives
  in the *candidate* axis and its scoring, never in the behavioral base (the two-axis
  principle, §AH).

For *why* the normalization passes look the way they do, read [normalization](normalization.md).

## Analysis resource boundaries

Normal product queries have no implicit candidate-count ceiling. Above one million
unique pairs, stable batches replace the full candidate/rejected-score arrays while
preserving accepted evidence and the connected-seed selection policy. The source/unit
cache remains reusable, but large product populations bypass persistent pair indexes.
The ordinary 262,144-pair emission batch is scored in 4,096-pair parallel chunks.
Repeated-input rows retain one bounded score map per worker. Streaming heaps keep
only the existing global and per-file connected-seed reservations, comparing
scores and source-pair order as candidates arrive. No rejected-pair array needs
sorting; each worker's final reservations are merged under the same policy.
The row/bucket/source-span and inverted-feature indexes are linear in the input
and its memberships; per-worker intersection arrays scale with feature classes.
Repeated accepted rows share sorted right-hand targets rather than expanding each
left occurrence into another edge array. Their iterator retains every admitted
pair, exact score and source-pair order; group accumulation therefore uses the
same floating-point result. Unit-score rows count eligible pairs directly when
the running sum is an exactly represented nonnegative integer no larger than
2^53. Their sum saturates at 2^53, exactly where repeated +1 rounds back to
itself. Other heavily repeated rows index sufficiently long runs of bit-identical
scores. Within one floating-point exponent interval, two ordinary additions settle
half-spacing ties and establish the repeated bit increment. A jump remains inside
that interval; boundary crossings still execute ordinary additions. Short runs,
negative values and nonfinite inputs retain the ordered fold. Subnormal spacing,
rounding ties, overflow, sparse slices and the exact sequential result are tested.
Sparse same-file exclusions divide accepted targets
into slices, so accumulation needs no source or group lookup for every cross-file
pair. Connected pricing looks up only its selected pair
questions instead of copying the entire accepted graph into a hash set.
Before materializing coverage, the detector applies ranking's existing site
collapse and retains the strongest original edge per site pair, with the same
witness tie rule. Each retained site edge is backed by an actually accepted source
pair. Coverage explicitly distinguishes raw member coordinates from reported-site
coordinates. Large site graphs use 64-site blocks with a shared palette of exact
scores and witness kinds; mixed blocks preserve each value. Full relation counts
and group metrics precede that projection. Ranking and coverage obligations share
these immutable blocks through `AcceptedEdges`; iteration reconstructs exact edges
without allocating a second graph. Graphs with more than one million possible
reported-site pairs defer this final projection until a consumer reads their edges.
The owned recipe retains complete accepted rows, site mappings and witness inputs,
including the query's anchor floor. An exact nonempty fact lets ranking transfer
coverage obligations without forcing projection; concurrent readers share one
materialized graph. Small graphs retain eager construction. This postpones only
the internal edge representation, never scoring, grouping or coverage decisions. Repeated rows reuse connectivity only after
all remaining targets are in one component, so skipped unions are redundant.
Site projection also skips a later cross-file row occurrence only when an earlier
occurrence has the same reported site and complete witness inputs. Within-file
exclusions are always evaluated for the individual occurrence. Equivalent cross-file
right targets retain their latest occurrence, which covers every earlier left
endpoint. Equivalence compares site, score, exact-value eligibility and the ordered
anchor hashes/weights used by pair witnesses; source metadata is kept separately.
Distinct accepted rows and reported site graphs can still be large; these
representations introduce no evidence cap or candidate omission.
These internal execution choices do not change recall or explicit budget accounting.

`--max-candidate-pairs` and `NOSE_MAX_CANDIDATE_PAIRS` optionally impose a positive
work ceiling. This preflight counts the unique union across channels, excludes
ineligible equal-span same-file pairs, and fails without partial results if exceeded.
It applies to clean, cached and watch queries; research-detect accepts the environment
ceiling and still materializes its requested diagnostic dump. Library integrations can
call `ensure_candidate_budget` with their own limit.
Exact-only semantic runs generate equal-value buckets directly, because their
scorer cannot accept unequal value fingerprints. Fuzzy value LSH remains enabled
for near/abstraction runs. Incremental candidate indexes and cache option keys
use the same distinction.

Parser metadata and a constant-memory tree cursor check output before recursive
lowering: syntax depth is limited to 8,192 and the syntax tree to 2,000,000 nodes.
The stored descendant count bounds both size and possible remaining depth; the
cursor skips a subtree only when even a single chain would fit the depth limit. Exceeding
either produces a source-specific analysis error, including embedded script,
style and markup regions. Common scope and identifier traversals use explicit
work stacks; CLI workers reserve 64 MiB rather than 1 GiB. These are analysis
resource limits, not claims that arbitrarily deep source can be processed.

Witness source locations use a separate, optional per-unit occurrence map. Ordinary
fingerprints retain their original hash-consed graph and creation-span behavior;
only requested value-DAG export enables occurrence tracking. The map rejects
out-of-unit and foreign locations and leaves disjoint occurrences unavailable.
See the [graded witness contract](graded-witness.md) for the source-evidence boundary.


The concurrent string interner uses the same fast table hasher as other internal
indexes. This affects table placement only: symbol content hashes retain their
existing FNV-1a definition, and occurrence/order-dependent interner keys remain
excluded from persistent fingerprints.

C-header admission checks lexical hints before invoking its clean-C safeguard.
An unhinted header cannot be excluded by that safeguard, so it needs no extra
parse. Hinted headers retain the same rule: admission needs a complete,
error-free C tree within the original node/depth limits.

Tree-sitter's minimum error cost after stack condensation is the lower bound
used by its finished-tree selection. The small vendored delta also reports
positive finite minima through its progress signal, including missing tokens.
If no error-free finished tree exists, C admission can then cancel a parse whose
result is already false. Cancellation resets the pooled parser before another
file. Grammar, recovery, tree selection and header inclusion rules stay the same.
The [contributor workflow](contributing.md#vendored-parser-dependency) owns source
provenance and the corresponding CI requirements.

Source digest serialization writes hexadecimal digits from a fixed-size stack
buffer; its public 64-character lowercase representation stays unchanged. Site
edge construction reuses its last exact palette entry before consulting the
palette map, preserving score bits and witness categories.
