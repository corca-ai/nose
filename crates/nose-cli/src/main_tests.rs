use crate::family_display::family_summary;
use crate::legacy_prelude::*;
use crate::query_model::{family_existing_helper, family_spotclass, query_family_json, short_id};
use crate::surfaces::{
    effective_surface, family_is_compiled_css_pipeline, has_version_tag, looks_compiled_css,
    span_is_declarations,
};
use nose_detect::{LineSpan, Loc, LocInit, RefactorFamily};

mod declarations;
mod query_family;
mod surface_hints;
