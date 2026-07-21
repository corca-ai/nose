"""Order-aware paired runtime estimator for query-regression reports."""

from __future__ import annotations

import math
import statistics
from dataclasses import dataclass
from typing import Any


POLICY = "paired-alternating-sign-v1"
POSITION_NEUTRAL_POLICY = "paired-position-neutral-sign-v2"
MIN_BLOCKS = 5
MIN_BLOCKS_PER_ORDER = 2
ORDERS = ("baseline-current", "current-baseline")


class ControlEvidenceError(ValueError):
    pass


@dataclass(frozen=True)
class Block:
    iteration: int
    order: str
    baseline: float
    current: float

    @property
    def delta(self) -> float:
        return self.current - self.baseline


def _median(values: list[float]) -> float:
    if not values:
        raise ControlEvidenceError("cannot estimate an empty block stratum")
    return statistics.median(values)


def _run_order(
    report: dict[str, Any],
    baseline: tuple[int, dict[str, Any]],
    current: tuple[int, dict[str, Any]],
) -> str:
    baseline_index, baseline_run = baseline
    current_index, current_run = current
    explicit = baseline_run.get("pair_order")
    if explicit is not None:
        if explicit not in ORDERS or current_run.get("pair_order") != explicit:
            raise ControlEvidenceError("paired runs disagree on explicit pair_order")
        expected_positions = (0, 1) if explicit == ORDERS[0] else (1, 0)
        positions = (baseline_run.get("pair_position"), current_run.get("pair_position"))
        if positions != expected_positions:
            raise ControlEvidenceError("paired runs have invalid explicit pair_position")
        return explicit
    if report.get("schema") == "nose.query_regression_harness.v3":
        raise ControlEvidenceError("v3 paired runs require explicit order metadata")
    return ORDERS[0] if baseline_index < current_index else ORDERS[1]


def _paired_runs(
    report: dict[str, Any], repo: str
) -> list[tuple[int, str, dict[str, Any], dict[str, Any]]]:
    by_iteration: dict[int, dict[str, tuple[int, dict[str, Any]]]] = {}
    for index, run in enumerate(report.get("runs", [])):
        if run.get("repo") != repo:
            continue
        iteration = run.get("iteration")
        label = run.get("label")
        if not isinstance(iteration, int) or label not in ("baseline", "current"):
            raise ControlEvidenceError(f"{repo}: malformed paired run identity")
        labels = by_iteration.setdefault(iteration, {})
        if label in labels:
            raise ControlEvidenceError(f"{repo}: duplicate {label} block {iteration}")
        labels[label] = (index, run)
    pairs = []
    for iteration, labels in sorted(by_iteration.items()):
        if set(labels) != {"baseline", "current"}:
            raise ControlEvidenceError(f"{repo}: incomplete block {iteration}")
        baseline = labels["baseline"]
        current = labels["current"]
        pairs.append(
            (iteration, _run_order(report, baseline, current), baseline[1], current[1])
        )
    return pairs


def _blocks(
    report: dict[str, Any],
    *,
    scope: str,
    repo: str | None,
    stage: str | None,
) -> list[Block]:
    repos = report.get("repos", []) if repo is None else [repo]
    paired = {name: _paired_runs(report, name) for name in repos}
    iterations = sorted({row[0] for rows in paired.values() for row in rows})
    blocks = []
    for iteration in iterations:
        selected = []
        for name in repos:
            matches = [row for row in paired[name] if row[0] == iteration]
            if len(matches) != 1:
                raise ControlEvidenceError(
                    f"{scope}: block {iteration} does not cover the complete repository set"
                )
            selected.append(matches[0])
        orders = {row[1] for row in selected}
        if len(orders) != 1:
            raise ControlEvidenceError(f"{scope}: block {iteration} has mixed pair order")

        def value(run: dict[str, Any]) -> float:
            if stage is None:
                raw = run.get("elapsed_ms")
            else:
                stages = run.get("stages_ms", {})
                raw = stages.get(stage, 0.0) if isinstance(stages, dict) else None
            if isinstance(raw, bool) or not isinstance(raw, (int, float)):
                raise ControlEvidenceError(f"{scope}: block {iteration} has non-numeric time")
            number = float(raw)
            if not math.isfinite(number) or number < 0:
                raise ControlEvidenceError(f"{scope}: block {iteration} has invalid time")
            return number

        blocks.append(
            Block(
                iteration=iteration,
                order=next(iter(orders)),
                baseline=sum(value(row[2]) for row in selected),
                current=sum(value(row[3]) for row in selected),
            )
        )
    return blocks


