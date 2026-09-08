//! Explain measured extraction support without turning similarity into a verdict.
use nose_detect::{Loc, RefactorFamily};
use serde_json::{json, Value};

pub(crate) fn assessment(f: &RefactorFamily, shared: u32, params: u32) -> Value {
    let measured = f.display_params.is_some();
    let (support, explanation) = if f.languages > 1 {
        ("cross-language-comparison", "Compare the detector relation across languages; direct helper reuse is not established by language tags.")
    } else if !measured {
        ("source-evidence-unavailable", "Complete source-line measurements are unavailable; semantic similarity alone does not establish an extractable body.")
    } else if shared == 0 {
        ("no-shared-source", "The bounded alignment found no invariant source lines; the detector relation remains independent evidence. Inspect source differences.")
    } else if f.shared_weight <= 0.0 {
        ("common-syntax-only", "Shared lines have no substantive ranking weight (common syntax or pervasive idioms); inspect the relation and source differences.")
    } else {
        ("shared-source", "Shared source is an inspection hint; compare differing regions and call contracts before deciding whether reuse is useful.")
    };
    let mut checks = Vec::new();
    if params > 0 && measured {
        checks.push("varying-regions-are-not-proven-parameters");
    }
    if f.scope == "mixed" {
        checks.push("production-test-boundary");
    }
    let witness = crate::query_model::witness_token(f.witness.as_ref().map(|w| w.kind()));
    json!({"support":support,"explanation":explanation,"shared_lines":shared,"varying_regions":params,
        "measurement":if measured { "source-line-alignment" } else { "unavailable" },
        "hint_kind":"inspection","measurement_scope":{"member_limit":8,"line_limit_per_member":120,"coverage":"inspect-source-evidence-for-coverage"},
        "relation":{"explanation":relation_explanation(f),"witness":witness,"scope":"detector-witness","meaning":"The detector relation holds within its modeled scope; literal overlap does not strengthen or invalidate it."},
        "structural_correspondence":{"status":if f.witness.as_ref().is_some_and(|w| w.graded.is_some()) {"available"} else {"not-available"},"scope":"graded-pair-only","meaning":"When available, graded and graded_pair describe one pair; they are not family-wide equivalence proof."},
        "unassessed":["ownership-and-dependency-direction","visibility-and-call-contract","refactoring-benefit"],
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

/// Build the membership index only if a displayed slice needs a primary lookup.
pub(crate) struct SelectionReasons<'a> {
    families: &'a [&'a RefactorFamily],
    ids: std::sync::OnceLock<std::collections::HashSet<String>>,
}

impl<'a> SelectionReasons<'a> {
    pub(crate) fn new(families: &'a [&'a RefactorFamily]) -> Self {
        Self {
            families,
            ids: std::sync::OnceLock::new(),
        }
    }

    pub(crate) fn reason(
        &self,
        id: &str,
        groups: &crate::query_opportunities::OpportunityGroups,
    ) -> Option<Value> {
        let primary = groups.primary_of.get(id)?;
        let ids = self.ids.get_or_init(|| {
            self.families
                .iter()
                .map(|f| crate::baseline::family_id(f))
                .collect()
        });
        if ids.contains(primary) {
            return None;
        }
        Some(json!({"kind":"recovered-overlap","primary_id":primary,
            "meaning":"This overlapping slice matches the current selection; its fuller primary is outside the selected filters or surface."}))
    }
}

/// Report known source boundaries without inferring that a span is extractable.
pub(crate) fn boundary(location: &Loc) -> Value {
    let (kind, meaning) = if location.is_fragment {
        (
            "exact-fragment",
            "An exact-fragment source region; extraction safety still requires caller review.",
        )
    } else if matches!(
        location.kind,
        nose_il::UnitKind::Function | nose_il::UnitKind::Method | nose_il::UnitKind::Class
    ) {
        ("named-unit", "A detected function, method or class region; this does not establish an extraction plan.")
    } else if location.enclosing_unit.is_some() {
        ("contained-region", "A detected source region within the reported enclosing unit; inspect surrounding control flow.")
    } else {
        ("unclassified-region", "No enclosing function, method or class was established; this region may cross declaration boundaries.")
    };
    json!({"kind":kind,"unit_kind":location.kind,"meaning":meaning,
        "enclosing_unit":location.enclosing_unit,"fragment_kind":location.fragment_kind,
        "extraction_safety":"unassessed"})
}

pub(crate) fn relation_explanation(f: &RefactorFamily) -> &'static str {
    match f.witness.as_ref().map(|w| w.kind()) {
        Some("copy-paste-run") => "copy-paste is a matching token run; shared counts invariant whole source lines across the bounded member alignment. Token overlap can exist with zero shared whole lines.",
        Some("exact-value-graph") => "exact is eligible value-fingerprint equality within the modeled semantics; shared separately measures invariant whole source lines.",
        _ => "The detector relation and invariant whole source lines measure different evidence; inspect the witness and source differences independently.",
    }
}

pub(crate) fn row_note(f: &RefactorFamily) -> &'static str {
    if f.witness
        .as_ref()
        .is_some_and(|w| w.kind() == "copy-paste-run")
        && f.display_params.is_some()
        && crate::query_model::all_copies_shared(f).0 == 0
    {
        " · matching tokens; no invariant whole lines"
    } else {
        ""
    }
}
