//! End-to-end CLI tests: run the built `nose` binary against temp projects and
//! check user-visible behavior.

#[path = "cli/support.rs"]
mod support;

pub(crate) use support::*;

#[path = "cli/commands.rs"]
mod commands;
#[path = "cli/exact_fragments.rs"]
mod exact_fragments;
#[path = "cli/generated_surfaces.rs"]
mod generated_surfaces;
#[path = "cli/modes.rs"]
mod modes;
#[path = "cli/query_base.rs"]
mod query_base;
#[path = "cli/semantic_boundaries.rs"]
mod semantic_boundaries;
#[path = "cli/semantic_core.rs"]
mod semantic_core;
#[path = "cli/semantic_idioms.rs"]
mod semantic_idioms;
