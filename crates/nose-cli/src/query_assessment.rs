//! Explain measured extraction support without turning similarity into a verdict.
use nose_detect::{Loc, RefactorFamily};
use serde_json::{json, Value};

pub(crate) fn assessment(f: &RefactorFamily, shared: u32, params: u32) -> Value {
    let measured = f.display_params.is_some();
    let (support, explanation) = if f.languages > 1 {
        ("cross-language-comparison", "Matched computations cross language boundaries; inspect behavior and ownership before choosing language-specific changes.")
    } else if !measured {
        ("source-evidence-unavailable", "Complete source-line measurements are unavailable; semantic similarity alone does not establish an extractable body.")
    } else if shared == 0 {
        ("no-shared-source", "No invariant source lines were measured; compare the differing behavior before proposing an extraction.")
    } else if f.shared_weight <= 0.0 {
        ("common-syntax-only", "Shared source lacks substantive ranking evidence; syntax in common is insufficient to justify a helper.")
    } else {
        ("shared-source", "Shared source supports inspecting a common implementation; dependency direction and behavioral differences still determine whether extraction is useful.")
    };
    let mut checks = vec![
        "ownership-and-dependency-direction",
        "visibility-and-call-contract",
        "effects-and-error-behavior",
    ];
    if params > 0 {
        checks.push("varying-regions-are-not-proven-parameters");
    }
    if f.scope == "mixed" {
        checks.push("production-test-boundary");
    }
    json!({"support":support,"explanation":explanation,"shared_lines":shared,"varying_regions":params,
        "measurement":if measured { "source-line-alignment" } else { "unavailable" },
        "checks":checks,"verdict":"caller-review-required"})
}

pub(crate) fn scope(location: &Loc) -> Value {
    let mut reasons = Vec::new();
    if nose_detect::is_test_path(&location.file) {
        reasons.push("test-path-convention");
    }
    if location
        .origin
        .has_evidence(nose_il::UnitEvidenceFlag::TestContext)
    {
        reasons.push("frontend-test-context");
    }
    if location.in_test_module {
        reasons.push("enclosing-test-context");
    }
    let test = nose_detect::is_test_loc(location);
    if test && reasons.is_empty() {
        reasons.push("test-name-convention");
    }
    if !test {
        reasons.push("no-recognized-test-evidence");
    }
    json!({"scope":if test { "test" } else { "prod" },"reasons":reasons})
}

pub(crate) fn selection_reason(
    f: &RefactorFamily,
    groups: &crate::query_opportunities::OpportunityGroups,
    selection: &[&RefactorFamily],
) -> Option<Value> {
    let id = crate::baseline::family_id(f);
    let primary = groups.primary_of.get(&id)?;
    if selection
        .iter()
        .any(|f| crate::baseline::family_id(f) == *primary)
    {
        return None;
    }
    Some(json!({"kind":"recovered-overlap","primary_id":primary,
        "meaning":"This overlapping slice matches the current selection; its fuller primary is outside the selected filters or surface."}))
}
