# Source regions, content identity, and correspondence

Nose identifies source-backed regions independently of clone-family membership.
An admitted singleton has the same region representation as a clone member.
This is a substrate for clone navigation, incremental analysis, refactoring
history, and caller-owned reviews. The detector's semantic equivalence claim
and a historical correspondence are separate claims.

## Four contracts

| Object | Meaning | Changes when |
|---|---|---|
| Source address | SHA-256 of the analyzed original buffer and a half-open byte range; a logical file path distinguishes occurrences | The file snapshot or selected coordinates change |
| Region content key | Versioned SHA-256 over exact selected bytes, language, unit origin/kind, and fragment classification | Selected content or classification changes |
| Correspondence | Evidence relating observations in two explicit snapshots | The supplied snapshots, extraction profile, or candidate budget change |
| Review record | Caller-owned decision with target relations, reason, scope, and applicability conditions | The caller makes or revises a decision |

Line numbers remain navigation/display coordinates. Whole-unit and admitted
fragment selectors preserve frontend byte spans. Syntax copy-paste and connected/bounded-window selectors
include entire selected lines, preserving CRLF and final newlines. For copy-paste
runs, the module container is excluded from the source selector: its whole-file
span is not evidence that every byte in the file was matched. Existing navigation
spans remain unchanged and can be wider than the selected source region.
The source digest always covers the original containing buffer, including for
embedded Vue/Svelte/HTML regions; masked frontend buffers are not original sources.
Invalid or absent spans produce unavailable identity, never a clamped selection.

These are content-derived references, not authentication of a supplied snapshot.
A consumer must trust the snapshot producer. Comparing snapshots checks their
schema, addresses, and uniqueness; it cannot certify external evidence or recover
an edit history that was never recorded.

## Query identity

[Query JSON](query-json.md) v10 adds `locations[].region`,
`locations[].region_key`, and `families[].review_key`, each explicitly nullable.
`region` has `source_digest`, `start_byte`, `end_byte`, and `content_digest`.
All digests are 64 lowercase hexadecimal SHA-256 digits.

`region_key` has domain `nose.region-content/v1`. `review_key` has domain
`nose.review-content/v1`: it binds the sorted, duplicate-preserving multiset of
member signatures, member analysis fingerprints and relative shared selectors,
the direct accepted-edge signatures, witness kind/value-node evidence, and
sorted semantic-law provenance. Hash input uses length framing and deterministic
named MessagePack records. Internal Rust `Hash` is not the public key encoding.
Ranking, paths, absolute lines/bytes, presentation order, and optional grading
are excluded. Keys can be shared by different occurrences and different
families, including graphs whose equal-content vertices cannot be distinguished.
They are content/evidence classes, not graph-isomorphism or lineage identifiers.

Existing family/member IDs, baseline matching, structured ignores, and SARIF
identities retain their contracts. A changed member multiplicity changes the
review key; replacing a member with indistinguishable content/evidence need not.
Test/production scope is a separate review condition, not a path hidden in the key.
Consumers must check the actual target relation and their scope policy before
reusing a decision. Key equality alone never approves newly copied code.

Review keys cover every detector witness kind, including abstraction templates,
connected/bounded windows, and locked external near/exact pack evidence. Abstraction
signatures bind the claim, template, hole positions/classes, and caveats; representative
left/right lines and classes are presentation only. Pack signatures bind pack/row content,
lane, trust/assurance, dependency coordinate and versions, the sorted multiset of dependency
source-content digests, receipt content for exact claims, caveats, and member-relative call
selectors. Dependency source paths and absolute occurrence paths/lines are excluded.
Family pack summaries must be fully accounted for by member evidence.

