mod baseline;
mod baseline_comparison;
mod cache;
mod capabilities;
mod cli_args;
mod command_dispatch;
mod config;
mod detect_command;
mod detect_pipeline;
mod diagnostic_commands;
mod divergence;
mod falsify;
mod family_display;
mod fnv;
mod ignores;
mod il_command;
mod legacy_prelude;
#[cfg(test)]
mod main_tests;
mod markdown;
mod oracle_gate;
mod path_utils;
mod query_baseline_gate;
mod query_commands;
mod query_dashboard;
mod query_dataset;
mod query_family_text;
mod query_markdown;
mod query_model;
mod query_open;
mod query_opportunities;
mod query_options;
mod query_sarif;
mod query_semantic_packs;
mod query_terms;
mod query_views;
mod query_witness;
mod recall_loss_report;
mod report_text;
mod runtime;
mod schema_versions;
mod semantic_pack;
mod source_lines;
mod style;
mod surfaces;
mod timing;
mod verify_admission;
mod verify_census;
mod verify_collect;
mod verify_report;

use anyhow::Result;

pub fn install_broken_pipe_guard() {
    runtime::install_broken_pipe_guard();
}

pub const STACK_SIZE: usize = runtime::STACK_SIZE;

pub fn run_command() -> Result<()> {
    command_dispatch::run()
}
