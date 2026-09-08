use super::{is_default_surface, OpportunityGroups, SurfaceOverrides};
use nose_detect::RefactorFamily;
use serde_json::{json, Value};

pub(super) fn describe(
    families: &[RefactorFamily],
    overrides: &SurfaceOverrides,
    opportunities: &OpportunityGroups,
) -> Value {
    let (mut default_unfolded, mut default_folded, mut other_surfaces, mut all_folded) =
        (0_usize, 0_usize, 0_usize, 0_usize);
    for family in families {
        if is_default_surface(family, overrides) {
            if opportunities.is_default_slice(family) {
                default_folded += 1;
            } else {
                default_unfolded += 1;
            }
        } else {
            other_surfaces += 1;
        }
        all_folded += usize::from(opportunities.is_slice(family));
    }
    json!({
        "families": families.len(),
        "default_unfolded": default_unfolded,
        "default_folded": default_folded,
        "other_surfaces": other_surfaces,
        "all_unfolded": families.len() - all_folded,
        "all_folded": all_folded,
        "scope": "after-baseline-and-ignores-before-presentation",
        "meaning": "Unfolded counts precede row limits. Captures precede baseline/ignore processing and coalesce duplicate observation addresses; their population may differ. Markdown findings are separate."
    })
}

pub(super) fn render(population: &Value) {
    println!(
        "  Report population: {} code families after baseline/ignores, before presentation.",
        population["families"]
    );
    println!(
        "  default: {} unfolded + {} folded; other surfaces: {}. all: {} unfolded + {} folded (before row limits).",
        population["default_unfolded"], population["default_folded"],
        population["other_surfaces"], population["all_unfolded"], population["all_folded"]
    );
}