The base `nose.review-content/v1` and `nose.review-member/v1` encodings remain unchanged.
Completing source/proof projections corrects some pre-release v10 keys: whole-file
syntax containers no longer bind unrelated headers, occurrence-salted near analyses
no longer bind absolute coordinates, and out-of-unit anchors bind their own source. Additional evidence uses framed, versioned domains:
`nose.review-abstraction/v1`, `nose.review-pack-member/v1`, and
`nose.review-shared-source/v1`. The last binds the actual selected source content when an
inlined shared anchor is outside the caller; its distance from the caller is not identity.

Keys are unavailable only when required source/evidence is missing or inconsistent
(e.g. an invalid span, an unlocated foreign-source anchor, or an unaccounted pack claim).
A caller must not substitute the old family ID when the new key is null. Discover support
through `query_region_identity_v1` and `query_review_key_v1` through the
[capabilities contract](capabilities.md), which lists supported features.

## Explore changes between saved analyses

| Existing infrastructure | Role in this extension |
| --- | --- |
| Query dataset | Captures admitted families with resolved analysis settings, before presentation/ignore selection |
| `since=FILE` / baseline | Existing acceptance-based family status; retains its original meaning |
| `regions snapshot/compare` | Singleton-aware unit census and conservative region correspondence; the matcher is reused on captured family members |
| `base=` / `semantic_change` | Existing missed-propagation and semantic-delta evidence; integration remains a later qualified step |
| Stage CAS | Existing analysis reuse and dependency invalidation; capture reuses the normal cached pipeline |
| Watch v1 | Complete dashboard replacement, not a complete family-census artifact; its protocol is unchanged |

```sh
nose query src --save-analysis before.json
# Edit source, then capture with the same analysis settings.
nose query src --save-analysis after.json
nose query --before before.json --after after.json
nose query --before before.json --after after.json group=reason
nose query --before before.json --after after.json evidence=recheck
```

Follow each `next:` command to narrow a group or open `change=<id> full`. Human and
JSON views use the same selection; `--format json` carries executable `next` links.
`nose capabilities` exposes `query.analysis` with fields, values, views, formats,
limits and syntax. `--before`/`--after` take no source roots or analysis flags:
the saved inputs own that context. Generated commands preserve both inputs, the
candidate budget and filters, including shell-quoted paths. A capture's initial
next command compares it with itself, so its evidence is immediately explorable.

`--save-analysis` writes a new `nose.analysis/v1` file and never overwrites an
existing one. It captures every admitted code family from the normal dataset
before display limits, surface classification, opportunity folding, and structured
ignore/baseline application. It accepts normal analysis modes, config, roots and
cache settings but rejects query terms, explicit ignores, gates, watch and baseline
writes. Configured ignore decisions do not filter the capture. Markdown findings,
singletons outside detected families, and source bodies are not included. The
population is explicitly `admitted-query-families`, not a census of all source.
Use `regions snapshot` for the separate singleton-aware unit census.

The artifact records resolved thresholds/channels, engine profile, exclude rules,
pack influence and lock decision, roots and the working-directory base for member
paths, scanned/skipped counts, every family's
member source addresses, review keys and independent evidence projections. Details
include witness kind/size, pack/dependency/receipt provenance, semantic laws and
abstraction templates. Internal analysis fingerprints remain opaque: a changed
analysis projection is not a reconstructed semantic edit. Root paths are provenance,
not content identity; consumers still own project/target scope when connecting reviews.
Filesystem discovery follows the frontend's normal ignore and language admission
rules; ignored or unsupported files are outside this population. Changing discovery
rules requires treating the resulting observations within their declared scopes.

Comparison uses only the two explicit artifacts, bounded to 128 MiB each, and
reuses conservative region correspondence to propose family relations. It spends
one combined budget on region candidates and family membership-index visits
(default 100,000). `matched` requires exact, unambiguous region membership;
`candidate`, `ambiguous`, `unresolved`, `unmatched-current` and `budget-exceeded`
remain non-approving evidence. Simultaneously moved identical copies can remain
ambiguous. A unique overlapping family can be a membership-change candidate,
never automatic split/merge ancestry. Evidence reuse is disabled for incompatible
profiles, incomplete inputs, missing required evidence or incomplete comparisons.

