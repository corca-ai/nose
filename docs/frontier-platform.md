# Type-4 frontier evidence platform

How nose chooses its **next** Type-4 expansion target by corpus evidence rather than by
language/API habit or raw hit count. Back to [type4-benchmark](type4-benchmark.md); the
substrate that fragment work migrates onto is [fragment-contracts](fragment-contracts.md).

The platform is `bench/type4/frontier_platform.py`, a companion to the prevalence ranker
`prioritize_frontier.py` (which is left byte-stable so its
[`FRONTIER_PRIORITIES.md`](../bench/type4/FRONTIER_PRIORITIES.md) stays reproducible). It
emits `bench/type4/frontier_platform.v1.json` and a markdown report describing the same
data.

## Two layers, never mixed

The single most important rule (issue #36's hard lesson) is that a regex prevalence scan is
a **queue signal**, not proof:

- **Queue signal** — how broadly a semantic axis *appears* in the pinned 105-repo corpus.
  The platform may suggest "covered / likely miss / needs audit", but it **never finalizes
  a structured frontier status**.
- **Evidence** — `real_frontier.v1.json` records, which are human-verified with a detector
  run and a proof invariant. The platform only *reads* that store (to mark which axes
  already carry human evidence); it never writes status into it.

Conflating the two reproduces the bias the platform exists to remove: a pattern that is
ubiquitous is not therefore an unsolved frontier.

## Presence-based ranking (not raw count)

The corpus is balanced at 15 repos per language across 7 languages, so "a big language
dominates" is an *occurrence-frequency* bias, not a corpus-imbalance one. The headline rank
is therefore **breadth**, and raw occurrence is reported but is only the last tiebreak —
it can never reorder axes that differ on breadth:

- **repo presence breadth** — how many of the 105 repos exhibit the axis;
- **primary-language breadth** — how many of the corpus's primary languages exhibit it. The
  denominator is **derived** from `corpus.json`'s `primary_language` set (7 languages), not
  a hard-coded list — so a `.js` file inside a TypeScript repo cannot invent a new corpus
  language. The file-extension `source_language_breadth` is kept as a separate *diagnostic*
  (denominator = the source languages actually observed) and never drives the ranking;
- **dev vs held-out generalization** — the 58/47 dev/held-out split is reported separately;
  `dev` drives ranking/triage, held-out is a generalization check, and a `dev-only` axis is
  marked as weaker evidence.

The demonstration in the current report: `null_option_presence` has the **largest** raw
occurrence (~126k) yet ranks below `membership_contains`, which appears in more repos. Raw
count does not win.

## Curated fields (controlled vocabulary, never estimated)

Subjective axes are a controlled vocabulary curated per axis — seeded from
`prioritize_frontier`'s reviewed constants, never auto-estimated into fake numbers (the tool
fails loud on an out-of-vocabulary value):

| field | values |
|---|---|
| `implementation_cost` | `low` `medium` `high` `unknown` |
| `soundness_risk` | `low` `medium` `high` `unknown` |
| `substrate_required` | `none` `fragment-contract` `receiver-place` `effect-algebra` `oracle` `unknown` |
| `evidence_tier` | `pattern-signal` `detector-suggested` `manually-audited` `frontier-recorded` |

