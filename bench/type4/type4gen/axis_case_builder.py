"""Domain-independent materialization of semantic-axis benchmark cases."""

from __future__ import annotations

from pathlib import Path

from type4gen.axis_evidence import axis_evidence
from type4gen.axis_policy import AxisPolicy
from type4gen.axis_proposals import AXIS_PROPOSALS
from type4gen.case_io import (
    rel_source_path,
    source_record,
    stable_id,
    write_source,
)
from type4gen.model import (
    SEMANTIC_SCOPE,
    Surface,
    capability_exact_supported,
    surface_capability,
)


def materialize_axis_item(
    out_dir: Path,
    capabilities: dict,
    proposal_id: str,
    surface: Surface,
    semantic_status: str,
    split: str,
    negative_tag: str | None = None,
    *,
    policies: dict[str, AxisPolicy],
) -> dict:
    proposal = AXIS_PROPOSALS[proposal_id]
    axis = proposal["axis"]
    policy = policies[axis]
    capability = surface_capability(capabilities, surface, axis)
    negative = semantic_status == "not_equivalent"
    case_id = stable_id(
        proposal_id,
        surface.key,
        semantic_status,
        split,
        negative_tag or "positive",
    )
    left, right = policy.variants(surface, proposal_id, negative)
    left_path = rel_source_path(case_id, "left", surface)
    right_path = rel_source_path(case_id, "right", surface)
    write_source(out_dir, left_path, left.source)
    write_source(out_dir, right_path, right.source)
    exact_supported = capability_exact_supported(capabilities, surface, axis)
    equivalent = semantic_status == "equivalent"
    transform_tags = [axis, "semantic-axis"]
    if negative_tag is not None:
        transform_tags += ["hard-negative", negative_tag]
    return {
        "case_id": case_id,
        "proposal_id": proposal_id,
        "split": split,
        "semantic_status": semantic_status,
        "expected_exact_detect": equivalent and exact_supported,
        "semantic_scope": SEMANTIC_SCOPE,
        "transform_tags": transform_tags,
        "matrix": {
            "computation": axis,
            "representations": [left.representation, right.representation],
            "data_shape": policy.data_shape,
            "language_relation": "same-surface",
            "negative_tag": negative_tag,
            "semantic_axes": [axis],
            "capabilities": {axis: capability},
            "template_split": split,
        },
        "left": source_record(surface, left, left_path),
        "right": source_record(surface, right, right_path),
        "evidence": axis_evidence(axis, semantic_status, negative, proposal_id),
        "llm_proposal": {
            "why": proposal["why"],
            "complexity_budget": {
                "max_lines": 12,
                "max_branch_count": 0,
                "max_primary_transforms": 1,
                "max_secondary_transforms": 1,
            },
        },
    }


def materialize_axis_cross_item(
    out_dir: Path,
    capabilities: dict,
    proposal_id: str,
    left_surface: Surface,
    right_surface: Surface,
    semantic_status: str,
    split: str,
    negative_tag: str | None = None,
    *,
    policies: dict[str, AxisPolicy],
) -> dict:
    proposal = AXIS_PROPOSALS[proposal_id]
    axis = proposal["axis"]
    policy = policies[axis]
    negative = semantic_status == "not_equivalent"
    case_id = stable_id(
        proposal_id,
        left_surface.key,
        right_surface.key,
        semantic_status,
        split,
        negative_tag or "positive",
    )
    left = policy.variants(left_surface, proposal_id, False)[0]
    right = policy.variants(right_surface, proposal_id, negative)[1]
    left_path = rel_source_path(case_id, "left", left_surface)
    right_path = rel_source_path(case_id, "right", right_surface)
    write_source(out_dir, left_path, left.source)
    write_source(out_dir, right_path, right.source)
    left_capability = surface_capability(capabilities, left_surface, axis)
    right_capability = surface_capability(capabilities, right_surface, axis)
    equivalent = semantic_status == "equivalent"
    expected = (
        equivalent
        and capability_exact_supported(capabilities, left_surface, axis)
        and capability_exact_supported(capabilities, right_surface, axis)
    )
    transform_tags = [axis, "semantic-axis"]
    if negative_tag is not None:
        transform_tags += ["hard-negative", negative_tag]
    return {
        "case_id": case_id,
        "proposal_id": proposal_id,
        "split": split,
        "semantic_status": semantic_status,
        "expected_exact_detect": expected,
        "semantic_scope": SEMANTIC_SCOPE,
        "transform_tags": transform_tags,
        "matrix": {
            "computation": axis,
            "representations": [left.representation, right.representation],
            "data_shape": policy.data_shape,
            "language_relation": "cross-surface",
            "negative_tag": negative_tag,
            "semantic_axes": [axis],
            "capabilities": {
                f"{axis}:left": left_capability,
                f"{axis}:right": right_capability,
            },
            "template_split": split,
        },
        "left": source_record(left_surface, left, left_path),
        "right": source_record(right_surface, right, right_path),
        "evidence": axis_evidence(axis, semantic_status, negative, proposal_id),
        "llm_proposal": {
            "why": proposal["why"],
            "complexity_budget": {
                "max_lines": 12,
                "max_branch_count": 0,
                "max_primary_transforms": 1,
                "max_secondary_transforms": 1,
            },
        },
    }