The `nose.analysis-changes/v1` response provides dashboard/list/group/change views,
input profiles and roots, completeness, work counts, total/selected/shown counts,
change rows and next commands. Filters use `reason`, `correspondence`, `evidence`
(`retained` or `recheck`), `scope`, `lang`, `path` and `witness`; equality/set OR,
negation and path substrings follow query's familiar syntax. `group=FIELD` facets
the selection. `change=ID` selects an unambiguous change-observation prefix, separate
from existing family `id=` and many-to-one `review_key`. Each detailed item embeds
its before/after observations, including past members absent from the workspace.
Source text is explicitly `not-stored`; no implicit filesystem read fills it in.

Reasons may overlap: member multiplicity/content, source address, scope, witness,
analysis, packs, laws, abstraction and review evidence can all change together.
These are observed facet differences, not causal attribution to a particular edit.
`unchanged_evidence` means an unambiguous matched family has the same non-null
review key and scope under compatible profiles with complete coverage. It never
approves a newly added copy. `evidence=recheck` includes uncertainty; it is not a
consumer's final review disposition.

`top=N` only limits displayed rows/groups (`top=0` emits all); group counts can
overlap and display truncation never changes the comparison. Missing rows mean
unmatched observations within this population, not deleted code or completed
refactoring. Ordinary dashboard JSON, filtered lists, baseline files and region
snapshots are rejected as analysis inputs rather than treated as complete censuses.
This surface does not change `since=FILE` baseline statuses, `base=`/`semantic_change`
evidence or gates, SARIF, or watch v1's complete-dashboard replacement contract.
The comparison and capture work run only when requested; ordinary query detection
and ranking are unchanged. Reproduce the four-mode header-edit and legacy-output audit
with `python3 bench/regions/analysis_changes.py --nose target/release/nose`;
`--baseline-nose <binary>` additionally compares ordinary output and the existing
`since=` workload. The [audit harness](../bench/regions/analysis_changes.py) records
retained/recheck counts, output sizes, work budgets and worker equality. See [agent recipe](agent-recipe.md) for the exploration loop.

## Explicit region snapshots

```sh
nose regions snapshot . > before.json
# Edit or move source files.
nose regions snapshot . > after.json
nose regions compare before.json after.json > correspondence.json
nose regions compare before.json after.json --max-candidates 10000
```

`nose.regions/v1` captures all units admitted by the default normalization and
unit-extraction gates at minimum one line/token, including singletons. It is not
an inventory of every AST node or every possible slice. Paths are relative to
the capture root (the containing directory for a single file). The profile
records the engine package version and fixed extraction settings. External
semantic packs and custom query settings are not part of this command's v1 profile.
Input snapshots are bounded to 128 MiB each.

Each region carries an `observation_id`, `file`, `lang`, `kind`, optional `name`,
`in_test`, source address, `content_key`, `analysis_key`, and nullable `value_key`. An observation ID
binds its path, language, kind, source region, and content key. Its analysis key
summarizes the admitted value/return/guard features, exact-safety facts,
fragment proof facts, and semantic laws. It is not a universal semantic hash. The detector deliberately salts some unproven
operations with source coordinates to prevent false merges. Only those units are
replayed for review analysis with deterministic first-use occurrence labels; all
original detection fingerprints and admission decisions remain unchanged. File-level
structural and literal-sensitive subtree hashes are shared across unit builders and
review replay rather than repeatedly traversing the entire file. Distinct
unproven occurrences remain distinct during replay. These review-only values are
never supplied to equivalence or candidate matching. `value_key` indexes the value fingerprint
only for units admitted by the strict-safety gate. Equal values propose a
refactoring candidate; they do not establish shared history.
`unavailable_regions` counts admitted units without valid source provenance.

