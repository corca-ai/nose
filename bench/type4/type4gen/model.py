from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path

SEMANTIC_SCOPE = (
    "pure integer lists, deterministic evaluation, no overflow modeling; "
    "C `(int *xs, int n)` and aligned `(int *a, int *b, int n)` cases treat `n` "
    "as the exact logical length of the traversed array(s); "
    "C predicate reductions use 1/0 as boolean true/false"
)
PROPERTY_INPUTS = [
    [],
    [-1, 0, 2],
    [0],
    [0, -1],
    [0, 2],
    [1, -2, 3],
    [1, 0],
    [1, 2],
    [2],
    [2, 3],
    [3],
    [-2, 4],
]
PAIR_PROPERTY_INPUTS = [
    {"a": [], "b": []},
    {"a": [1], "b": [2]},
    {"a": [1, 2], "b": [3, 4]},
    {"a": [-1, 2, 0], "b": [4, -3, 5]},
    {"a": [0, 2, 3], "b": [7, 0, -1]},
    {"a": [-2, 4], "b": [-5, 6]},
]

@dataclass(frozen=True)
class Surface:
    key: str
    language: str
    extension: str
    wrapper: str = "base"


@dataclass(frozen=True)
class GenerationFilter:
    axes: frozenset[str]
    proposal_prefixes: tuple[str, ...]

    @property
    def active(self) -> bool:
        return bool(self.axes or self.proposal_prefixes)

    def include_axis(self, axis: str) -> bool:
        return not self.axes or axis in self.axes

    def include_proposal(self, proposal_id: str) -> bool:
        return not self.proposal_prefixes or any(
            proposal_id.startswith(prefix) for prefix in self.proposal_prefixes
        )

    def include_base_proposal(self, proposal: dict) -> bool:
        if not self.include_proposal(proposal["proposal_id"]):
            return False
        if not self.axes:
            return True
        return "aggregate_reduction" in self.axes or proposal["operation"] in self.axes

    def include_axis_proposal(self, proposal_id: str, axis: str) -> bool:
        return self.include_axis(axis) and self.include_proposal(proposal_id)


@dataclass(frozen=True)
class Variant:
    representation: str
    source: str
    entrypoint: str
    start_line: int = 1


@dataclass(frozen=True)
class Operation:
    key: str
    kind: str
    positive_predicate: str
    negative_predicate: str | None = None
    negative_kind: str | None = None
    positive_contribution: str = "identity"
    negative_contribution: str | None = None
    positive_init: int = 0
    negative_init: int | None = None
    negative_reason: str = "predicate mutation"
    arity: int = 1


SURFACES = [
    Surface("python", "python", "py"),
    Surface("javascript", "javascript", "js"),
    Surface("typescript", "typescript", "ts"),
    Surface("go", "go", "go"),
    Surface("rust", "rust", "rs"),
    Surface("java", "java", "java"),
    Surface("c", "c", "c"),
    Surface("ruby", "ruby", "rb"),
    Surface("vue", "javascript", "vue", "script"),
    Surface("svelte", "javascript", "svelte", "script"),
    Surface("html", "javascript", "html", "script"),
]
JS_LIKE_SURFACES = {"javascript", "typescript", "vue", "svelte", "html"}

OPERATIONS = {
    "sum_positive": Operation(
        "sum_positive",
        "sum",
        "gt0",
        negative_init=1,
        negative_reason="initial accumulator 0 -> 1",
    ),
    "count_positive": Operation("count_positive", "count", "gt0", "ge0"),
    "any_positive": Operation("any_positive", "any", "gt0", "ge0"),
    "all_nonnegative": Operation("all_nonnegative", "all", "ge0", "gt0"),
    "product_positive": Operation(
        "product_positive",
        "product",
        "gt0",
        positive_init=1,
        negative_init=2,
        negative_reason="initial accumulator 1 -> 2",
    ),
    "sum_even": Operation("sum_even", "sum", "even", "odd"),
    "count_negative": Operation("count_negative", "count", "lt0", "le0"),
    "any_zero": Operation("any_zero", "any", "eq0", "ne0"),
    "all_nonzero": Operation("all_nonzero", "all", "ne0", "gt0"),
    "product_nonzero": Operation("product_nonzero", "product", "ne0", "gt0", positive_init=1),
    "sum_small": Operation("sum_small", "sum", "lt3", "le3"),
    "count_small": Operation("count_small", "count", "lt3", "le3"),
    "any_even": Operation("any_even", "any", "even", "odd"),
    "max_seed_zero": Operation(
        "max_seed_zero",
        "max",
        "true",
        negative_kind="min",
        positive_init=0,
        negative_reason="max selection -> min selection",
    ),
    "min_seed_zero": Operation(
        "min_seed_zero",
        "min",
        "true",
        negative_kind="max",
        positive_init=0,
        negative_reason="min selection -> max selection",
    ),
    "sum_positive_squares": Operation(
        "sum_positive_squares",
        "sum",
        "gt0",
        positive_contribution="square",
        negative_contribution="identity",
        negative_reason="per-element contribution x*x -> x",
    ),
    "product_nonzero_squares": Operation(
        "product_nonzero_squares",
        "product",
        "ne0",
        positive_contribution="square",
        negative_contribution="identity",
        positive_init=1,
        negative_reason="per-element contribution x*x -> x",
    ),
    "dot_product": Operation(
        "dot_product",
        "sum",
        "true",
        arity=2,
        positive_contribution="pair_product",
        negative_contribution="pair_sum",
        negative_reason="pair contribution x*y -> x+y",
    ),
    "sum_abs_all": Operation(
        "sum_abs_all",
        "sum_abs",
        "true",
        negative_kind="sum",
        negative_reason="absolute contribution abs(x) -> x",
    ),
}

REQUIRED_PROPOSAL_FIELDS = {
    "proposal_id",
    "operation",
    "why",
    "positive_spec",
    "negative_mutations",
    "transform_tags",
    "complexity_budget",
}
REQUIRED_BUDGET_FIELDS = {
    "max_lines",
    "max_branch_count",
    "max_primary_transforms",
    "max_secondary_transforms",
}


def snake_to_camel(name: str) -> str:
    parts = name.split("_")
    return parts[0] + "".join(p.title() for p in parts[1:])


def snake_to_pascal(name: str) -> str:
    return "".join(p.title() for p in name.split("_"))


def js_script_wrap(surface: Surface, source: str) -> str:
    if surface.key == "vue":
        return f"<template><div></div></template>\n<script>\n{source}\n</script>\n"
    if surface.key == "svelte":
        return f"<script>\n{source}\n</script>\n<div></div>\n"
    if surface.key == "html":
        return f"<!doctype html>\n<script>\n{source}\n</script>\n"
    return source


def js_start_line(surface: Surface) -> int:
    if surface.key == "svelte":
        return 2
    if surface.key in {"vue", "html"}:
        return 3
    return 1


def load_capabilities(path: Path) -> dict:
    return json.loads(path.read_text())


def surface_capability(capabilities: dict, surface: Surface, axis: str) -> str:
    return capabilities["surfaces"].get(surface.key, {}).get(axis, "unsupported")


def capability_exact_supported(capabilities: dict, surface: Surface, axis: str) -> bool:
    return surface_capability(capabilities, surface, axis) in {"supported", "partial"}


def js_axis_source(surface: Surface, body: str, entrypoint: str = "axisCase") -> Variant:
    return Variant("axis", js_script_wrap(surface, body), entrypoint, js_start_line(surface))


