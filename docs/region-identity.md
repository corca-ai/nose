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
original detection fingerprints and admission decisions remain unchanged. Distinct
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