`nose.region-correspondence/v1` consumes only the two explicit snapshots, without
reading the workspace, Git, or a mutable cache. It returns `profile_matches`,
`complete`, `candidates_examined`, and `correspondences[]`:

| Kind | Interpretation |
|---|---|
| `unchanged` | The same snapshot address is present |
| `content-match` | Exact region content maps to one candidate after reserving unchanged addresses and checking competing claims |
| `value-candidate` | Admitted value fingerprints agree despite different source/name/location; advisory only |
| `modified-candidate` | A unique file/language/kind/name candidate has different content; advisory only |
| `copied-candidate` | Unmatched equal content appears while an old occurrence is retained; the donor is not asserted |
| `ambiguous` | Multiple candidates or competing old observations; no forced tie-break |
| `unresolved` | An old region has no eligible correspondence |
| `unmatched-current` | A current region has no asserted old correspondence |
| `budget-exceeded` | The candidate bucket could not be examined completely |

`before` is an old observation ID or null; `after` is an ordered array of current
IDs. Ordering is deterministic regardless of input array order. Content/name
indexes avoid an all-pairs comparison. The global candidate budget defaults to
100,000. An over-budget bucket emits no truncated candidate list that could be
mistaken for uniqueness. `complete` describes provenance/budget coverage, not
successful historical identification; ambiguous/unresolved outcomes remain valid.

`unchanged_evidence` requires an unambiguous unchanged/content match, equal
analysis fingerprints, equal test scope, equal profiles, and complete coverage.
It is a fact for a caller's policy, **not an approval or a proof of ancestry**.
A delete-and-identical-recreate can be observationally identical to a move.
Persist correspondence results as evidence and keep review decisions separately.

## Engine boundary and further validation

The shipped matcher is conservative exact-content correspondence plus named
modification and admitted-value candidates. It does not claim automatic split/merge ancestry,
arbitrary extraction/inlining tracking, similarity confidence calibration, or
permission to rewrite code. A fragment that survives extraction verbatim can
match by content; rewritten value-equivalent regions can become advisory candidates; other
refactorings remain unresolved. Future
refactoring-aware alignment belongs behind the same evidence boundary and must
be benchmarked against this baseline before it can improve decision reuse.

The source buffer is shared across a file's embedded regions and normalization.
Source signatures are computed from already-loaded buffers, not family-level
filesystem reads. Raw/resolved cache hits reattach the current loaded snapshot;
unit/stream cache schema v5 preserves source evidence on warm and incremental
paths. Source buffers in syntax streams trade cache/storage space for exact
original-byte provenance; performance evidence must include that cost.

Verification covers byte selectors/Unicode/CRLF, source edits and moves,
multiplicity, order independence, proof changes, repeated identical copies,
competing matches, scope/profile/dependency changes, missing provenance, and
candidate caps. CLI tests exercise uncached/cold/warm/edited cache paths and
comparison after deleting the workspace source. See the
[contribution gates](contributing.md) for workspace checks. Run the reproducible
[controlled edit evaluation](../bench/regions/evaluate.py) with
`python3 bench/regions/evaluate.py --nose target/release/nose`.

## Development verification (2026-09-05)

The initial implementation passed 2,271 workspace tests and the controlled evaluation's
28 cases across Python, JavaScript, Rust, and Go. Every controlled case was also
repeated with one and four Rayon workers; snapshots and comparisons were byte
identical. No edit/copy/ambiguous/budget case incorrectly reported unchanged
evidence. This small constructed population does not estimate a real-history
false-transfer rate, and it does not qualify automatic review reuse.

A 1,000-copy unit test shifts every file and requires exactly 1,000 candidate
examinations within a 1,000-candidate global budget. Tests also cover a moved,
renamed, value-equivalent rewrite that remains an advisory candidate.