def _eligible(blocks: list[Block]) -> tuple[bool, str | None]:
    counts = {order: sum(block.order == order for block in blocks) for order in ORDERS}
    if len(blocks) < MIN_BLOCKS:
        return False, f"need at least {MIN_BLOCKS} complete blocks"
    if min(counts.values()) < MIN_BLOCKS_PER_ORDER:
        return False, f"need at least {MIN_BLOCKS_PER_ORDER} blocks in each order"
    if abs(counts[ORDERS[0]] - counts[ORDERS[1]]) > 1:
        return False, "pair orders are not balanced"
    return True, None


def _order_neutral_effect(blocks: list[Block]) -> float:
    return sum(
        _median([block.delta for block in blocks if block.order == order])
        for order in ORDERS
    ) / 2.0


def _sign_tail(successes: int, total: int) -> float:
    return sum(math.comb(total, count) for count in range(successes, total + 1)) / (2**total)


def estimate_metric(
    blocks: list[Block],
    control_blocks: list[Block],
    *,
    max_delta_pct: float,
    min_delta_ms: float,
    position_neutral: bool = False,
) -> dict[str, Any]:
    policy = POSITION_NEUTRAL_POLICY if position_neutral else POLICY
    eligible, reason = _eligible(blocks)
    control_eligible, control_reason = _eligible(control_blocks)
    if not eligible or not control_eligible:
        return {
            "policy": policy,
            "state": "insufficient",
            "reason": reason or f"control: {control_reason}",
            "blocks": len(blocks),
            "control_blocks": len(control_blocks),
        }
    if [(block.iteration, block.order) for block in blocks] != [
        (block.iteration, block.order) for block in control_blocks
    ]:
        return {
            "policy": policy,
            "state": "insufficient",
            "reason": "primary and control block designs differ",
            "blocks": len(blocks),
            "control_blocks": len(control_blocks),
        }
    raw_effect = _order_neutral_effect(blocks)
    control_effect = _order_neutral_effect(control_blocks)
    correction = max(control_effect, 0.0)
    adjusted_effect = raw_effect - correction
    baseline_ms = _median([block.baseline for block in blocks])
    adjusted_pct = (adjusted_effect / baseline_ms) * 100.0 if baseline_ms > 0 else None
    # Materiality is the conjunction of the absolute and relative thresholds.
    # A zero baseline has no finite relative increase, so it cannot satisfy both.
    point_material = (
        adjusted_effect > min_delta_ms
        and adjusted_pct is not None
        and adjusted_pct > max_delta_pct
    )
    adjusted_blocks = [(block, block.delta - correction) for block in blocks]
    successes = sum(
        delta > min_delta_ms
        and block.baseline > 0
        and (delta / block.baseline) * 100.0 > max_delta_pct
        for block, delta in adjusted_blocks
    )
    sign_p = _sign_tail(successes, len(blocks))
    order_consistent = True
    any_order_material = False
    order_rows = {}
    for order in ORDERS:
        order_blocks = [(block, delta) for block, delta in adjusted_blocks if block.order == order]
        order_delta = _median([delta for _, delta in order_blocks])
        order_baseline = _median([block.baseline for block, _ in order_blocks])
        order_pct = (order_delta / order_baseline) * 100.0 if order_baseline > 0 else None
        material = (
            order_delta > min_delta_ms
            and order_pct is not None
            and order_pct > max_delta_pct
        )
        any_order_material = any_order_material or material
        order_consistent = order_consistent and material
        order_rows[order] = {
            "blocks": len(order_blocks),
            "adjusted_median_ms": order_delta,
            "adjusted_median_pct": order_pct,
            "material": material,
        }
    supported = sign_p <= 0.05
    triggered = point_material and supported and (position_neutral or order_consistent)
    order_conflict = any_order_material and not order_consistent
    state = (
        "triggered"
        if triggered
        else "inconclusive"
        if point_material or (order_conflict and not position_neutral)
        else "within-threshold"
    )
    return {
        "policy": policy,
        "state": state,
        "blocks": len(blocks),
        "raw_effect_ms": raw_effect,
        "control_effect_ms": control_effect,
        "control_correction_ms": correction,
        "adjusted_effect_ms": adjusted_effect,
        "adjusted_effect_pct": adjusted_pct,
        "supporting_blocks": successes,
        "sign_test_p": sign_p,
        "supported": supported,
        "order_consistent": order_consistent,
        "order_conflict": order_conflict,
        "declared_order_conflict_ignored": position_neutral and order_conflict,
        "position_neutral_samples": position_neutral,
        "by_order": order_rows,
    }


