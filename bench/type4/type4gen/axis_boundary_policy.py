"""Boundary/import axis policies."""

from __future__ import annotations

from type4gen.axis_boundaries import (
    axis_projection_variant,
    axis_python_docstring_variant,
    axis_unsafe_boundary_variant,
    import_axis_supported,
    import_axis_variant,
    projection_axis_supported,
    python_docstring_axis_supported,
)
from type4gen.axis_policy import (
    AxisPolicy,
    boundary_case_plan,
    default_case_plans,
)
from type4gen.model import Surface, Variant


def import_variants(
    surface: Surface,
    proposal_id: str,
    negative: bool,
) -> tuple[Variant, Variant]:
    return (
        import_axis_variant(surface, proposal_id, False, False),
        import_axis_variant(surface, proposal_id, negative, True),
    )


def projection_variants(
    surface: Surface,
    proposal_id: str,
    negative: bool,
) -> tuple[Variant, Variant]:
    return (
        axis_projection_variant(surface, proposal_id, False, False),
        axis_projection_variant(surface, proposal_id, negative, True),
    )


def python_docstring_variants(
    surface: Surface,
    proposal_id: str,
    negative: bool,
) -> tuple[Variant, Variant]:
    return (
        axis_python_docstring_variant(surface, proposal_id, False, False),
        axis_python_docstring_variant(surface, proposal_id, negative, True),
    )


def unsafe_boundary_variants(
    surface: Surface,
    _proposal_id: str,
    _negative: bool,
) -> tuple[Variant, Variant]:
    return (
        axis_unsafe_boundary_variant(surface, False),
        axis_unsafe_boundary_variant(surface, True),
    )


def import_case_plans(
    surface: Surface,
    proposal_id: str,
    _capability: str,
):
    if not import_axis_supported(surface, proposal_id):
        return ()
    if proposal_id in {"axis_import_unsafe_boundary", "axis_import_reexport_boundary"}:
        return boundary_case_plan("unproven-import-binding", "unknown")
    if proposal_id == "axis_import_namespace_member_wrong_boundary":
        return boundary_case_plan("import-member-boundary")
    if proposal_id == "axis_import_namespace_shadowed_param_fake_receiver_boundary":
        return boundary_case_plan("import-namespace-shadow-boundary")
    return default_case_plans(surface, proposal_id, _capability)


def projection_case_plans(
    surface: Surface,
    proposal_id: str,
    capability: str,
):
    if not projection_axis_supported(surface, proposal_id):
        return ()
    if proposal_id in {
        "axis_projection_default_boundary",
        "axis_projection_dynamic_key_boundary",
    }:
        return boundary_case_plan("unproven-projection-binding")
    return default_case_plans(surface, proposal_id, capability)


def python_docstring_case_plans(
    surface: Surface,
    proposal_id: str,
    capability: str,
):
    if not python_docstring_axis_supported(surface, proposal_id):
        return ()
    if proposal_id in {
        "axis_python_docstring_returned_string_boundary",
        "axis_python_docstring_assigned_string_boundary",
        "axis_python_docstring_fstring_boundary",
    }:
        return boundary_case_plan("python-docstring-boundary")
    return default_case_plans(surface, proposal_id, capability)


def unsafe_boundary_case_plans(
    _surface: Surface,
    _proposal_id: str,
    _capability: str,
):
    return boundary_case_plan("unproven-free-binding", "unknown")


AXIS_POLICIES = {
    "import_identity": AxisPolicy(import_variants, "scalar<int>", import_case_plans),
    "projection_identity": AxisPolicy(
        projection_variants,
        "record<today:int,tomorrow:int>",
        projection_case_plans,
    ),
    "python_docstring_noop": AxisPolicy(
        python_docstring_variants,
        "python-callable",
        python_docstring_case_plans,
    ),
    "unsafe_boundary": AxisPolicy(
        unsafe_boundary_variants,
        "scalar<int>",
        unsafe_boundary_case_plans,
    ),
}
