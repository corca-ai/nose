"""Compatibility facade and orchestration for semantic-axis cases."""

from __future__ import annotations

from pathlib import Path

from type4gen.axis_case_builder import (
    materialize_axis_cross_item,
    materialize_axis_item,
)
from type4gen.axis_proposals import AXIS_PROPOSALS
from type4gen.axis_registry import AXIS_POLICIES
from type4gen.case_io import cross_pairs
from type4gen.model import (
    SURFACES,
    GenerationFilter,
    Surface,
    Variant,
    surface_capability,
)


def axis_variants(
    surface: Surface,
    proposal_id: str,
    axis: str,
    negative: bool,
) -> tuple[Variant, Variant]:
    try:
        policy = AXIS_POLICIES[axis]
    except KeyError:
        raise ValueError(f"unknown axis: {axis}") from None
    return policy.variants(surface, proposal_id, negative)


def axis_data_shape(axis: str) -> str:
    policy = AXIS_POLICIES.get(axis)
    return policy.data_shape if policy is not None else "scalar<int>"


def make_axis_item(
    out_dir: Path,
    capabilities: dict,
    proposal_id: str,
    surface: Surface,
    semantic_status: str,
    split: str,
    negative_tag: str | None = None,
) -> dict:
    return materialize_axis_item(
        out_dir,
        capabilities,
        proposal_id,
        surface,
        semantic_status,
        split,
        negative_tag,
        policies=AXIS_POLICIES,
    )


def generate_axis_items(
    out_dir: Path,
    capabilities: dict,
    generation_filter: GenerationFilter,
) -> list[dict]:
    items: list[dict] = []
    for surface in SURFACES:
        for proposal_id, proposal in AXIS_PROPOSALS.items():
            axis = proposal["axis"]
            if not generation_filter.include_axis_proposal(proposal_id, axis):
                continue
            capability = surface_capability(capabilities, surface, axis)
            policy = AXIS_POLICIES[axis]
            for plan in policy.same_surface_plans(surface, proposal_id, capability):
                negative_tag = plan.negative_tag
                if negative_tag == "semantic-mutation":
                    negative_tag = f"{axis}-semantic-mutation"
                items.append(
                    make_axis_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        surface,
                        plan.semantic_status,
                        plan.split,
                        negative_tag,
                    )
                )
    return items


def make_axis_cross_item(
    out_dir: Path,
    capabilities: dict,
    proposal_id: str,
    left_surface: Surface,
    right_surface: Surface,
    semantic_status: str,
    split: str,
    negative_tag: str | None = None,
) -> dict:
    return materialize_axis_cross_item(
        out_dir,
        capabilities,
        proposal_id,
        left_surface,
        right_surface,
        semantic_status,
        split,
        negative_tag,
        policies=AXIS_POLICIES,
    )