def _uses_position_neutral_samples(report: dict[str, Any]) -> bool:
    measurement = report.get("measurement")
    if not isinstance(measurement, dict):
        return False
    samples = measurement.get("samples_per_observation", 1)
    return isinstance(samples, int) and not isinstance(samples, bool) and samples >= 5


def runtime_signals(
    report: dict[str, Any],
    control: dict[str, Any] | None,
    *,
    max_delta_pct: float,
    min_delta_ms: float,
) -> list[dict[str, Any]]:
    position_neutral = _uses_position_neutral_samples(report)
    if control is not None and _uses_position_neutral_samples(control) != position_neutral:
        raise ControlEvidenceError("primary and control sample aggregation designs differ")
    specifications: list[tuple[str, str | None, str | None]] = [("aggregate", None, None)]
    for repo in sorted(report.get("repos", [])):
        specifications.append(("repo", repo, None))
        stages = sorted(
            {
                stage
                for run in report.get("runs", [])
                if run.get("repo") == repo and isinstance(run.get("stages_ms"), dict)
                for stage in run["stages_ms"]
            }
        )
        specifications.extend(("stage", repo, stage) for stage in stages)
    signals = []
    for scope, repo, stage in specifications:
        blocks = _blocks(report, scope=scope, repo=repo, stage=stage)
        control_blocks = (
            _blocks(control, scope=scope, repo=repo, stage=stage)
            if control is not None
            else [Block(block.iteration, block.order, 0.0, 0.0) for block in blocks]
        )
        evidence = estimate_metric(
            blocks,
            control_blocks,
            max_delta_pct=max_delta_pct,
            min_delta_ms=min_delta_ms,
            position_neutral=position_neutral,
        )
        baseline_ms = _median([block.baseline for block in blocks]) if blocks else 0.0
        current_ms = _median([block.current for block in blocks]) if blocks else 0.0
        raw_delta_ms = evidence.get("raw_effect_ms", current_ms - baseline_ms)
        control_delta_ms = evidence.get("control_effect_ms", 0.0)
        adjusted_delta_ms = evidence.get("adjusted_effect_ms", raw_delta_ms)
        adjusted_delta_pct = evidence.get(
            "adjusted_effect_pct",
            (adjusted_delta_ms / baseline_ms) * 100.0 if baseline_ms > 0 else None,
        )
        signals.append(
            {
                "scope": scope,
                "repo": repo,
                "stage": stage,
                "baseline_ms": baseline_ms,
                "current_ms": current_ms,
                "raw_delta_ms": raw_delta_ms,
                "control_delta_ms": control_delta_ms,
                "adjusted_delta_ms": adjusted_delta_ms,
                "adjusted_delta_pct": adjusted_delta_pct,
                "triggered": evidence["state"] == "triggered",
                "inconclusive": evidence["state"] in ("inconclusive", "insufficient"),
                "evidence": evidence,
            }
        )
    return signals


