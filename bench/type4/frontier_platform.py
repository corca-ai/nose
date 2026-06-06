#!/usr/bin/env python3
"""Corpus-balanced Type-4 frontier evidence platform (issue #44).

This is a *companion* to ``prioritize_frontier.py``. It reuses that tool's candidate
and probe definitions (the single source of truth for what each semantic axis matches)
but answers a different question: **which semantic invariants can we trust as the next
Type-4 expansion target, and which must we NOT trust yet** — recorded reproducibly, with
language/repo bias removed.

It deliberately keeps two layers apart (issue #44 / #36 decision):

* **Queue signal** — regex *presence* of an axis across the pinned 105-repo corpus. This
  is prevalence, not proof. It can SUGGEST "covered / likely miss / needs audit" but it
  NEVER finalizes a structured frontier status.
* **Evidence layer** — ``real_frontier.v1.json`` records, which are human-verified with a
  detector run and a proof invariant. This tool only *reads* that layer (to mark which
  axes already carry human evidence); it never writes status into it.

Design choices that follow the #44 final decision:

* **Presence-based normalization.** Ranking is driven by *breadth* — how many repos and
  languages exhibit an axis, and whether it generalizes from the dev split to held-out —
  NOT by raw occurrence count (which over-represents idioms frequent in one language or a
  large repo). Raw/weighted counts are reported but never drive the ranking.
* **dev = ranking/triage, held-out = generalization check.** A dev-only axis is marked as
  weaker evidence; an axis that also spreads to held-out is preferred.
* **Curated, not estimated.** ``implementation_cost`` / ``soundness_risk`` /
  ``substrate_required`` are a controlled vocabulary curated per axis (seeded from
  ``prioritize_frontier``'s reviewed constants) — never auto-estimated into fake numbers.
* **Stable existing artifacts.** ``prioritize_frontier.py`` and ``FRONTIER_PRIORITIES.md``
  are untouched; this tool emits its own ``frontier_platform.v1.json`` + markdown.

Outputs (``--json-out`` / ``--markdown-out``) describe the same data. The JSON records
reproducibility identity (corpus commit digest, candidate signature, tool version, build
ref, and — when the detector probe runs — the nose binary identity).
"""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(HERE))

import prioritize_frontier as pf  # noqa: E402  (reuse candidate/probe defs + scan helpers)
import frontier_axes as fa  # noqa: E402  (Team B extra axes, kept out of the frozen prioritizer)

TOOL_VERSION = "frontier-platform/1"
SCHEMA_VERSION = 1

# The union of the frozen prioritizer axes and the Team B extra axes (issue #50 decision 1).
# `prioritize_frontier.py` stays byte-stable; new axes live in `frontier_axes.py`.
ALL_CANDIDATES = list(pf.CANDIDATES) + list(fa.EXTRA_CANDIDATES)
ALL_PROBES = {**pf.PROBES_BY_CANDIDATE, **fa.EXTRA_PROBES_BY_CANDIDATE}

# The corpus is balanced per primary language, so "larger language dominates by raw count"
# is an OCCURRENCE-frequency bias, not a corpus-imbalance one. The platform answers it by
# ranking on presence breadth. Both language universes used for breadth are DERIVED — the
# ranking denominator from the corpus's `primary_language` set, the diagnostic denominator
# from the source-file languages actually observed — never a hard-coded list (issue #44).

# ---------------------------------------------------------------------------
# Controlled vocabulary (issue #44 final decision, point 5/7).
# ---------------------------------------------------------------------------
IMPLEMENTATION_COST = {"low", "medium", "high", "unknown"}
SOUNDNESS_RISK = {"low", "medium", "high", "unknown"}
SUBSTRATE_REQUIRED = {
    "none",
    "fragment-contract",
    "receiver-place",
    "effect-algebra",
    "oracle",
    "unknown",
}
EVIDENCE_TIER = {
    "pattern-signal",
    "detector-suggested",
    "manually-audited",
    "frontier-recorded",
}
RECOMMENDATION_CATEGORY = {
    "all-language",
    "multi-language",
    "language-family",
    "single-language",
    "soundness-fix",
    "product-noise-ranking-only",
}

