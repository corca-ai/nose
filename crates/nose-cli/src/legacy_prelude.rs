//! Temporary compatibility prelude for CLI modules that still use ambient imports.
//!
//! Keep this module small and shrink it as modules move to explicit `crate::...`
//! imports. It exists so the crate root can stay as module declarations plus the
//! public run API instead of acting as the implicit owner of every helper.

pub(crate) use crate::baseline;
pub(crate) use crate::cli_args::*;
pub(crate) use crate::falsify;
pub(crate) use crate::markdown;
pub(crate) use crate::oracle_gate::*;
pub(crate) use crate::path_utils::*;
pub(crate) use crate::query_markdown::*;
pub(crate) use crate::query_opportunities::*;
pub(crate) use crate::query_options::*;
pub(crate) use crate::query_sarif::*;
pub(crate) use crate::query_terms::{family_at, parse_query, Query};
pub(crate) use crate::query_witness::*;
pub(crate) use crate::report_text::*;
pub(crate) use crate::schema_versions;
pub(crate) use crate::source_lines::*;
pub(crate) use crate::style;
pub(crate) use crate::surfaces::{
    classify_surface_overrides, is_default_report_family, surface_omission_note, SurfaceOverrides,
};
pub(crate) use crate::timing::*;
pub(crate) use crate::verify_census;
pub(crate) use crate::verify_collect::*;
pub(crate) use anyhow::{Context, Result};
pub(crate) use nose_il::{Corpus, Interner, Lang};
pub(crate) use rayon::prelude::*;
pub(crate) use std::path::PathBuf;
