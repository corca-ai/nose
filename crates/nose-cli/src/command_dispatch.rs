use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

use crate::cli_args::{Cli, Cmd, SemanticPackCmd, StatsFormat};
use crate::detect_command::{cmd_detect, DetectArgs};
use crate::diagnostic_commands::{
    cmd_ceiling, cmd_eval, cmd_features, cmd_gap_impact, cmd_stats, cmd_value_census,
};
use crate::il_command::cmd_il;
use crate::oracle_gate::{cmd_behavioral_gate, verify_battery, verify_probes};
use crate::path_utils::{paths_as_refs, warn_if_empty};
use crate::query_commands::run_query_cmd;
use crate::verify_collect::collect_verify_recs;
use crate::verify_report::{
    print_verify_exclusions, print_verify_json, report_falsify, report_verify_calibration,
    report_verify_completeness, report_verify_soundness,
};
use crate::{capabilities, semantic_pack, verify_census};

pub(crate) fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Il {
            path,
            format,
            normalized,
            no_cfg_norm,
        } => cmd_il(path, format, normalized, no_cfg_norm),
        Cmd::Capabilities => capabilities::print(),
        Cmd::SemanticPack { cmd } => match cmd {
            SemanticPackCmd::Check { paths, format } => semantic_pack::cmd_check(paths, format),
            SemanticPackCmd::AdoptionGates { format } => semantic_pack::cmd_adoption_gates(format),
            SemanticPackCmd::Compatibility { format } => semantic_pack::cmd_compatibility(format),
            SemanticPackCmd::Inventory { format } => semantic_pack::cmd_inventory(format),
        },
        cmd @ Cmd::Detect { .. } => run_detect_cmd(cmd),
        Cmd::Eval {
            gold,
            predictions,
            hard_negatives,
            corpus,
        } => cmd_eval(gold, predictions, hard_negatives, corpus),
        cmd @ Cmd::Query { .. } => run_query_cmd(cmd),
        Cmd::Ceiling {
            gold,
            units,
            candidates,
        } => cmd_ceiling(gold, units, candidates),
        Cmd::Stats { paths, top, format } => cmd_stats(paths, top, format == StatsFormat::Json),
        Cmd::GapImpact { paths, top, format } => {
            cmd_gap_impact(paths, top, format == StatsFormat::Json)
        }
        Cmd::Features {
            paths,
            min_lines,
            min_tokens,
            no_cfg_norm,
            no_blocks,
        } => cmd_features(paths, min_lines, min_tokens, no_cfg_norm, no_blocks),
        Cmd::ValueCensus { paths, no_cfg_norm } => cmd_value_census(paths, no_cfg_norm),
        Cmd::Verify {
            paths,
            no_cfg_norm,
            json,
            max_violations,
            leads,
            recall_loss_report,
            exclusion_census,
            falsify,
        } => cmd_verify(VerifyArgs {
            paths,
            no_cfg_norm,
            json,
            max_violations,
            leads,
            recall_loss_report,
            exclusion_census,
            falsify,
        }),
        Cmd::BehavioralGate {
            paths,
            manifest,
            battery,
        } => cmd_behavioral_gate(paths, manifest, battery),
    }
}

fn run_detect_cmd(cmd: Cmd) -> Result<()> {
    let Cmd::Detect {
        paths,
        min_lines,
        min_tokens,
        threshold,
        candidates,
        minhash_k,
        bands,
        no_cfg_norm,
        dce,
        no_blocks,
        out,
        summary,
        bench_schema,
        repos_root,
        dump,
    } = cmd
    else {
        unreachable!("run_detect_cmd requires Cmd::Detect")
    };
    cmd_detect(DetectArgs {
        paths,
        min_lines,
        min_tokens,
        threshold,
        candidates,
        minhash_k,
        bands,
        no_cfg_norm,
        dce,
        no_blocks,
        out,
        summary,
        bench_schema,
        repos_root,
        dump,
    })
}