# Curated per-axis metadata. These are NOT auto-estimated: they are reviewed judgments,
# seeded from prioritize_frontier's reviewed `implementation_cost`/`soundness_risk`
# integers and re-expressed in the decision's controlled vocabulary, plus the
# `substrate_required` routing for #43 and a short rationale. Any axis absent here falls
# back to `unknown` (fail-loud, never fabricated).
#
# `substrate_required` note: all eight current prevalence axes are value-graph / type-fact
# semantic invariants over whole expressions — they are NOT sub-function fragment shapes,
# so none of them are #33 fragment-substrate (#43) targets. `#43` migrates the *fragment*
# shapes (ConditionalGuard / LoopEffect / SelfFieldBody, …), which are not in this set.
CURATED: dict[str, dict] = {
    "collection_empty_check": {
        "implementation_cost": "low",
        "soundness_risk": "low",
        "substrate_required": "none",
        "rationale": "Emptiness predicates lower to a value-graph length/size fact; no "
        "fragment substrate or receiver-place proof is involved.",
    },
    "string_prefix_suffix": {
        "implementation_cost": "low",
        "soundness_risk": "low",
        "substrate_required": "none",
        "rationale": "Prefix/suffix predicates are pure string-builtin value facts.",
    },
    "membership_contains": {
        "implementation_cost": "medium",
        "soundness_risk": "medium",
        "substrate_required": "none",
        "rationale": "Remaining work is dynamic-receiver/element provenance and type "
        "facts (import/immutability), not the #33 fragment substrate.",
    },
    "null_option_presence": {
        "implementation_cost": "medium",
        "soundness_risk": "medium",
        "substrate_required": "none",
        "rationale": "Presence/defaulting is a value-graph option fact; alias/effect "
        "guard variants need pointer-alias facts, still not a fragment shape.",
    },
    "map_default_lookup": {
        "implementation_cost": "high",
        "soundness_risk": "high",
        "substrate_required": "none",
        "rationale": "Open work is receiver/key/default provenance + whole-file mutation "
        "exclusion (type/provenance facts), not the fragment substrate.",
    },
    "numeric_minmax_abs": {
        "implementation_cost": "low",
        "soundness_risk": "low",
        "substrate_required": "none",
        "rationale": "Scalar min/max/abs are value-graph numeric facts.",
    },
    "property_type_guard": {
        "implementation_cost": "low",
        "soundness_risk": "medium",
        "substrate_required": "none",
        "rationale": "typeof property guards are value-graph type-tag facts.",
    },
    "own_property_guard": {
        "implementation_cost": "low",
        "soundness_risk": "medium",
        "substrate_required": "none",
        "rationale": "Own-property guards are value-graph key-presence facts.",
    },
}

# Curated audit conclusion for the current corpus + axis state (issue #44 acceptance:
# "at least one recommendation OR an explicit no-batch conclusion, backed by real examples
# and hard-negative ideas"). This is a HUMAN judgment, not auto-derived — it is recorded
# here so the structured output is self-contained for the next team. It must be revisited
# when a new candidate axis is added or the prioritizer's coverage changes.
# The exact candidate-id set this human judgment was made against. `validate_conclusion`
# fails build/selftest if prioritize_frontier's axes drift from this set, so a new or
# removed axis cannot silently inherit a stale "no-batch" verdict.
AUDIT_CONCLUSION_CANDIDATES = [
    "collection_empty_check",
    "map_default_lookup",
    "membership_contains",
    "null_option_presence",
    "numeric_minmax_abs",
    "own_property_guard",
    "property_type_guard",
    "string_prefix_suffix",
]
AUDIT_CONCLUSION = {
    "verdict": "no-implementation-ready-batch",
    "applies_to_candidates": AUDIT_CONCLUSION_CANDIDATES,
    "generated_against": "the eight prevalence axes currently defined in prioritize_frontier.py",
    "summary": (
        "No implementation-ready real-miss batch is supported by this pass. Every "
        "high-breadth axis is either already covered by the strict frontier or already "
        "carries human-verified evidence; the broad-probe queue is fully drained (100% "
        "probe coverage, zero uncovered forms across all 8 axes), so prevalence offers no "
        "new uncovered-gap signal to promote."
    ),
    "evidence_pointers": [
        "Top-breadth axes membership_contains and collection_empty_check are "
        "frontier-recorded (human evidence: unsupported / closed) — high prevalence is not "
        "next work (the #36 lesson, now visible via evidence_tier).",
        "null_option_presence has the largest raw occurrence (~126k) yet is a covered-"
        "current axis ranked below membership_contains on breadth — the presence-based "
        "ranking deliberately refuses to promote it on raw count.",
        "All eight axes report 100% broad-probe coverage and zero uncovered samples, so "
        "the detector-suggested probe has no gap location to investigate.",
    ],
    "what_a_future_batch_would_need": (
        "A future real-miss batch needs a NEW axis whose breadth is wide, whose broad "
        "probe surfaces UNCOVERED forms, and whose semantic equivalence a human can pin to "
        "a narrow proof invariant with a concrete hard-negative sibling — recorded in "
        "real_frontier.v1.json, not inferred from prevalence."
    ),
    "hard_negative_ideas": [
        "membership_contains: substring `contains` vs element membership; mutated or "
        "append-expanded receiver bindings; shadowed constructor/type/package; untyped "
        "dynamic receiver — all must stay non-equivalent.",
        "map_default_lookup: absent-key semantics beyond a proven zero default; receiver "
        "mutation/effects between binding and lookup; cross-file unproven map provenance.",
        "null_option_presence: effectful guard bodies and pointer/reference aliasing that "
        "change observable behavior must not merge with pure presence checks.",
    ],
}

# Merged curated metadata over the union of prioritizer + Team B axes.
CURATED_ALL = {**CURATED, **fa.EXTRA_CURATED}

# Union staleness guard (issue #50 decision 1): covers the prioritizer axes PLUS the Team B
# extra axes. This is DISTINCT from the #44 `AUDIT_CONCLUSION` guard, which stays scoped to
# the eight prevalence axes. A new or removed axis anywhere in the union fails build +
# selftest, so a target packet / conclusion cannot silently drift.
EXPECTED_UNION_AXES = [
    "collection_empty_check",
    "map_default_lookup",
    "membership_contains",
    "null_option_presence",
    "numeric_clamp",
    "numeric_minmax_abs",
    "own_property_guard",
    "property_type_guard",
    "string_prefix_suffix",
]

