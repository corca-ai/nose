"""Validated registry of domain-owned semantic-axis policies."""

from __future__ import annotations

from type4gen.axis_boundary_policy import AXIS_POLICIES as BOUNDARY_POLICIES
from type4gen.axis_collection_policy import AXIS_POLICIES as COLLECTION_POLICIES
from type4gen.axis_map_policy import AXIS_POLICIES as MAP_POLICIES
from type4gen.axis_membership_policy import AXIS_POLICIES as MEMBERSHIP_POLICIES
from type4gen.axis_policy import AxisPolicy
from type4gen.axis_proposals import AXIS_PROPOSALS
from type4gen.axis_scalar_policy import AXIS_POLICIES as SCALAR_POLICIES


def build_axis_policies() -> dict[str, AxisPolicy]:
    policies: dict[str, AxisPolicy] = {}
    for domain_policies in (
        BOUNDARY_POLICIES,
        COLLECTION_POLICIES,
        MEMBERSHIP_POLICIES,
        MAP_POLICIES,
        SCALAR_POLICIES,
    ):
        overlap = policies.keys() & domain_policies.keys()
        if overlap:
            raise ValueError(f"duplicate axis policies: {sorted(overlap)}")
        policies.update(domain_policies)

    proposal_axes = {proposal["axis"] for proposal in AXIS_PROPOSALS.values()}
    missing = proposal_axes - policies.keys()
    extra = policies.keys() - proposal_axes
    if missing or extra:
        raise ValueError(
            f"axis policy registry mismatch: missing={sorted(missing)}, extra={sorted(extra)}"
        )
    return policies


AXIS_POLICIES = build_axis_policies()