Three interleaved uncached runs of each pre-change/current release binary on the
same `crates/` tree preserved every legacy query field after removing only the
new identity fields and schema version:

| Mode | Before median | Current median | Before peak RSS | Current peak RSS |
|---|---:|---:|---:|---:|
| semantic | 0.948 s | 0.936 s | 230.8 MiB | 237.7 MiB |
| syntax | 0.637 s | 0.651 s | 270.8 MiB | 276.0 MiB |

These are local development samples, including process startup, taken alongside
other checks. They expose a small memory cost; they do not establish a speedup
or a release performance bound. Reproduce the query workload with
`nose query crates all top=0 --mode semantic --format json` and `--mode syntax`.

The review-identity completion audit runs all four query modes on the same Rust corpus,
checks coverage by witness kind, compares the legacy output with an optional baseline
binary, and repeats after a comment is inserted in every Rust file and the tree is moved.
It also compares two/four worker outputs. Run it with
`python3 bench/regions/review_identity.py --nose target/release/nose`;
`--baseline-nose <binary>` additionally checks legacy detection output.
External near/exact integration tests use actual locked packs; they check source movement
and content edits, with near-lane cold/warm/incremental caches and dependency-version
changes. Unit contracts additionally invalidate changed receipts, pack/row content,
trust, caveats, template holes, and out-of-unit source anchors.

### Completion evidence (2026-09-05)

On the same `8aac5f05` Rust source corpus, comparing the `edbc6469` baseline binary
with the completed implementation yielded:

| Query mode | Before available / total | After available / total |
|---|---:|---:|
| semantic | 42 / 42 | 42 / 42 |
| syntax | 680 / 680 | 680 / 680 |
| near | 597 / 683 | 683 / 683 |
| abstraction | 0 / 12 | 12 / 12 |

All 1,417 query rows retained their review keys after an unrelated leading comment
was inserted in every Rust file and after the corpus tree moved. Every legacy family
field matched between the baseline and current binaries, and two/four worker output
was identical. The correction changed 12 previously available near keys and 3 syntax
keys in this corpus; semantic keys were unchanged, and abstraction had no previously issued keys.
The supported evidence kinds include exact, copy-paste, structural, shared-sub-DAG,
connected, and bounded-window witnesses. Locked external packs are exercised separately
by the CLI integration tests, since the Rust self-query does not use external packs.

The completed implementation passed 2,279 workspace tests, Clippy with warnings denied,
Rust API documentation, and the 28-case, four-language snapshot correspondence evaluation.
The current product tree's blind oracle replay retained 54 exact groups with zero false
merges and zero canonicalization violations. This establishes Nose's review-identity
contract without requiring access to a downstream repository; downstream scope and
approval policy remain caller-owned.

The final hash-sharing implementation (`acb121de`) reproduced the coverage and movement
checks above, passed all 2,279 workspace tests, and passed the complete
`./scripts/check-ci-local.sh --fast --jobs 2` run. All seven pinned semantic-smoke
repositories had byte-identical semantic query JSON relative to `edbc6469`.

The initial implementation's isolated performance follow-up confirmed an asciidoctor
normalization/extraction increase of 14.85 ms (25.74%). The file-level literal-sensitive
hash sharing removed the repeated whole-file traversal. The final default smoke had
no confirmed regression but exited unsuccessfully because the six-block asciidoctor
stage estimate straddled the 5 ms order-specific boundary. Both selected repositories
were therefore measured independently with 12 alternating blocks and five samples per
observation, alongside an equally sized same-binary control. This separate checker run
passed every stage with the unchanged 5% **and** 5 ms thresholds; the original failed
smoke receipt is retained rather than rewritten as a pass.

| Follow-up repository | Baseline median | Current median | Order/control-adjusted delta |
|---|---:|---:|---:|
| alacritty | 170.58 ms | 170.65 ms | -1.56 ms / -0.91% |
| asciidoctor | 126.60 ms | 129.21 ms | +1.76 ms / +1.39% |

