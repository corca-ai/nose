use crate::baseline;
use crate::family_display::family_summary;
use crate::oracle_gate::verify_battery_over_budget;
use crate::query_model::{family_existing_helper, family_spotclass, query_family_json, short_id};
use crate::query_opportunities::{family_hint, hint_reasons, OpportunityGroups};
use crate::query_witness::{decorator_difference, decorator_prefix};
use crate::source_lines::{classify_param, line_diff, shared_lines_of, FileLineCache};
use crate::surfaces::{
    effective_surface, family_actionability_reason, family_is_compiled_css_pipeline,
    has_version_tag, is_default_report_family, looks_compiled_css, span_is_declarations,
    surface_omission_note, SurfaceOverrides,
};
use nose_detect::{LineSpan, Loc, LocInit, RefactorFamily};

mod declarations;
mod query_family;
mod surface_hints;