# Platform recommendation categories are NOT frontier statuses. They derive from the axis
# language scope; `soundness-fix` and `product-noise-ranking-only` are reserved curated
# overrides (none of the current axes are either) that route to #43-adjacent soundness work
# and to #45 product-noise/ranking work respectively.
SCOPE_TO_CATEGORY = {
    "all-language": "all-language",
    "multi-language": "multi-language",
    "language-family": "language-family",
    "single-language": "single-language",
}


def curated_for(candidate_id: str) -> dict:
    meta = CURATED_ALL.get(candidate_id)
    if meta is None:
        return {
            "implementation_cost": "unknown",
            "soundness_risk": "unknown",
            "substrate_required": "unknown",
            "rationale": "No curated review recorded for this axis.",
        }
    return meta


def validate_vocab() -> None:
    """Fail loud if any curated value escapes the controlled vocabulary, or if any axis in
    the union has no curated review (a silent `unknown` fallback would violate the
    'curated, not estimated' principle — issue #44 decision 5 / #50 decision 1)."""
    for cid, meta in CURATED_ALL.items():
        assert meta["implementation_cost"] in IMPLEMENTATION_COST, cid
        assert meta["soundness_risk"] in SOUNDNESS_RISK, cid
        assert meta["substrate_required"] in SUBSTRATE_REQUIRED, cid
    missing = sorted(c.candidate_id for c in ALL_CANDIDATES if c.candidate_id not in CURATED_ALL)
    assert not missing, f"axes missing curated metadata: {missing}"


def validate_conclusion() -> None:
    """Fail loud if the PRIORITIZER's axis set drifts from the set the #44 audit conclusion
    was written against, so a stale 'no-batch' verdict cannot be reused after a new or removed
    prioritizer axis (issue #44 decision 2). Scoped to the eight prevalence axes."""
    current = sorted(c.candidate_id for c in pf.CANDIDATES)
    assert current == sorted(AUDIT_CONCLUSION_CANDIDATES), (
        "prioritizer axis set changed since the audit conclusion was written; revisit "
        f"AUDIT_CONCLUSION. expected {sorted(AUDIT_CONCLUSION_CANDIDATES)}, got {current}"
    )


def validate_union() -> None:
    """Fail loud if the UNION axis set (prioritizer + Team B extras) drifts from the recorded
    expectation, so target packets and conclusions cannot silently drift (issue #50)."""
    current = sorted(c.candidate_id for c in ALL_CANDIDATES)
    assert current == sorted(EXPECTED_UNION_AXES), (
        "union axis set changed; update EXPECTED_UNION_AXES and revisit packets/guards. "
        f"expected {sorted(EXPECTED_UNION_AXES)}, got {current}"
    )


def union_signature() -> str:
    """A stable signature over the union axis defs (ids + patterns + probes), so a regex
    change in any axis is visible in the reproducibility identity."""
    payload = {
        "axes": [
            {
                "candidate_id": c.candidate_id,
                "scope": c.scope,
                "patterns": sorted((s.pattern_id, s.lang, s.regex.pattern) for s in c.patterns),
                "probes": sorted(
                    (s.probe_id, s.lang, s.regex.pattern)
                    for s in ALL_PROBES.get(c.candidate_id, ())
                ),
            }
            for c in ALL_CANDIDATES
        ]
    }
    return hashlib.sha256(json.dumps(payload, sort_keys=True).encode()).hexdigest()


# ---------------------------------------------------------------------------
# Presence-based corpus scan (queue signal layer).
# ---------------------------------------------------------------------------
def presence_scan(repos: list[dict], max_bytes: int, sample_limit: int) -> dict:
    """Accumulate per-axis REPO PRESENCE (binary per repo) plus uncovered-probe gaps.

    Unlike ``prioritize_frontier.analyze`` (which sums occurrences), this records the SET
    of repos / languages / splits where each axis appears, so breadth can be normalized
    independently of how often an idiom recurs inside any one repo or language.
    """
    buckets = {
        c.candidate_id: {
            "repos": {},  # repo_id -> {split, primary_language, langs:set, raw}
            "languages": set(),
            "gap_repos": set(),  # repos with an uncovered broad-probe hit
            "gap_samples": [],
            "samples": [],
        }
        for c in ALL_CANDIDATES
    }
    # The corpus source-language universe (file-extension languages actually present), so
    # the diagnostic source-language breadth denominator is derived, not hard-coded.
    corpus_source_languages: set[str] = set()

    for repo in repos:
        repo_path = repo["path"]
        split = repo.get("split") or "unknown"
        for path, lang in pf.iter_source_files(repo_path, max_bytes):
            corpus_source_languages.add(lang)
            try:
                text = path.read_text(errors="ignore")
            except OSError:
                continue
            rel = str(path.relative_to(repo_path))
            for candidate in ALL_CANDIDATES:
                specs = [s for s in candidate.patterns if s.lang == lang]
                probes = [
                    s
                    for s in ALL_PROBES.get(candidate.candidate_id, ())
                    if s.lang == lang
                ]
                if not specs and not probes:
                    continue
                bucket = buckets[candidate.candidate_id]
                extracted_spans = []
                raw_for_file = 0
                for spec in specs:
                    for match in spec.regex.finditer(text):
                        if pf.is_comment_only_line(text, match.start(), lang):
                            continue
                        if pf.match_filter_reason(candidate.candidate_id, lang, match):
                            continue
                        raw_for_file += 1
                        extracted_spans.append((match.start(), match.end()))
                        if len(bucket["samples"]) < sample_limit:
                            sample = pf.make_sample(
                                repo, rel, lang, text, match.start(), match.end()
                            )
                            sample["pattern_id"] = spec.pattern_id
                            bucket["samples"].append(sample)
                if raw_for_file:
                    rstat = bucket["repos"].setdefault(
                        repo["id"],
                        {
                            "split": split,
                            "primary_language": repo.get("primary_language") or "",
                            "langs": set(),
                            "raw": 0,
                        },
                    )
                    rstat["langs"].add(lang)
                    rstat["raw"] += raw_for_file
                    bucket["languages"].add(lang)
                # Broad-probe gap: a probe hit not covered by any extraction span is a
                # real-code form the current detector may not capture — an AUDIT cue only.
                for probe_spec in probes:
                    for match in probe_spec.regex.finditer(text):
                        if pf.is_comment_only_line(text, match.start(), lang):
                            continue
                        if pf.match_filter_reason(candidate.candidate_id, lang, match):
                            continue
                        span = (match.start(), match.end())
                        if any(pf.spans_overlap(span, e) for e in extracted_spans):
                            continue
                        bucket["gap_repos"].add(repo["id"])
                        if len(bucket["gap_samples"]) < sample_limit:
                            sample = pf.make_sample(
                                repo, rel, lang, text, match.start(), match.end()
                            )
                            sample["probe_id"] = probe_spec.probe_id
                            bucket["gap_samples"].append(sample)
    return buckets, sorted(corpus_source_languages)