These are local runtime measurements, not a universal performance bound. Raw primary,
focused, and expanded-sample reports are in the ignored development artifact directory
`target/review-identity-completion/semantic-smoke-final/`; the successful follow-up is
`balanced-check-status.json` / `balanced-summary.md`.

## Analysis exploration verification (2026-09-05)

The first #987 delivery (`2e9ad816`) adds capture and offline EDA; it does not
implement the later divergent-edit policy, automatic extraction history, cache
optimization or history-ranking tranches. All 2,291 workspace tests passed,
including seven CLI scenarios and five comparison-core cases. CLI cases execute
emitted next commands with quoted paths, multi-root/config inputs, missing source,
profile/coverage/budget changes, new copies, cache reuse, invalid combinations and
partial-input rejection. Core tests cover actual pack receipt changes, analysis
and member test-scope changes, ambiguous moved copies and disappearing families.
Clippy, Rust API documentation, wiki checks and the complete
`./scripts/check-ci-local.sh --fast --jobs 2` run also passed.

On the same `2e9ad816` Rust source corpus, the controlled header-edit audit reported:

| Mode | Captured families | Existing `since=` changed rows | Retained evidence | Recheck observations |
| --- | ---: | ---: | ---: | ---: |
| semantic | 42 | 32 | 35 | 7 |
| syntax | 697 | 681 | 670 | 27 |
| near | 689 | 675 | 666 | 23 |
| abstraction | 12 | 12 | 12 | 0 |

These are counts across mode-specific populations, not unique repository-wide
families or approval decisions. Of 1,440 observations, 1,383 retained review
evidence and 57 remained for recheck; the existing baseline temporal lens marked
1,400 current rows changed under the same edit. The new unresolved/ambiguous cases
are retained for inspection. All comparisons had complete candidate/provenance
coverage and identical two/four-worker output. Ordinary full query JSON in all
four modes remained identical to the pre-change release binary on the same corpus.

Each change dashboard displayed five observations; the saved census remained
complete. Offline comparison took approximately 6–65 ms in single local release
observations, including process startup. These samples are diagnostic and do not
establish a runtime regression bound. The audit records full/landing/recheck byte
counts separately and does not treat a shorter partial result as improved coverage.
The reproducible report is in the ignored `target/analysis-changes/audit.json`;
the checked harness above owns regeneration. The current product blind oracle
retained 54 exact groups with zero false merges and zero canonicalization violations.

## Research basis

- [NCBI accessions and versions](https://ncbi.nlm.nih.gov/genbank/sequenceids/)
  separate a sequence record from its sequence versions.
- [GA4GH refget sequence collections](https://ga4gh.github.io/refget/seqcols/)
  separate content, names, coordinates, and order-invariant content comparisons.
- [HAL liftover](https://github.com/ComparativeGenomicsToolkit/hal#liftover)
  represents interval mappings and follows duplication relationships.
- [Minimap2](https://github.com/lh3/minimap2#algorithm-overview) motivates indexed
  candidate retrieval before expensive alignment; similarity is not identity.
- [W3C Web Annotation](https://www.w3.org/TR/annotation-model/) distinguishes
  positional and contextual selectors.
- [CAD persistent identification](https://academic.oup.com/jcde/article/3/2/161/5743368)
  treats patterned copies, splits, and merges explicitly.
- [Fellegi–Sunter record linkage](https://nhis.ipums.org/nhis/resources/Fellegi69.pdf)
  includes an undecided outcome rather than forcing every match.
- [CodeTracker block history](https://arxiv.org/html/2409.16185v1) provides a Java
  refactoring-aware comparison target, with narrower block coverage than arbitrary regions.

These motivate Nose's design; their empirical accuracy does not transfer to this
implementation. [Design and direction](design.md) remains the owner of the
soundness, determinism, and single-binary constraints.
