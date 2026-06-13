# Scan-JSON agent-usability re-validation, 2026-06-13

The [2026-06-10 audit](scanjson-agent-audit-2026-06-10.md) measured whether an LLM agent
([design](design.md) §2's consumer 1) can decide and act from scan JSON alone, and scored
**14 of 18** families decidable — with a ranked list of five evidence gaps behind the four
failures. Since then those gaps have been filled, most recently by the
[graded witness](graded-witness.md) (#315, which targeted gaps 1–2). This re-validation
asks: **do the fixes actually close the gaps?** The data artifact is
[bench/labels/scanjson_agent_revalidate_2026_06_13.json](../bench/labels/scanjson_agent_revalidate_2026_06_13.json).

## Protocol

The four previously-failed families were re-located in the current detector output (the
*same discriminating cases*; detection has shifted since June 10, so this is the same
protocol on the same cases, not byte-identical families), plus a fresh four-repo sample
(top default family of `flask`, `gin`, `rxjs`, `jedis`). An independent agent decided
worthy / not-worthy / **undecidable** for each using **only the scan-JSON object** — no
source access — and named the JSON fields that drove the call. Decisions were then graded
against the known truth.

## The five gaps are closed

Each gap from the 2026-06-10 ranking now carries its evidence in the JSON:

| gap (2026-06-10) | closing evidence (confirmed in output) |
|---|---|
| 1. no equivalence witness on default families | `witness.kind` on every family + `mean_value_jaccard`/`mean_shape_jaccard` for near; `witness.graded` for same-language near ([graded witness](graded-witness.md), #315) |
| 2. no difference evidence | `varying_spots` (per-side text) + `witness.graded.spots` (classified holes with text) — #315 |
| 3. generated markers neither surfaced nor honored | `looks_generated` per location + `recommended_surface: "generated"` (off the default surface) |
| 4. block locations carry no enclosing unit name | `enclosing_unit` (file/span/kind/name) on every block location |
| 5. Rust inline `mod tests` scope-tagged `prod` | `scope: "test"` (inline-test-module awareness in `is_test_loc`) |

## Score

**8 of 8 decidable from the JSON alone** — the four previously-failed cases are all now
decidable, and all four decisions are correct against the truth:

| case | 2026-06-10 | now | closing field |
|---|---|---|---|
| antlr4 cross-lang (`PredictionContext`) | undecidable | not-worthy (cross-language port) | `languages: 4` + the witness's `mean_value_jaccard` |
| arrow `locales.py` | wrongly *worthy* | not-worthy (parallel-by-design locale code) | `varying_spots` (the `German`↔`Sinhala` difference evidence) |
| cmark `scanners.c` | wrongly *worthy* | not-worthy (generated) | `recommended_surface: "generated"` + `looks_generated` |
| alacritty `vi_mode.rs` | wrong scope | not-worthy (test scaffold) | `scope: "test"` |

The fresh sample was also 4/4 decidable, with the graded witness actively driving two of
them: `gin` (one `input` hole, `DebugMode`↔`TestMode` — a parameterized test) and `jedis`
(`caveat_names: [endpoint, protocol]` — per-class setup, not a clean hoist).

## What this concludes — and does not

The consumer-1 loop [design](design.md) §2 calls **the** lever now closes on this sample:
the JSON carries *why* (witness kind/Jaccard), *what differs* (varying spots, graded
holes), *where* (spans, enclosing names), and the *non-action signals* (generated, test
scope) an agent needs to decide without re-deriving the analysis.

Honest limits: the sample is small (8) and every family resolved to *not-worthy* — this
set has no genuine prod refactor target to confirm the *worthy* direction with equal force
(the `worthy` arm is exercised by the [benchmark](benchmark.md)'s P@10, not here). The
weakest decision (`flask`) rests on `scope: test` + `shared_lines: 2` alone — a 2-line body
carries no `varying_spots`/`graded`/`enclosing`, so it is decided on size, not content. The
recurring fresh-repo head-of-ranking audit ([design](design.md) §2c) remains the standing
instrument; this confirms the #315 evidence investment paid off against the gaps it
targeted.

*See also: [scanjson-agent-audit-2026-06-10](scanjson-agent-audit-2026-06-10.md) ·
[graded-witness](graded-witness.md) · [scan JSON](scan-json.md) · [design](design.md) ·
[agent-recipe](agent-recipe.md).*