# ---------------------------------------------------------------------------
# Normalized breadth metrics.
# ---------------------------------------------------------------------------
def _fraction(n: int, d: int) -> float:
    return round(n / d, 4) if d else 0.0


def breadth_metrics(
    bucket: dict,
    split_totals: dict[str, int],
    corpus_primary_languages: list[str],
    corpus_source_languages: list[str],
) -> dict:
    repos = bucket["repos"]
    dev_repos = sorted(r for r, s in repos.items() if s["split"] == "dev")
    heldout_repos = sorted(r for r, s in repos.items() if s["split"] == "heldout")
    total_repos = sum(split_totals.values())
    dev_breadth = _fraction(len(dev_repos), split_totals.get("dev", 0))
    heldout_breadth = _fraction(len(heldout_repos), split_totals.get("heldout", 0))
    # RANKING breadth: distinct *corpus primary languages* of the repos where the axis
    # appears, over the corpus's own primary-language set (derived, not hard-coded). This is
    # the balanced-corpus definition: a `.js` file inside a TypeScript repo does not invent a
    # new corpus language.
    primary_present = sorted(
        {s["primary_language"] for s in repos.values() if s.get("primary_language")}
    )
    # DIAGNOSTIC: file-extension source languages where the axis matched, over the
    # corpus-observed source-language universe. Reported, never used for ranking.
    source_langs = sorted(bucket["languages"])
    # Generalization: an axis present on dev but absent on held-out is weaker evidence.
    if not dev_repos and not heldout_repos:
        generalization = "absent"
    elif dev_repos and not heldout_repos:
        generalization = "dev-only"
    elif heldout_repos and not dev_repos:
        generalization = "heldout-only"
    else:
        generalization = "both-splits"
    return {
        "repo_breadth": _fraction(len(repos), total_repos),
        "repo_presence": len(repos),
        "primary_language_breadth": _fraction(
            len(primary_present), len(corpus_primary_languages)
        ),
        "primary_language_presence": len(primary_present),
        "primary_languages": primary_present,
        "source_language_breadth": _fraction(
            len(source_langs), len(corpus_source_languages)
        ),
        "source_language_presence": len(source_langs),
        "source_languages": source_langs,
        "dev_breadth": dev_breadth,
        "dev_presence": len(dev_repos),
        "heldout_breadth": heldout_breadth,
        "heldout_presence": len(heldout_repos),
        "generalization": generalization,
        "gap_repo_presence": len(bucket["gap_repos"]),
        "raw_occurrences": sum(r["raw"] for r in repos.values()),
    }


def presence_rank_key(metrics: dict) -> tuple:
    """Presence-first ordering. Breadth (repo + corpus primary-language) dominates; raw
    occurrence is the last tiebreak so it can never reorder axes that differ on breadth
    (issue #44 decision 3)."""
    return (
        metrics["repo_breadth"],
        metrics["primary_language_breadth"],
        # Reward generalization to held-out over dev-only prevalence.
        1 if metrics["generalization"] == "both-splits" else 0,
        metrics["heldout_breadth"],
        metrics["raw_occurrences"],
    )


# ---------------------------------------------------------------------------
# Evidence layer cross-reference (read-only) + detector-suggested probe.
# ---------------------------------------------------------------------------
def load_frontier_evidence(path: Path) -> dict[str, list[dict]]:
    """Map prevalence candidate_id -> human-recorded real_frontier items (by axis prefix).

    Read-only: this never writes or finalizes a status. It only surfaces that an axis
    already carries human-verified evidence (and which statuses)."""
    by_axis: dict[str, list[dict]] = {}
    if not path.exists():
        return by_axis
    doc = json.loads(path.read_text())
    for item in doc.get("items", []):
        axis = str(item.get("candidate_axis", ""))
        head = axis.split("/")[0].strip()
        by_axis.setdefault(head, []).append(
            {
                "case_id": item.get("case_id"),
                "status": item.get("status"),
                "candidate_axis": axis,
                "proof_invariant": item.get("proof_invariant"),
            }
        )
    return by_axis