def run_self_test() -> None:
    def blocks(deltas: list[float], baseline: float = 100.0) -> list[Block]:
        return [
            Block(index, ORDERS[(index - 1) % 2], baseline, baseline + delta)
            for index, delta in enumerate(deltas, 1)
        ]

    stable = estimate_metric(
        blocks([12.0] * 6), blocks([0.0] * 6), max_delta_pct=5.0, min_delta_ms=5.0
    )
    assert stable["state"] == "triggered" and stable["sign_test_p"] < 0.05
    positive = estimate_metric(
        blocks([12.0] * 6), blocks([3.0] * 6), max_delta_pct=5.0, min_delta_ms=5.0
    )
    assert positive["control_correction_ms"] == 3.0
    negative = estimate_metric(
        blocks([6.0] * 6), blocks([-3.0] * 6), max_delta_pct=5.0, min_delta_ms=5.0
    )
    assert negative["control_effect_ms"] == -3.0
    assert negative["control_correction_ms"] == 0.0
    order_bias = estimate_metric(
        blocks([12.0, -12.0] * 3),
        blocks([0.0] * 6),
        max_delta_pct=5.0,
        min_delta_ms=5.0,
    )
    assert order_bias["state"] == "inconclusive" and order_bias["order_conflict"]
    neutral_order_bias = estimate_metric(
        blocks([12.0, -12.0] * 3),
        blocks([0.0] * 6),
        max_delta_pct=5.0,
        min_delta_ms=5.0,
        position_neutral=True,
    )
    assert neutral_order_bias["state"] == "within-threshold"
    assert neutral_order_bias["declared_order_conflict_ignored"]
    noisy = estimate_metric(
        blocks([20.0, 20.0, -5.0, 20.0, -5.0, 20.0]),
        blocks([0.0] * 6),
        max_delta_pct=5.0,
        min_delta_ms=5.0,
    )
    assert noisy["state"] == "inconclusive" and not noisy["supported"]
    missing = estimate_metric(
        blocks([12.0] * 4), blocks([0.0] * 4), max_delta_pct=5.0, min_delta_ms=5.0
    )
    assert missing["state"] == "insufficient"
    zero_baseline = estimate_metric(
        blocks([12.0] * 6, baseline=0.0),
        blocks([0.0] * 6, baseline=0.0),
        max_delta_pct=5.0,
        min_delta_ms=5.0,
    )
    assert zero_baseline["adjusted_effect_pct"] is None
    assert zero_baseline["supporting_blocks"] == 0
    assert zero_baseline["state"] == "within-threshold"

    def run(iteration: int, label: str) -> dict[str, Any]:
        return {
            "iteration": iteration,
            "label": label,
            "pair_order": ORDERS[(iteration - 1) % 2],
            "pair_position": 0 if (iteration % 2 == 1) == (label == "baseline") else 1,
            "repo": "repo-a",
        }

    incomplete_report = {
        "schema": "nose.query_regression_harness.v3",
        "runs": [run(1, "baseline")],
    }
    try:
        _paired_runs(incomplete_report, "repo-a")
    except ControlEvidenceError as error:
        assert "incomplete block" in str(error)
    else:
        raise AssertionError("an incomplete pair must not be inferred")
    duplicate_report = {
        "schema": "nose.query_regression_harness.v3",
        "runs": [run(1, "baseline"), run(1, "baseline")],
    }
    try:
        _paired_runs(duplicate_report, "repo-a")
    except ControlEvidenceError as error:
        assert "duplicate baseline" in str(error)
    else:
        raise AssertionError("duplicate rounds must not be silently dropped")
