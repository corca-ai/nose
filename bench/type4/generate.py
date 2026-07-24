#!/usr/bin/env python3
"""Generate the seed corpus for the evidence-carrying Type-4 benchmark factory."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
from pathlib import Path

from type4gen.aggregate import (
    EMITTERS,
    evidence_contract_negative,
    evidence_negative,
    evidence_positive,
)
from type4gen.axis_boundaries import (
    axis_projection_variant,
    axis_python_docstring_variant,
    axis_unsafe_boundary_variant,
    import_axis_supported,
    import_axis_variant,
    projection_axis_supported,
    python_docstring_axis_supported,
)
from type4gen.axis_collections import (
    axis_collection_empty_variant,
    axis_own_property_variant,
    axis_record_guard_variant,
    axis_string_prefix_variant,
    collection_empty_axis_supported,
    own_property_axis_supported,
    record_guard_axis_supported,
    string_prefix_axis_supported,
)
from type4gen.axis_maps import (
    axis_map_default_lookup_variant,
    axis_map_default_variant,
    axis_map_key_membership_variant,
    literal_map_default_axis_supported,
    map_default_lookup_axis_supported,
    map_key_membership_axis_supported,
)
from type4gen.axis_membership import (
    axis_membership_literal_variant,
    literal_membership_axis_supported,
)
from type4gen.axis_scalar import (
    axis_c_u16_be_byte_pack_variant,
    axis_c_u32_be_byte_pack_variant,
    axis_callee_identity_variant,
    axis_hof_filter_map_variant,
    axis_immutable_binding_variant,
    axis_java_dead_loop_variant,
    axis_java_low_bit_toggle_variant,
    axis_null_presence_variant,
    axis_nullish_variant,
    axis_numeric_clamp_variant,
    axis_scalar_abs_variant,
    axis_scalar_minmax_variant,
    axis_table_access_variant,
    axis_total_order_compare_variant,
    c_u16_be_byte_pack_axis_supported,
    c_u32_be_byte_pack_axis_supported,
    java_dead_loop_axis_supported,
    java_low_bit_toggle_axis_supported,
    null_presence_axis_supported,
    nullish_axis_supported,
    numeric_clamp_axis_supported,
    scalar_abs_axis_supported,
    total_order_compare_axis_supported,
)
from type4gen.axis_proposals import AXIS_PROPOSALS
from type4gen.model import (
    OPERATIONS,
    PROPERTY_INPUTS,
    REQUIRED_BUDGET_FIELDS,
    REQUIRED_PROPOSAL_FIELDS,
    SEMANTIC_SCOPE,
    SURFACES,
    GenerationFilter,
    Surface,
    Variant,
    capability_exact_supported,
    load_capabilities,
    surface_capability,
)

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_PROPOSALS = ROOT / "bench" / "type4" / "proposals.v1.json"
DEFAULT_CAPABILITIES = ROOT / "bench" / "type4" / "capabilities.v1.json"


def stable_id(*parts: str) -> str:
    h = hashlib.sha256()
    for p in parts:
        h.update(p.encode())
        h.update(b"\0")
    return h.hexdigest()[:16]


def rel_source_path(case_id: str, side: str, surface: Surface) -> Path:
    return Path("sources") / case_id / f"{side}.{surface.extension}"


def source_record(surface: Surface, variant: Variant, path: Path) -> dict:
    return {
        "language": surface.language,
        "surface": surface.key,
        "representation": variant.representation,
        "path": path.as_posix(),
        "entrypoint": variant.entrypoint,
        "start_line": variant.start_line,
        "end_line": variant.start_line + len(variant.source.rstrip("\n").splitlines()) - 1,
    }


def write_source(out_dir: Path, rel_path: Path, source: str) -> None:
    full = out_dir / rel_path
    full.parent.mkdir(parents=True, exist_ok=True)
    full.write_text(source)


def validate_proposals(proposals_doc: dict) -> None:
    seen: set[str] = set()
    for proposal in proposals_doc.get("proposals", []):
        missing = REQUIRED_PROPOSAL_FIELDS - proposal.keys()
        if missing:
            raise ValueError(f"{proposal.get('proposal_id', '<unknown>')} missing fields: {sorted(missing)}")
        if proposal["proposal_id"] in seen:
            raise ValueError(f"duplicate proposal_id: {proposal['proposal_id']}")
        seen.add(proposal["proposal_id"])
        if proposal["operation"] not in OPERATIONS:
            raise ValueError(
                f"{proposal['proposal_id']} references unknown operation {proposal['operation']}"
            )
        budget = proposal["complexity_budget"]
        missing_budget = REQUIRED_BUDGET_FIELDS - budget.keys()
        if missing_budget:
            raise ValueError(f"{proposal['proposal_id']} missing budget fields: {sorted(missing_budget)}")
        for field in REQUIRED_BUDGET_FIELDS:
            if not isinstance(budget[field], int) or budget[field] < 0:
                raise ValueError(f"{proposal['proposal_id']} budget {field} must be a non-negative integer")


def check_variant_budget(proposal: dict, surface: Surface, variant: Variant) -> None:
    budget = proposal["complexity_budget"]
    lines = len(variant.source.rstrip("\n").splitlines())
    if lines > budget["max_lines"]:
        raise ValueError(
            f"{proposal['proposal_id']} {surface.key}:{variant.representation} has "
            f"{lines} lines > budget {budget['max_lines']}"
        )
    branch_count = len(re.findall(r"\bif\b", variant.source))
    if branch_count > budget["max_branch_count"]:
        raise ValueError(
            f"{proposal['proposal_id']} {surface.key}:{variant.representation} has "
            f"{branch_count} branches > budget {budget['max_branch_count']}"
        )


def make_item(
    out_dir: Path,
    proposal: dict,
    left_surface: Surface,
    right_surface: Surface,
    right_representation: str,
    semantic_status: str,
    cross_label: str,
    split: str,
    negative_tag: str | None = None,
) -> dict:
    operation = proposal["operation"]
    if operation not in OPERATIONS:
        raise ValueError(f"{proposal['proposal_id']} references unknown operation {operation}")
    negative = semantic_status == "not_equivalent"
    case_id = stable_id(
        proposal["proposal_id"],
        left_surface.key,
        right_surface.key,
        right_representation,
        semantic_status,
        cross_label,
        negative_tag or "positive",
    )
    left = EMITTERS[left_surface.key](operation, "loop", False)
    right = EMITTERS[right_surface.key](operation, right_representation, negative)
    check_variant_budget(proposal, left_surface, left)
    check_variant_budget(proposal, right_surface, right)
    left_path = rel_source_path(case_id, "left", left_surface)
    right_path = rel_source_path(case_id, "right", right_surface)
    write_source(out_dir, left_path, left.source)
    write_source(out_dir, right_path, right.source)
    equivalent = semantic_status == "equivalent"
    evidence = evidence_positive(operation) if equivalent else evidence_negative(operation)
    transform_tags = proposal["transform_tags"].copy()
    if negative_tag is not None:
        transform_tags += ["hard-negative", negative_tag]
    return {
        "case_id": case_id,
        "proposal_id": proposal["proposal_id"],
        "split": split,
        "semantic_status": semantic_status,
        "expected_exact_detect": equivalent,
        "semantic_scope": SEMANTIC_SCOPE,
        "transform_tags": transform_tags,
        "matrix": {
            "computation": operation,
            "representations": ["loop", right_representation],
            "data_shape": "aligned-list<int>" if OPERATIONS[operation].arity == 2 else "list<int>",
            "language_relation": cross_label,
            "negative_tag": negative_tag,
            "semantic_axes": ["aggregate_reduction"],
            "capabilities": {},
            "template_split": split,
        },
        "left": source_record(left_surface, left, left_path),
        "right": source_record(right_surface, right, right_path),
        "evidence": evidence,
        "llm_proposal": {
            "why": proposal["why"],
            "complexity_budget": proposal["complexity_budget"],
        },
    }


def make_c_contract_negative_item(out_dir: Path, proposal: dict, representation: str) -> dict:
    operation = proposal["operation"]
    if operation not in OPERATIONS:
        raise ValueError(f"{proposal['proposal_id']} references unknown operation {operation}")
    surface = next(s for s in SURFACES if s.key == "c")
    case_id = stable_id(
        proposal["proposal_id"],
        "c",
        representation,
        "not_equivalent",
        "c-contract-hard-negative",
    )
    left = EMITTERS["c"](operation, "loop", False)
    right = EMITTERS["c"](operation, representation, False)
    check_variant_budget(proposal, surface, left)
    check_variant_budget(proposal, surface, right)
    left_path = rel_source_path(case_id, "left", surface)
    right_path = rel_source_path(case_id, "right", surface)
    write_source(out_dir, left_path, left.source)
    write_source(out_dir, right_path, right.source)
    return {
        "case_id": case_id,
        "proposal_id": proposal["proposal_id"],
        "split": "heldout",
        "semantic_status": "not_equivalent",
        "expected_exact_detect": False,
        "semantic_scope": SEMANTIC_SCOPE,
        "transform_tags": proposal["transform_tags"]
        + ["c-contract-hard-negative", representation],
        "matrix": {
            "computation": operation,
            "representations": ["loop", representation],
            "data_shape": "aligned-list<int>" if OPERATIONS[operation].arity == 2 else "list<int>",
            "language_relation": "same-surface",
            "negative_tag": representation,
            "semantic_axes": ["aggregate_reduction", "pointer_length_contract"],
            "capabilities": {},
            "template_split": "heldout",
        },
        "left": source_record(surface, left, left_path),
        "right": source_record(surface, right, right_path),
        "evidence": evidence_contract_negative(operation, representation),
        "llm_proposal": {
            "why": (
                "Adversarial C pointer-length sibling: exact detection must not merge "
                "partial traversal with the full `(xs, n)` contract."
            ),
            "complexity_budget": proposal["complexity_budget"],
        },
    }


def axis_variants(
    surface: Surface,
    proposal_id: str,
    axis: str,
    negative: bool,
) -> tuple[Variant, Variant]:
    if proposal_id.startswith("axis_import_"):
        return (
            import_axis_variant(surface, proposal_id, False, False),
            import_axis_variant(surface, proposal_id, negative, True),
        )
    if axis == "nullish_default":
        return (
            axis_nullish_variant(surface, proposal_id, False, False),
            axis_nullish_variant(surface, proposal_id, negative, True),
        )
    if axis == "own_property_guard":
        return (
            axis_own_property_variant(surface, proposal_id, False, False),
            axis_own_property_variant(surface, proposal_id, negative, True),
        )
    if axis == "record_shape_guard":
        return (
            axis_record_guard_variant(surface, proposal_id, False, False),
            axis_record_guard_variant(surface, proposal_id, negative, True),
        )
    if axis == "collection_empty_check":
        return (
            axis_collection_empty_variant(surface, proposal_id, False, False),
            axis_collection_empty_variant(surface, proposal_id, negative, True),
        )
    if axis == "string_prefix_suffix":
        return (
            axis_string_prefix_variant(surface, proposal_id, False, False),
            axis_string_prefix_variant(surface, proposal_id, negative, True),
        )
    if axis == "literal_collection_membership":
        return (
            axis_membership_literal_variant(surface, proposal_id, False, False),
            axis_membership_literal_variant(surface, proposal_id, negative, True),
        )
    if axis == "map_key_membership":
        return (
            axis_map_key_membership_variant(surface, proposal_id, False, False),
            axis_map_key_membership_variant(surface, proposal_id, negative, True),
        )
    if axis == "literal_map_default_lookup":
        return (
            axis_map_default_variant(surface, proposal_id, False, False),
            axis_map_default_variant(surface, proposal_id, negative, True),
        )
    if axis == "map_default_lookup":
        return (
            axis_map_default_lookup_variant(surface, proposal_id, False, False),
            axis_map_default_lookup_variant(surface, proposal_id, negative, True),
        )
    if axis == "null_presence_predicate":
        return (
            axis_null_presence_variant(surface, proposal_id, False, False),
            axis_null_presence_variant(surface, proposal_id, negative, True),
        )
    if axis == "numeric_minmax_abs":
        if proposal_id.startswith(
            (
                "axis_scalar_min_",
                "axis_scalar_max_",
                "axis_scalar_rust_min_",
                "axis_scalar_rust_max_",
            )
        ):
            return (
                axis_scalar_minmax_variant(surface, proposal_id, False, False),
                axis_scalar_minmax_variant(surface, proposal_id, negative, True),
            )
        return (
            axis_scalar_abs_variant(surface, proposal_id, False, False),
            axis_scalar_abs_variant(surface, proposal_id, negative, True),
        )
    if axis == "numeric_clamp":
        return (
            axis_numeric_clamp_variant(surface, proposal_id, False, False),
            axis_numeric_clamp_variant(surface, proposal_id, negative, True),
        )
    if axis == "hof_filter_map":
        return (
            axis_hof_filter_map_variant(surface, proposal_id, False, False),
            axis_hof_filter_map_variant(surface, proposal_id, negative, True),
        )
    if axis == "total_order_compare":
        return (
            axis_total_order_compare_variant(surface, proposal_id, False, False),
            axis_total_order_compare_variant(surface, proposal_id, negative, True),
        )
    if axis == "java_statically_false_loop":
        return (
            axis_java_dead_loop_variant(surface, proposal_id, False, False),
            axis_java_dead_loop_variant(surface, proposal_id, negative, True),
        )
    if axis == "java_integer_low_bit_toggle":
        return (
            axis_java_low_bit_toggle_variant(surface, proposal_id, False, False),
            axis_java_low_bit_toggle_variant(surface, proposal_id, negative, True),
        )
    if axis == "c_u16_be_byte_pack":
        return (
            axis_c_u16_be_byte_pack_variant(surface, proposal_id, False, False),
            axis_c_u16_be_byte_pack_variant(surface, proposal_id, negative, True),
        )
    if axis == "c_u32_be_byte_pack":
        return (
            axis_c_u32_be_byte_pack_variant(surface, proposal_id, False, False),
            axis_c_u32_be_byte_pack_variant(surface, proposal_id, negative, True),
        )
    if axis == "immutable_binding":
        return (
            axis_immutable_binding_variant(surface, False, False),
            axis_immutable_binding_variant(surface, negative, True),
        )
    if axis == "proven_callee_identity":
        return (
            axis_callee_identity_variant(surface, False, False),
            axis_callee_identity_variant(surface, negative, True),
        )
    if axis == "table_access":
        return (
            axis_table_access_variant(surface, False, False),
            axis_table_access_variant(surface, negative, True),
        )
    if axis == "projection_identity":
        return (
            axis_projection_variant(surface, proposal_id, False, False),
            axis_projection_variant(surface, proposal_id, negative, True),
        )
    if axis == "python_docstring_noop":
        return (
            axis_python_docstring_variant(surface, proposal_id, False, False),
            axis_python_docstring_variant(surface, proposal_id, negative, True),
        )
    if axis == "unsafe_boundary":
        return (
            axis_unsafe_boundary_variant(surface, False),
            axis_unsafe_boundary_variant(surface, True),
        )
    raise ValueError(f"unknown axis: {axis}")


def axis_data_shape(axis: str) -> str:
    return {
        "collection_empty_check": "list<int>",
        "literal_collection_membership": "set<string>",
        "map_key_membership": "map<string,string>+key",
        "literal_map_default_lookup": "map<string,int>+key",
        "map_default_lookup": "map<string,int>+key+fallback",
        "null_presence_predicate": "nullable<T>+alternate",
        "hof_filter_map": "list<int>+optional-emission",
        "nullish_default": "nullable<int>+fallback",
        "numeric_clamp": "scalar<int>+bounds",
        "numeric_minmax_abs": "scalar<int>+alternate",
        "projection_identity": "record<today:int,tomorrow:int>",
        "python_docstring_noop": "python-callable",
        "string_prefix_suffix": "string",
        "table_access": "map<string,int>",
        "total_order_compare": "ordered-scalar-pair",
        "java_statically_false_loop": "java-array-iteration",
        "java_integer_low_bit_toggle": "java-int-edge-key",
        "c_u16_be_byte_pack": "c-byte-buffer",
        "c_u32_be_byte_pack": "c-byte-buffer",
    }.get(axis, "scalar<int>")


def axis_evidence(axis: str, status: str, negative: bool, proposal_id: str | None = None) -> dict:
    if status == "equivalent":
        if axis == "literal_collection_membership":
            return {
                "level": "E1",
                "kind": f"same-spec-{axis}",
                "property_inputs": [
                    {"value": "red", "other": "green"},
                    {"value": "blue", "other": "green"},
                    {"value": "green", "other": "red"},
                ],
                "outputs": [],
            }
        if axis == "map_key_membership":
            return {
                "level": "E1",
                "kind": f"same-spec-{axis}",
                "property_inputs": [
                    {
                        "lookup": {"red": "apple", "blue": "berry"},
                        "other_lookup": {"green": "grape"},
                        "key": "red",
                        "other": "green",
                    },
                    {
                        "lookup": {"red": "apple", "blue": "berry"},
                        "other_lookup": {"green": "grape"},
                        "key": "green",
                        "other": "red",
                    },
                ],
                "outputs": [],
            }
        if axis == "literal_map_default_lookup":
            if proposal_id and proposal_id.startswith("axis_map_default_go_zero_bool_"):
                return {
                    "level": "E1",
                    "kind": f"same-spec-{axis}",
                    "property_inputs": [
                        {
                            "lookup": {"red": True, "blue": False},
                            "other_lookup": {"red": False, "blue": False},
                            "key": "red",
                            "other_key": "green",
                            "fallback": False,
                            "other_default": True,
                        },
                        {
                            "lookup": {"red": True, "blue": False},
                            "other_lookup": {"red": False, "blue": False},
                            "key": "green",
                            "other_key": "red",
                            "fallback": False,
                            "other_default": True,
                        },
                    ],
                    "outputs": [],
                }
            if proposal_id and proposal_id.startswith("axis_map_default_go_zero_float_"):
                return {
                    "level": "E1",
                    "kind": f"same-spec-{axis}",
                    "property_inputs": [
                        {
                            "lookup": {"red": 1.5, "blue": 2.5},
                            "other_lookup": {"red": 9.5, "blue": 2.5},
                            "key": "red",
                            "other_key": "green",
                            "fallback": 0.0,
                            "other_default": 9.0,
                        },
                        {
                            "lookup": {"red": 1.5, "blue": 2.5},
                            "other_lookup": {"red": 9.5, "blue": 2.5},
                            "key": "green",
                            "other_key": "red",
                            "fallback": 0.0,
                            "other_default": 9.0,
                        },
                    ],
                    "outputs": [],
                }
            if proposal_id == "axis_map_default_go_zero_nil_pointer_identity":
                return {
                    "level": "E1",
                    "kind": f"same-spec-{axis}",
                    "property_inputs": [
                        {
                            "lookup": {"red": None, "blue": None},
                            "other_lookup": {"red": "apple", "blue": "berry"},
                            "key": "red",
                            "other_key": "green",
                            "fallback": None,
                            "other_default": "missing",
                        },
                        {
                            "lookup": {"red": None, "blue": None},
                            "other_lookup": {"red": "apple", "blue": "berry"},
                            "key": "green",
                            "other_key": "red",
                            "fallback": None,
                            "other_default": "missing",
                        },
                    ],
                    "outputs": [],
                }
            if proposal_id and proposal_id.startswith("axis_map_default_go_zero_"):
                return {
                    "level": "E1",
                    "kind": f"same-spec-{axis}",
                    "property_inputs": [
                        {
                            "lookup": {"red": "apple", "blue": "berry"},
                            "other_lookup": {"red": "apricot", "blue": "berry"},
                            "key": "red",
                            "other_key": "green",
                            "fallback": "",
                            "other_default": "missing",
                        },
                        {
                            "lookup": {"red": "apple", "blue": "berry"},
                            "other_lookup": {"red": "apricot", "blue": "berry"},
                            "key": "green",
                            "other_key": "red",
                            "fallback": "",
                            "other_default": "missing",
                        },
                    ],
                    "outputs": [],
                }
            return {
                "level": "E1",
                "kind": f"same-spec-{axis}",
                "property_inputs": [
                    {"key": "red", "other": "green"},
                    {"key": "blue", "other": "green"},
                    {"key": "green", "other": "red"},
                ],
                "outputs": [],
            }
        if axis == "map_default_lookup":
            return {
                "level": "E1",
                "kind": f"same-spec-{axis}",
                "property_inputs": [
                    {
                        "lookup": {"red": 1, "blue": 2},
                        "other_lookup": {"red": 9, "blue": 2},
                        "key": "red",
                        "other_key": "green",
                        "fallback": 0,
                        "other_default": 9,
                    },
                    {
                        "lookup": {"red": 1, "blue": 2},
                        "other_lookup": {"red": 9, "blue": 2},
                        "key": "green",
                        "other_key": "red",
                        "fallback": 0,
                        "other_default": 9,
                    },
                ],
                "outputs": [],
            }
        if axis == "null_presence_predicate":
            return {
                "level": "E1",
                "kind": f"same-spec-{axis}",
                "property_inputs": [
                    {"value": None, "other": 1},
                    {"value": 1, "other": None},
                    {"value": 0, "other": None},
                ],
                "outputs": [],
            }
        if axis == "nullish_default":
            return {
                "level": "E1",
                "kind": f"same-spec-{axis}",
                "property_inputs": [
                    {"value": 5, "fallback": 0, "other": 7, "other_default": 9},
                    {"value": None, "fallback": 0, "other": 7, "other_default": 9},
                ],
                "outputs": [],
            }
        if axis == "numeric_minmax_abs":
            property_inputs = (
                [
                    {"left": 2, "right": 5, "other": 1},
                    {"left": -4, "right": 3, "other": 2},
                    {"left": 7, "right": 7, "other": -3},
                ]
                if proposal_id
                and (
                    proposal_id.startswith(("axis_scalar_min_", "axis_scalar_max_"))
                    or proposal_id.startswith(
                        ("axis_scalar_rust_min_", "axis_scalar_rust_max_")
                    )
                )
                else [
                    {"value": -3, "other": 4},
                    {"value": 0, "other": -2},
                    {"value": 5, "other": -7},
                ]
            )
            return {
                "level": "E1",
                "kind": f"same-spec-{axis}",
                "property_inputs": property_inputs,
                "outputs": [],
            }
        if axis == "numeric_clamp":
            return {
                "level": "E1",
                "kind": f"same-spec-{axis}",
                "property_inputs": [
                    {"x": -5, "lo": 0, "hi": 10},
                    {"x": 4, "lo": 0, "hi": 10},
                    {"x": 15, "lo": 0, "hi": 10},
                    {"x": 5, "lo": 10, "hi": 0},
                ],
                "claim": "The exiting invalid-bound guard proves lo <= hi on the return path.",
                "outputs": [],
            }
        if axis == "hof_filter_map":
            return {
                "level": "E1",
                "kind": f"same-spec-{axis}",
                "property_inputs": PROPERTY_INPUTS,
                "claim": "Rust filter_map emits Some(value), drops None, and matches explicit filter+map for the same predicate and emitted value.",
                "outputs": [],
            }
        if axis == "string_prefix_suffix":
            return {
                "level": "E1",
                "kind": f"same-spec-{axis}",
                "property_inputs": ["prelude", "case-suf", "other"],
                "outputs": [],
            }
        if axis == "python_docstring_noop":
            return {
                "level": "E1",
                "kind": f"same-spec-{axis}",
                "property_inputs": [
                    {"i": 1, "j": 1, "values": [1, 2, 3], "value": 2},
                    {"i": 1, "j": 2, "values": [1], "value": -3},
                ],
                "outputs": [],
            }
        if axis == "total_order_compare":
            return {
                "level": "E1",
                "kind": f"same-spec-{axis}",
                "property_inputs": [
                    {"left": -1, "right": 2},
                    {"left": 4, "right": 4},
                    {"left": 7, "right": 3},
                ],
                "claim": "Ascending three-way total-order comparator returns -1, 0, or 1 from the same ordered pair.",
                "outputs": [],
            }
        if axis == "java_statically_false_loop":
            return {
                "level": "E1",
                "kind": f"same-spec-{axis}",
                "property_inputs": [
                    {"numVertices": 0, "strideInBytes": 4},
                    {"numVertices": 1, "strideInBytes": 4},
                ],
                "claim": "`found=true` makes `!found && ...` false on loop entry, so the loop body and update are unreachable.",
                "outputs": [],
            }
        if axis == "java_integer_low_bit_toggle":
            return {
                "level": "E1",
                "kind": f"same-spec-{axis}",
                "property_inputs": [
                    {"edgeKey": -3},
                    {"edgeKey": 0},
                    {"edgeKey": 7},
                ],
                "claim": "For Java primitive integers, even values take `+1` and odd values take `-1`, exactly toggling bit 0.",
                "outputs": [],
            }
        return {
            "level": "E1",
            "kind": f"same-spec-{axis}",
            "property_inputs": [0, 1, 4],
            "outputs": [],
        }
    if status == "unknown":
        return {
            "level": "E0",
            "kind": f"unproven-{axis}-boundary",
            "property_inputs": [],
            "outputs": [],
        }
    if axis == "proven_callee_identity":
        left_output = 3
        right_output = 4
    elif axis == "string_prefix_suffix":
        value = "case-suf" if proposal_id == "axis_string_suffix_identity" else "prelude"
        return {
            "level": "E2",
            "kind": f"counterexample-{axis}",
            "counterexample": {
                "input": {"value": value, "other": "other"},
                "left": True,
                "right": False,
            },
        }
    elif axis == "literal_collection_membership":
        if proposal_id == "axis_membership_module_mutated_boundary":
            counterexample = {
                "input": {"value": "green", "other": "red"},
                "left": False,
                "right": True,
            }
        elif proposal_id in {
            "axis_membership_go_slices_mutated_boundary",
            "axis_membership_rust_local_mutated_boundary",
            "axis_membership_rust_std_mutated_boundary",
        }:
            counterexample = {
                "input": {"value": "green", "other": "red"},
                "left": False,
                "right": True,
            }
        elif proposal_id == "axis_membership_substring_boundary":
            counterexample = {
                "input": {"value": "predator", "other": "green"},
                "left": False,
                "right": True,
            }
        else:
            counterexample = {
                "input": {"value": "red", "other": "green"},
                "left": True,
                "right": False,
            }
        return {
            "level": "E2",
            "kind": f"counterexample-{axis}",
            "counterexample": counterexample,
        }
    elif axis == "python_docstring_noop":
        if proposal_id == "axis_python_docstring_fstring_boundary":
            counterexample = {
                "input": {"value": "red", "observer": "records calls"},
                "left": {"return": 1, "effects": []},
                "right": {"return": 1, "effects": ["observe(red)"]},
            }
        elif proposal_id == "axis_python_docstring_assigned_string_boundary":
            counterexample = {"input": {}, "left": "red", "right": "blue"}
        else:
            counterexample = {"input": {}, "left": "red", "right": "blue"}
        return {
            "level": "E2",
            "kind": f"counterexample-{axis}",
            "counterexample": counterexample,
        }
    elif axis == "map_key_membership":
        counterexample = {
            "input": {
                "lookup": {"red": "apple", "blue": "berry"},
                "other_lookup": {"green": "grape"},
                "key": "red",
                "other": "green",
            },
            "left": True,
            "right": False,
        }
        return {
            "level": "E2",
            "kind": f"counterexample-{axis}",
            "counterexample": counterexample,
        }
    elif axis == "literal_map_default_lookup":
        if proposal_id in {
            "axis_map_default_literal_identity",
            "axis_map_default_js_map_inline_identity",
            "axis_map_default_js_map_local_identity",
            "axis_map_default_js_map_has_get_identity",
            "axis_map_default_js_object_hasown_identity",
            "axis_map_default_js_object_call_identity",
            "axis_map_default_js_object_negated_identity",
            "axis_map_default_wrong_default_boundary",
            "axis_map_default_js_map_wrong_default_boundary",
            "axis_map_default_js_object_wrong_default_boundary",
            "axis_map_default_java_map_of_identity",
            "axis_map_default_java_map_of_entries_identity",
            "axis_map_default_java_map_local_identity",
            "axis_map_default_java_map_wrong_default_boundary",
            "axis_map_default_rust_hashmap_from_identity",
            "axis_map_default_rust_btreemap_from_identity",
            "axis_map_default_rust_hashmap_local_identity",
            "axis_map_default_rust_wrong_default_boundary",
            "axis_map_default_module_js_map_identity",
            "axis_map_default_module_ts_map_identity",
            "axis_map_default_module_java_map_identity",
            "axis_map_default_module_wrong_default_boundary",
            "axis_map_default_ruby_fetch_block_int_identity",
        }:
            counterexample = {
                "input": {"key": "green", "other": "red"},
                "left": 0,
                "right": 9,
            }
        elif proposal_id == "axis_map_default_ruby_fetch_block_string_identity":
            counterexample = {
                "input": {"key": "green", "other": "red"},
                "left": "",
                "right": 9,
            }
        elif proposal_id == "axis_map_default_ruby_fetch_block_bool_identity":
            counterexample = {
                "input": {"key": "green", "other": "red"},
                "left": False,
                "right": 9,
            }
        elif proposal_id in {
            "axis_map_default_go_map_inline_identity",
            "axis_map_default_go_map_local_identity",
            "axis_map_default_go_map_var_identity",
            "axis_map_default_go_map_wrong_key_boundary",
        }:
            counterexample = {
                "input": {"key": "red", "other": "green"},
                "left": 1,
                "right": 0,
            }
        elif proposal_id in {
            "axis_map_default_go_zero_string_inline_identity",
            "axis_map_default_go_zero_string_local_identity",
            "axis_map_default_go_zero_wrong_key_boundary",
        }:
            counterexample = {
                "input": {"key": "red", "other": "green"},
                "left": "apple",
                "right": "",
            }
        elif proposal_id == "axis_map_default_go_zero_bool_inline_identity":
            counterexample = {
                "input": {"key": "red", "other": "green"},
                "left": True,
                "right": False,
            }
        elif proposal_id in {
            "axis_map_default_go_zero_float_inline_identity",
            "axis_map_default_go_zero_float_local_identity",
        }:
            counterexample = {
                "input": {"key": "red", "other": "green"},
                "left": 1.5,
                "right": 0.0,
            }
        elif proposal_id == "axis_map_default_go_zero_nil_pointer_identity":
            counterexample = {
                "input": {"key": "red", "other": "green"},
                "left": None,
                "right": "apple",
            }
        elif proposal_id in {
            "axis_map_default_wrong_map_boundary",
            "axis_map_default_js_map_wrong_map_boundary",
            "axis_map_default_js_object_wrong_map_boundary",
            "axis_map_default_java_map_wrong_map_boundary",
            "axis_map_default_rust_wrong_map_boundary",
            "axis_map_default_go_map_wrong_map_boundary",
            "axis_map_default_rust_mutated_boundary",
            "axis_map_default_module_wrong_map_boundary",
            "axis_map_default_module_mutated_boundary",
            "axis_map_default_module_shadowed_boundary",
        }:
            counterexample = {
                "input": {"key": "red", "other": "green"},
                "left": 1,
                "right": 9,
            }
        elif proposal_id == "axis_map_default_go_zero_wrong_map_boundary":
            counterexample = {
                "input": {"key": "red", "other": "green"},
                "left": True,
                "right": False,
            }
        elif proposal_id == "axis_map_default_go_zero_mixed_value_boundary":
            counterexample = {
                "input": {"key": "blue", "other": "green"},
                "left": "berry",
                "right": False,
            }
        elif proposal_id in {
            "axis_map_default_js_object_unguarded_boundary",
            "axis_map_default_js_object_in_boundary",
        }:
            counterexample = {
                "input": {"key": "toString", "other": "green"},
                "left": 0,
                "right": "prototype property value",
            }
        elif proposal_id == "axis_map_default_js_object_method_boundary":
            counterexample = {
                "input": {
                    "key": "red",
                    "other": "green",
                    "environment": "Object.prototype.hasOwnProperty patched to return false",
                },
                "left": 1,
                "right": 0,
            }
        elif proposal_id == "axis_map_default_js_object_shadowed_boundary":
            counterexample = {
                "input": {
                    "key": "red",
                    "other": "green",
                    "Object": {"hasOwn": "returns false"},
                },
                "left": 1,
                "right": 0,
            }
        else:
            counterexample = {
                "input": {"key": "red", "other": "green"},
                "left": 1,
                "right": 0,
            }
        return {
            "level": "E2",
            "kind": f"counterexample-{axis}",
            "counterexample": counterexample,
        }
    elif axis == "map_default_lookup":
        input_values = {
            "lookup": {"red": 1, "blue": 2},
            "other_lookup": {"red": 9, "blue": 2},
            "key": "red",
            "other_key": "green",
            "fallback": 0,
            "other_default": 9,
        }
        if proposal_id in {
            "axis_map_fallback_wrong_default_boundary",
            "axis_map_fallback_ts_wrong_default_boundary",
            "axis_map_fallback_python_wrong_default_boundary",
        }:
            input_values["key"] = "green"
            input_values["other_key"] = "red"
            counterexample = {
                "input": input_values,
                "left": 0,
                "right": 9,
            }
        elif proposal_id in {
            "axis_map_fallback_wrong_map_boundary",
            "axis_map_fallback_ts_wrong_map_boundary",
            "axis_map_fallback_python_wrong_map_boundary",
        }:
            counterexample = {
                "input": input_values,
                "left": 1,
                "right": 9,
            }
        else:
            counterexample = {
                "input": input_values,
                "left": 1,
                "right": 0,
            }
        return {
            "level": "E2",
            "kind": f"counterexample-{axis}",
            "counterexample": counterexample,
        }
    elif axis == "null_presence_predicate":
        if proposal_id in {
            "axis_null_presence_wrong_value_boundary",
            "axis_null_presence_iflet_wrong_value_boundary",
        }:
            counterexample = {
                "input": {"value": None, "other": 1},
                "left": True,
                "right": False,
            }
        else:
            counterexample = {
                "input": {"value": None, "other": 1},
                "left": True,
                "right": False,
            }
        return {
            "level": "E2",
            "kind": f"counterexample-{axis}",
            "counterexample": counterexample,
        }
    elif axis == "nullish_default":
        input_values = {"value": None, "fallback": 0, "other": 7, "other_default": 9}
        if proposal_id == "axis_option_wrong_value_boundary":
            input_values["value"] = 5
            counterexample = {
                "input": input_values,
                "left": 5,
                "right": 7,
            }
        elif proposal_id == "axis_nullish_truthy_boundary":
            input_values["value"] = 0
            input_values["fallback"] = 9
            counterexample = {
                "input": input_values,
                "left": 0,
                "right": 9,
            }
        else:
            counterexample = {
                "input": input_values,
                "left": 0,
                "right": 9,
            }
        return {
            "level": "E2",
            "kind": f"counterexample-{axis}",
            "counterexample": counterexample,
        }
    elif axis == "numeric_minmax_abs":
        if proposal_id in {
            "axis_scalar_min_wrong_value_boundary",
            "axis_scalar_max_wrong_value_boundary",
            "axis_scalar_rust_min_wrong_value_boundary",
            "axis_scalar_rust_max_wrong_value_boundary",
        }:
            is_min = proposal_id in {
                "axis_scalar_min_wrong_value_boundary",
                "axis_scalar_rust_min_wrong_value_boundary",
            }
            counterexample = {
                "input": {"left": 2, "right": 5, "other": -1},
                "left": (2 if is_min else 5) - 1,
                "right": (-1 if is_min else 2) - 1,
            }
        elif proposal_id in {
            "axis_scalar_min_shadowed_math_boundary",
            "axis_scalar_max_shadowed_math_boundary",
        }:
            is_min = proposal_id == "axis_scalar_min_shadowed_math_boundary"
            counterexample = {
                "input": {"left": 2, "right": 5, "other": 1},
                "left": (2 if is_min else 5) + 1,
                "right": 1,
            }
        elif proposal_id in {
            "axis_scalar_min_function_identity",
            "axis_scalar_max_function_identity",
            "axis_scalar_rust_min_method_identity",
            "axis_scalar_rust_max_method_identity",
        }:
            is_min = proposal_id in {
                "axis_scalar_min_function_identity",
                "axis_scalar_rust_min_method_identity",
            }
            counterexample = {
                "input": {"left": 2, "right": 5, "other": 1},
                "left": (2 if is_min else 5) + 1,
                "right": (5 if is_min else 2) + 1,
            }
        elif proposal_id in {
            "axis_scalar_abs_wrong_value_boundary",
            "axis_scalar_rust_abs_wrong_value_boundary",
        }:
            counterexample = {
                "input": {"value": -3, "other": 4},
                "left": 7,
                "right": 8,
            }
        elif proposal_id in {
            "axis_scalar_rust_abs_custom_method_boundary",
            "axis_scalar_rust_min_custom_method_boundary",
            "axis_scalar_rust_max_custom_method_boundary",
        }:
            counterexample = {
                "input": {"method": "custom receiver method returns 0"},
                "left": "numeric intrinsic result",
                "right": 0,
            }
        elif proposal_id == "axis_scalar_abs_shadowed_math_boundary":
            counterexample = {
                "input": {"value": -3, "other": 4},
                "left": 7,
                "right": 4,
            }
        else:
            counterexample = {
                "input": {"value": -3, "other": 4},
                "left": 7,
                "right": 1,
            }
        return {
            "level": "E2",
            "kind": f"counterexample-{axis}",
            "counterexample": counterexample,
        }
    elif axis == "numeric_clamp":
        if proposal_id == "axis_numeric_clamp_unproven_boundary":
            counterexample = {
                "input": {"x": 5, "lo": 10, "hi": 0},
                "left": 0,
                "right": 10,
            }
        elif proposal_id == "axis_numeric_clamp_float_boundary":
            counterexample = {
                "input": {"x": "NaN", "lo": 0.0, "hi": 10.0},
                "left": "NaN-sensitive min/max result",
                "right": "requires separate float-domain proof",
            }
        else:
            counterexample = {
                "input": {"x": 5, "lo": 0, "hi": 10},
                "left": 5,
                "right": 0,
            }
        return {
            "level": "E2",
            "kind": f"counterexample-{axis}",
            "counterexample": counterexample,
        }
    elif axis == "hof_filter_map":
        if proposal_id == "axis_hof_filter_map_none_boundary":
            counterexample = {
                "input": {"xs": [-1, 2]},
                "left": [4],
                "right": [None, 4],
            }
        elif proposal_id == "axis_hof_filter_map_value_boundary":
            counterexample = {
                "input": {"xs": [2]},
                "left": [4],
                "right": [6],
            }
        else:
            counterexample = {
                "input": {"xs": [2]},
                "left": [0],
                "right": [],
            }
        return {
            "level": "E2",
            "kind": f"counterexample-{axis}",
            "counterexample": counterexample,
        }
    elif axis == "total_order_compare":
        if proposal_id == "axis_total_order_compare_equal_boundary":
            counterexample = {
                "input": {"left": 4, "right": 4},
                "left": 0,
                "right": -1,
            }
        elif proposal_id == "axis_total_order_compare_wrong_value_boundary":
            counterexample = {
                "input": {"left": 7, "right": 3},
                "left": 1,
                "right": 2,
            }
        else:
            counterexample = {
                "input": {"left": -1, "right": 2},
                "left": -1,
                "right": 1,
            }
        return {
            "level": "E2",
            "kind": f"counterexample-{axis}",
            "counterexample": counterexample,
        }
    elif axis == "java_statically_false_loop":
        if proposal_id == "axis_java_dead_loop_false_init_boundary":
            right = "body can execute because found starts false"
        elif proposal_id == "axis_java_dead_loop_positive_guard_boundary":
            right = "body can execute because found starts true and the guard is positive"
        elif proposal_id == "axis_java_dead_loop_guard_identity":
            right = "wrong reachable return value"
        else:
            right = "body can execute after the guard variable is reassigned"
        return {
            "level": "E2",
            "kind": f"counterexample-{axis}",
            "counterexample": {
                "input": {"numVertices": 1, "strideInBytes": 4},
                "left": "first index is returned before comparing elements",
                "right": right,
            },
        }
    elif axis == "java_integer_low_bit_toggle":
        if proposal_id == "axis_java_low_bit_toggle_positive_one_boundary":
            counterexample = {"input": {"edgeKey": -1}, "left": -2, "right": 0}
        elif proposal_id in {
            "axis_java_low_bit_toggle_xor_two_boundary",
            "axis_java_low_bit_toggle_even_identity",
        }:
            counterexample = {"input": {"edgeKey": 0}, "left": 1, "right": 2}
        elif proposal_id == "axis_java_low_bit_toggle_wrong_delta_boundary":
            counterexample = {"input": {"edgeKey": 3}, "left": 2, "right": 1}
        else:
            counterexample = {"input": {"edgeKey": 0}, "left": 1, "right": -1}
        return {
            "level": "E2",
            "kind": f"counterexample-{axis}",
            "counterexample": counterexample,
        }
    else:
        left_output = 8
        right_output = 9
    return {
        "level": "E2",
        "kind": f"counterexample-{axis}",
        "counterexample": {"input": 1, "left": left_output, "right": right_output},
    }


def make_axis_item(
    out_dir: Path,
    capabilities: dict,
    proposal_id: str,
    surface: Surface,
    semantic_status: str,
    split: str,
    negative_tag: str | None = None,
) -> dict:
    proposal = AXIS_PROPOSALS[proposal_id]
    axis = proposal["axis"]
    capability = surface_capability(capabilities, surface, axis)
    negative = semantic_status == "not_equivalent"
    case_id = stable_id(
        proposal_id,
        surface.key,
        semantic_status,
        split,
        negative_tag or "positive",
    )
    left, right = axis_variants(surface, proposal_id, axis, negative)
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
            "data_shape": axis_data_shape(axis),
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
            if proposal_id.startswith("axis_import_") and not import_axis_supported(surface, proposal_id):
                continue
            if proposal_id.startswith("axis_nullish_") and not nullish_axis_supported(
                surface, proposal_id
            ):
                continue
            if proposal_id.startswith("axis_option_") and not nullish_axis_supported(
                surface, proposal_id
            ):
                continue
            if proposal_id.startswith("axis_null_presence_") and not null_presence_axis_supported(
                surface, proposal_id
            ):
                continue
            if proposal_id.startswith("axis_scalar_") and not scalar_abs_axis_supported(surface, proposal_id):
                continue
            if proposal_id.startswith("axis_scalar_rust_"):
                continue
            if proposal_id.startswith(
                "axis_numeric_clamp_"
            ) and not numeric_clamp_axis_supported(surface, proposal_id):
                continue
            if proposal_id.startswith("axis_hof_filter_map_"):
                continue
            if proposal_id.startswith(
                "axis_total_order_compare_"
            ) and not total_order_compare_axis_supported(surface, proposal_id):
                continue
            if proposal_id.startswith("axis_java_dead_loop_") and not java_dead_loop_axis_supported(
                surface, proposal_id
            ):
                continue
            if proposal_id.startswith(
                "axis_java_low_bit_toggle_"
            ) and not java_low_bit_toggle_axis_supported(surface, proposal_id):
                continue
            if proposal_id.startswith(
                "axis_c_u16_be_byte_pack_"
            ) and not c_u16_be_byte_pack_axis_supported(surface, proposal_id):
                continue
            if proposal_id.startswith(
                "axis_c_u32_be_byte_pack_"
            ) and not c_u32_be_byte_pack_axis_supported(surface, proposal_id):
                continue
            if proposal_id.startswith("axis_own_property_") and not own_property_axis_supported(
                surface, proposal_id
            ):
                continue
            if proposal_id in {
                "axis_own_property_in_boundary",
                "axis_own_property_method_boundary",
                "axis_own_property_shadow_boundary",
            }:
                items.append(
                    make_axis_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        surface,
                        "not_equivalent",
                        "heldout",
                        "unproven-own-property-guard",
                    )
                )
                continue
            if proposal_id.startswith("axis_record_guard_") and not record_guard_axis_supported(
                surface, proposal_id
            ):
                continue
            if proposal_id.startswith("axis_collection_") and not collection_empty_axis_supported(
                surface, proposal_id
            ):
                continue
            if proposal_id.startswith("axis_string_") and not string_prefix_axis_supported(
                surface, proposal_id
            ):
                continue
            if proposal_id.startswith("axis_membership_") and not literal_membership_axis_supported(
                surface, proposal_id
            ):
                continue
            if proposal_id.startswith("axis_membership_python_alias_"):
                continue
            if proposal_id.startswith("axis_membership_python_deque_"):
                continue
            if proposal_id.startswith("axis_membership_ruby_set_"):
                continue
            if proposal_id.startswith("axis_membership_set_"):
                continue
            if proposal_id.startswith("axis_membership_array_some_"):
                continue
            if proposal_id.startswith("axis_membership_array_every_"):
                continue
            if proposal_id.startswith("axis_membership_array_indexof_"):
                continue
            if proposal_id.startswith("axis_membership_array_findindex_"):
                continue
            if proposal_id.startswith("axis_membership_array_filter_length_"):
                continue
            if proposal_id.startswith("axis_membership_java_"):
                continue
            if proposal_id.startswith("axis_membership_module_"):
                continue
            if proposal_id.startswith("axis_membership_local_"):
                continue
            if proposal_id.startswith("axis_membership_go_slices_"):
                continue
            if proposal_id.startswith("axis_membership_rust_local_"):
                continue
            if proposal_id.startswith("axis_membership_rust_std_"):
                continue
            if proposal_id.startswith("axis_map_key_") and not map_key_membership_axis_supported(
                surface, proposal_id
            ):
                continue
            if proposal_id.startswith(
                ("axis_map_key_python_keys_", "axis_map_key_ts_array_from_keys_")
            ):
                continue
            if proposal_id.startswith(
                (
                    "axis_map_default_js_map_",
                    "axis_map_default_js_object_",
                    "axis_map_default_java_map_",
                    "axis_map_default_rust_",
                    "axis_map_default_go_map_",
                    "axis_map_default_go_zero_",
                    "axis_map_default_module_",
                )
            ):
                continue
            if proposal_id.startswith("axis_map_default_") and not literal_map_default_axis_supported(
                surface, proposal_id
            ):
                continue
            if proposal_id.startswith("axis_map_fallback_") and not map_default_lookup_axis_supported(
                surface, proposal_id
            ):
                continue
            if proposal_id.startswith(
                (
                    "axis_map_fallback_ts_",
                    "axis_map_fallback_python_",
                    "axis_map_fallback_java_",
                )
            ):
                continue
            if proposal_id in {
                "axis_collection_threshold_boundary",
                "axis_collection_wrong_receiver_boundary",
                "axis_collection_typed_domain_array_boundary",
                "axis_collection_typed_domain_string_boundary",
            }:
                items.append(
                    make_axis_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        surface,
                        "not_equivalent",
                        "heldout",
                        "typed-empty-domain-boundary"
                        if proposal_id.startswith("axis_collection_typed_domain_")
                        else "collection-empty-boundary",
                    )
                )
                continue
            if proposal_id in {
                "axis_java_dead_loop_false_init_boundary",
                "axis_java_dead_loop_positive_guard_boundary",
                "axis_java_dead_loop_reassigned_guard_boundary",
            }:
                items.append(
                    make_axis_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        surface,
                        "not_equivalent",
                        "heldout",
                        "java-dead-loop-boundary",
                    )
                )
                continue
            if proposal_id in {
                "axis_java_low_bit_toggle_reversed_branch_boundary",
                "axis_java_low_bit_toggle_xor_two_boundary",
                "axis_java_low_bit_toggle_positive_one_boundary",
                "axis_java_low_bit_toggle_wrong_delta_boundary",
            }:
                items.append(
                    make_axis_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        surface,
                        "not_equivalent",
                        "heldout",
                        "java-low-bit-toggle-boundary",
                    )
                )
                continue
            if proposal_id in {
                "axis_c_u16_be_byte_pack_wrong_order_boundary",
                "axis_c_u16_be_byte_pack_overlap_boundary",
                "axis_c_u16_be_byte_pack_wrong_byte_boundary",
                "axis_c_u16_be_byte_pack_unproven_alias_boundary",
            }:
                items.append(
                    make_axis_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        surface,
                        "not_equivalent",
                        "heldout",
                        "c-u16-byte-pack-boundary",
                    )
                )
                continue
            if proposal_id in {
                "axis_c_u32_be_byte_pack_uncasted_high_boundary",
                "axis_c_u32_be_byte_pack_wrong_order_boundary",
                "axis_c_u32_be_byte_pack_wrong_byte_boundary",
                "axis_c_u32_be_byte_pack_wrong_alias_boundary",
            }:
                items.append(
                    make_axis_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        surface,
                        "not_equivalent",
                        "heldout",
                        "c-u32-byte-pack-boundary",
                    )
                )
                continue
            if proposal_id in {
                "axis_string_affix_boundary",
                "axis_string_direction_boundary",
                "axis_string_wrong_receiver_boundary",
            }:
                items.append(
                    make_axis_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        surface,
                        "not_equivalent",
                        "heldout",
                        "string-prefix-suffix-boundary",
                    )
                )
                continue
            if proposal_id in {
                "axis_membership_wrong_element_boundary",
                "axis_membership_wrong_collection_boundary",
                "axis_membership_substring_boundary",
                "axis_membership_unproven_receiver_boundary",
                "axis_membership_typed_wrong_element_boundary",
                "axis_membership_typed_string_boundary",
                "axis_membership_python_factory_wrong_element_boundary",
                "axis_membership_python_factory_wrong_collection_boundary",
                "axis_membership_python_factory_shadowed_boundary",
                "axis_membership_local_wrong_element_boundary",
                "axis_membership_local_wrong_collection_boundary",
                "axis_membership_local_mutated_boundary",
                "axis_membership_array_some_wrong_element_boundary",
                "axis_membership_array_some_wrong_collection_boundary",
                "axis_membership_array_every_wrong_element_boundary",
                "axis_membership_array_every_wrong_collection_boundary",
                "axis_membership_array_indexof_wrong_element_boundary",
                "axis_membership_array_indexof_wrong_collection_boundary",
                "axis_membership_array_findindex_wrong_element_boundary",
                "axis_membership_array_findindex_wrong_collection_boundary",
                "axis_membership_array_filter_length_wrong_element_boundary",
                "axis_membership_array_filter_length_wrong_collection_boundary",
                "axis_membership_array_filter_length_absence_wrong_element_boundary",
                "axis_membership_array_filter_length_absence_wrong_collection_boundary",
            }:
                items.append(
                    make_axis_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        surface,
                        "not_equivalent",
                        "heldout",
                        "literal-membership-boundary",
                    )
                )
                continue
            if proposal_id in {
                "axis_map_key_wrong_key_boundary",
                "axis_map_key_wrong_map_boundary",
                "axis_map_key_value_boundary",
            }:
                items.append(
                    make_axis_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        surface,
                        "not_equivalent",
                        "heldout",
                        "map-key-membership-boundary",
                    )
                )
                continue
            if proposal_id in {
                "axis_map_default_wrong_key_boundary",
                "axis_map_default_wrong_default_boundary",
                "axis_map_default_wrong_map_boundary",
            }:
                items.append(
                    make_axis_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        surface,
                        "not_equivalent",
                        "heldout",
                        "literal-map-default-boundary",
                    )
                )
                continue
            if proposal_id in {
                "axis_map_fallback_wrong_key_boundary",
                "axis_map_fallback_wrong_default_boundary",
                "axis_map_fallback_wrong_map_boundary",
            }:
                items.append(
                    make_axis_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        surface,
                        "not_equivalent",
                        "heldout",
                        "map-default-boundary",
                    )
                )
                continue
            if proposal_id in {
                "axis_record_guard_array_boundary",
                "axis_record_guard_null_boundary",
            }:
                items.append(
                    make_axis_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        surface,
                        "not_equivalent",
                        "heldout",
                        "incomplete-record-guard",
                    )
                )
                continue
            if proposal_id == "axis_nullish_truthy_boundary":
                items.append(
                    make_axis_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        surface,
                        "not_equivalent",
                        "heldout",
                        "truthy-default-boundary",
                    )
                )
                continue
            if proposal_id in {
                "axis_option_wrong_default_boundary",
                "axis_option_wrong_value_boundary",
            }:
                items.append(
                    make_axis_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        surface,
                        "not_equivalent",
                        "heldout",
                        "option-default-boundary",
                    )
                )
                continue
            if proposal_id in {
                "axis_null_presence_nonnull_boundary",
                "axis_null_presence_wrong_value_boundary",
                "axis_null_presence_iflet_none_boundary",
                "axis_null_presence_iflet_wrong_value_boundary",
            }:
                items.append(
                    make_axis_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        surface,
                        "not_equivalent",
                        "heldout",
                        "null-presence-boundary",
                    )
                )
                continue
            if proposal_id in {
                "axis_scalar_abs_sign_boundary",
                "axis_scalar_abs_wrong_value_boundary",
                "axis_scalar_abs_shadowed_math_boundary",
                "axis_scalar_min_wrong_value_boundary",
                "axis_scalar_max_wrong_value_boundary",
                "axis_scalar_min_shadowed_math_boundary",
                "axis_scalar_max_shadowed_math_boundary",
            }:
                items.append(
                    make_axis_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        surface,
                        "not_equivalent",
                        "heldout",
                        "numeric-abs-boundary",
                    )
                )
                continue
            if proposal_id in {
                "axis_numeric_clamp_unproven_boundary",
                "axis_numeric_clamp_swapped_bounds_boundary",
                "axis_numeric_clamp_float_boundary",
            }:
                items.append(
                    make_axis_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        surface,
                        "not_equivalent",
                        "heldout",
                        "numeric-clamp-boundary",
                    )
                )
                continue
            if proposal_id in {
                "axis_total_order_compare_descending_boundary",
                "axis_total_order_compare_equal_boundary",
                "axis_total_order_compare_wrong_value_boundary",
            }:
                items.append(
                    make_axis_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        surface,
                        "not_equivalent",
                        "heldout",
                        "total-order-compare-boundary",
                    )
                )
                continue
            if proposal_id.startswith("axis_projection_") and not projection_axis_supported(
                surface, proposal_id
            ):
                continue
            if proposal_id.startswith(
                "axis_python_docstring_"
            ) and not python_docstring_axis_supported(surface, proposal_id):
                continue
            if proposal_id in {
                "axis_projection_default_boundary",
                "axis_projection_dynamic_key_boundary",
            }:
                items.append(
                    make_axis_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        surface,
                        "not_equivalent",
                        "heldout",
                        "unproven-projection-binding",
                    )
                )
                continue
            if proposal_id in {
                "axis_python_docstring_returned_string_boundary",
                "axis_python_docstring_assigned_string_boundary",
                "axis_python_docstring_fstring_boundary",
            }:
                items.append(
                    make_axis_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        surface,
                        "not_equivalent",
                        "heldout",
                        "python-docstring-boundary",
                    )
                )
                continue
            if proposal_id in {"axis_import_unsafe_boundary", "axis_import_reexport_boundary"}:
                items.append(
                    make_axis_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        surface,
                        "unknown",
                        "heldout",
                        "unproven-import-binding",
                    )
                )
                continue
            if proposal_id == "axis_import_namespace_member_wrong_boundary":
                items.append(
                    make_axis_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        surface,
                        "not_equivalent",
                        "heldout",
                        "import-member-boundary",
                    )
                )
                continue
            if proposal_id in {
                "axis_import_namespace_shadowed_param_fake_receiver_boundary",
            }:
                items.append(
                    make_axis_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        surface,
                        "not_equivalent",
                        "heldout",
                        "import-namespace-shadow-boundary",
                    )
                )
                continue
            if axis == "table_access" and capability != "supported":
                continue
            if axis == "unsafe_boundary":
                items.append(
                    make_axis_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        surface,
                        "unknown",
                        "heldout",
                        "unproven-free-binding",
                    )
                )
                continue
            items.append(
                make_axis_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    surface,
                    "equivalent",
                    "dev",
                )
            )
            items.append(
                make_axis_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    surface,
                    "not_equivalent",
                    "heldout",
                    f"{axis}-semantic-mutation",
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
    proposal = AXIS_PROPOSALS[proposal_id]
    axis = proposal["axis"]
    negative = semantic_status == "not_equivalent"
    case_id = stable_id(
        proposal_id,
        left_surface.key,
        right_surface.key,
        semantic_status,
        split,
        negative_tag or "positive",
    )
    left = axis_variants(left_surface, proposal_id, axis, False)[0]
    right = axis_variants(right_surface, proposal_id, axis, negative)[1]
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
            "data_shape": axis_data_shape(axis),
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


def generate_hof_filter_map_cross_items(
    out_dir: Path,
    capabilities: dict,
    generation_filter: GenerationFilter,
) -> list[dict]:
    if not generation_filter.include_axis("hof_filter_map"):
        return []
    surface_by_key = {surface.key: surface for surface in SURFACES}
    rust = surface_by_key["rust"]
    reference_surfaces = [surface_by_key["python"], surface_by_key["javascript"]]
    items: list[dict] = []
    if generation_filter.include_proposal("axis_hof_filter_map_identity"):
        for left_surface in reference_surfaces:
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    "axis_hof_filter_map_identity",
                    left_surface,
                    rust,
                    "equivalent",
                    "heldout",
                )
            )
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    "axis_hof_filter_map_identity",
                    left_surface,
                    rust,
                    "not_equivalent",
                    "heldout",
                    "hof-filter-map-semantic-mutation",
                )
            )
    for proposal_id in (
        "axis_hof_filter_map_none_boundary",
        "axis_hof_filter_map_value_boundary",
        "axis_hof_filter_map_falsey_boundary",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for left_surface in reference_surfaces:
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    rust,
                    "not_equivalent",
                    "heldout",
                    "hof-filter-map-boundary",
                )
            )
    return items


def generate_string_prefix_cross_items(
    out_dir: Path,
    capabilities: dict,
    cross_mode: str,
    generation_filter: GenerationFilter,
) -> list[dict]:
    if not generation_filter.include_axis("string_prefix_suffix"):
        return []
    surfaces = [s for s in SURFACES if string_prefix_axis_supported(s, "axis_string_prefix_identity")]
    items: list[dict] = []
    for left_surface, right_surface in cross_pairs(surfaces, cross_mode):
        for proposal_id in ("axis_string_prefix_identity", "axis_string_suffix_identity"):
            if not generation_filter.include_proposal(proposal_id):
                continue
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    right_surface,
                    "equivalent",
                    "heldout",
                )
            )
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    right_surface,
                    "not_equivalent",
                    "heldout",
                    "string_prefix_suffix-semantic-mutation",
                )
            )
        for proposal_id in (
            "axis_string_affix_boundary",
            "axis_string_direction_boundary",
            "axis_string_wrong_receiver_boundary",
        ):
            if not generation_filter.include_proposal(proposal_id):
                continue
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    right_surface,
                    "not_equivalent",
                    "heldout",
                    "string-prefix-suffix-boundary",
                )
            )
    return items


def generate_literal_membership_cross_items(
    out_dir: Path,
    capabilities: dict,
    cross_mode: str,
    generation_filter: GenerationFilter,
) -> list[dict]:
    if not generation_filter.include_axis("literal_collection_membership"):
        return []
    surfaces = [
        s
        for s in SURFACES
        if literal_membership_axis_supported(s, "axis_membership_literal_identity")
    ]
    items: list[dict] = []
    for left_surface, right_surface in cross_pairs(surfaces, cross_mode):
        if generation_filter.include_proposal("axis_membership_literal_identity"):
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    "axis_membership_literal_identity",
                    left_surface,
                    right_surface,
                    "equivalent",
                    "heldout",
                )
            )
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    "axis_membership_literal_identity",
                    left_surface,
                    right_surface,
                    "not_equivalent",
                    "heldout",
                    "literal_collection_membership-semantic-mutation",
                )
            )
        for proposal_id in (
            "axis_membership_wrong_element_boundary",
            "axis_membership_wrong_collection_boundary",
            "axis_membership_substring_boundary",
        ):
            if not generation_filter.include_proposal(proposal_id):
                continue
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    right_surface,
                    "not_equivalent",
                    "heldout",
                    "literal-membership-boundary",
                )
            )
    if generation_filter.include_proposal("axis_membership_typed_receiver_identity"):
        typed_surfaces = [
            s
            for s in SURFACES
            if literal_membership_axis_supported(s, "axis_membership_typed_receiver_identity")
        ]
        for left_surface, right_surface in cross_pairs(typed_surfaces, cross_mode):
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    "axis_membership_typed_receiver_identity",
                    left_surface,
                    right_surface,
                    "equivalent",
                    "heldout",
                )
            )
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    "axis_membership_typed_receiver_identity",
                    left_surface,
                    right_surface,
                    "not_equivalent",
                    "heldout",
                    "literal_collection_membership-semantic-mutation",
                )
            )
        for proposal_id in (
            "axis_membership_typed_wrong_element_boundary",
            "axis_membership_typed_string_boundary",
        ):
            if not generation_filter.include_proposal(proposal_id):
                continue
            boundary_surfaces = [
                s for s in SURFACES if literal_membership_axis_supported(s, proposal_id)
            ]
            for left_surface, right_surface in cross_pairs(boundary_surfaces, cross_mode):
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        left_surface,
                        right_surface,
                        "not_equivalent",
                        "heldout",
                        "literal-membership-boundary",
                    )
                )

    surface_by_key = {surface.key: surface for surface in SURFACES}
    typefact_reference_surfaces = [
        surface_by_key["python"],
        surface_by_key["typescript"],
        surface_by_key["go"],
        surface_by_key["rust"],
        surface_by_key["java"],
    ]
    if cross_mode == "ring":
        typefact_reference_surfaces = [surface_by_key["typescript"]]
    elif cross_mode == "none":
        typefact_reference_surfaces = []
    typefact_right_surface_by_proposal = {
        "axis_membership_typefact_python_tuple_identity": surface_by_key["python"],
        "axis_membership_python_alias_sequence_identity": surface_by_key["python"],
        "axis_membership_python_alias_container_identity": surface_by_key["python"],
        "axis_membership_python_alias_set_identity": surface_by_key["python"],
        "axis_membership_typefact_java_queue_identity": surface_by_key["java"],
        "axis_membership_typefact_rust_vecdeque_identity": surface_by_key["rust"],
    }
    for proposal_id, right_surface in typefact_right_surface_by_proposal.items():
        if not generation_filter.include_proposal(proposal_id):
            continue
        for left_surface in typefact_reference_surfaces:
            if left_surface.key == right_surface.key:
                continue
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    right_surface,
                    "equivalent",
                    "heldout",
                )
            )
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    right_surface,
                    "not_equivalent",
                    "heldout",
                    "literal_collection_membership-semantic-mutation",
                )
            )
    for proposal_id in (
        "axis_membership_python_alias_wrong_element_boundary",
        "axis_membership_python_alias_wrong_receiver_boundary",
        "axis_membership_python_alias_unresolved_boundary",
        "axis_membership_python_alias_shadowed_boundary",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        right_surface = surface_by_key["python"]
        for left_surface in typefact_reference_surfaces:
            if left_surface.key == right_surface.key:
                continue
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    right_surface,
                    "not_equivalent",
                    "heldout",
                    "literal-membership-boundary",
                )
            )
    python_factory_reference_surfaces = [
        surface_by_key["python"],
        surface_by_key["typescript"],
        surface_by_key["go"],
        surface_by_key["rust"],
        surface_by_key["java"],
    ]
    if cross_mode == "ring":
        python_factory_reference_surfaces = [surface_by_key["typescript"]]
    elif cross_mode == "none":
        python_factory_reference_surfaces = []
    python_factory_right = surface_by_key["python"]
    for proposal_id in (
        "axis_membership_python_set_factory_identity",
        "axis_membership_python_tuple_factory_identity",
        "axis_membership_python_frozenset_factory_identity",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for left_surface in python_factory_reference_surfaces:
            if left_surface.key == python_factory_right.key:
                continue
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    python_factory_right,
                    "equivalent",
                    "heldout",
                )
            )
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    python_factory_right,
                    "not_equivalent",
                    "heldout",
                    "literal_collection_membership-semantic-mutation",
                )
            )
    python_deque_reference_surfaces = python_factory_reference_surfaces
    python_deque_right = surface_by_key["python"]
    for proposal_id in (
        "axis_membership_python_deque_import_identity",
        "axis_membership_python_deque_alias_identity",
        "axis_membership_python_deque_namespace_identity",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for left_surface in python_deque_reference_surfaces:
            if left_surface.key == python_deque_right.key:
                continue
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    python_deque_right,
                    "equivalent",
                    "heldout",
                )
            )
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    python_deque_right,
                    "not_equivalent",
                    "heldout",
                    "literal_collection_membership-semantic-mutation",
                )
            )
    for proposal_id in (
        "axis_membership_python_deque_wrong_element_boundary",
        "axis_membership_python_deque_wrong_collection_boundary",
        "axis_membership_python_deque_missing_import_boundary",
        "axis_membership_python_deque_shadowed_boundary",
        "axis_membership_python_deque_mutated_boundary",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for left_surface in python_deque_reference_surfaces:
            if left_surface.key == python_deque_right.key:
                continue
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    python_deque_right,
                    "not_equivalent",
                    "heldout",
                    "literal-membership-boundary",
                )
            )
    local_constructed_reference_surfaces = [
        surface_by_key["python"],
        surface_by_key["ruby"],
        surface_by_key["javascript"],
        surface_by_key["typescript"],
    ]
    if cross_mode == "ring":
        local_constructed_reference_surfaces = [surface_by_key["python"]]
    elif cross_mode == "none":
        local_constructed_reference_surfaces = []
    local_constructed_right_surface_by_proposal = {
        "axis_membership_local_go_slice_identity": surface_by_key["go"],
        "axis_membership_local_java_list_identity": surface_by_key["java"],
        "axis_membership_local_rust_vec_identity": surface_by_key["rust"],
    }
    for proposal_id, right_surface in local_constructed_right_surface_by_proposal.items():
        if not generation_filter.include_proposal(proposal_id):
            continue
        for left_surface in local_constructed_reference_surfaces:
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    right_surface,
                    "equivalent",
                    "heldout",
                )
            )
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    right_surface,
                    "not_equivalent",
                    "heldout",
                    "literal_collection_membership-semantic-mutation",
                )
            )
    for proposal_id in (
        "axis_membership_local_wrong_element_boundary",
        "axis_membership_local_wrong_collection_boundary",
        "axis_membership_local_mutated_boundary",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for right_surface in (
            surface_by_key["go"],
            surface_by_key["java"],
            surface_by_key["rust"],
        ):
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    surface_by_key["python"],
                    right_surface,
                    "not_equivalent",
                    "heldout",
                    "literal-membership-boundary",
                )
            )
    set_reference_surfaces = [
        surface_by_key["python"],
        surface_by_key["javascript"],
        surface_by_key["typescript"],
        surface_by_key["go"],
        surface_by_key["rust"],
        surface_by_key["ruby"],
    ]
    set_right_surfaces = [surface_by_key["javascript"], surface_by_key["typescript"]]
    if cross_mode == "ring":
        set_reference_surfaces = [surface_by_key["python"]]
    elif cross_mode == "none":
        set_reference_surfaces = []
    for proposal_id in (
        "axis_membership_set_inline_identity",
        "axis_membership_set_local_identity",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for right_surface in set_right_surfaces:
            for left_surface in set_reference_surfaces:
                if left_surface.key == right_surface.key:
                    continue
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        left_surface,
                        right_surface,
                        "equivalent",
                        "heldout",
                    )
                )
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        left_surface,
                        right_surface,
                        "not_equivalent",
                        "heldout",
                        "literal_collection_membership-semantic-mutation",
                    )
                )
    if generation_filter.include_proposal("axis_membership_set_param_identity"):
        typed_reference_surfaces = [
            surface_by_key["python"],
            surface_by_key["go"],
            surface_by_key["rust"],
            surface_by_key["java"],
        ]
        if cross_mode == "ring":
            typed_reference_surfaces = [surface_by_key["python"]]
        elif cross_mode == "none":
            typed_reference_surfaces = []
        for left_surface in typed_reference_surfaces:
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    "axis_membership_set_param_identity",
                    left_surface,
                    surface_by_key["typescript"],
                    "equivalent",
                    "heldout",
                )
            )
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    "axis_membership_set_param_identity",
                    left_surface,
                    surface_by_key["typescript"],
                    "not_equivalent",
                    "heldout",
                    "literal_collection_membership-semantic-mutation",
                )
            )
    for proposal_id in (
        "axis_membership_set_wrong_element_boundary",
        "axis_membership_set_wrong_collection_boundary",
        "axis_membership_set_untyped_receiver_boundary",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for right_surface in set_right_surfaces:
            for left_surface in set_reference_surfaces:
                if left_surface.key == right_surface.key:
                    continue
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        left_surface,
                        right_surface,
                        "not_equivalent",
                        "heldout",
                        "literal-membership-boundary",
                    )
                )
    array_some_reference_surfaces = [
        surface_by_key["python"],
        surface_by_key["ruby"],
        surface_by_key["javascript"],
        surface_by_key["typescript"],
    ]
    array_some_right_surfaces = [
        surface_by_key["javascript"],
        surface_by_key["typescript"],
        surface_by_key["vue"],
        surface_by_key["svelte"],
        surface_by_key["html"],
    ]
    if cross_mode == "ring":
        array_some_reference_surfaces = [surface_by_key["python"]]
    elif cross_mode == "none":
        array_some_reference_surfaces = []
    if generation_filter.include_proposal("axis_membership_array_some_identity"):
        for right_surface in array_some_right_surfaces:
            for left_surface in array_some_reference_surfaces:
                if left_surface.key == right_surface.key:
                    continue
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        "axis_membership_array_some_identity",
                        left_surface,
                        right_surface,
                        "equivalent",
                        "heldout",
                    )
                )
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        "axis_membership_array_some_identity",
                        left_surface,
                        right_surface,
                        "not_equivalent",
                        "heldout",
                        "literal_collection_membership-semantic-mutation",
                    )
                )
    for proposal_id in (
        "axis_membership_array_some_wrong_element_boundary",
        "axis_membership_array_some_wrong_collection_boundary",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for right_surface in array_some_right_surfaces:
            for left_surface in array_some_reference_surfaces:
                if left_surface.key == right_surface.key:
                    continue
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        left_surface,
                        right_surface,
                        "not_equivalent",
                        "heldout",
                        "literal-membership-boundary",
                    )
                )
    array_every_reference_surfaces = [
        surface_by_key["python"],
        surface_by_key["ruby"],
        surface_by_key["javascript"],
        surface_by_key["typescript"],
    ]
    array_every_right_surfaces = [
        surface_by_key["javascript"],
        surface_by_key["typescript"],
        surface_by_key["vue"],
        surface_by_key["svelte"],
        surface_by_key["html"],
    ]
    if cross_mode == "ring":
        array_every_reference_surfaces = [surface_by_key["python"]]
    elif cross_mode == "none":
        array_every_reference_surfaces = []
    if generation_filter.include_proposal("axis_membership_array_every_absence_identity"):
        for right_surface in array_every_right_surfaces:
            for left_surface in array_every_reference_surfaces:
                if left_surface.key == right_surface.key:
                    continue
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        "axis_membership_array_every_absence_identity",
                        left_surface,
                        right_surface,
                        "equivalent",
                        "heldout",
                    )
                )
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        "axis_membership_array_every_absence_identity",
                        left_surface,
                        right_surface,
                        "not_equivalent",
                        "heldout",
                        "literal_collection_membership-semantic-mutation",
                    )
                )
    for proposal_id in (
        "axis_membership_array_every_wrong_element_boundary",
        "axis_membership_array_every_wrong_collection_boundary",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for right_surface in array_every_right_surfaces:
            for left_surface in array_every_reference_surfaces:
                if left_surface.key == right_surface.key:
                    continue
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        left_surface,
                        right_surface,
                        "not_equivalent",
                        "heldout",
                        "literal-membership-boundary",
                    )
                )
    array_indexof_reference_surfaces = [
        surface_by_key["python"],
        surface_by_key["ruby"],
        surface_by_key["javascript"],
        surface_by_key["typescript"],
    ]
    array_indexof_right_surfaces = [
        surface_by_key["javascript"],
        surface_by_key["typescript"],
        surface_by_key["vue"],
        surface_by_key["svelte"],
        surface_by_key["html"],
    ]
    if cross_mode == "ring":
        array_indexof_reference_surfaces = [surface_by_key["python"]]
    elif cross_mode == "none":
        array_indexof_reference_surfaces = []
    if generation_filter.include_proposal("axis_membership_array_indexof_identity"):
        for right_surface in array_indexof_right_surfaces:
            for left_surface in array_indexof_reference_surfaces:
                if left_surface.key == right_surface.key:
                    continue
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        "axis_membership_array_indexof_identity",
                        left_surface,
                        right_surface,
                        "equivalent",
                        "heldout",
                    )
                )
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        "axis_membership_array_indexof_identity",
                        left_surface,
                        right_surface,
                        "not_equivalent",
                        "heldout",
                        "literal_collection_membership-semantic-mutation",
                    )
                )
    for proposal_id in (
        "axis_membership_array_indexof_wrong_element_boundary",
        "axis_membership_array_indexof_wrong_collection_boundary",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for right_surface in array_indexof_right_surfaces:
            for left_surface in array_indexof_reference_surfaces:
                if left_surface.key == right_surface.key:
                    continue
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        left_surface,
                        right_surface,
                        "not_equivalent",
                        "heldout",
                        "literal-membership-boundary",
                    )
                )
    array_findindex_reference_surfaces = [
        surface_by_key["python"],
        surface_by_key["ruby"],
        surface_by_key["javascript"],
        surface_by_key["typescript"],
    ]
    array_findindex_right_surfaces = [
        surface_by_key["javascript"],
        surface_by_key["typescript"],
        surface_by_key["vue"],
        surface_by_key["svelte"],
        surface_by_key["html"],
    ]
    if cross_mode == "ring":
        array_findindex_reference_surfaces = [surface_by_key["python"]]
    elif cross_mode == "none":
        array_findindex_reference_surfaces = []
    if generation_filter.include_proposal("axis_membership_array_findindex_identity"):
        for right_surface in array_findindex_right_surfaces:
            for left_surface in array_findindex_reference_surfaces:
                if left_surface.key == right_surface.key:
                    continue
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        "axis_membership_array_findindex_identity",
                        left_surface,
                        right_surface,
                        "equivalent",
                        "heldout",
                    )
                )
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        "axis_membership_array_findindex_identity",
                        left_surface,
                        right_surface,
                        "not_equivalent",
                        "heldout",
                        "literal_collection_membership-semantic-mutation",
                    )
                )
    for proposal_id in (
        "axis_membership_array_findindex_wrong_element_boundary",
        "axis_membership_array_findindex_wrong_collection_boundary",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for right_surface in array_findindex_right_surfaces:
            for left_surface in array_findindex_reference_surfaces:
                if left_surface.key == right_surface.key:
                    continue
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        left_surface,
                        right_surface,
                        "not_equivalent",
                        "heldout",
                        "literal-membership-boundary",
                    )
                )
    array_filter_length_reference_surfaces = [
        surface_by_key["python"],
        surface_by_key["ruby"],
        surface_by_key["javascript"],
        surface_by_key["typescript"],
    ]
    array_filter_length_right_surfaces = [
        surface_by_key["javascript"],
        surface_by_key["typescript"],
        surface_by_key["vue"],
        surface_by_key["svelte"],
        surface_by_key["html"],
    ]
    if cross_mode == "ring":
        array_filter_length_reference_surfaces = [surface_by_key["python"]]
    elif cross_mode == "none":
        array_filter_length_reference_surfaces = []
    if generation_filter.include_proposal("axis_membership_array_filter_length_identity"):
        for right_surface in array_filter_length_right_surfaces:
            for left_surface in array_filter_length_reference_surfaces:
                if left_surface.key == right_surface.key:
                    continue
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        "axis_membership_array_filter_length_identity",
                        left_surface,
                        right_surface,
                        "equivalent",
                        "heldout",
                    )
                )
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        "axis_membership_array_filter_length_identity",
                        left_surface,
                        right_surface,
                        "not_equivalent",
                        "heldout",
                        "literal_collection_membership-semantic-mutation",
                    )
                )
    for proposal_id in (
        "axis_membership_array_filter_length_wrong_element_boundary",
        "axis_membership_array_filter_length_wrong_collection_boundary",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for right_surface in array_filter_length_right_surfaces:
            for left_surface in array_filter_length_reference_surfaces:
                if left_surface.key == right_surface.key:
                    continue
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        left_surface,
                        right_surface,
                        "not_equivalent",
                        "heldout",
                        "literal-membership-boundary",
                    )
                )
    array_filter_length_absence_reference_surfaces = [
        surface_by_key["python"],
        surface_by_key["ruby"],
        surface_by_key["javascript"],
        surface_by_key["typescript"],
    ]
    array_filter_length_absence_right_surfaces = [
        surface_by_key["javascript"],
        surface_by_key["typescript"],
        surface_by_key["vue"],
        surface_by_key["svelte"],
        surface_by_key["html"],
    ]
    if cross_mode == "ring":
        array_filter_length_absence_reference_surfaces = [surface_by_key["python"]]
    elif cross_mode == "none":
        array_filter_length_absence_reference_surfaces = []
    if generation_filter.include_proposal("axis_membership_array_filter_length_absence_identity"):
        for right_surface in array_filter_length_absence_right_surfaces:
            for left_surface in array_filter_length_absence_reference_surfaces:
                if left_surface.key == right_surface.key:
                    continue
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        "axis_membership_array_filter_length_absence_identity",
                        left_surface,
                        right_surface,
                        "equivalent",
                        "heldout",
                    )
                )
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        "axis_membership_array_filter_length_absence_identity",
                        left_surface,
                        right_surface,
                        "not_equivalent",
                        "heldout",
                        "literal_collection_membership-semantic-mutation",
                    )
                )
    for proposal_id in (
        "axis_membership_array_filter_length_absence_wrong_element_boundary",
        "axis_membership_array_filter_length_absence_wrong_collection_boundary",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for right_surface in array_filter_length_absence_right_surfaces:
            for left_surface in array_filter_length_absence_reference_surfaces:
                if left_surface.key == right_surface.key:
                    continue
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        left_surface,
                        right_surface,
                        "not_equivalent",
                        "heldout",
                        "literal-membership-boundary",
                    )
                )
    module_reference_surfaces = [surface_by_key["python"], surface_by_key["ruby"]]
    if cross_mode == "ring":
        module_reference_surfaces = [surface_by_key["python"]]
    elif cross_mode == "none":
        module_reference_surfaces = []
    module_right_surfaces_by_proposal = {
        "axis_membership_module_js_set_identity": [surface_by_key["javascript"]],
        "axis_membership_module_ts_set_identity": [surface_by_key["typescript"]],
        "axis_membership_module_java_list_identity": [surface_by_key["java"]],
        "axis_membership_module_python_tuple_identity": [surface_by_key["python"]],
        "axis_membership_module_python_set_identity": [surface_by_key["python"]],
    }
    for proposal_id, module_right_surfaces in module_right_surfaces_by_proposal.items():
        if not generation_filter.include_proposal(proposal_id):
            continue
        for right_surface in module_right_surfaces:
            for left_surface in module_reference_surfaces:
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        left_surface,
                        right_surface,
                        "equivalent",
                        "heldout",
                    )
                )
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        left_surface,
                        right_surface,
                        "not_equivalent",
                        "heldout",
                        "literal_collection_membership-semantic-mutation",
                    )
                )
    module_right_surfaces = [
        surface_by_key["javascript"],
        surface_by_key["typescript"],
        surface_by_key["java"],
    ]
    for proposal_id in (
        "axis_membership_module_wrong_element_boundary",
        "axis_membership_module_wrong_collection_boundary",
        "axis_membership_module_shadowed_boundary",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for right_surface in module_right_surfaces:
            for left_surface in module_reference_surfaces:
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        left_surface,
                        right_surface,
                        "not_equivalent",
                        "heldout",
                        "literal-membership-boundary",
                    )
                )
    if generation_filter.include_proposal("axis_membership_module_mutated_boundary"):
        for right_surface in (surface_by_key["javascript"], surface_by_key["typescript"]):
            for left_surface in module_reference_surfaces:
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        "axis_membership_module_mutated_boundary",
                        left_surface,
                        right_surface,
                        "not_equivalent",
                        "heldout",
                        "literal-membership-boundary",
                    )
                )
    if generation_filter.include_proposal("axis_membership_module_python_mutated_boundary"):
        for left_surface in module_reference_surfaces:
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    "axis_membership_module_python_mutated_boundary",
                    left_surface,
                    surface_by_key["python"],
                    "not_equivalent",
                    "heldout",
                    "literal-membership-boundary",
                )
            )
    go_slices_right = surface_by_key["go"]
    for proposal_id in (
        "axis_membership_go_slices_package_identity",
        "axis_membership_go_slices_alias_package_identity",
        "axis_membership_go_slices_const_package_identity",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for left_surface in module_reference_surfaces:
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    go_slices_right,
                    "equivalent",
                    "heldout",
                )
            )
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    go_slices_right,
                    "not_equivalent",
                    "heldout",
                    "literal_collection_membership-semantic-mutation",
                )
            )
    for proposal_id in (
        "axis_membership_go_slices_wrong_element_boundary",
        "axis_membership_go_slices_wrong_collection_boundary",
        "axis_membership_go_slices_mutated_boundary",
        "axis_membership_go_slices_unimported_boundary",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for left_surface in module_reference_surfaces:
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    go_slices_right,
                    "not_equivalent",
                    "heldout",
                    "literal-membership-boundary",
                )
            )
    rust_local_right = surface_by_key["rust"]
    for proposal_id in (
        "axis_membership_rust_local_array_identity",
        "axis_membership_rust_local_typed_array_identity",
        "axis_membership_rust_local_slice_ref_identity",
        "axis_membership_rust_std_hashset_identity",
        "axis_membership_rust_std_btreeset_identity",
        "axis_membership_rust_std_vecdeque_identity",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for left_surface in module_reference_surfaces:
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    rust_local_right,
                    "equivalent",
                    "heldout",
                )
            )
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    rust_local_right,
                    "not_equivalent",
                    "heldout",
                    "literal_collection_membership-semantic-mutation",
                )
            )
    for proposal_id in (
        "axis_membership_rust_local_wrong_element_boundary",
        "axis_membership_rust_local_wrong_collection_boundary",
        "axis_membership_rust_local_mutated_boundary",
        "axis_membership_rust_local_custom_receiver_boundary",
        "axis_membership_rust_std_wrong_element_boundary",
        "axis_membership_rust_std_wrong_collection_boundary",
        "axis_membership_rust_std_mutated_boundary",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for left_surface in module_reference_surfaces:
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    rust_local_right,
                    "not_equivalent",
                    "heldout",
                    "literal-membership-boundary",
                )
            )
    ruby_set_right = surface_by_key["ruby"]
    for proposal_id in (
        "axis_membership_ruby_set_new_include_identity",
        "axis_membership_ruby_set_new_member_identity",
        "axis_membership_ruby_set_local_identity",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for left_surface in module_reference_surfaces:
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    ruby_set_right,
                    "equivalent",
                    "heldout",
                )
            )
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    ruby_set_right,
                    "not_equivalent",
                    "heldout",
                    "literal_collection_membership-semantic-mutation",
                )
            )
    for proposal_id in (
        "axis_membership_ruby_set_wrong_element_boundary",
        "axis_membership_ruby_set_wrong_collection_boundary",
        "axis_membership_ruby_set_missing_require_boundary",
        "axis_membership_ruby_set_shadowed_boundary",
        "axis_membership_ruby_set_mutated_boundary",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for left_surface in module_reference_surfaces:
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    ruby_set_right,
                    "not_equivalent",
                    "heldout",
                    "literal-membership-boundary",
                )
            )
    return items


def generate_java_factory_membership_cross_items(
    out_dir: Path,
    capabilities: dict,
    cross_mode: str,
    generation_filter: GenerationFilter,
) -> list[dict]:
    if cross_mode == "none" or not generation_filter.include_axis("literal_collection_membership"):
        return []
    surface_by_key = {surface.key: surface for surface in SURFACES}
    java_surface = surface_by_key["java"]
    reference_surfaces = [
        s
        for s in SURFACES
        if s.key != "java" and literal_membership_axis_supported(s, "axis_membership_literal_identity")
    ]
    if cross_mode == "ring":
        reference_surfaces = reference_surfaces[:1]
    items: list[dict] = []
    for proposal_id in (
        "axis_membership_java_list_of_identity",
        "axis_membership_java_set_of_identity",
        "axis_membership_java_arrays_aslist_identity",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for left_surface in reference_surfaces:
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    java_surface,
                    "equivalent",
                    "heldout",
                )
            )
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    java_surface,
                    "not_equivalent",
                    "heldout",
                    "literal_collection_membership-semantic-mutation",
                )
            )
    for proposal_id in (
        "axis_membership_java_list_of_wrong_element_boundary",
        "axis_membership_java_set_of_wrong_element_boundary",
        "axis_membership_java_arrays_aslist_wrong_element_boundary",
        "axis_membership_java_list_of_wrong_collection_boundary",
        "axis_membership_java_set_of_wrong_collection_boundary",
        "axis_membership_java_arrays_aslist_wrong_collection_boundary",
        "axis_membership_java_list_of_shadowed_boundary",
        "axis_membership_java_set_of_shadowed_boundary",
        "axis_membership_java_arrays_aslist_shadowed_boundary",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for left_surface in reference_surfaces:
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    java_surface,
                    "not_equivalent",
                    "heldout",
                    "literal-membership-boundary",
                )
            )
    return items


def generate_map_key_membership_cross_items(
    out_dir: Path,
    capabilities: dict,
    cross_mode: str,
    generation_filter: GenerationFilter,
) -> list[dict]:
    if not generation_filter.include_axis("map_key_membership"):
        return []
    surfaces = [
        s
        for s in SURFACES
        if map_key_membership_axis_supported(s, "axis_map_key_membership_identity")
    ]
    surface_by_key = {s.key: s for s in SURFACES}
    items: list[dict] = []
    for left_surface, right_surface in cross_pairs(surfaces, cross_mode):
        if generation_filter.include_proposal("axis_map_key_membership_identity"):
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    "axis_map_key_membership_identity",
                    left_surface,
                    right_surface,
                    "equivalent",
                    "heldout",
                )
            )
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    "axis_map_key_membership_identity",
                    left_surface,
                    right_surface,
                    "not_equivalent",
                    "heldout",
                    "map_key_membership-semantic-mutation",
                )
            )
        for proposal_id in (
            "axis_map_key_wrong_key_boundary",
            "axis_map_key_wrong_map_boundary",
            "axis_map_key_value_boundary",
        ):
            if not generation_filter.include_proposal(proposal_id):
                continue
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    right_surface,
                    "not_equivalent",
                    "heldout",
                    "map-key-membership-boundary",
                )
            )
    special_views = [
        (
            surface_by_key["python"],
            (
                "axis_map_key_python_keys_in_identity",
                "axis_map_key_python_keys_contains_identity",
            ),
            (
                "axis_map_key_python_keys_wrong_key_boundary",
                "axis_map_key_python_keys_wrong_map_boundary",
                "axis_map_key_python_keys_value_boundary",
            ),
        ),
        (
            surface_by_key["typescript"],
            ("axis_map_key_ts_array_from_keys_identity",),
            (
                "axis_map_key_ts_array_from_keys_wrong_key_boundary",
                "axis_map_key_ts_array_from_keys_wrong_map_boundary",
                "axis_map_key_ts_array_from_keys_value_boundary",
            ),
        ),
    ]
    for right_surface, positive_proposals, boundary_proposals in special_views:
        reference_surfaces = [s for s in surfaces if s.key != right_surface.key]
        for proposal_id in positive_proposals:
            if not generation_filter.include_proposal(proposal_id):
                continue
            for left_surface in reference_surfaces:
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        left_surface,
                        right_surface,
                        "equivalent",
                        "heldout",
                    )
                )
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        left_surface,
                        right_surface,
                        "not_equivalent",
                        "heldout",
                        "map_key_membership-semantic-mutation",
                    )
                )
        for proposal_id in boundary_proposals:
            if not generation_filter.include_proposal(proposal_id):
                continue
            for left_surface in reference_surfaces:
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        left_surface,
                        right_surface,
                        "not_equivalent",
                        "heldout",
                        "map-key-membership-boundary",
                    )
                )
    return items


def generate_literal_map_default_cross_items(
    out_dir: Path,
    capabilities: dict,
    cross_mode: str,
    generation_filter: GenerationFilter,
) -> list[dict]:
    if not generation_filter.include_axis("literal_map_default_lookup"):
        return []
    surfaces = [
        s
        for s in SURFACES
        if literal_map_default_axis_supported(s, "axis_map_default_literal_identity")
    ]
    items: list[dict] = []
    for left_surface, right_surface in cross_pairs(surfaces, cross_mode):
        if generation_filter.include_proposal("axis_map_default_literal_identity"):
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    "axis_map_default_literal_identity",
                    left_surface,
                    right_surface,
                    "equivalent",
                    "heldout",
                )
            )
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    "axis_map_default_literal_identity",
                    left_surface,
                    right_surface,
                    "not_equivalent",
                    "heldout",
                    "literal_map_default_lookup-semantic-mutation",
                )
            )
        for proposal_id in (
            "axis_map_default_wrong_key_boundary",
            "axis_map_default_wrong_default_boundary",
            "axis_map_default_wrong_map_boundary",
        ):
            if not generation_filter.include_proposal(proposal_id):
                continue
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    right_surface,
                    "not_equivalent",
                    "heldout",
                    "literal-map-default-boundary",
                )
            )

    surface_by_key = {surface.key: surface for surface in SURFACES}
    reference_surfaces = [surface_by_key["python"], surface_by_key["ruby"]]
    right_surfaces = [surface_by_key["javascript"], surface_by_key["typescript"]]
    if cross_mode == "ring":
        reference_surfaces = [surface_by_key["python"]]
    elif cross_mode == "none":
        reference_surfaces = []
    ruby_block_reference_surfaces = [surface_by_key["ruby"]]
    ruby_block_right_surfaces = [surface_by_key["ruby"]]
    for proposal_id in (
        "axis_map_default_ruby_fetch_block_int_identity",
        "axis_map_default_ruby_fetch_block_string_identity",
        "axis_map_default_ruby_fetch_block_bool_identity",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for right_surface in ruby_block_right_surfaces:
            for left_surface in ruby_block_reference_surfaces:
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        left_surface,
                        right_surface,
                        "equivalent",
                        "heldout",
                    )
                )
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        left_surface,
                        right_surface,
                        "not_equivalent",
                        "heldout",
                        "literal_map_default_lookup-semantic-mutation",
                    )
                )
    for proposal_id in (
        "axis_map_default_js_map_inline_identity",
        "axis_map_default_js_map_local_identity",
        "axis_map_default_js_map_has_get_identity",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for right_surface in right_surfaces:
            for left_surface in reference_surfaces:
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        left_surface,
                        right_surface,
                        "equivalent",
                        "heldout",
                    )
                )
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        left_surface,
                        right_surface,
                        "not_equivalent",
                        "heldout",
                        "literal_map_default_lookup-semantic-mutation",
                    )
                )
    for proposal_id in (
        "axis_map_default_js_map_wrong_key_boundary",
        "axis_map_default_js_map_wrong_default_boundary",
        "axis_map_default_js_map_wrong_map_boundary",
        "axis_map_default_js_map_untyped_receiver_boundary",
        "axis_map_default_js_map_shadowed_constructor_boundary",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for right_surface in right_surfaces:
            for left_surface in reference_surfaces:
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        left_surface,
                        right_surface,
                        "not_equivalent",
                        "heldout",
                        "literal-map-default-boundary",
                    )
                )
    java_right_surfaces = [surface_by_key["java"]]
    for proposal_id in (
        "axis_map_default_java_map_of_identity",
        "axis_map_default_java_map_of_entries_identity",
        "axis_map_default_java_map_local_identity",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for right_surface in java_right_surfaces:
            for left_surface in reference_surfaces:
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        left_surface,
                        right_surface,
                        "equivalent",
                        "heldout",
                    )
                )
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        left_surface,
                        right_surface,
                        "not_equivalent",
                        "heldout",
                        "literal_map_default_lookup-semantic-mutation",
                    )
                )
    for proposal_id in (
        "axis_map_default_java_map_wrong_key_boundary",
        "axis_map_default_java_map_wrong_default_boundary",
        "axis_map_default_java_map_wrong_map_boundary",
        "axis_map_default_java_map_shadowed_factory_boundary",
        "axis_map_default_java_map_type_shadow_boundary",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for right_surface in java_right_surfaces:
            for left_surface in reference_surfaces:
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        left_surface,
                        right_surface,
                        "not_equivalent",
                        "heldout",
                        "literal-map-default-boundary",
                    )
                )
    rust_right_surfaces = [surface_by_key["rust"]]
    for proposal_id in (
        "axis_map_default_rust_hashmap_from_identity",
        "axis_map_default_rust_btreemap_from_identity",
        "axis_map_default_rust_hashmap_local_identity",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for right_surface in rust_right_surfaces:
            for left_surface in reference_surfaces:
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        left_surface,
                        right_surface,
                        "equivalent",
                        "heldout",
                    )
                )
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        left_surface,
                        right_surface,
                        "not_equivalent",
                        "heldout",
                        "literal_map_default_lookup-semantic-mutation",
                    )
                )
    for proposal_id in (
        "axis_map_default_rust_wrong_key_boundary",
        "axis_map_default_rust_wrong_default_boundary",
        "axis_map_default_rust_wrong_map_boundary",
        "axis_map_default_rust_mutated_boundary",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for right_surface in rust_right_surfaces:
            for left_surface in reference_surfaces:
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        left_surface,
                        right_surface,
                        "not_equivalent",
                        "heldout",
                        "literal-map-default-boundary",
                    )
                )
    go_right_surfaces = [surface_by_key["go"]]
    for proposal_id in (
        "axis_map_default_go_map_inline_identity",
        "axis_map_default_go_map_local_identity",
        "axis_map_default_go_map_var_identity",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for right_surface in go_right_surfaces:
            for left_surface in reference_surfaces:
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        left_surface,
                        right_surface,
                        "equivalent",
                        "heldout",
                    )
                )
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        left_surface,
                        right_surface,
                        "not_equivalent",
                        "heldout",
                        "literal_map_default_lookup-semantic-mutation",
                    )
                )
    for proposal_id in (
        "axis_map_default_go_map_wrong_key_boundary",
        "axis_map_default_go_map_wrong_map_boundary",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for right_surface in go_right_surfaces:
            for left_surface in reference_surfaces:
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        left_surface,
                        right_surface,
                        "not_equivalent",
                        "heldout",
                        "literal-map-default-boundary",
                    )
                )
    for proposal_id in (
        "axis_map_default_go_zero_string_inline_identity",
        "axis_map_default_go_zero_string_local_identity",
        "axis_map_default_go_zero_bool_inline_identity",
        "axis_map_default_go_zero_float_inline_identity",
        "axis_map_default_go_zero_float_local_identity",
        "axis_map_default_go_zero_nil_pointer_identity",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for right_surface in go_right_surfaces:
            for left_surface in reference_surfaces:
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        left_surface,
                        right_surface,
                        "equivalent",
                        "heldout",
                    )
                )
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        left_surface,
                        right_surface,
                        "not_equivalent",
                        "heldout",
                        "literal_map_default_lookup-semantic-mutation",
                    )
                )
    for proposal_id in (
        "axis_map_default_go_zero_wrong_key_boundary",
        "axis_map_default_go_zero_wrong_map_boundary",
        "axis_map_default_go_zero_mixed_value_boundary",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for right_surface in go_right_surfaces:
            for left_surface in reference_surfaces:
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        left_surface,
                        right_surface,
                        "not_equivalent",
                        "heldout",
                        "literal-map-default-boundary",
                    )
                )
    module_right_surfaces_by_proposal = {
        "axis_map_default_module_js_map_identity": [surface_by_key["javascript"]],
        "axis_map_default_module_ts_map_identity": [surface_by_key["typescript"]],
        "axis_map_default_module_java_map_identity": [surface_by_key["java"]],
    }
    for proposal_id, module_right_surfaces in module_right_surfaces_by_proposal.items():
        if not generation_filter.include_proposal(proposal_id):
            continue
        for right_surface in module_right_surfaces:
            for left_surface in reference_surfaces:
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        left_surface,
                        right_surface,
                        "equivalent",
                        "heldout",
                    )
                )
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        left_surface,
                        right_surface,
                        "not_equivalent",
                        "heldout",
                        "literal_map_default_lookup-semantic-mutation",
                    )
                )
    module_right_surfaces = [
        surface_by_key["javascript"],
        surface_by_key["typescript"],
        surface_by_key["java"],
    ]
    for proposal_id in (
        "axis_map_default_module_wrong_key_boundary",
        "axis_map_default_module_wrong_default_boundary",
        "axis_map_default_module_wrong_map_boundary",
        "axis_map_default_module_shadowed_boundary",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for right_surface in module_right_surfaces:
            for left_surface in reference_surfaces:
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        left_surface,
                        right_surface,
                        "not_equivalent",
                        "heldout",
                        "literal-map-default-boundary",
                    )
                )
    if generation_filter.include_proposal("axis_map_default_module_mutated_boundary"):
        for right_surface in (surface_by_key["javascript"], surface_by_key["typescript"]):
            for left_surface in reference_surfaces:
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        "axis_map_default_module_mutated_boundary",
                        left_surface,
                        right_surface,
                        "not_equivalent",
                        "heldout",
                        "literal-map-default-boundary",
                    )
                )
    for proposal_id in (
        "axis_map_default_js_object_hasown_identity",
        "axis_map_default_js_object_call_identity",
        "axis_map_default_js_object_negated_identity",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for right_surface in right_surfaces:
            for left_surface in reference_surfaces:
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        left_surface,
                        right_surface,
                        "equivalent",
                        "heldout",
                    )
                )
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        left_surface,
                        right_surface,
                        "not_equivalent",
                        "heldout",
                        "literal_map_default_lookup-semantic-mutation",
                    )
                )
    for proposal_id in (
        "axis_map_default_js_object_wrong_key_boundary",
        "axis_map_default_js_object_wrong_default_boundary",
        "axis_map_default_js_object_wrong_map_boundary",
        "axis_map_default_js_object_unguarded_boundary",
        "axis_map_default_js_object_in_boundary",
        "axis_map_default_js_object_method_boundary",
        "axis_map_default_js_object_shadowed_boundary",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for right_surface in right_surfaces:
            for left_surface in reference_surfaces:
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        left_surface,
                        right_surface,
                        "not_equivalent",
                        "heldout",
                        "literal-map-default-boundary",
                    )
                )
    return items


def generate_map_default_lookup_cross_items(
    out_dir: Path,
    capabilities: dict,
    cross_mode: str,
    generation_filter: GenerationFilter,
) -> list[dict]:
    if not generation_filter.include_axis("map_default_lookup"):
        return []
    surfaces = [
        s
        for s in SURFACES
        if map_default_lookup_axis_supported(s, "axis_map_fallback_identity")
    ]
    items: list[dict] = []
    for left_surface, right_surface in cross_pairs(surfaces, cross_mode):
        if generation_filter.include_proposal("axis_map_fallback_identity"):
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    "axis_map_fallback_identity",
                    left_surface,
                    right_surface,
                    "equivalent",
                    "heldout",
                )
            )
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    "axis_map_fallback_identity",
                    left_surface,
                    right_surface,
                    "not_equivalent",
                    "heldout",
                    "map_default_lookup-semantic-mutation",
                )
            )
        for proposal_id in (
            "axis_map_fallback_wrong_key_boundary",
            "axis_map_fallback_wrong_default_boundary",
            "axis_map_fallback_wrong_map_boundary",
        ):
            if not generation_filter.include_proposal(proposal_id):
                continue
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    right_surface,
                    "not_equivalent",
                    "heldout",
                    "map-default-boundary",
                )
            )
    surface_by_key = {surface.key: surface for surface in SURFACES}
    ts_surface = surface_by_key["typescript"]
    reference_surfaces = [
        surface_by_key["go"],
        surface_by_key["java"],
        surface_by_key["rust"],
    ]
    if cross_mode == "ring":
        reference_surfaces = [surface_by_key["go"]]
    elif cross_mode == "none":
        reference_surfaces = []

    for proposal_id in (
        "axis_map_fallback_ts_nullish_identity",
        "axis_map_fallback_ts_has_get_identity",
        "axis_map_fallback_ts_temp_guard_identity",
        "axis_map_fallback_ts_guard_return_identity",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for left_surface in reference_surfaces:
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    ts_surface,
                    "equivalent",
                    "heldout",
                )
            )
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    ts_surface,
                    "not_equivalent",
                    "heldout",
                    "map_default_lookup-semantic-mutation",
                )
            )
    java_surface = surface_by_key["java"]
    if generation_filter.include_proposal("axis_map_fallback_java_guard_return_identity"):
        for left_surface in reference_surfaces:
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    "axis_map_fallback_java_guard_return_identity",
                    left_surface,
                    java_surface,
                    "equivalent",
                    "heldout",
                )
            )
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    "axis_map_fallback_java_guard_return_identity",
                    left_surface,
                    java_surface,
                    "not_equivalent",
                    "heldout",
                    "map_default_lookup-semantic-mutation",
                )
            )
    for proposal_id in (
        "axis_map_fallback_ts_wrong_key_boundary",
        "axis_map_fallback_ts_wrong_default_boundary",
        "axis_map_fallback_ts_wrong_map_boundary",
        "axis_map_fallback_ts_untyped_boundary",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for left_surface in reference_surfaces:
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    ts_surface,
                    "not_equivalent",
                    "heldout",
                    "map-default-boundary",
                )
            )
    python_surface = surface_by_key["python"]
    for proposal_id in (
        "axis_map_fallback_python_dict_get_identity",
        "axis_map_fallback_python_mapping_get_identity",
        "axis_map_fallback_python_mutable_mapping_get_identity",
        "axis_map_fallback_python_alias_mapping_identity",
        "axis_map_fallback_python_alias_mutable_mapping_identity",
        "axis_map_fallback_python_alias_dict_identity",
        "axis_map_fallback_python_guard_return_identity",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for left_surface in reference_surfaces:
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    python_surface,
                    "equivalent",
                    "heldout",
                )
            )
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    python_surface,
                    "not_equivalent",
                    "heldout",
                    "map_default_lookup-semantic-mutation",
                )
            )
    for proposal_id in (
        "axis_map_fallback_python_wrong_key_boundary",
        "axis_map_fallback_python_wrong_default_boundary",
        "axis_map_fallback_python_wrong_map_boundary",
        "axis_map_fallback_python_untyped_boundary",
        "axis_map_fallback_python_alias_wrong_key_boundary",
        "axis_map_fallback_python_alias_wrong_default_boundary",
        "axis_map_fallback_python_alias_wrong_map_boundary",
        "axis_map_fallback_python_alias_unresolved_boundary",
        "axis_map_fallback_python_alias_shadowed_boundary",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for left_surface in reference_surfaces:
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    python_surface,
                    "not_equivalent",
                    "heldout",
                    "map-default-boundary",
                )
            )
    return items


def generate_null_presence_cross_items(
    out_dir: Path,
    capabilities: dict,
    cross_mode: str,
    generation_filter: GenerationFilter,
) -> list[dict]:
    if not generation_filter.include_axis("null_presence_predicate"):
        return []
    surfaces = [
        s
        for s in SURFACES
        if null_presence_axis_supported(s, "axis_null_presence_method_identity")
    ]
    items: list[dict] = []
    for left_surface, right_surface in cross_pairs(surfaces, cross_mode):
        if generation_filter.include_proposal("axis_null_presence_method_identity"):
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    "axis_null_presence_method_identity",
                    left_surface,
                    right_surface,
                    "equivalent",
                    "heldout",
                )
            )
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    "axis_null_presence_method_identity",
                    left_surface,
                    right_surface,
                    "not_equivalent",
                    "heldout",
                    "null_presence_predicate-semantic-mutation",
                )
            )
        for proposal_id in (
            "axis_null_presence_nonnull_boundary",
            "axis_null_presence_wrong_value_boundary",
        ):
            if not generation_filter.include_proposal(proposal_id):
                continue
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    right_surface,
                    "not_equivalent",
                    "heldout",
                    "null-presence-boundary",
                )
            )
    return items


def generate_scalar_abs_cross_items(
    out_dir: Path,
    capabilities: dict,
    cross_mode: str,
    generation_filter: GenerationFilter,
) -> list[dict]:
    if not generation_filter.include_axis("numeric_minmax_abs"):
        return []
    surfaces = [
        s
        for s in SURFACES
        if scalar_abs_axis_supported(s, "axis_scalar_abs_function_identity")
    ]
    items: list[dict] = []
    for left_surface, right_surface in cross_pairs(surfaces, cross_mode):
        for proposal_id in (
            "axis_scalar_abs_function_identity",
            "axis_scalar_min_function_identity",
            "axis_scalar_max_function_identity",
        ):
            if not generation_filter.include_proposal(proposal_id):
                continue
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    right_surface,
                    "equivalent",
                    "heldout",
                )
            )
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    right_surface,
                    "not_equivalent",
                    "heldout",
                    "numeric_minmax_abs-semantic-mutation",
                )
            )
        for proposal_id in (
            "axis_scalar_abs_sign_boundary",
            "axis_scalar_abs_wrong_value_boundary",
            "axis_scalar_min_wrong_value_boundary",
            "axis_scalar_max_wrong_value_boundary",
        ):
            if not generation_filter.include_proposal(proposal_id):
                continue
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    right_surface,
                    "not_equivalent",
                    "heldout",
                    "numeric-abs-boundary",
                )
            )
    return items


def generate_rust_numeric_method_cross_items(
    out_dir: Path,
    capabilities: dict,
    cross_mode: str,
    generation_filter: GenerationFilter,
) -> list[dict]:
    if cross_mode == "none" or not generation_filter.include_axis("numeric_minmax_abs"):
        return []
    rust_surface = next(s for s in SURFACES if s.key == "rust")
    reference_surfaces = [
        s
        for s in SURFACES
        if s.key != "rust" and scalar_abs_axis_supported(s, "axis_scalar_abs_function_identity")
    ]
    if cross_mode == "ring":
        reference_surfaces = reference_surfaces[:3]
    items: list[dict] = []
    for proposal_id in (
        "axis_scalar_rust_abs_method_identity",
        "axis_scalar_rust_min_method_identity",
        "axis_scalar_rust_max_method_identity",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for left_surface in reference_surfaces:
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    rust_surface,
                    "equivalent",
                    "heldout",
                )
            )
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    rust_surface,
                    "not_equivalent",
                    "heldout",
                    "numeric_minmax_abs-semantic-mutation",
                )
            )
    for proposal_id in (
        "axis_scalar_rust_abs_wrong_value_boundary",
        "axis_scalar_rust_min_wrong_value_boundary",
        "axis_scalar_rust_max_wrong_value_boundary",
        "axis_scalar_rust_abs_custom_method_boundary",
        "axis_scalar_rust_min_custom_method_boundary",
        "axis_scalar_rust_max_custom_method_boundary",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for left_surface in reference_surfaces:
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    rust_surface,
                    "not_equivalent",
                    "heldout",
                    "numeric-rust-method-boundary",
                )
            )
    return items


def cross_pairs(surfaces: list[Surface], mode: str) -> list[tuple[Surface, Surface]]:
    if mode == "none":
        return []
    if mode == "ring":
        return [(surfaces[i], surfaces[(i + 1) % len(surfaces)]) for i in range(len(surfaces))]
    if mode == "all":
        return [(a, b) for i, a in enumerate(surfaces) for b in surfaces[i + 1 :]]
    raise ValueError(f"unknown cross mode: {mode}")


def split_filters(values: list[str] | None) -> tuple[str, ...]:
    if not values:
        return ()
    parts: list[str] = []
    for value in values:
        parts.extend(part.strip() for part in value.split(",") if part.strip())
    return tuple(dict.fromkeys(parts))


def generate(
    out_dir: Path,
    proposal_file: Path,
    capability_file: Path,
    cross_mode: str,
    clean: bool,
    generation_filter: GenerationFilter,
) -> dict:
    if clean and out_dir.exists():
        shutil.rmtree(out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    proposal_file = proposal_file.resolve()
    capability_file = capability_file.resolve()
    proposals_doc = json.loads(proposal_file.read_text())
    capabilities = load_capabilities(capability_file)
    validate_proposals(proposals_doc)
    items = []
    for proposal in proposals_doc["proposals"]:
        if not generation_filter.include_base_proposal(proposal):
            continue
        for surface in SURFACES:
            items.append(
                make_item(
                    out_dir,
                    proposal,
                    surface,
                    surface,
                    "aggregate",
                    "equivalent",
                    "same-surface",
                    "dev",
                )
            )
            items.append(
                make_item(
                    out_dir,
                    proposal,
                    surface,
                    surface,
                    "aggregate",
                    "not_equivalent",
                    "same-surface",
                    "heldout",
                    "aggregate-semantic-mutation",
                )
            )
            items.append(
                make_item(
                    out_dir,
                    proposal,
                    surface,
                    surface,
                    "loop",
                    "not_equivalent",
                    "same-surface",
                    "heldout",
                    "same-template-semantic-mutation",
                )
            )
            if OPERATIONS[proposal["operation"]].arity == 1:
                items.append(
                    make_item(
                        out_dir,
                        proposal,
                        surface,
                        surface,
                        "indexed_loop",
                        "equivalent",
                        "same-surface",
                        "heldout",
                    )
                )
                items.append(
                    make_item(
                        out_dir,
                        proposal,
                        surface,
                        surface,
                        "indexed_loop",
                        "not_equivalent",
                        "same-surface",
                        "heldout",
                        "indexed-template-semantic-mutation",
                    )
                )
        for representation in ("c_start_one", "c_stride_two"):
            items.append(make_c_contract_negative_item(out_dir, proposal, representation))
        for left_surface, right_surface in cross_pairs(SURFACES, cross_mode):
            items.append(
                make_item(
                    out_dir,
                    proposal,
                    left_surface,
                    right_surface,
                    "loop",
                    "equivalent",
                    "cross-surface",
                    "heldout",
                )
            )
            items.append(
                make_item(
                    out_dir,
                    proposal,
                    left_surface,
                    right_surface,
                    "loop",
                    "not_equivalent",
                    "cross-surface",
                    "heldout",
                    "cross-template-semantic-mutation",
                )
            )
    items.extend(generate_axis_items(out_dir, capabilities, generation_filter))
    items.extend(generate_hof_filter_map_cross_items(out_dir, capabilities, generation_filter))
    items.extend(
        generate_string_prefix_cross_items(out_dir, capabilities, cross_mode, generation_filter)
    )
    items.extend(
        generate_literal_membership_cross_items(
            out_dir, capabilities, cross_mode, generation_filter
        )
    )
    items.extend(
        generate_java_factory_membership_cross_items(
            out_dir, capabilities, cross_mode, generation_filter
        )
    )
    items.extend(
        generate_map_key_membership_cross_items(
            out_dir, capabilities, cross_mode, generation_filter
        )
    )
    items.extend(
        generate_literal_map_default_cross_items(
            out_dir, capabilities, cross_mode, generation_filter
        )
    )
    items.extend(
        generate_map_default_lookup_cross_items(
            out_dir, capabilities, cross_mode, generation_filter
        )
    )
    items.extend(
        generate_null_presence_cross_items(
            out_dir, capabilities, cross_mode, generation_filter
        )
    )
    items.extend(
        generate_scalar_abs_cross_items(
            out_dir, capabilities, cross_mode, generation_filter
        )
    )
    items.extend(
        generate_rust_numeric_method_cross_items(
            out_dir, capabilities, cross_mode, generation_filter
        )
    )
    return {
        "schema_version": "0.1.0",
        "source": {
            "generator": "bench/type4/generate.py",
            "proposal_file": str(proposal_file.relative_to(ROOT)),
            "capability_file": str(capability_file.relative_to(ROOT)),
            "cross_mode": cross_mode,
            "axis_filter": sorted(generation_filter.axes),
            "proposal_prefix_filter": list(generation_filter.proposal_prefixes),
        },
        "items": items,
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out-dir", required=True, type=Path)
    parser.add_argument("--proposal-file", default=DEFAULT_PROPOSALS, type=Path)
    parser.add_argument("--capability-file", default=DEFAULT_CAPABILITIES, type=Path)
    parser.add_argument("--cross", choices=["none", "ring", "all"], default="ring")
    parser.add_argument(
        "--axis",
        action="append",
        help="only generate cases whose semantic axis/computation matches this value; may be repeated or comma-separated",
    )
    parser.add_argument(
        "--proposal-prefix",
        action="append",
        help="only generate proposal ids with this prefix; may be repeated or comma-separated",
    )
    parser.add_argument("--no-clean", action="store_true", help="do not clear the output directory first")
    args = parser.parse_args()
    generation_filter = GenerationFilter(
        axes=frozenset(split_filters(args.axis)),
        proposal_prefixes=split_filters(args.proposal_prefix),
    )
    manifest = generate(
        args.out_dir,
        args.proposal_file,
        args.capability_file,
        args.cross,
        clean=not args.no_clean,
        generation_filter=generation_filter,
    )
    manifest_path = args.out_dir / "manifest.json"
    manifest_path.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
    by_status: dict[str, int] = {}
    for item in manifest["items"]:
        by_status[item["semantic_status"]] = by_status.get(item["semantic_status"], 0) + 1
    print(f"wrote {len(manifest['items'])} items to {manifest_path}")
    print("status:", ", ".join(f"{k}={v}" for k, v in sorted(by_status.items())))


if __name__ == "__main__":
    main()