def detector_suggest(
    nose_binary: Path, repos_root: Path, samples: list[dict], limit: int
) -> dict:
    """Run `nose scan --mode semantic` on the files of up to `limit` gap samples and
    SUGGEST whether each axis location is already covered by a reported semantic family.

    This is a *suggestion* tier only (evidence_tier=detector-suggested). It never sets a
    structured frontier status; a human still confirms semantic equivalence and proof.
    """
    suggestions = []
    seen_files: set[tuple[str, str]] = set()
    for sample in samples:
        if len(suggestions) >= limit:
            break
        repo = sample["repo"]
        rel = sample["path"]
        key = (repo, rel)
        if key in seen_files:
            continue
        seen_files.add(key)
        target = repos_root / repo / rel
        if not target.is_file():
            continue
        cmd = [
            str(nose_binary),
            "scan",
            str(target),
            "--mode",
            "semantic",
            "--format",
            "json",
            "--top",
            "0",
            "--min-size",
            "1",
            "--min-lines",
            "1",
        ]
        try:
            proc = subprocess.run(
                cmd, capture_output=True, text=True, timeout=120, cwd=repos_root
            )
        except (OSError, subprocess.TimeoutExpired) as exc:
            suggestions.append({**_sample_ref(sample), "suggestion": "error", "detail": str(exc)})
            continue
        suggestions.append(
            {
                **_sample_ref(sample),
                **classify_probe(proc.returncode, proc.stdout, proc.stderr, rel, sample.get("line")),
            }
        )
    return {
        "probed": len(suggestions),
        "likely_covered": sum(1 for s in suggestions if s["suggestion"] == "likely-covered"),
        "likely_miss": sum(1 for s in suggestions if s["suggestion"] == "likely-miss"),
        "errors": sum(1 for s in suggestions if s["suggestion"] == "error"),
        "samples": suggestions,
    }


def _sample_ref(sample: dict) -> dict:
    return {
        "repo": sample["repo"],
        "path": sample["path"],
        "line": sample.get("line"),
        "language": sample.get("language"),
        "probe_id": sample.get("probe_id"),
    }


def classify_probe(
    returncode: int, stdout: str, stderr: str, rel: str, line: int | None
) -> dict:
    """Classify one scan result as a detector suggestion. A non-zero exit is a detector/CLI
    failure, NOT a miss — recording it as `likely-miss` would pollute the triage queue with
    crashes — so it maps to `error`. Otherwise a reported family overlapping the probe line
    suggests the product output already surfaces it (`likely-covered`); absence is a
    candidate miss to AUDIT (`likely-miss`). Never a finalized status."""
    if returncode != 0:
        return {"suggestion": "error", "detail": f"exit {returncode}: {stderr.strip()[:200]}"}
    families = _families_on_line(stdout, rel, line)
    return {
        "suggestion": "likely-covered" if families else "likely-miss",
        "families_on_line": families,
    }


def _families_on_line(stdout: str, rel: str, line: int | None) -> int:
    if line is None or not stdout.strip():
        return 0
    try:
        report = json.loads(stdout)
    except json.JSONDecodeError:
        return 0
    count = 0
    for fam in report.get("families", []):
        for loc in fam.get("locations", []):
            lf = loc.get("file", "")
            if not (lf == rel or lf.endswith("/" + rel) or lf.endswith(rel)):
                continue
            if loc.get("start_line", 0) <= line <= loc.get("end_line", 0):
                count += 1
                break
    return count


# ---------------------------------------------------------------------------
# Reproducibility identity.
# ---------------------------------------------------------------------------
def corpus_identity(corpus_path: Path) -> dict:
    """Stable corpus identity from corpus.json (id/split/language/commit) — independent of
    file mtimes, so it reproduces across machines and checkouts."""
    doc = json.loads(corpus_path.read_text())
    repos = doc.get("repositories", [])
    h = hashlib.sha256()
    for repo in sorted(repos, key=lambda r: r["id"]):
        for field in ("id", "split", "primary_language", "commit"):
            h.update(str(repo.get(field, "")).encode())
            h.update(b"\x00")
    return {
        "corpus_path": str(corpus_path),
        "corpus_schema_version": doc.get("schema_version"),
        "repo_count": len(repos),
        "commit_digest": h.hexdigest(),
    }


def nose_identity(nose_binary: Path) -> dict:
    out = {"binary_path": str(nose_binary)}
    try:
        out["version"] = subprocess.run(
            [str(nose_binary), "--version"], capture_output=True, text=True, timeout=30
        ).stdout.strip()
    except (OSError, subprocess.TimeoutExpired):
        out["version"] = None
    try:
        out["sha256"] = hashlib.sha256(nose_binary.read_bytes()).hexdigest()
        out["size_bytes"] = nose_binary.stat().st_size
    except OSError:
        out["sha256"] = None
    return out


def git_build_ref(explicit: str | None) -> str | None:
    if explicit:
        return explicit
    try:
        return subprocess.run(
            ["git", "rev-parse", "HEAD"], cwd=ROOT, capture_output=True, text=True, timeout=30
        ).stdout.strip() or None
    except (OSError, subprocess.TimeoutExpired):
        return None