`substrate_required` is the routing signal for [#43]: all eight current prevalence axes are
value-graph / type-fact invariants over whole expressions, so they are `none` — the #33
fragment substrate (#43) migrates the fragment *shapes*, which are not in this set.

## Recommendation categories (platform-only)

Categories are **not** frontier statuses; they live only in the platform output:
`all-language`, `multi-language`, `language-family`, `single-language`, `soundness-fix`,
`product-noise-ranking-only`. The last two are reserved routing categories
(`product-noise-ranking-only` → [#45]).

## New axes (`frontier_axes.py`)

New corpus-driven candidate axes do **not** go into `prioritize_frontier.py` (frozen so its
`FRONTIER_PRIORITIES.md` stays reproducible). They live in `bench/type4/frontier_axes.py` as
`EXTRA_CANDIDATES`, and `frontier_platform.py` unions them with the prioritizer axes for
scanning, ranking, and validation. Each new axis must be a genuine *semantic invariant*
(never a language-specific API spelling) and carry controlled-vocabulary `EXTRA_CURATED`
metadata. Two staleness guards run on every build and `--selftest`: the #44
`validate_conclusion` stays scoped to the eight prevalence axes, and a separate
`validate_union` (plus a recorded `union_signature`) covers the combined set, so a packet or
conclusion cannot silently drift when an axis is added or removed.

The first extra axis is `numeric_clamp` — `min(max(x, lo), hi)` clamp composition, a real
under-merge whose identity and hard negatives are machine-checked in
[`formal/Clamp.lean`](../formal/Clamp.lean).

## Target packets (`frontier_target_packets.v1.json`)

An *implementation-ready* candidate becomes a **target packet** in a separate artifact —
never mixed into the `real_frontier.v1.json` evidence store. A packet LINKS one or more
`real_frontier` `case_id`s (the human-verified evidence) and adds the routing the consuming
team needs:

- `owner_route` ∈ `team-a-detector` · `team-c-product` · `proof-fact-prerequisite`. The
  team's issue number is a *separate* `owner_issue` field, never baked into the route value.
  `proof-fact-prerequisite` means "a proof fact is needed first" — **not** something Team A
  can implement now.
- the assembled schema (`packet_id`, `semantic_claim`, `locations` with repo/split/primary
  language, `current_detector_result`, `proof_invariant`, `hard_negative_siblings`, breadth,
  `evidence_tier`, `curated`, `why_now`, `blocked_by`, `notes`) is validated on emit.

A packet's contract ends at the proof invariant and target evidence; it never writes a
detector implementation plan for Team A/C. Generate alongside the platform run:

```sh
python3 bench/type4/frontier_platform.py --repos-root /path/to/bench/repos \
  --packets-json-out bench/type4/frontier_target_packets.v1.json \
  --packets-md-out bench/type4/frontier_target_packets.md
```

## Reproducibility

Each run records its identity: a corpus **commit digest** (computed from `corpus.json`'s
per-repo id/split/language/commit, so it is mtime-independent and reproduces across
machines), the candidate signature, the **union signature** (over all axis ids + patterns +
probes), the tool version, the build ref, and — when the detector probe runs — the nose
binary path/version/sha256. Output is deterministic (byte-identical across runs). Regenerate
with:

```sh
python3 bench/type4/frontier_platform.py \
  --repos-root /path/to/bench/repos \
  --json-out bench/type4/frontier_platform.v1.json \
  --markdown-out bench/type4/frontier_platform.md
```

Add `--with-detector-probe --nose-binary ./target/release/nose` to attach the
detector-*suggested* tier; it records the nose binary identity and never finalizes a
status. `--selftest` runs corpus-free correctness checks.

The committed artifacts are **machine-independent**: paths are repo-root-relative and
`build_ref` defaults to `null` (embedding the live `git HEAD` would make an artifact stale
the moment it is committed). So they regenerate byte-identically from the command above. The
corpus **location** (`--repos-root`), an explicit `--build-ref`, and the nose binary identity
are machine-local provenance that is *excluded* from the byte-identity claim — the corpus
**commit digest** (from `corpus.json`) is what identifies the content. Evidence records in
`real_frontier.v1.json` keep an absolute `binary_path`/`build_ref` on purpose: that is the
provenance of the detector run that produced the evidence.

## Audit template

A "no implementation-ready batch" conclusion is a valid, evidence-backed result, and is the
current verdict (the broad-probe queue for all 8 axes is fully drained — 100% coverage,
zero uncovered forms — and the top axes already carry human evidence). When a real miss
*is* found, record it in `real_frontier.v1.json` (no new statuses — see #36) using this
skeleton, so the next worker need not redo the corpus pass:

```json
{
  "case_id": "<axis>__<repo>__<short-tag>",
  "status": "real-miss | already-covered | hard-negative | unsupported | closed",
  "candidate_axis": "<prioritizer axis> / <narrow invariant>",
  "repo": "<pinned corpus id>",
  "language": "<primary language>",
  "locations": [{"path": "<repo-relative>", "span": "<start-end>", "snippet": "<code>"}],
  "semantic_claim": "<concrete equivalence or non-equivalence claim>",
  "evidence": "<same-spec construction or counterexample>",
  "detector": {
    "current_detector_miss": true,
    "binary_path": "<path>", "nose_version": "<nose --version>", "build_ref": "<git sha>",
    "baseline_command": "nose scan <files> --mode semantic --format json --top 0 --min-size 1 --min-lines 1",
    "baseline_result": "<what the run showed>"
  },
  "proof_invariant": "<narrow proof fact required to merge soundly>",
  "hard_negative_siblings": ["<adjacent case that must stay non-equivalent>"],
  "batch": null,
  "notes": "<short audit note>"
}
```

## What success means

Not "more candidates". The platform succeeds when it records, reproducibly, **which
invariant is trustworthy as the next target and why — and what must not yet be trusted**.
The detector batch itself is separate work ([#43] for fragment-shape migration,
detector PRs for new proof facts); this platform only produces the evidence to choose it.

[#43]: https://github.com/corca-ai/nose/issues/43
[#45]: https://github.com/corca-ai/nose/issues/45
