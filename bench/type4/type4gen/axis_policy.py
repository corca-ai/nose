"""Shared policy types for semantic-axis case generation."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Callable

from type4gen.model import Surface, Variant


@dataclass(frozen=True)
class AxisCasePlan:
    semantic_status: str
    split: str
    negative_tag: str | None = None


VariantFactory = Callable[[Surface, str, bool], tuple[Variant, Variant]]
SameSurfacePlanner = Callable[[Surface, str, str], tuple[AxisCasePlan, ...]]


@dataclass(frozen=True)
class AxisPolicy:
    variants: VariantFactory
    data_shape: str
    same_surface_plans: SameSurfacePlanner


DEFAULT_CASE_PLANS = (
    AxisCasePlan("equivalent", "dev"),
    AxisCasePlan("not_equivalent", "heldout", "semantic-mutation"),
)


def default_case_plans(
    _surface: Surface,
    _proposal_id: str,
    _capability: str,
) -> tuple[AxisCasePlan, ...]:
    return DEFAULT_CASE_PLANS


def boundary_case_plan(tag: str, status: str = "not_equivalent") -> tuple[AxisCasePlan, ...]:
    return (AxisCasePlan(status, "heldout", tag),)