# ---------------------------------------------------------------------------
# Build the platform result.
# ---------------------------------------------------------------------------
def build(
    corpus_path: Path,
    repos_root: Path,
    max_bytes: int,
    sample_limit: int,
    real_frontier: Path,
    nose_binary: Path | None,
    detector_probe_limit: int,
    build_ref: str | None,
) -> dict:
    validate_vocab()
    validate_conclusion()
    validate_union()
    repos = pf.load_repos(corpus_path, repos_root)
    split_totals: dict[str, int] = {}
    for repo in repos:
        split_totals[repo.get("split") or "unknown"] = (
            split_totals.get(repo.get("split") or "unknown", 0) + 1
        )
    # The corpus's own primary-language set (derived, not hard-coded) is the ranking
    # language-breadth denominator.
    corpus_primary_languages = sorted(
        {r["primary_language"] for r in repos if r.get("primary_language")}
    )
    buckets, corpus_source_languages = presence_scan(repos, max_bytes, sample_limit)
    evidence = load_frontier_evidence(real_frontier)

    candidates_out = []
    for candidate in ALL_CANDIDATES:
        bucket = buckets[candidate.candidate_id]
        metrics = breadth_metrics(
            bucket, split_totals, corpus_primary_languages, corpus_source_languages
        )
        curated = curated_for(candidate.candidate_id)
        records = evidence.get(candidate.candidate_id, [])
        # evidence_tier: pattern-signal by default; upgrade if human evidence exists.
        # (detector-suggested is attached separately below when the probe runs.)
        tier = "frontier-recorded" if records else "pattern-signal"
        category = SCOPE_TO_CATEGORY.get(candidate.scope, candidate.scope)
        detector = None
        if nose_binary is not None and detector_probe_limit > 0:
            detector = detector_suggest(
                nose_binary, repos_root, bucket["gap_samples"], detector_probe_limit
            )
            if detector["probed"] and tier == "pattern-signal":
                tier = "detector-suggested"
        candidates_out.append(
            {
                "candidate_id": candidate.candidate_id,
                "title": candidate.title,
                "scope": candidate.scope,
                "prioritizer_status": candidate.status,
                "recommendation_category": category,
                "evidence_tier": tier,
                "curated": {
                    "implementation_cost": curated["implementation_cost"],
                    "soundness_risk": curated["soundness_risk"],
                    "substrate_required": curated["substrate_required"],
                    "rationale": curated["rationale"],
                },
                "routing": {
                    # Fields that let downstream issues consume without re-deriving.
                    "issue_43_substrate_target": curated["substrate_required"] != "none",
                    "issue_45_product_noise": category == "product-noise-ranking-only",
                    "issue_37_subset_repos": sorted(bucket["repos"].keys())[:12],
                },
                "breadth": metrics,
                "human_evidence": {
                    "count": len(records),
                    "statuses": sorted({r["status"] for r in records}),
                    "records": records,
                },
                "detector_suggested": detector,
                "samples": bucket["samples"][:sample_limit],
                "gap_samples": bucket["gap_samples"][:sample_limit],
            }
        )

    candidates_out.sort(key=lambda c: presence_rank_key(c["breadth"]), reverse=True)
    for rank, c in enumerate(candidates_out, start=1):
        c["presence_rank"] = rank

    identity = {
        "tool_version": TOOL_VERSION,
        "schema_version": SCHEMA_VERSION,
        "build_ref": git_build_ref(build_ref),
        "candidate_signature": pf.candidate_signature(),
        "union_signature": union_signature(),
        "union_axes": sorted(c.candidate_id for c in ALL_CANDIDATES),
        "corpus": corpus_identity(corpus_path),
        "split_totals": dict(sorted(split_totals.items())),
        "corpus_primary_languages": corpus_primary_languages,
        "corpus_source_languages": corpus_source_languages,
        "repos_root": str(repos_root),
        "max_bytes_per_file": max_bytes,
        "nose_binary": nose_identity(nose_binary) if nose_binary is not None else None,
    }
    return {
        "schema_version": SCHEMA_VERSION,
        "tool_version": TOOL_VERSION,
        "identity": identity,
        "primary_languages": corpus_primary_languages,
        "source_languages": corpus_source_languages,
        "audit_conclusion": AUDIT_CONCLUSION,
        "candidates": candidates_out,
        "vocabulary": {
            "implementation_cost": sorted(IMPLEMENTATION_COST),
            "soundness_risk": sorted(SOUNDNESS_RISK),
            "substrate_required": sorted(SUBSTRATE_REQUIRED),
            "evidence_tier": sorted(EVIDENCE_TIER),
            "recommendation_category": sorted(RECOMMENDATION_CATEGORY),
        },
    }


