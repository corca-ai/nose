#!/usr/bin/env python3
"""Team B (#50) extra frontier axes, kept OUT of `prioritize_frontier.py`.

`prioritize_frontier.py` and its `FRONTIER_PRIORITIES.md` are byte-stable (#44 / #41
reproducibility). New corpus-driven candidate axes discovered by the frontier evidence
workflow live here instead; `frontier_platform.py` unions `prioritize_frontier.CANDIDATES`
with `EXTRA_CANDIDATES` below.

An axis here is a genuine *semantic invariant*, never a language-specific API spelling. Each
new axis must carry curated controlled-vocabulary metadata in `EXTRA_CURATED` (no
auto-estimation; `unknown` when genuinely unknown) and is covered by the union staleness
guard in `frontier_platform.py`.

Discovery discipline (issue #50 decision 4): an axis reaches a target packet only after
corpus signal → detector-suggested probe → a human confirms the semantic claim, proof
invariant, and hard negatives → dev AND held-out presence. Prevalence alone never promotes.
"""

from __future__ import annotations

import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import prioritize_frontier as pf  # noqa: E402  (reuse Candidate / PatternSpec / helpers)


# numeric_clamp — scalar clamp via bounded min/max composition.
#
# `min(max(x, lo), hi)` and `max(min(x, hi), lo)` and `x < lo ? lo : (x > hi ? hi : x)` all
# denote the same value for `lo <= hi`, yet the value graph converges none of them (unlike
# the structurally-similar abs idiom, which it DOES canonicalize). It is prevalent across all
# seven corpus primary languages on both splits. The identity (and its hard negatives) is
# machine-checked in `formal/Clamp.lean`.
#
# Patterns are the nested-call / built-in `clamp` forms ONLY. The `.min(...).max(...)` method
# chain is intentionally excluded: it false-positives on schema/query builders such as Zod
# `z.string().min(1).max(100)`, which is not a numeric clamp.
EXTRA_CANDIDATES = [
    pf.Candidate(
        "numeric_clamp",
        "Scalar clamp (bounded min/max composition)",
        "all-language",
        3,
        3,
        "open",
        "min(max(x,lo),hi) clamp composition is a real under-merge vs the two-comparison "
        "form; prevalent across all seven languages on both splits; composes already-proven "
        "scalar min/max facts. Backed by formal/Clamp.lean.",
        "Audit corpus clamp pairs; require the lo<=hi precondition and reject swapped bound "
        "order, wrong nesting, and float NaN as hard negatives.",
        (
            pf.pat("c_clamp_nested", "c", r"\b(?:min|MIN)\s*\(\s*(?:max|MAX)\s*\(|\b(?:max|MAX)\s*\(\s*(?:min|MIN)\s*\(", "high"),
            pf.pat("go_clamp_nested", "go", r"\bmin\s*\(\s*max\s*\(|\bmax\s*\(\s*min\s*\(", "high"),
            pf.pat("java_clamp_math", "java", r"\bMath\.min\s*\(\s*Math\.max\s*\(|\bMath\.max\s*\(\s*Math\.min\s*\(", "high"),
            pf.pat("java_clamp_method", "java", r"\.\s*clamp\s*\(", "high"),
            pf.pat("js_clamp_math", "javascript", r"\bMath\.min\s*\(\s*Math\.max\s*\(|\bMath\.max\s*\(\s*Math\.min\s*\(", "high"),
            pf.pat("py_clamp_nested", "python", r"\bmin\s*\(\s*max\s*\(|\bmax\s*\(\s*min\s*\(", "high"),
            pf.pat("ruby_clamp_method", "ruby", r"\.\s*clamp\s*\(", "high"),
            pf.pat("rust_clamp_method", "rust", r"\.\s*clamp\s*\(", "high"),
            pf.pat("rust_clamp_nested", "rust", r"\bmin\s*\(\s*max\s*\(|\bmax\s*\(\s*min\s*\(", "high"),
            pf.pat("ts_clamp_math", "typescript", r"\bMath\.min\s*\(\s*Math\.max\s*\(|\bMath\.max\s*\(\s*Math\.min\s*\(", "high"),
        ),
    ),
]

# Broad probes for gap detection, per candidate. Same family as the patterns here (the
# nested/`clamp` forms are already the precise signal), so the broad-probe gap is ~empty —
# the axis's value is the under-merge it represents, not an uncovered-probe queue.
EXTRA_PROBES_BY_CANDIDATE: dict[str, tuple] = {
    "numeric_clamp": (
        pf.probe("clamp_go", "go", r"\bmin\s*\(\s*max\s*\(|\bmax\s*\(\s*min\s*\("),
        pf.probe("clamp_py", "python", r"\bmin\s*\(\s*max\s*\(|\bmax\s*\(\s*min\s*\("),
        pf.probe("clamp_rs", "rust", r"\.\s*clamp\s*\("),
    ),
}

# Curated controlled-vocabulary metadata for the extra axes (issue #50 decision 1/5).
# Controlled vocabulary only; never auto-estimated. `substrate_required` routes #43/#49.
EXTRA_CURATED: dict[str, dict] = {
    "numeric_clamp": {
        "implementation_cost": "medium",
        "soundness_risk": "medium",
        # Value-graph canonicalization composing proven scalar min/max facts — not a
        # sub-function fragment shape, so not the #33 fragment substrate.
        "substrate_required": "none",
        "rationale": "A value-graph clamp canonicalization over already-proven scalar "
        "min/max facts; lo<=hi precondition and float NaN are the soundness boundaries "
        "(machine-checked in formal/Clamp.lean).",
    },
}