struct VerifyArgs {
    paths: Vec<PathBuf>,
    no_cfg_norm: bool,
    json: bool,
    max_violations: Option<usize>,
    leads: Option<PathBuf>,
    recall_loss_report: Option<PathBuf>,
    exclusion_census: Option<PathBuf>,
    falsify: bool,
}

fn cmd_verify(args: VerifyArgs) -> Result<()> {
    let VerifyArgs {
        paths,
        no_cfg_norm,
        json,
        max_violations,
        leads,
        recall_loss_report,
        exclusion_census,
        falsify,
    } = args;
    let refs = paths_as_refs(&paths);
    let corpus = nose_frontend::lower_corpus_many(&refs);
    warn_if_empty(&corpus, &paths);
    let opts = nose_normalize::NormalizeOptions {
        cfg_norm: !no_cfg_norm,
        ..Default::default()
    };
    // Mine the literals the corpus branches on, to probe value-keyed branches. The
    // battery is identical for every unit (a function uses only its first `arity`
    // inputs), so behavior vectors are always length-comparable.
    let battery = verify_battery(&verify_probes(&corpus));
    let oracle = collect_verify_recs(&corpus, &opts, &battery, exclusion_census.is_some());
    if let Some(path) = &exclusion_census {
        verify_census::write_report(path, &oracle.census)?;
        println!("exclusion census written to {}", path.display());
    }
    if let Some(path) = &recall_loss_report {
        crate::recall_loss_report::write_report(
            path,
            &corpus,
            &oracle,
            &paths,
            no_cfg_norm,
            max_violations,
        )?;
    }

    if json {
        return print_verify_json(&oracle);
    }

    println!("=== value-graph oracle (soundness + completeness) ===");
    println!(
        "units: {} total, {} interpretable ({} excluded)",
        oracle.total,
        oracle.recs.len(),
        oracle.total - oracle.recs.len()
    );
    print_verify_exclusions(&oracle.exclusions);

    // --- Canon preservation: full-normalize behavior must equal pre-canon core behavior. ---
    println!("\nCANON PRESERVATION — normalization preserves behavior:");
    println!(
        "  units checked (interpretable both ways): {}",
        oracle.canon_checked
    );
    if oracle.canon_violations.is_empty() {
        println!("  PRESERVED: every canon-changed unit computes the same thing ✓");
    } else {
        println!(
            "  [!] {} unit(s) whose behavior CHANGED under canonicalization:",
            oracle.canon_violations.len()
        );
        for loc in &oracle.canon_violations {
            println!("    {loc}");
        }
    }

    let mut n_violations = report_verify_soundness(&oracle.recs);
    report_verify_completeness(&oracle.recs, leads.as_deref())?;
    report_verify_calibration(&oracle.recs);
    if falsify {
        // Falsification search (#317): augment the fixed battery with a per-group
        // distinguishing-input search. Any hit is a false merge the battery's input
        // starvation missed; count it toward the gate so `--falsify --max-violations 0` is
        // the stronger engine the issue calls for.
        n_violations += report_falsify(&corpus, &opts, &oracle.recs, &verify_probes(&corpus));
    }

    // CI soundness gate: fail if false merges exceed the budget, or if any normalization
    // pass changes a unit's behavior vs the pre-canon core IL. The independent oracle thus
    // becomes a permanent regression gate on the detection campaign: a new canon that
    // introduces a real false merge, or changes behavior before it even collides with a twin,
    // trips this gate.
    if let Some(budget) = max_violations {
        if !oracle.canon_violations.is_empty() {
            anyhow::bail!(
                "verify gate: {} canon-preservation violations",
                oracle.canon_violations.len()
            );
        }
        if n_violations > budget {
            anyhow::bail!("verify gate: {n_violations} false merges exceed the budget of {budget}");
        }
        println!("\nGATE: {n_violations} ≤ {budget} false merges — OK ✓");
    }
    Ok(())
}