# ---------------------------------------------------------------------------
# Markdown report (same data as the JSON).
# ---------------------------------------------------------------------------
def markdown_report(result: dict) -> str:
    idy = result["identity"]
    lines = [
        "# Type-4 frontier evidence platform",
        "",
        "Companion to `prioritize_frontier.py`. Ranks candidate semantic invariants by",
        "**presence breadth** across the pinned corpus (not raw occurrence), separates the",
        "regex **queue signal** from human-verified **evidence**, and records reproducibility",
        "identity. Generated by `bench/type4/frontier_platform.py`; see",
        "[frontier-platform](../../docs/frontier-platform.md).",
        "",
        "## Reproducibility identity",
        "",
        f"- tool: `{idy['tool_version']}` · schema `{result['schema_version']}`",
        f"- build ref: `{idy['build_ref']}`",
        f"- corpus: {idy['corpus']['repo_count']} repos · commit digest "
        f"`{idy['corpus']['commit_digest'][:16]}…` · splits {idy['split_totals']}",
        f"- candidate signature: `{idy['candidate_signature'][:16]}…`",
    ]
    if idy.get("nose_binary"):
        nb = idy["nose_binary"]
        lines.append(
            f"- nose binary: `{nb.get('version')}` · sha256 "
            f"`{(nb.get('sha256') or '')[:16]}…`"
        )
    else:
        lines.append("- nose binary: not probed (pattern-signal only)")
    ac = result["audit_conclusion"]
    lines += [
        "",
        "## Audit conclusion (curated)",
        "",
        f"**Verdict: {ac['verdict']}.** {ac['summary']}",
        "",
        "Evidence:",
    ]
    lines += [f"- {p}" for p in ac["evidence_pointers"]]
    lines += ["", f"_What a future batch would need:_ {ac['what_a_future_batch_would_need']}", ""]
    lines += ["Hard-negative ideas to keep non-equivalent:"]
    lines += [f"- {h}" for h in ac["hard_negative_ideas"]]
    lines += [
        "",
        "## Presence-ranked candidates",
        "",
        "Breadth is the headline; raw occurrence is shown but never drives the rank.",
        "",
        "| rank | axis | category | evidence tier | repo breadth | primary-lang breadth | dev | heldout | generalization | cost | risk | substrate | human evidence | raw occ |",
        "|---:|---|---|---|---:|---:|---:|---:|---|---|---|---|---|---:|",
    ]
    for c in result["candidates"]:
        b = c["breadth"]
        cur = c["curated"]
        he = c["human_evidence"]
        he_txt = f"{he['count']} ({', '.join(he['statuses'])})" if he["count"] else "—"
        lines.append(
            "| {rank} | `{axis}` | {cat} | {tier} | {rb:.0%} ({rp}) | {lb:.0%} ({lp}) | "
            "{db:.0%} ({dp}) | {hb:.0%} ({hp}) | {gen} | {cost} | {risk} | {sub} | {he} | {raw} |".format(
                rank=c["presence_rank"],
                axis=c["candidate_id"],
                cat=c["recommendation_category"],
                tier=c["evidence_tier"],
                rb=b["repo_breadth"],
                rp=b["repo_presence"],
                lb=b["primary_language_breadth"],
                lp=b["primary_language_presence"],
                db=b["dev_breadth"],
                dp=b["dev_presence"],
                hb=b["heldout_breadth"],
                hp=b["heldout_presence"],
                gen=b["generalization"],
                cost=cur["implementation_cost"],
                risk=cur["soundness_risk"],
                sub=cur["substrate_required"],
                he=he_txt,
                raw=b["raw_occurrences"],
            )
        )
    lines += ["", "## Per-axis detail", ""]
    for c in result["candidates"]:
        b = c["breadth"]
        lines.append(f"### `{c['candidate_id']}` — {c['title']}")
        lines.append("")
        lines.append(
            f"- category: **{c['recommendation_category']}** · evidence tier: "
            f"**{c['evidence_tier']}** · prioritizer status: `{c['prioritizer_status']}`"
        )
        lines.append(
            f"- presence: {b['repo_presence']} repos / {b['primary_language_presence']} "
            f"primary langs ({', '.join(b['primary_languages'])}) · "
            f"source langs {', '.join(b['source_languages'])} · "
            f"dev {b['dev_presence']} · heldout {b['heldout_presence']} · {b['generalization']}"
        )
        lines.append(
            f"- curated: cost `{c['curated']['implementation_cost']}` · risk "
            f"`{c['curated']['soundness_risk']}` · substrate `{c['curated']['substrate_required']}`"
        )
        lines.append(f"  - rationale: {c['curated']['rationale']}")
        if c["human_evidence"]["count"]:
            for r in c["human_evidence"]["records"]:
                lines.append(
                    f"  - human evidence: `{r['case_id']}` → **{r['status']}** "
                    f"({r['candidate_axis']})"
                )
        if c.get("detector_suggested"):
            d = c["detector_suggested"]
            lines.append(
                f"  - detector-suggested: probed {d['probed']} gap loc(s) → "
                f"{d['likely_covered']} likely-covered, {d['likely_miss']} likely-miss "
                "(suggestion only; not a finalized status)"
            )
        lines.append("")
    return "\n".join(lines) + "\n"


