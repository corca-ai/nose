#!/usr/bin/env python3
"""Generate the seed corpus for the evidence-carrying Type-4 benchmark factory."""

from __future__ import annotations

import argparse
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
from type4gen.axis_cases import (
    axis_data_shape,
    axis_variants,
    cross_pairs,
    generate_axis_items,
    make_axis_cross_item,
    make_axis_item,
)
from type4gen.axis_collection_cases import (
    generate_string_prefix_cross_items,
)
from type4gen.axis_map_cases import (
    generate_literal_map_default_cross_items,
    generate_map_default_lookup_cross_items,
    generate_map_key_membership_cross_items,
)
from type4gen.axis_membership_cases import (
    generate_java_factory_membership_cross_items,
    generate_literal_membership_cross_items,
)
from type4gen.axis_proposals import AXIS_PROPOSALS
from type4gen.axis_scalar_cases import (
    generate_hof_filter_map_cross_items,
    generate_null_presence_cross_items,
    generate_rust_numeric_method_cross_items,
    generate_scalar_abs_cross_items,
)
from type4gen.axis_evidence import axis_evidence
from type4gen.case_io import (
    rel_source_path,
    source_record,
    stable_id,
    write_source,
)
from type4gen.model import (
    OPERATIONS,
    REQUIRED_BUDGET_FIELDS,
    REQUIRED_PROPOSAL_FIELDS,
    SEMANTIC_SCOPE,
    SURFACES,
    GenerationFilter,
    Surface,
    Variant,
    load_capabilities,
)

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_PROPOSALS = ROOT / "bench" / "type4" / "proposals.v1.json"
DEFAULT_CAPABILITIES = ROOT / "bench" / "type4" / "capabilities.v1.json"


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