def selftest() -> int:
    """Corpus-free correctness checks. The live detector probe legitimately finds zero
    gaps on the current mature axes, so the gap/family logic is proven here on synthetic
    inputs instead."""
    validate_vocab()  # also asserts every current axis is curated (no silent `unknown`)
    validate_conclusion()  # asserts the prioritizer axis set matches the #44 conclusion
    validate_union()  # asserts the union axis set (prioritizer + extras) matches expectation
    # Every curated axis routes a recommendation category and a known substrate value.
    for c in ALL_CANDIDATES:
        assert SCOPE_TO_CATEGORY.get(c.scope, c.scope) in RECOMMENDATION_CATEGORY, c.candidate_id
        assert curated_for(c.candidate_id)["substrate_required"] in SUBSTRATE_REQUIRED

    # Presence ranking: breadth dominates raw occurrence. A wide-breadth/low-raw axis must
    # outrank a narrow-breadth/huge-raw axis.
    wide = {"repo_breadth": 0.9, "primary_language_breadth": 0.8, "generalization": "both-splits",
            "heldout_breadth": 0.9, "raw_occurrences": 10}
    narrow = {"repo_breadth": 0.2, "primary_language_breadth": 0.2, "generalization": "dev-only",
              "heldout_breadth": 0.0, "raw_occurrences": 10_000_000}
    assert presence_rank_key(wide) > presence_rank_key(narrow), "breadth must beat raw count"

    # Breadth metrics: corpus-derived primary-language denominator, source-language is a
    # separate diagnostic, and generalization classification.
    totals = {"dev": 2, "heldout": 2}
    primary = ["go", "java", "python", "rust"]  # 4 corpus primary languages
    source = ["go", "java", "javascript", "python", "rust", "typescript"]
    # A Go-primary repo whose axis also matched .js/.ts source files: primary breadth counts
    # ONE primary language (go), NOT the source-file languages.
    dev_only = breadth_metrics(
        {"repos": {"a": {"split": "dev", "primary_language": "go", "langs": {"go"}, "raw": 1}},
         "languages": {"go", "javascript"}, "gap_repos": set()}, totals, primary, source)
    assert dev_only["generalization"] == "dev-only", dev_only["generalization"]
    assert dev_only["primary_language_presence"] == 1, "one primary language (go)"
    assert dev_only["primary_language_breadth"] == round(1 / 4, 4), "denominator = corpus primaries"
    assert dev_only["source_language_presence"] == 2, "source langs are a separate diagnostic"
    both = breadth_metrics(
        {"repos": {"a": {"split": "dev", "primary_language": "go", "langs": {"go"}, "raw": 1},
                   "b": {"split": "heldout", "primary_language": "java", "langs": {"java"}, "raw": 1}},
         "languages": {"go", "java"}, "gap_repos": set()}, totals, primary, source)
    assert both["generalization"] == "both-splits"
    assert both["dev_breadth"] == 0.5 and both["heldout_breadth"] == 0.5
    assert both["primary_language_presence"] == 2

    # Family-on-line detection (the detector-suggested probe's covered/miss kernel).
    report = json.dumps({"families": [{"locations": [
        {"file": "src/x.go", "start_line": 10, "end_line": 12}]}]})
    assert _families_on_line(report, "src/x.go", 11) == 1, "overlapping line => covered"
    assert _families_on_line(report, "src/x.go", 99) == 0, "non-overlapping line => miss"
    assert _families_on_line("", "src/x.go", 11) == 0, "no families => miss"
    assert _families_on_line("not json", "src/x.go", 11) == 0, "bad json => miss, no crash"

    # Probe classification: a non-zero exit is `error`, never `likely-miss` (must not
    # pollute the triage queue with detector crashes).
    assert classify_probe(0, report, "", "src/x.go", 11)["suggestion"] == "likely-covered"
    assert classify_probe(0, "", "", "src/x.go", 11)["suggestion"] == "likely-miss"
    assert classify_probe(3, "", "boom", "src/x.go", 11)["suggestion"] == "error"
    assert classify_probe(101, "partial", "panic", "src/x.go", 11)["suggestion"] == "error"

    # The audit conclusion is self-contained for the next team.
    for key in ("verdict", "summary", "evidence_pointers", "hard_negative_ideas",
                "what_a_future_batch_would_need"):
        assert AUDIT_CONCLUSION.get(key), key
    print("selftest OK")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--selftest", action="store_true", help="run corpus-free correctness checks")
    ap.add_argument("--corpus", type=Path, default=pf.DEFAULT_CORPUS)
    ap.add_argument("--repos-root", type=Path, default=pf.DEFAULT_REPOS_ROOT)
    ap.add_argument("--max-bytes", type=int, default=512_000)
    ap.add_argument("--sample-limit", type=int, default=8)
    ap.add_argument(
        "--real-frontier", type=Path, default=HERE / "real_frontier.v1.json"
    )
    ap.add_argument("--nose-binary", type=Path, default=None)
    ap.add_argument(
        "--with-detector-probe",
        action="store_true",
        help="run `nose scan` on gap samples to SUGGEST covered/miss (needs --nose-binary)",
    )
    ap.add_argument("--detector-probe-limit", type=int, default=6)
    ap.add_argument("--build-ref", default=None)
    ap.add_argument("--json-out", type=Path, default=None)
    ap.add_argument("--markdown-out", type=Path, default=None)
    args = ap.parse_args()

    if args.selftest:
        return selftest()

    nose_binary = None
    if args.with_detector_probe:
        if args.nose_binary is None:
            ap.error("--with-detector-probe requires --nose-binary")
        nose_binary = args.nose_binary

    result = build(
        corpus_path=args.corpus,
        repos_root=args.repos_root,
        max_bytes=args.max_bytes,
        sample_limit=args.sample_limit,
        real_frontier=args.real_frontier,
        nose_binary=nose_binary,
        detector_probe_limit=args.detector_probe_limit if nose_binary else 0,
        build_ref=args.build_ref,
    )

    text = json.dumps(result, indent=2, sort_keys=True) + "\n"
    if args.json_out:
        args.json_out.write_text(text)
    else:
        sys.stdout.write(text)
    if args.markdown_out:
        args.markdown_out.write_text(markdown_report(result))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
