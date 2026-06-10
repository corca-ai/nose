//! `nose` — multi-language code clone detector CLI.

mod baseline;
mod cache;
mod config;
mod fnv;
mod ignores;
mod review;
mod semantic_pack;
mod verify_census;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use nose_il::{Corpus, FileId, Interner, Lang};
use rayon::prelude::*;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "nose",
    version,
    about = "Find code clone families across syntax, semantic, and near-duplicate channels",
    long_about = "nose lowers each language into one normalized IL and finds duplicated code.\n\
                  • `nose scan <paths>` — default: syntax + semantic clone families\n\
                  • `nose scan <paths> --mode near` — opt into Type-3 near-duplicates\n\
                  • `nose stats <paths>`    — IL lowering coverage\n\
                  • `nose il <file>`        — inspect the IL\n\
                  • `nose capabilities`     — machine-readable integration contract"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Emit the machine-readable capability contract for integrations.
    Capabilities,
    /// Dump the IL for a source file (inspection / debugging the transpiler).
    Il {
        /// Path to a source file.
        path: PathBuf,
        /// Output format.
        #[arg(long, default_value = "sexpr")]
        format: Format,
        /// Show normalized (canonical) IL instead of raw.
        #[arg(long)]
        normalized: bool,
        /// Disable control-flow normalization (ablation).
        #[arg(long)]
        no_cfg_norm: bool,
    },
    /// Validate semantic-pack v0 manifests and declared conformance fixtures.
    #[command(name = "semantic-pack")]
    SemanticPack {
        #[command(subcommand)]
        cmd: SemanticPackCmd,
    },
    /// Research interface for raw unit clone pairs/groups.
    /// Hidden: `scan` is the user-facing command; `detect` is the strict/research
    /// and benchmark interface (`--bench-schema`, `--dump`, …).
    #[command(hide = true)]
    Detect {
        /// Paths to source files or directories (recursively scanned).
        #[arg(required = true)]
        paths: Vec<PathBuf>,
        /// Minimum unit line count.
        #[arg(long, default_value_t = 5)]
        min_lines: u32,
        /// Minimum unit token (IL node) count.
        #[arg(long, default_value_t = 24)]
        min_tokens: usize,
        /// Acceptance threshold in `[0,1]`. Defaults: 0.86 on the strict research
        /// path, 0.70 with --candidates. Lower for recall, raise for precision.
        #[arg(long, value_parser = parse_threshold)]
        threshold: Option<f64>,
        /// Candidate mode: disable the behavioral-precision gates and default the
        /// threshold to 0.70. Surfaces near-duplicate FAMILIES (locale classes,
        /// comparison operators, sync/async wrappers) for human review. Use the
        /// default strict path for behavioral-clone research runs.
        #[arg(long)]
        candidates: bool,
        /// MinHash signature length (LSH).
        #[arg(long, default_value_t = 128, value_parser = parse_minhash_k)]
        minhash_k: usize,
        /// LSH band count (more bands → catches lower-similarity candidates).
        #[arg(long, default_value_t = 32, value_parser = parse_bands)]
        bands: usize,
        /// Disable control-flow normalization (ablation).
        #[arg(long)]
        no_cfg_norm: bool,
        /// Enable dead-code / dead-assignment elimination (experimental).
        #[arg(long)]
        dce: bool,
        /// Disable sub-function block units (loops/ifs/try plus exact statement
        /// fragments). Blocks are ON by default — they lift recall and pool-precision
        /// by catching fragment-level clones; `--no-blocks` reverts to
        /// function/method/class units only.
        #[arg(long)]
        no_blocks: bool,
        /// Write the report JSON here instead of stdout.
        #[arg(long)]
        out: Option<PathBuf>,
        /// Print a human-readable summary instead of JSON.
        #[arg(long)]
        summary: bool,
        /// Emit predictions in the benchmark schema (needs --repos-root).
        #[arg(long)]
        bench_schema: bool,
        /// Root whose immediate subdirectories are repo ids (for path→repo
        /// mapping when emitting benchmark-schema predictions).
        #[arg(long)]
        repos_root: Option<PathBuf>,
        /// Write diagnostic dump (units.json, candidates.json, predictions.json)
        /// to this directory. Requires --repos-root.
        #[arg(long)]
        dump: Option<PathBuf>,
    },
    /// Find clone families. Default: syntax (CPD) + exact semantic. Passing --mode
    /// replaces that default with exactly the channels listed.
    Scan {
        /// Paths to source files or directories (recursively scanned).
        #[arg(required = true)]
        paths: Vec<PathBuf>,
        /// How many top families to show (`0` = all). [default: 30]
        #[arg(long)]
        top: Option<usize>,
        /// Only families with at least this many duplicated sites. [default: 2]
        #[arg(long)]
        min_members: Option<usize>,
        /// Hide families whose refactoring value is below this (noise floor on
        /// large repos). 0 shows every family. [default: 0]
        #[arg(long, value_parser = parse_min_value)]
        min_value: Option<f64>,
        /// Rank families by: `extractability` (how cleanly it folds into one helper —
        /// the default), `value` (raw duplicated volume), `sites` (most copies), or
        /// `hazard` (experimental divergent-edit propensity).
        #[arg(long)]
        sort: Option<SortKey>,
        /// Read defaults from this config file (else `nose.toml`/`.nose.toml`).
        #[arg(long, value_name = "FILE")]
        config: Option<PathBuf>,
        /// Detection channels to run. Omit for `syntax,semantic`. If present, this
        /// replaces the default; pass a comma-list or repeat it, e.g.
        /// `--mode syntax,near` or `--mode syntax --mode semantic`. Fuzzy channels
        /// take an optional acceptance threshold inline: `--mode near:0.8`.
        #[arg(
            long,
            value_delimiter = ',',
            num_args = 1,
            action = clap::ArgAction::Append,
            value_parser = parse_scan_mode,
            value_name = "MODE"
        )]
        mode: Vec<ScanMode>,
        /// Extra views (repeatable / comma-list): `diff` (each family as a unified
        /// diff of its two copies), `proposal` (an extraction skeleton), `hotspots`
        /// (directories ranked by duplicated lines). e.g. `--show diff,hotspots`.
        #[arg(long, value_delimiter = ',', value_name = "VIEW")]
        show: Vec<ShowView>,
        /// Cache per-file analysis under this directory. Re-runs reuse the cache for
        /// unchanged files (keyed by content hash), skipping parse/normalize/extract
        /// — much faster on repeated invocations (CI, pre-commit, iterating).
        #[arg(long, value_name = "DIR")]
        cache_dir: Option<PathBuf>,
        /// CI gate — exit non-zero when families are reported: `any` (any reported
        /// family fails) or `new` (only families new/changed vs `--baseline` fail;
        /// requires `--baseline`). e.g. `nose scan src --mode syntax --fail-on any`.
        #[arg(long, value_name = "WHAT")]
        fail_on: Option<FailOn>,
        /// Baseline file of already-accepted families. Families recorded here are
        /// hidden from the report and don't trip `--fail-on`, so a run flags only
        /// *new* duplication — the way to adopt on a codebase that already has clones.
        #[arg(long, value_name = "FILE")]
        baseline: Option<PathBuf>,
        /// Structured ignore file for intentionally suppressed families. Defaults
        /// to `nose.ignore.json` when that file exists.
        #[arg(long, value_name = "FILE")]
        ignore_file: Option<PathBuf>,
        /// Local semantic-pack v0 manifest file or directory to load. Repeatable;
        /// each path is an explicit opt-in and currently contributes provenance metadata only.
        #[arg(long = "semantic-pack", value_name = "FILE_OR_DIR")]
        semantic_pack: Vec<PathBuf>,
        /// Write the current families to the `--baseline` file (accept today's state)
        /// and exit, instead of reporting.
        #[arg(long, requires = "baseline")]
        write_baseline: bool,
        /// Output format.
        #[arg(long, default_value = "human")]
        format: ReportFormat,
        /// Skip files matching a gitignore-style glob (repeatable), e.g.
        /// `--exclude tests --exclude 'vendor/**' --exclude '**/*.generated.ts'`.
        /// (.gitignore is already respected automatically.)
        #[arg(long)]
        exclude: Vec<String>,
        /// Ignore units or syntax copy-paste runs smaller than this size, measured in
        /// IL tokens (the unit's node count). [default: 24]
        #[arg(long)]
        min_size: Option<usize>,
        /// Advanced: also require this many source lines (most users only need
        /// --min-size). [default: 5]
        #[arg(long, hide = true)]
        min_lines: Option<u32>,
    },
    /// Flag clone families changed inconsistently in a diff: a copy was edited but its
    /// sibling clones were not — a likely un-propagated change. Needs a git repository.
    /// e.g. `nose review --base origin/main` in CI, or `nose review` for local changes.
    Review {
        /// Paths to scan (recursively). Defaults to the current directory.
        paths: Vec<PathBuf>,
        /// Compare the working tree against this git ref (`origin/main` for a PR branch;
        /// the default `HEAD` reviews uncommitted local changes).
        #[arg(long, default_value = "HEAD")]
        base: String,
        /// Detection channels, like `scan`: `syntax`, `semantic`, `near[:T]` (comma-list
        /// or repeatable). Omit for `syntax,semantic`.
        #[arg(
            long,
            value_delimiter = ',',
            num_args = 1,
            action = clap::ArgAction::Append,
            value_parser = parse_scan_mode,
            value_name = "MODE"
        )]
        mode: Vec<ScanMode>,
        /// Ignore units smaller than this size, in IL tokens. [default: 24]
        #[arg(long)]
        min_size: Option<usize>,
        /// Advanced: also require this many source lines. [default: 5]
        #[arg(long, hide = true)]
        min_lines: Option<u32>,
        /// Skip paths matching a gitignore-style glob (repeatable).
        #[arg(long)]
        exclude: Vec<String>,
        /// Read defaults from this config file (else `nose.toml`/`.nose.toml`).
        #[arg(long, value_name = "FILE")]
        config: Option<PathBuf>,
        /// Structured ignore file for accepted divergences (same format as `scan`).
        /// Defaults to `nose.ignore.json` when it exists.
        #[arg(long, value_name = "FILE")]
        ignore_file: Option<PathBuf>,
        /// Output format.
        #[arg(long, default_value = "human")]
        format: ReportFormat,
        /// Show at most N findings (0 = all). [default: 30]
        #[arg(long)]
        top: Option<usize>,
        /// Exit non-zero if any family changed inconsistently (CI gate).
        #[arg(long)]
        fail: bool,
    },
    /// Recall-ceiling diagnostic: split gold recall across unit-extraction /
    /// candidate-generation stages. (Hidden — benchmark/research tooling.)
    #[command(hide = true)]
    Ceiling {
        #[arg(long)]
        gold: PathBuf,
        #[arg(long)]
        units: PathBuf,
        #[arg(long)]
        candidates: PathBuf,
    },
    /// Score predictions against a gold set (precision/recall/F1, macro, HN-FP).
    /// (Hidden — benchmark/research tooling.)
    #[command(hide = true)]
    Eval {
        /// Gold set JSON.
        #[arg(long)]
        gold: PathBuf,
        /// Predictions JSON (benchmark schema).
        #[arg(long)]
        predictions: PathBuf,
        /// Hard-negatives JSON (precision guard); optional.
        #[arg(long)]
        hard_negatives: Option<PathBuf>,
        /// Corpus JSON with dev/heldout split (for macro F1); optional.
        #[arg(long)]
        corpus: Option<PathBuf>,
    },
    /// Report IL lowering coverage (Raw ratio + top unhandled constructs).
    Stats {
        /// Paths to source files or directories (recursively scanned).
        #[arg(required = true)]
        paths: Vec<PathBuf>,
        /// How many top unhandled surface kinds to list.
        #[arg(long, default_value_t = 30)]
        top: usize,
        /// Emit JSON instead of a human-readable table.
        #[arg(long)]
        json: bool,
    },
    /// Dump per-unit detection features (value-graph / shape / return fingerprints)
    /// as JSON — the raw signal, before candidate generation or thresholding. Lets a
    /// convergence evaluator measure representation-convergence and behavioral-separation
    /// directly on the fingerprints, free of gate/threshold/cluster confounds.
    /// (Hidden — research.)
    #[command(hide = true)]
    Features {
        /// Paths to source files or directories (recursively scanned).
        #[arg(required = true)]
        paths: Vec<PathBuf>,
        /// Minimum unit line count.
        #[arg(long, default_value_t = 3)]
        min_lines: u32,
        /// Minimum unit token (IL node) count.
        #[arg(long, default_value_t = 8)]
        min_tokens: usize,
        /// Disable control-flow normalization (ablation).
        #[arg(long)]
        no_cfg_norm: bool,
        /// Disable sub-function block units (loops/ifs/try plus exact statement fragments).
        #[arg(long)]
        no_blocks: bool,
    },
    /// Soundness oracle: verify that value-fingerprint-equal units actually compute
    /// the same thing. Interprets each function on a battery of inputs and reports any
    /// fingerprint-equal pair whose behavior differs (a false merge — the cardinal sin
    /// of a clone detector). Also reports completeness. (Hidden — research.)
    #[command(hide = true)]
    Verify {
        /// Paths to source files or directories (recursively scanned).
        #[arg(required = true)]
        paths: Vec<PathBuf>,
        /// Disable control-flow normalization (ablation).
        #[arg(long)]
        no_cfg_norm: bool,
        /// Emit interpretable units as JSON `{units:[{file,start_line,end_line,
        /// behavior,trivial}], exclusions:{...}}` (the oracle's behavioral ground truth
        /// plus fail-closed exclusion counts) instead of the soundness/completeness report.
        /// Used by the value-add evaluator.
        #[arg(long)]
        json: bool,
        /// CI soundness gate: exit non-zero if the false-merge count EXCEEDS this budget.
        /// Use `--max-violations 0` on real code (the SOUND invariant); on the synthetic
        /// Type-4 corpus use the characterized baseline (its residual is oracle-fidelity
        /// artifacts — see experiments §A2), so a new real false merge from a future canon
        /// pushes the count over budget and fails the gate.
        #[arg(long)]
        max_violations: Option<usize>,
        /// Write the oracle's UNDER-MERGED groups (behavior-equal on the battery but
        /// fingerprint-split — candidate MISSED clones) to a JSON file, sorted by structural
        /// nearness. Feeds the detection campaign with oracle-discovered convergence leads
        /// (vj ≥ 0.7 are the strongest: structurally near AND behavior-equal).
        #[arg(long)]
        leads: Option<PathBuf>,
        /// Write a JSON census of the units the oracle could NOT interpret — exclusion
        /// reasons, the construct tags they carry, and how much fingerprint-merge mass
        /// is unverified per construct. The instrument for ranking oracle-coverage work
        /// by unverified-merge mass instead of by guess.
        #[arg(long)]
        exclusion_census: Option<PathBuf>,
    },
    /// EXPERIMENT (leaps 2+3): measure a behavioral-equivalence ACCEPTANCE gate — group
    /// interpretable units by their behavior on an input battery (two units are
    /// "accepted" iff identical on every input) and, against a Type-4 manifest, report
    /// the recall it recovers BEYOND exact-fingerprint matching and the hard-negative
    /// false merges it would introduce. `--battery wide` is the leap-3 bounded checker
    /// (a much larger structured input domain — does more checking kill the false merges?).
    BehavioralGate {
        /// Path to the generated Type-4 corpus `sources/` directory.
        #[arg(required = true)]
        paths: Vec<PathBuf>,
        /// The Type-4 manifest.json with labeled positive/negative pairs.
        #[arg(long)]
        manifest: PathBuf,
        /// Input battery: `standard` (leap 2) or `wide` (leap 3, larger domain).
        #[arg(long, default_value = "standard")]
        battery: BatteryKind,
    },
}

#[derive(Subcommand)]
enum SemanticPackCmd {
    /// Check local semantic-pack v0 manifests for structural conformance.
    Check {
        /// Semantic-pack manifest file or directory of direct `*.json` manifests.
        #[arg(required = true, value_name = "FILE_OR_DIR")]
        paths: Vec<PathBuf>,
        /// Output format.
        #[arg(long, default_value = "human")]
        format: semantic_pack::CheckFormat,
    },
}

#[derive(Clone, Copy, PartialEq, clap::ValueEnum)]
enum BatteryKind {
    Standard,
    Wide,
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum Format {
    Sexpr,
    Json,
}

#[derive(Clone, Copy, PartialEq, clap::ValueEnum)]
pub(crate) enum ReportFormat {
    /// Ranked, human-readable terminal report.
    Human,
    /// Machine-readable JSON report with a versioned top-level schema.
    Json,
    /// Markdown report (for PRs / issues / docs).
    Markdown,
    /// SARIF 2.1.0 (GitHub code-scanning / PR annotations).
    Sarif,
}

/// One `--mode` channel. Fuzzy modes carry their acceptance threshold inline
/// (`near:0.8` / `abstraction:0.5`), so there is no separate `--threshold` flag
/// to mis-combine.
#[derive(Clone, Copy, PartialEq, serde::Deserialize)]
#[serde(try_from = "String")]
enum ScanMode {
    /// CPD-style syntax copy-paste runs (the Type-1/2 floor).
    Syntax,
    /// Exact value-fingerprint Type-4 semantic clones.
    Semantic,
    /// Fuzzy Type-3 near-duplicate candidates, with an optional acceptance threshold.
    Near(Option<f64>),
    /// Experimental weak refactoring-template witnesses over near candidates.
    Abstraction(Option<f64>),
}

impl std::str::FromStr for ScanMode {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, String> {
        let s = s.trim();
        match s {
            "syntax" => Ok(ScanMode::Syntax),
            "semantic" => Ok(ScanMode::Semantic),
            "near" => Ok(ScanMode::Near(None)),
            "abstraction" => Ok(ScanMode::Abstraction(None)),
            _ => {
                if let Some(t) = s.strip_prefix("near:") {
                    let v: f64 = t
                        .parse()
                        .map_err(|_| format!("invalid near threshold in `{s}`"))?;
                    if !(0.0..=1.0).contains(&v) {
                        return Err(format!("near threshold must be in [0,1], got {v}"));
                    }
                    Ok(ScanMode::Near(Some(v)))
                } else if let Some(t) = s.strip_prefix("abstraction:") {
                    let v: f64 = t
                        .parse()
                        .map_err(|_| format!("invalid abstraction threshold in `{s}`"))?;
                    if !(0.0..=1.0).contains(&v) {
                        return Err(format!("abstraction threshold must be in [0,1], got {v}"));
                    }
                    Ok(ScanMode::Abstraction(Some(v)))
                } else {
                    Err(format!(
                        "unknown mode `{s}` (expected syntax, semantic, near, near:T, abstraction, or abstraction:T)"
                    ))
                }
            }
        }
    }
}

impl TryFrom<String> for ScanMode {
    type Error = String;
    fn try_from(s: String) -> std::result::Result<Self, String> {
        s.parse()
    }
}

/// Borrow a slice of owned `PathBuf`s as `&Path` references — the form the detection entry
/// points take. Used by every scan/refactor subcommand that holds its input paths as a
/// `Vec<PathBuf>`.
fn paths_as_refs(paths: &[PathBuf]) -> Vec<&std::path::Path> {
    paths.iter().map(|p| p.as_path()).collect()
}

fn parse_scan_mode(s: &str) -> std::result::Result<ScanMode, String> {
    s.parse()
}

fn parse_positive_usize(s: &str, label: &str) -> std::result::Result<usize, String> {
    let value = s
        .parse::<usize>()
        .map_err(|_| format!("{label} must be positive"))?;
    if value == 0 {
        Err(format!("{label} must be positive"))
    } else {
        Ok(value)
    }
}

fn parse_minhash_k(s: &str) -> std::result::Result<usize, String> {
    parse_positive_usize(s, "minhash-k")
}

fn parse_bands(s: &str) -> std::result::Result<usize, String> {
    parse_positive_usize(s, "bands")
}

const THRESHOLD_ERROR: &str = "threshold must be a finite number in [0,1]";

fn valid_threshold(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

fn parse_threshold(s: &str) -> std::result::Result<f64, String> {
    let value = s.parse::<f64>().map_err(|_| THRESHOLD_ERROR.to_string())?;
    if valid_threshold(value) {
        Ok(value)
    } else {
        Err(THRESHOLD_ERROR.to_string())
    }
}

const MIN_VALUE_ERROR: &str = "min-value must be a finite non-negative number";

fn valid_min_value(value: f64) -> bool {
    value.is_finite() && value >= 0.0
}

fn parse_min_value(s: &str) -> std::result::Result<f64, String> {
    let value = s.parse::<f64>().map_err(|_| MIN_VALUE_ERROR.to_string())?;
    if valid_min_value(value) {
        Ok(value)
    } else {
        Err(MIN_VALUE_ERROR.to_string())
    }
}

fn validate_min_value(value: f64) -> Result<f64> {
    if valid_min_value(value) {
        Ok(value)
    } else {
        anyhow::bail!("{MIN_VALUE_ERROR}")
    }
}

#[derive(Clone, Copy)]
struct ScanChannels {
    syntax: bool,
    semantic: bool,
    near: bool,
    abstraction: bool,
    /// The shared fuzzy acceptance threshold, if one was given in the mode spec.
    threshold: Option<f64>,
}

impl ScanChannels {
    fn resolve(cli: Vec<ScanMode>, cfg: Vec<ScanMode>) -> Result<Self> {
        let selected = if !cli.is_empty() {
            cli
        } else if !cfg.is_empty() {
            cfg
        } else {
            vec![ScanMode::Syntax, ScanMode::Semantic]
        };
        let mut channels = ScanChannels {
            syntax: false,
            semantic: false,
            near: false,
            abstraction: false,
            threshold: None,
        };
        for mode in selected {
            match mode {
                ScanMode::Syntax => channels.syntax = true,
                ScanMode::Semantic => channels.semantic = true,
                ScanMode::Near(t) => {
                    channels.near = true;
                    channels.set_threshold("near", t)?;
                }
                ScanMode::Abstraction(t) => {
                    channels.abstraction = true;
                    channels.set_threshold("abstraction", t)?;
                }
            }
        }
        if !channels.syntax && !channels.semantic && !channels.near && !channels.abstraction {
            anyhow::bail!(
                "--mode must include at least one of syntax, semantic, near, or abstraction"
            );
        }
        Ok(channels)
    }

    fn set_threshold(&mut self, mode: &'static str, threshold: Option<f64>) -> Result<()> {
        let Some(next) = threshold else {
            return Ok(());
        };
        if let Some(prev) = self.threshold {
            if (prev - next).abs() > f64::EPSILON {
                anyhow::bail!(
                    "conflicting --mode thresholds: near and abstraction share one acceptance threshold; got {prev} and {mode}:{next}"
                );
            }
        }
        self.threshold = Some(next);
        Ok(())
    }

    fn structural(self) -> bool {
        self.semantic || self.near || self.abstraction
    }

    fn report_label(self, count: usize) -> &'static str {
        let singular = count == 1;
        match (
            self.syntax,
            self.semantic,
            self.near,
            self.abstraction,
            singular,
        ) {
            (true, false, false, false, true) => "syntax clone family",
            (true, false, false, false, false) => "syntax clone families",
            (false, true, false, false, true) => "semantic clone family",
            (false, true, false, false, false) => "semantic clone families",
            (false, false, true, false, true) => "near-duplicate family",
            (false, false, true, false, false) => "near-duplicate families",
            (false, false, false, true, true) => "abstraction candidate family",
            (false, false, false, true, false) => "abstraction candidate families",
            (_, _, _, _, true) => "clone family",
            (_, _, _, _, false) => "clone families",
        }
    }

    fn markdown_title(self) -> &'static str {
        match (self.syntax, self.semantic, self.near, self.abstraction) {
            (true, false, false, false) => "Syntax Clone Families",
            (false, true, false, false) => "Semantic Clone Families",
            (false, false, true, false) => "Near-Duplicate Families",
            (false, false, false, true) => "Abstraction Candidate Families",
            _ => "Clone Families",
        }
    }

    fn threshold(self) -> f64 {
        if self.near || self.abstraction {
            self.threshold.unwrap_or(if self.abstraction && !self.near {
                0.50
            } else {
                0.70
            })
        } else {
            1.0
        }
    }

    fn abstraction_only(self) -> bool {
        self.abstraction && !self.syntax && !self.semantic && !self.near
    }
}

/// How to rank families — what "most worth your attention first" means.
#[derive(Clone, Copy, PartialEq, clap::ValueEnum, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
enum SortKey {
    /// How cleanly it extracts: invariant (shared) lines × copies × spread, penalized
    /// by the number of parameters the helper would need. Surfaces the duplication you
    /// can actually fold into one helper, not the biggest block that merely *looks*
    /// similar (a *fixability* axis). The default.
    Extractability,
    /// Raw duplicated volume: removable lines × similarity × spread. The most
    /// *code* you'd delete, even if the copies diverge a lot (more manual work).
    Value,
    /// Most copies first — the most-repeated patterns.
    Sites,
    /// Divergent-edit *hazard*: how likely a family is to be edited inconsistently
    /// (one copy fixed, the siblings missed) and cause a bug. A severity axis, not a
    /// fixability one — surfaces copies that share little text yet are behaviorally the
    /// same (the invisible siblings a developer won't update). Calibrated against mined
    /// history as a *divergence-propensity* signal — it is NOT yet a validated *harm*
    /// ranker (an LLM-gold audit found ~chance harm discrimination); see
    /// `docs/hazard-ranking.md`. Opt-in via `--sort hazard`.
    Hazard,
}

impl SortKey {
    fn json_name(self) -> &'static str {
        match self {
            SortKey::Extractability => "extractability",
            SortKey::Value => "value",
            SortKey::Sites => "sites",
            SortKey::Hazard => "hazard",
        }
    }

    /// The ranking score for `f` under this key (higher = ranked first).
    fn score(self, f: &nose_detect::RefactorFamily) -> f64 {
        match self {
            SortKey::Extractability => f.extractability(),
            SortKey::Value => f.value,
            SortKey::Sites => f.members as f64,
            SortKey::Hazard => f.hazard(),
        }
    }
}

/// Plain-language name of the active ranking, shown once in the header (the per-family
/// lines don't repeat a cryptic score — the order already conveys it).
fn sort_name(s: SortKey) -> &'static str {
    match s {
        SortKey::Extractability => "extractability (cleanest to fold into one helper)",
        SortKey::Value => "raw duplicated volume",
        SortKey::Sites => "number of copies",
        SortKey::Hazard => "divergent-edit hazard (most likely to be edited inconsistently)",
    }
}

/// CI fail-gate policy, selected with `--fail-on`.
#[derive(Clone, Copy, PartialEq, Eq, clap::ValueEnum, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
enum FailOn {
    /// Any reported family (after filters) fails the run.
    Any,
    /// Only families new or changed vs `--baseline` fail. Requires `--baseline`.
    New,
}

/// Extra per-report views (human/markdown), selected with `--show`.
#[derive(Clone, Copy, PartialEq, Eq, clap::ValueEnum, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
enum ShowView {
    /// Each family inline as a unified diff between its two representative copies.
    Diff,
    /// An extraction skeleton per family: the shared structure with varying spots as ⟨param N⟩.
    Proposal,
    /// After the report, directories/modules ranked by total duplicated lines.
    Hotspots,
}

/// What `scan` actually looked at: the file count and per-language breakdown, shown as
/// a one-line header. A repo where `.gitignore`/`--exclude` pruned vendored deps scans
/// far fewer files than sit on disk; surfacing the count (and which languages) makes
/// that scope visible instead of a silent gap the reader has to guess at. (The *ignored*
/// count is deliberately not shown — computing it means descending into the very trees
/// `.gitignore` exists to skip, slow on exactly the big repos where it matters.)
struct ScanScope {
    files: usize,
    /// `(language name, file count)`, largest first.
    langs: Vec<(&'static str, usize)>,
}

impl ScanScope {
    fn from_corpus(corpus: &Corpus) -> Self {
        let mut counts: std::collections::HashMap<&'static str, usize> =
            std::collections::HashMap::new();
        for f in &corpus.files {
            *counts.entry(f.meta.lang.name()).or_insert(0) += 1;
        }
        let mut langs: Vec<(&'static str, usize)> = counts.into_iter().collect();
        // Largest language first; name as a stable tie-break for deterministic output.
        langs.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        ScanScope {
            files: corpus.files.len(),
            langs,
        }
    }

    /// `scanned 1113 files · typescript 900 · tsx 213` (languages omitted when unknown).
    fn summary(&self) -> String {
        let unit = if self.files == 1 { "file" } else { "files" };
        if self.langs.is_empty() {
            return format!("scanned {} {unit}", self.files);
        }
        let langs = self
            .langs
            .iter()
            .map(|(l, n)| format!("{l} {n}"))
            .collect::<Vec<_>>()
            .join(" · ");
        format!("scanned {} {unit} · {langs}", self.files)
    }
}

const SCAN_JSON_SCHEMA_VERSION: u32 = 1;
const CAPABILITIES_SCHEMA_VERSION: u32 = 1;

#[derive(serde::Serialize)]
struct CapabilitiesReport {
    schema_version: u32,
    tool: CapabilitiesTool,
    platform: CapabilitiesPlatform,
    interfaces: CapabilitiesInterfaces,
    commands: CapabilitiesCommands,
    schemas: CapabilitiesSchemas,
    scan: CapabilitiesScan,
    semantic_packs: CapabilitiesSemanticPacks,
    il: CapabilitiesIl,
    stats: CapabilitiesStats,
}

#[derive(serde::Serialize)]
struct CapabilitiesTool {
    name: &'static str,
    version: &'static str,
}

#[derive(serde::Serialize)]
struct CapabilitiesPlatform {
    os: &'static str,
    arch: &'static str,
    family: &'static str,
}

#[derive(serde::Serialize)]
struct CapabilitiesInterfaces {
    capabilities_json: bool,
    version_json: bool,
    doctor_json: bool,
}

#[derive(serde::Serialize)]
struct CapabilitiesCommands {
    stable: Vec<&'static str>,
}

#[derive(serde::Serialize)]
struct CapabilitiesSchemas {
    capabilities: Vec<u32>,
    scan_json: Vec<u32>,
    semantic_packs: Vec<&'static str>,
    semantic_pack_conformance: Vec<u32>,
}

#[derive(serde::Serialize)]
struct CapabilitiesScan {
    modes: Vec<&'static str>,
    default_modes: Vec<&'static str>,
    output_formats: Vec<&'static str>,
    sort_keys: Vec<&'static str>,
    config_keys: Vec<&'static str>,
    capabilities: std::collections::BTreeMap<&'static str, bool>,
}

#[derive(serde::Serialize)]
struct CapabilitiesSemanticPacks {
    api_versions: Vec<&'static str>,
    loading: Vec<&'static str>,
    conformance: Vec<&'static str>,
    conformance_output_formats: Vec<&'static str>,
    trust: Vec<&'static str>,
    external_packs_enabled_by_default: bool,
    external_pack_influence: &'static str,
}

#[derive(serde::Serialize)]
struct CapabilitiesIl {
    output_formats: Vec<&'static str>,
    normalized: bool,
    cfg_norm_toggle: bool,
}

#[derive(serde::Serialize)]
struct CapabilitiesStats {
    output_formats: Vec<&'static str>,
}

impl CapabilitiesReport {
    fn current() -> Self {
        CapabilitiesReport {
            schema_version: CAPABILITIES_SCHEMA_VERSION,
            tool: CapabilitiesTool {
                name: "nose",
                version: env!("CARGO_PKG_VERSION"),
            },
            platform: CapabilitiesPlatform {
                os: std::env::consts::OS,
                arch: std::env::consts::ARCH,
                family: std::env::consts::FAMILY,
            },
            interfaces: CapabilitiesInterfaces {
                capabilities_json: true,
                version_json: false,
                doctor_json: false,
            },
            commands: CapabilitiesCommands {
                stable: vec![
                    "capabilities",
                    "il",
                    "review",
                    "scan",
                    "semantic-pack",
                    "stats",
                ],
            },
            schemas: CapabilitiesSchemas {
                capabilities: vec![CAPABILITIES_SCHEMA_VERSION],
                scan_json: vec![SCAN_JSON_SCHEMA_VERSION],
                semantic_packs: vec![nose_semantics::SEMANTIC_PACK_API_VERSION],
                semantic_pack_conformance: vec![semantic_pack::CONFORMANCE_SCHEMA_VERSION],
            },
            scan: CapabilitiesScan {
                modes: vec!["syntax", "semantic", "near"],
                default_modes: vec!["syntax", "semantic"],
                output_formats: vec!["human", "json", "markdown", "sarif"],
                sort_keys: vec!["extractability", "value", "sites", "hazard"],
                config_keys: vec![
                    "exclude",
                    "ignore-file",
                    "min-lines",
                    "min-members",
                    "min-size",
                    "min-value",
                    "mode",
                    "semantic-packs",
                    "sort",
                    "top",
                ],
                capabilities: scan_capability_flags(),
            },
            semantic_packs: CapabilitiesSemanticPacks {
                api_versions: vec![nose_semantics::SEMANTIC_PACK_API_VERSION],
                loading: vec![
                    "compiled-first-party",
                    "local-manifest-file",
                    "local-manifest-directory",
                ],
                conformance: vec!["local-manifest-file", "local-manifest-directory"],
                conformance_output_formats: vec!["human", "json"],
                trust: vec![
                    "default-first-party",
                    "first-party-optional",
                    "external-opt-in",
                ],
                external_packs_enabled_by_default: false,
                external_pack_influence: "metadata-only",
            },
            il: CapabilitiesIl {
                output_formats: vec!["sexpr", "json"],
                normalized: true,
                cfg_norm_toggle: true,
            },
            stats: CapabilitiesStats {
                output_formats: vec!["human", "json"],
            },
        }
    }
}

fn scan_capability_flags() -> std::collections::BTreeMap<&'static str, bool> {
    [
        ("baseline", true),
        ("baseline_changed_detection", true),
        ("cache", true),
        ("ci_fail_gate", true),
        ("diff", true),
        ("hotspots", true),
        ("inline_suppression", true),
        ("proposal", true),
        ("semantic_pack_loading", true),
        ("structured_ignores", true),
    ]
    .into_iter()
    .collect()
}

#[derive(serde::Serialize)]
struct ScanJsonReport<'a> {
    schema_version: u32,
    tool_version: &'static str,
    scope: ScanJsonScope<'a>,
    semantic_packs: Vec<ScanJsonSemanticPack<'a>>,
    ranking: ScanJsonRanking,
    #[serde(skip_serializing_if = "Option::is_none")]
    baseline: Option<&'a BaselineSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ignore: Option<ignores::IgnoreSummary<'a>>,
    families: Vec<ScanJsonFamily<'a>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    ignored_families: Vec<ScanJsonIgnoredFamily<'a>>,
}

#[derive(serde::Serialize)]
struct ScanJsonScope<'a> {
    files: usize,
    languages: Vec<ScanJsonLanguage<'a>>,
}

#[derive(serde::Serialize)]
struct ScanJsonLanguage<'a> {
    language: &'a str,
    files: usize,
}

#[derive(serde::Serialize)]
struct ScanJsonSemanticPack<'a> {
    id: &'a str,
    hash: String,
    kind: &'static str,
    version: &'a str,
    display_name: &'a str,
    trust: &'static str,
    enabled_by_default: bool,
    source: &'static str,
    influence: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    provider: &'a str,
    repository: &'a str,
    license: &'a str,
    supported_languages: &'a [String],
    counts: ScanJsonSemanticPackCounts,
}

#[derive(serde::Serialize)]
struct ScanJsonSemanticPackCounts {
    evidence_producers: usize,
    contracts: usize,
    value_laws: usize,
    positive_fixtures: usize,
    hard_negatives: usize,
}

#[derive(serde::Serialize)]
struct ScanJsonRanking {
    sort: &'static str,
    total_families: usize,
    shown_families: usize,
    limit: Option<usize>,
    surface_counts: ScanJsonSurfaceCounts,
}

#[derive(Default, serde::Serialize)]
struct ScanJsonSurfaceCounts {
    #[serde(rename = "default")]
    default_count: usize,
    review: usize,
    hidden: usize,
    debug: usize,
    /// Families whose every location sits in a generated-header source — the
    /// ones the human report omits from default output (#224).
    generated: usize,
    fragments: ScanJsonFragmentSurfaceCounts,
}

#[derive(Default, serde::Serialize)]
struct ScanJsonFragmentSurfaceCounts {
    total: usize,
    #[serde(rename = "default")]
    default_count: usize,
    review: usize,
    hidden: usize,
    debug: usize,
}

impl ScanJsonSurfaceCounts {
    fn from_families(
        families: &[nose_detect::RefactorFamily],
        generated_sources: &std::collections::HashSet<String>,
    ) -> Self {
        let mut counts = Self::default();
        for family in families {
            let surface = effective_surface(family, generated_sources);
            counts.bump(surface);
            if family.locations.iter().any(|loc| loc.is_fragment) {
                counts.fragments.total += 1;
                counts.fragments.bump(surface);
            }
        }
        counts
    }

    fn bump(&mut self, surface: &str) {
        match surface {
            "default" => self.default_count += 1,
            "review" => self.review += 1,
            "hidden" => self.hidden += 1,
            "debug" => self.debug += 1,
            "generated" => self.generated += 1,
            _ => self.debug += 1,
        }
    }
}

impl ScanJsonFragmentSurfaceCounts {
    fn bump(&mut self, surface: &str) {
        match surface {
            "default" => self.default_count += 1,
            "review" => self.review += 1,
            "hidden" => self.hidden += 1,
            "debug" => self.debug += 1,
            _ => self.debug += 1,
        }
    }
}

#[derive(serde::Serialize)]
struct ScanJsonFamily<'a> {
    family_id: String,
    #[serde(flatten)]
    family: &'a nose_detect::RefactorFamily,
    recommended_surface: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    baseline_status: Option<&'static str>,
}

#[derive(serde::Serialize)]
struct ScanJsonIgnoredFamily<'a> {
    family_id: String,
    #[serde(flatten)]
    family: &'a nose_detect::RefactorFamily,
    recommended_surface: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    baseline_status: Option<&'static str>,
    ignore: &'a ignores::IgnoreMatch,
}

struct ScanJsonInput<'a> {
    scope: &'a ScanScope,
    sort: SortKey,
    top: usize,
    families: &'a [nose_detect::RefactorFamily],
    shown: &'a [&'a nose_detect::RefactorFamily],
    baseline: Option<&'a BaselineComparison>,
    ignore_set: Option<&'a ignores::IgnoreSet>,
    ignored_families: &'a [IgnoredFamily],
    semantic_packs: &'a nose_semantics::SemanticPackSet,
    generated_sources: &'a std::collections::HashSet<String>,
}

impl<'a> ScanJsonReport<'a> {
    fn new(input: ScanJsonInput<'a>) -> Self {
        let statuses = input.baseline.map(|b| &b.statuses);
        ScanJsonReport {
            schema_version: SCAN_JSON_SCHEMA_VERSION,
            tool_version: env!("CARGO_PKG_VERSION"),
            scope: ScanJsonScope {
                files: input.scope.files,
                languages: input
                    .scope
                    .langs
                    .iter()
                    .map(|(language, files)| ScanJsonLanguage {
                        language,
                        files: *files,
                    })
                    .collect(),
            },
            semantic_packs: input
                .semantic_packs
                .packs()
                .iter()
                .map(|pack| ScanJsonSemanticPack {
                    id: &pack.id,
                    hash: pack.hash_hex(),
                    kind: pack.kind.as_str(),
                    version: &pack.version,
                    display_name: &pack.display_name,
                    trust: pack.trust.as_manifest_str(),
                    enabled_by_default: pack.enabled_by_default,
                    source: pack.source.as_str(),
                    influence: pack.influence.as_str(),
                    path: pack
                        .manifest_path
                        .as_ref()
                        .map(|path| path.display().to_string()),
                    provider: &pack.provider,
                    repository: &pack.repository,
                    license: &pack.license,
                    supported_languages: &pack.supported_languages,
                    counts: ScanJsonSemanticPackCounts {
                        evidence_producers: pack.counts.evidence_producers,
                        contracts: pack.counts.contracts,
                        value_laws: pack.counts.value_laws,
                        positive_fixtures: pack.counts.positive_fixtures,
                        hard_negatives: pack.counts.hard_negatives,
                    },
                })
                .collect(),
            ranking: ScanJsonRanking {
                sort: input.sort.json_name(),
                total_families: input.families.len(),
                shown_families: input.shown.len(),
                limit: (input.top != 0).then_some(input.top),
                surface_counts: ScanJsonSurfaceCounts::from_families(
                    input.families,
                    input.generated_sources,
                ),
            },
            baseline: input.baseline.map(|b| &b.summary),
            ignore: input
                .ignore_set
                .map(|set| set.summary(input.ignored_families.len())),
            families: input
                .shown
                .iter()
                .map(|family| ScanJsonFamily {
                    family_id: baseline::family_id(family),
                    family,
                    recommended_surface: effective_surface(family, input.generated_sources),
                    baseline_status: statuses
                        .and_then(|s| s.get(&baseline::family_key(family)))
                        .map(BaselineStatus::as_str),
                })
                .collect(),
            ignored_families: input
                .ignored_families
                .iter()
                .map(|ignored| ScanJsonIgnoredFamily {
                    family_id: baseline::family_id(&ignored.family),
                    family: &ignored.family,
                    recommended_surface: ignored.family.recommended_surface(),
                    baseline_status: statuses
                        .and_then(|s| s.get(&baseline::family_key(&ignored.family)))
                        .map(BaselineStatus::as_str),
                    ignore: &ignored.ignore,
                })
                .collect(),
        }
    }
}

struct IgnoredFamily {
    family: nose_detect::RefactorFamily,
    ignore: ignores::IgnoreMatch,
}

#[derive(serde::Serialize)]
struct BaselineSummary {
    path: String,
    mode: &'static str,
    baseline_families: usize,
    new_families: usize,
    changed_families: usize,
    unchanged_families: usize,
    resolved_families: usize,
}

impl BaselineSummary {
    fn line(&self) -> String {
        format!(
            "baseline: {} new · {} changed · {} unchanged · {} resolved",
            self.new_families,
            self.changed_families,
            self.unchanged_families,
            self.resolved_families
        )
    }
}

#[derive(Clone, Copy)]
enum BaselineStatus {
    New,
    Changed,
}

impl BaselineStatus {
    fn as_str(&self) -> &'static str {
        match self {
            BaselineStatus::New => "new",
            BaselineStatus::Changed => "changed",
        }
    }
}

struct BaselineComparison {
    summary: BaselineSummary,
    statuses: std::collections::HashMap<u64, BaselineStatus>,
}

impl BaselineComparison {
    fn new(
        path: &std::path::Path,
        baseline: &baseline::Baseline,
        families: &[nose_detect::RefactorFamily],
    ) -> Self {
        let current_keys: std::collections::HashSet<u64> =
            families.iter().map(baseline::family_key).collect();
        let unchanged_families = baseline.keys.intersection(&current_keys).count();

        let mut changed_current = std::collections::HashSet::new();
        let mut changed_baseline = std::collections::HashSet::new();
        for family in families {
            let key = baseline::family_key(family);
            if baseline.keys.contains(&key) {
                continue;
            }
            let current_members = baseline::member_keys(family);
            if baseline
                .entries
                .iter()
                .filter(|entry| !current_keys.contains(&entry.key))
                .any(|entry| {
                    !entry.members.is_empty()
                        && baseline::member_sets_overlap(&entry.members, &current_members)
                })
            {
                changed_current.insert(key);
                for entry in baseline
                    .entries
                    .iter()
                    .filter(|entry| !current_keys.contains(&entry.key))
                {
                    if !entry.members.is_empty()
                        && baseline::member_sets_overlap(&entry.members, &current_members)
                    {
                        changed_baseline.insert(entry.key);
                    }
                }
            }
        }

        let mut statuses = std::collections::HashMap::new();
        for family in families {
            let key = baseline::family_key(family);
            if baseline.keys.contains(&key) {
                continue;
            }
            let status = if changed_current.contains(&key) {
                BaselineStatus::Changed
            } else {
                BaselineStatus::New
            };
            statuses.insert(key, status);
        }

        let resolved_families = baseline
            .keys
            .iter()
            .filter(|key| !current_keys.contains(key) && !changed_baseline.contains(key))
            .count();
        let changed_families = changed_current.len();
        let new_families = statuses.len().saturating_sub(changed_families);
        BaselineComparison {
            summary: BaselineSummary {
                path: path.display().to_string(),
                mode: "new-only",
                baseline_families: baseline.keys.len(),
                new_families,
                changed_families,
                unchanged_families,
                resolved_families,
            },
            statuses,
        }
    }
}

/// The line count of the family's representative copy — the denominator for "`N of M`
/// shared". It's the *first* (largest) site's own span, not the family-wide `mean_lines`:
/// the two largest members are what got diffed, so a family whose biggest copies run
/// longer than average must not read as "47/43 shared". Floored at `shared_lines` so the
/// fraction is never inverted.
fn representative_lines(f: &nose_detect::RefactorFamily) -> u32 {
    f.locations
        .first()
        .map(|l| l.end_line.saturating_sub(l.start_line) + 1)
        .unwrap_or(f.mean_lines)
        .max(f.shared_lines)
}

/// One plain-language line describing a family: how many copies, how much is actually
/// shared vs varies, how many lines you'd remove, and where the duplication lives. No
/// internal ranking numbers — those only order the list, they're not for the reader.
fn family_summary(f: &nose_detect::RefactorFamily) -> String {
    let detail = if f.languages > 1 {
        format!(
            "same logic in {} languages ({})",
            f.languages,
            family_langs(f)
        )
    } else {
        let rep = representative_lines(f);
        match f.params {
            0 => format!("{} of {rep} lines identical", f.shared_lines),
            1 => format!("{} of {rep} lines shared, 1 spot differs", f.shared_lines),
            p => format!("{} of {rep} lines shared, {p} spots differ", f.shared_lines),
        }
    };
    let scope = match f.scope {
        "test" => "  · in test code",
        "mixed" => "  · same code in tests and prod",
        _ => "",
    };
    format!(
        "{} copies · {detail} · ~{} lines removable{scope}",
        f.members,
        removable_lines(f)
    )
}

fn abstraction_witness_summary(witness: &nose_detect::AbstractionWitness) -> String {
    let caveats = if witness.caveats.is_empty() {
        "no caveats".to_string()
    } else {
        format!("caveats: {}", witness.caveats.join(", "))
    };
    let holes = witness
        .holes
        .iter()
        .map(|hole| format!("{} {} {}->{}", hole.kind, hole.role, hole.left, hole.right))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{}:{} · {} · {} · {}",
        witness.basis, witness.members_checked, witness.reason_code, holes, caveats
    )
}

/// Lines you'd actually delete by extracting one shared copy. For same-language
/// families this is the *invariant* lines folded out of each redundant copy
/// (`(copies−1) × shared_lines`) — not `(copies−1) × mean_lines`, which counts the
/// varying parts that *survive* extraction and so overstates the win (e.g. four
/// 38-line copies sharing only 15 lines remove ~45, not ~114). Cross-language families
/// have no shared-line count, so they keep the span-based estimate.
fn removable_lines(f: &nose_detect::RefactorFamily) -> u32 {
    let copies = f.members.saturating_sub(1) as u32;
    if f.languages == 1 && f.shared_lines > 0 {
        copies * f.shared_lines
    } else {
        f.dup_lines
    }
}

/// The honest similarity cell. A bare `sim 1.00` misleads — two same-language copies
/// can be structurally identical yet share *no* literal lines (a language idiom, or two
/// unrelated type literals with the same shape). For same-language families always
/// report the real shared-line count `18/42 shared · 2p` — 18 invariant lines of the 42
/// in the largest copy, even when it's `0/42` (nothing to extract). Only cross-language
/// families, which have no shared *source* lines to diff, fall back to structural `sim`.
fn similarity_cell(f: &nose_detect::RefactorFamily) -> String {
    if f.languages > 1 {
        return format!("sim {:.2}", f.mean_score);
    }
    let rep = representative_lines(f);
    format!("{}/{} shared · {}p", f.shared_lines, rep, f.params)
}

/// Stack size for the worker pool and the main worker thread. Lowering/normalization
/// walk the syntax tree recursively, so a pathologically deep file (minified bundle,
/// generated code) can need a deep stack — far more than the default ~2 MB (rayon
/// worker) or ~8 MB (main). Sized generously so nose never crashes on real repos.
/// Virtual only; pages commit lazily. See `deeply_nested_file_does_not_overflow`.
const STACK_SIZE: usize = 1024 * 1024 * 1024;

fn main() -> Result<()> {
    install_broken_pipe_guard();
    // rayon executes tasks both on its pool workers AND inline on the calling thread,
    // so enlarge the workers' stacks here and run the command body on a big-stack
    // thread below — otherwise a deep file lowered inline on a normal-stack thread
    // still overflows.
    let _ = rayon::ThreadPoolBuilder::new()
        .stack_size(STACK_SIZE)
        .build_global();
    std::thread::Builder::new()
        .stack_size(STACK_SIZE)
        .spawn(run)
        .expect("spawn worker thread")
        .join()
        .expect("worker thread panicked")
}

/// When a reader closes the pipe early — `nose scan … | head`, quitting a pager —
/// the next write to stdout fails with `BrokenPipe`, and `println!` turns that into
/// a panic (the ugly `failed printing to stdout` message). The Unix convention for a
/// filter is to stop quietly instead. The textbook fix is to reset the `SIGPIPE`
/// disposition to `SIG_DFL`, but that needs `unsafe` and this crate is `unsafe`-free
/// (`unsafe_code = "forbid"`), so we install a panic hook that recognizes the
/// broken-pipe panic and exits 0 without a backtrace, while leaving every other panic
/// to the normal hook (and the big-stack join handling above).
fn install_broken_pipe_guard() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if is_broken_pipe_panic(info) {
            std::process::exit(0);
        }
        default_hook(info);
    }));
}

/// True for the panic `println!`/`writeln!` raise when stdout (or stderr) is a broken
/// pipe. The payload is a `String` like `failed printing to stdout: Broken pipe
/// (os error 32)`; we match both the textual kind and the numeric `EPIPE` (32 on
/// Linux and macOS) so a localized `strerror` message is still caught.
fn is_broken_pipe_panic(info: &std::panic::PanicHookInfo<'_>) -> bool {
    let payload = info.payload();
    let msg = payload
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| payload.downcast_ref::<&str>().copied());
    matches!(msg, Some(m) if m.contains("Broken pipe") || m.contains("os error 32"))
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Il {
            path,
            format,
            normalized,
            no_cfg_norm,
        } => cmd_il(path, format, normalized, no_cfg_norm),
        Cmd::Capabilities => cmd_capabilities(),
        Cmd::SemanticPack { cmd } => match cmd {
            SemanticPackCmd::Check { paths, format } => semantic_pack::cmd_check(paths, format),
        },
        cmd @ Cmd::Detect { .. } => run_detect_cmd(cmd),
        Cmd::Eval {
            gold,
            predictions,
            hard_negatives,
            corpus,
        } => cmd_eval(gold, predictions, hard_negatives, corpus),
        cmd @ Cmd::Scan { .. } => run_scan_cmd(cmd),
        cmd @ Cmd::Review { .. } => run_review_cmd(cmd),
        Cmd::Ceiling {
            gold,
            units,
            candidates,
        } => cmd_ceiling(gold, units, candidates),
        Cmd::Stats { paths, top, json } => cmd_stats(paths, top, json),
        Cmd::Features {
            paths,
            min_lines,
            min_tokens,
            no_cfg_norm,
            no_blocks,
        } => cmd_features(paths, min_lines, min_tokens, no_cfg_norm, no_blocks),
        Cmd::Verify {
            paths,
            no_cfg_norm,
            json,
            max_violations,
            leads,
            exclusion_census,
        } => cmd_verify(
            paths,
            no_cfg_norm,
            json,
            max_violations,
            leads,
            exclusion_census,
        ),
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

fn run_scan_cmd(cmd: Cmd) -> Result<()> {
    let Cmd::Scan {
        paths,
        top,
        min_members,
        min_value,
        sort,
        config,
        mode,
        show,
        cache_dir,
        fail_on,
        baseline,
        ignore_file,
        semantic_pack,
        write_baseline,
        format,
        exclude,
        min_size,
        min_lines,
    } = cmd
    else {
        unreachable!("run_scan_cmd requires Cmd::Scan")
    };
    cmd_scan(ScanArgs {
        paths,
        top,
        min_members,
        min_value,
        sort,
        config,
        mode,
        show,
        cache_dir,
        fail_on,
        baseline,
        ignore_file,
        semantic_pack,
        write_baseline,
        format,
        exclude,
        min_size,
        min_lines,
    })
}

fn run_review_cmd(cmd: Cmd) -> Result<()> {
    let Cmd::Review {
        paths,
        base,
        mode,
        min_size,
        min_lines,
        exclude,
        config,
        ignore_file,
        format,
        top,
        fail,
    } = cmd
    else {
        unreachable!("run_review_cmd requires Cmd::Review")
    };
    let paths = if paths.is_empty() {
        vec![PathBuf::from(".")]
    } else {
        paths
    };
    review::cmd_review(review::ReviewArgs {
        paths,
        base,
        mode,
        min_size,
        min_lines,
        exclude,
        config,
        ignore_file,
        format,
        top,
        fail,
    })
}

/// Deterministic input battery for an `arity`-parameter function. The parameters range
/// over a fixed pool of small int-lists and scalars; for small arity the pool is
/// enumerated *combinatorially* (mixed-radix), so e.g. a 2-arg comparison sees `a<b`,
/// `a>b`, and `a==b` rather than a few coincidental diagonal pairs — the difference
/// between trusting the completeness signal and not. All units of the same arity run on
/// identical inputs (comparable); a list where a scalar is expected (or vice-versa)
/// yields `Err`, itself part of the behavior signature.
/// A fixed input *width* used for every unit regardless of its arity: a function
/// binds the first `arity` values and ignores the rest, so all units run the same
/// number of rows (the behavior-vector length must be arity-independent — two
/// fingerprint-equal units can differ in arity, e.g. constant functions).
const VERIFY_WIDTH: usize = 4;
/// Verify is a bounded oracle, not a stress test for every generated/parser-vendored
/// mega-function. Cap per-unit battery work before building the value fingerprint or
/// interpreting rows; units above the cap are reported as `battery-bail` and excluded.
const VERIFY_BATTERY_NODE_ROW_BUDGET: usize = 384_000; // 2k IL nodes * 192 standard rows.

fn verify_battery_over_budget(tokens: usize, battery_rows: usize) -> bool {
    tokens.saturating_mul(battery_rows) > VERIFY_BATTERY_NODE_ROW_BUDGET
}

fn verify_battery(probes: &[nose_normalize::Value]) -> Vec<Vec<nose_normalize::Value>> {
    use nose_normalize::Value;
    let l = |xs: &[i64]| Value::List(xs.iter().copied().map(Value::Int).collect());
    let pool = [
        l(&[1, 2, 3, 4]),
        Value::Int(3),
        Value::Int(0),
        Value::Int(-1),
        l(&[5, 1, 4, 2, 8]),
        Value::Int(7),
        l(&[]),
        Value::Int(2),
    ];
    let n = pool.len();
    // Part 1: combinatorial (mixed-radix) over the pool, width-VERIFY_WIDTH rows — a
    // 2-arg function's first two slots see `a<b`/`a>b`/`a==b`.
    const COUNT: usize = 64;
    let mut battery: Vec<Vec<Value>> = (0..COUNT)
        .map(|e| {
            (0..VERIFY_WIDTH)
                .map(|j| {
                    let radix = n.saturating_pow(j as u32).max(1);
                    pool[(e / radix) % n].clone()
                })
                .collect()
        })
        .collect();
    // Part 2: literal probes. For each value the corpus actually branches on (a mined
    // string/int constant), inject it at each position — so a value-keyed branch
    // (`fdNumber === 'ipc'`) is exercised instead of always falling through, which is
    // what makes two such functions look coincidentally equal. Row count stays fixed.
    let fill = pool[0].clone();
    for v in probes {
        for p in 0..VERIFY_WIDTH {
            let mut row = vec![fill.clone(); VERIFY_WIDTH];
            row[p] = v.clone();
            battery.push(row);
        }
    }
    battery
}

/// Mine the literal constants the corpus branches on — the top string-literal hashes
/// and small integers, as interpreter values — to seed the battery's probe inputs.
fn verify_probes(corpus: &Corpus) -> Vec<nose_normalize::Value> {
    use nose_il::Payload;
    use nose_normalize::Value;
    use std::collections::HashMap;
    let (mut strs, mut ints): (HashMap<u64, u32>, HashMap<i64, u32>) =
        (HashMap::new(), HashMap::new());
    for il in &corpus.files {
        for node in &il.nodes {
            match node.payload {
                Payload::LitStr(h) => *strs.entry(h).or_default() += 1,
                Payload::LitInt(v) => *ints.entry(v).or_default() += 1,
                _ => {}
            }
        }
    }
    fn top<K: Ord + Copy>(m: HashMap<K, u32>, k: usize) -> Vec<K> {
        let mut v: Vec<(K, u32)> = m.into_iter().collect();
        v.sort_unstable_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        v.truncate(k);
        v.into_iter().map(|(key, _)| key).collect()
    }
    let mut probes: Vec<Value> = top(strs, 16)
        .into_iter()
        .map(|h| Value::Str(vec![h]))
        .collect();
    probes.extend(top(ints, 16).into_iter().map(Value::Int));
    probes
}

/// Leap-3 "wide" battery: a much larger structured input domain than [`verify_battery`].
/// Bounded equivalence checking is "interpret on enough inputs that two functions which
/// differ anywhere differ HERE": more scalars (large, negative, boundary), more lists
/// (sorted/reversed/duplicate/negative/singleton/empty), a wider arity slot, and more
/// combinatorial rows. The leap-3 hypothesis: a finite battery merges some non-equivalent
/// pairs (the §AK risk); a wider domain should drive those false merges toward zero while
/// keeping the true positives. (Still not a proof — that is the SMT extension — but a much
/// stronger bounded checker.)
fn wide_battery(probes: &[nose_normalize::Value]) -> Vec<Vec<nose_normalize::Value>> {
    use nose_normalize::Value;
    let l = |xs: &[i64]| Value::List(xs.iter().copied().map(Value::Int).collect());
    let pool = [
        l(&[1, 2, 3, 4]),
        Value::Int(3),
        Value::Int(0),
        Value::Int(-1),
        l(&[5, 1, 4, 2, 8]),
        Value::Int(7),
        l(&[]),
        Value::Int(2),
        // wide additions: boundary/large/negative scalars and adversarial lists
        Value::Int(-7),
        Value::Int(100),
        Value::Int(1),
        l(&[2, 2, 2]),        // all-equal (separates min/max/dedup-sensitive)
        l(&[0]),              // singleton zero (separates *-fold from +-fold, presence)
        l(&[-3, -1, -2]),     // all-negative (separates abs/sign, min/max direction)
        l(&[4, 3, 2, 1]),     // reversed (separates order-sensitive from order-free)
        l(&[10, -10, 5, -5]), // mixed sign, zero-sum (separates sum from sum-abs)
    ];
    let n = pool.len();
    const WIDTH: usize = 5;
    const COUNT: usize = 243; // 3^5 — dense mixed-radix coverage over a low-entropy slice
    let mut battery: Vec<Vec<Value>> = (0..COUNT)
        .map(|e| {
            (0..WIDTH)
                .map(|j| {
                    let radix = n.saturating_pow(j as u32).max(1);
                    pool[(e / radix) % n].clone()
                })
                .collect()
        })
        .collect();
    let fill = pool[0].clone();
    for v in probes {
        for p in 0..WIDTH {
            let mut row = vec![fill.clone(); WIDTH];
            row[p] = v.clone();
            battery.push(row);
        }
    }
    battery
}

/// Trailing `sources/<id>/<file>` key shared by the corpus path and the manifest path,
/// so an interpreted unit can be matched to its manifest entry regardless of the prefix
/// the corpus was scanned under.
fn manifest_key(path: &str) -> String {
    match path.rfind("sources/") {
        Some(i) => path[i..].to_string(),
        None => path.to_string(),
    }
}

fn cmd_behavioral_gate(
    paths: Vec<PathBuf>,
    manifest: PathBuf,
    battery_kind: BatteryKind,
) -> Result<()> {
    let refs = paths_as_refs(&paths);
    let corpus = nose_frontend::lower_corpus_many(&refs);
    warn_if_empty(&corpus, &paths);
    let battery = match battery_kind {
        BatteryKind::Standard => verify_battery(&verify_probes(&corpus)),
        BatteryKind::Wide => wide_battery(&verify_probes(&corpus)),
    };
    let units = gate_units(&corpus, &battery);
    let m: GateManifest = serde_json::from_str(&std::fs::read_to_string(&manifest)?)?;
    let outcome = tally_gate(&m, &units);
    print_gate_report(battery_kind, battery.len(), &outcome);
    Ok(())
}

/// Index every `Func` in `il` by its source byte span, so a fully-normalized unit can
/// be matched (by span) to the same function in the pre-canon core IL.
fn func_span_index(il: &nose_il::Il) -> std::collections::HashMap<(u32, u32), nose_il::NodeId> {
    let mut index = std::collections::HashMap::new();
    let mut stk = vec![il.root];
    while let Some(x) = stk.pop() {
        if il.kind(x) == nose_il::NodeKind::Func {
            let s = il.node(x).span;
            index.entry((s.start_byte, s.end_byte)).or_insert(x);
        }
        stk.extend(il.children(x).iter().copied());
    }
    index
}

/// Interpret `root` on every battery row (under the unit's pointer-length contracts);
/// `None` when any input fails to run — the unit is not interpretable on this battery.
fn run_battery(
    il: &nose_il::Il,
    interner: &Interner,
    root: nose_il::NodeId,
    battery: &[Vec<nose_normalize::Value>],
    contracts: &[(u32, u32)],
) -> Option<Vec<nose_normalize::Behavior>> {
    let mut beh = Vec::with_capacity(battery.len());
    for inputs in battery {
        let row = apply_contracts(inputs, contracts);
        beh.push(nose_normalize::run_unit(il, interner, root, &row)?);
    }
    Some(beh)
}

/// Trivial behavior (constant / all-Err) is coincidental, never evidence of a
/// clone — exclude it from behavioral merging.
fn is_trivial_behavior(beh: &[nose_normalize::Behavior]) -> bool {
    use nose_normalize::Value;
    let distinct: std::collections::HashSet<&Value> = beh.iter().map(|b| &b.ret).collect();
    distinct.len() < 2
        || beh
            .iter()
            .all(|b| matches!(b.ret, Value::Null | Value::Err))
}

/// Stable hash of a behavior battery (equal hash ⟺ behaviorally equal on the battery).
fn behavior_hash(beh: &[nose_normalize::Behavior]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    beh.hash(&mut h);
    h.finish()
}

/// One interpretable record per generated source file (each holds exactly one function).
struct GateUnit {
    fp: Vec<u64>,
    beh_hash: u64,
    trivial: bool,
}

fn gate_units(
    corpus: &Corpus,
    battery: &[Vec<nose_normalize::Value>],
) -> std::collections::HashMap<String, GateUnit> {
    let opts = nose_normalize::NormalizeOptions::default();
    let oracle_opts = nose_normalize::NormalizeOptions {
        oracle: true,
        ..opts
    };
    let mut units = std::collections::HashMap::new();
    for il in &corpus.files {
        let n = nose_normalize::normalize(il, &corpus.interner, &opts);
        let core = nose_normalize::normalize(il, &corpus.interner, &oracle_opts);
        let core_func = func_span_index(&core);
        for u in &n.units {
            let root = u.root;
            if n.kind(root) != nose_il::NodeKind::Func {
                continue;
            }
            let span0 = n.node(root).span;
            let Some(&core_root) = core_func.get(&(span0.start_byte, span0.end_byte)) else {
                continue;
            };
            // Fingerprint + pointer-length contracts (n = len(array)) from one build.
            let (fp, contracts) =
                nose_normalize::value_fingerprint_and_contracts(&n, root, &corpus.interner);
            if fp.is_empty() {
                continue;
            }
            let Some(beh) = run_battery(&core, &corpus.interner, core_root, battery, &contracts)
            else {
                continue;
            };
            let trivial = is_trivial_behavior(&beh);
            units.insert(
                manifest_key(&il.meta.path),
                GateUnit {
                    fp,
                    beh_hash: behavior_hash(&beh),
                    trivial,
                },
            );
        }
    }
    units
}

// The manifest's labeled pairs, cross-referenced against the interpretable units.
#[derive(serde::Deserialize)]
struct GateSide {
    path: String,
}
#[derive(serde::Deserialize)]
struct GateItem {
    left: GateSide,
    right: GateSide,
    semantic_status: String,
    split: String,
}
#[derive(serde::Deserialize)]
struct GateManifest {
    items: Vec<GateItem>,
}

/// Per-class tally: did exact-fingerprint merge the pair? did the behavioral gate?
struct GateTally {
    pairs: usize,
    fp_merge: usize,
    beh_merge: usize,
    beh_only: usize, // behavioral merge that fingerprint missed (the leap value / cost)
}

impl GateTally {
    fn new() -> Self {
        GateTally {
            pairs: 0,
            fp_merge: 0,
            beh_merge: 0,
            beh_only: 0,
        }
    }
}

struct GateOutcome {
    pos: GateTally,
    neg: GateTally,
    pos_heldout: usize,
    pos_heldout_beh_only: usize,
    uninterp_pairs: usize,
}

/// Tally, restricted to pairs where BOTH units are interpretable (the slice this gate
/// can speak to).
fn tally_gate(
    m: &GateManifest,
    units: &std::collections::HashMap<String, GateUnit>,
) -> GateOutcome {
    let mut out = GateOutcome {
        pos: GateTally::new(),
        neg: GateTally::new(),
        pos_heldout: 0,
        pos_heldout_beh_only: 0,
        uninterp_pairs: 0,
    };
    for it in &m.items {
        let (lk, rk) = (manifest_key(&it.left.path), manifest_key(&it.right.path));
        let (Some(lu), Some(ru)) = (units.get(&lk), units.get(&rk)) else {
            out.uninterp_pairs += 1;
            continue;
        };
        let positive = it.semantic_status == "equivalent";
        let t = if positive { &mut out.pos } else { &mut out.neg };
        t.pairs += 1;
        let fp_merge = lu.fp == ru.fp;
        // A behavioral merge requires identical behavior on EVERY battery input and a
        // non-trivial behavior (constant/all-Err units never merge on behavior).
        let beh_merge = !lu.trivial && !ru.trivial && lu.beh_hash == ru.beh_hash;
        if fp_merge {
            t.fp_merge += 1;
        }
        if beh_merge {
            t.beh_merge += 1;
        }
        if beh_merge && !fp_merge {
            t.beh_only += 1;
            if positive && it.split == "heldout" {
                out.pos_heldout_beh_only += 1;
            }
        }
        if positive && it.split == "heldout" {
            out.pos_heldout += 1;
        }
    }
    out
}

fn print_gate_report(battery_kind: BatteryKind, battery_rows: usize, outcome: &GateOutcome) {
    let GateOutcome {
        pos,
        neg,
        pos_heldout,
        pos_heldout_beh_only,
        uninterp_pairs,
    } = outcome;
    let kind = match battery_kind {
        BatteryKind::Standard => "standard (leap 2)",
        BatteryKind::Wide => "wide (leap 3)",
    };
    println!("=== behavioral-equivalence acceptance gate — battery: {kind} ===");
    println!("battery rows: {battery_rows}");
    println!(
        "manifest pairs: {} interpretable-both / {} excluded (a unit not interpretable)",
        pos.pairs + neg.pairs,
        uninterp_pairs
    );
    println!();
    println!(
        "POSITIVES (should merge), interpretable slice = {}",
        pos.pairs
    );
    println!(
        "  exact-fingerprint recall : {}/{} ({:.1}%)",
        pos.fp_merge,
        pos.pairs,
        pct(pos.fp_merge, pos.pairs)
    );
    println!(
        "  behavioral-gate recall   : {}/{} ({:.1}%)",
        pos.beh_merge,
        pos.pairs,
        pct(pos.beh_merge, pos.pairs)
    );
    println!(
        "  → RECOVERED beyond fingerprint (leap value): {} (heldout: {}/{})",
        pos.beh_only, pos_heldout_beh_only, pos_heldout
    );
    println!();
    println!(
        "HARD NEGATIVES (must NOT merge), interpretable slice = {}",
        neg.pairs
    );
    println!(
        "  exact-fingerprint false merges: {}/{} ({:.1}%)",
        neg.fp_merge,
        neg.pairs,
        pct(neg.fp_merge, neg.pairs)
    );
    println!(
        "  behavioral-gate false merges  : {}/{} ({:.1}%)  ← the soundness cost",
        neg.beh_merge,
        neg.pairs,
        pct(neg.beh_merge, neg.pairs)
    );
    println!("  → INTRODUCED beyond fingerprint: {}", neg.beh_only);
}

fn pct(a: usize, b: usize) -> f64 {
    if b == 0 {
        0.0
    } else {
        100.0 * a as f64 / b as f64
    }
}

/// Rewrite a battery row to honor a unit's pointer-length contracts: set each length-param
/// slot to the length of its array-param slot, so the oracle interprets `f(xs, n)` under
/// `n = len(xs)` — the same convention the value graph used to merge it. Only applies when
/// the array slot is actually a list (else the unit Errs identically regardless). Returns
/// the row unchanged when there are no contracts (zero cost for the common case).
fn apply_contracts(
    row: &[nose_normalize::Value],
    contracts: &[(u32, u32)],
) -> Vec<nose_normalize::Value> {
    use nose_normalize::Value;
    let mut out = row.to_vec();
    // A length param shared by several arrays (aligned `f(a, b, n)`) is the SHARED logical
    // length: bind it to the MIN of those arrays' lengths, matching the `zip`-based form
    // (`sum(x*y for x,y in zip(a,b))` stops at the shorter). For a single array this is just
    // its length. Group contracts by length-position so the shared case is a min, not a
    // last-write race.
    let mut by_len: std::collections::BTreeMap<usize, Vec<usize>> =
        std::collections::BTreeMap::new();
    for &(arr_pos, len_pos) in contracts {
        by_len
            .entry(len_pos as usize)
            .or_default()
            .push(arr_pos as usize);
    }
    for (len_pos, arrs) in by_len {
        if len_pos >= out.len() {
            continue;
        }
        // If EVERY contracted array slot is a list, bind `n` to the MIN of their lengths (the
        // shared logical length). If any slot is NOT a list, `len` is undefined — bind `n =
        // Null` so `i < n` Errs and the unit Errs exactly as the `len(non-list)` form does,
        // instead of running an empty loop and returning the init value.
        let mut shared: Option<i64> = Some(i64::MAX);
        for arr_pos in arrs {
            match out.get(arr_pos) {
                Some(Value::List(xs)) => {
                    let l = xs.len() as i64;
                    shared = shared.map(|s| s.min(l));
                }
                _ => shared = None,
            }
        }
        out[len_pos] = match shared {
            Some(l) if l != i64::MAX => Value::Int(l),
            _ => Value::Null,
        };
    }
    out
}

fn cmd_verify(
    paths: Vec<PathBuf>,
    no_cfg_norm: bool,
    json: bool,
    max_violations: Option<usize>,
    leads: Option<PathBuf>,
    exclusion_census: Option<PathBuf>,
) -> Result<()> {
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

    let n_violations = report_verify_soundness(&oracle.recs);
    report_verify_completeness(&oracle.recs, leads.as_deref())?;
    report_verify_calibration(&oracle.recs);

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

/// One record per interpretable unit.
struct VerifyRec {
    fp: Vec<u64>,
    beh: Vec<nose_normalize::Behavior>,
    file: String,
    start: u32,
    end: u32,
    tokens: usize,
    loc: String,
    /// Can the exact `semantic` channel ever claim this unit (strict-exact-safe
    /// and above the degenerate-fingerprint floor)? Scopes the HARD gate.
    claimable: bool,
    /// Hash of the unit's declared parameter domains. The oracle binds battery
    /// rows under declared-type coercion, so two units are battery-COMPARABLE
    /// only when their declarations agree; a disagreement across different
    /// declarations is an advisory lead, not a hard violation.
    domain_sig: u64,
}

#[derive(Clone, Copy)]
enum VerifyExclusionReason {
    CoreMissing,
    BatteryBail,
    EmptyFingerprint,
    Uninterpretable,
}

impl VerifyExclusionReason {
    fn label(self) -> &'static str {
        match self {
            VerifyExclusionReason::CoreMissing => "core-missing",
            VerifyExclusionReason::BatteryBail => "battery-bail",
            VerifyExclusionReason::EmptyFingerprint => "empty-fingerprint",
            VerifyExclusionReason::Uninterpretable => "uninterpretable",
        }
    }
}

struct VerifyExcludedUnit {
    reason: VerifyExclusionReason,
    file: String,
    start: u32,
    end: u32,
    tokens: usize,
}

#[derive(Default)]
struct VerifyExclusions {
    core_missing: usize,
    battery_bail: usize,
    empty_fingerprint: usize,
    uninterpretable: usize,
    units: Vec<VerifyExcludedUnit>,
}

impl VerifyExclusions {
    fn record(
        &mut self,
        reason: VerifyExclusionReason,
        file: &str,
        span: nose_il::Span,
        tokens: usize,
    ) {
        match reason {
            VerifyExclusionReason::CoreMissing => self.core_missing += 1,
            VerifyExclusionReason::BatteryBail => self.battery_bail += 1,
            VerifyExclusionReason::EmptyFingerprint => self.empty_fingerprint += 1,
            VerifyExclusionReason::Uninterpretable => self.uninterpretable += 1,
        }
        self.units.push(VerifyExcludedUnit {
            reason,
            file: file.to_string(),
            start: span.start_line,
            end: span.end_line,
            tokens,
        });
    }

    fn append(&mut self, other: VerifyExclusions) {
        self.core_missing += other.core_missing;
        self.battery_bail += other.battery_bail;
        self.empty_fingerprint += other.empty_fingerprint;
        self.uninterpretable += other.uninterpretable;
        self.units.extend(other.units);
    }

    fn total(&self) -> usize {
        self.core_missing + self.battery_bail + self.empty_fingerprint + self.uninterpretable
    }
}

/// The oracle's interpretation pass: every interpretable unit's record, plus the
/// CANON PRESERVATION tallies — a stricter, pair-free soundness check: does the full
/// normalization pipeline preserve each unit's behavior vs the pre-canon core IL? A
/// mismatch is a behavior-changing canon bug, even if no corpus twin collides with it.
struct VerifyOracle {
    recs: Vec<VerifyRec>,
    total: usize,
    canon_checked: usize,
    canon_violations: Vec<String>,
    /// Per-unit census records (outcome + construct tags), populated only when
    /// the `--exclusion-census` instrument is requested.
    census: Vec<verify_census::CensusUnit>,
    census_enabled: bool,
    exclusions: VerifyExclusions,
}

fn collect_verify_recs(
    corpus: &Corpus,
    opts: &nose_normalize::NormalizeOptions,
    battery: &[Vec<nose_normalize::Value>],
    census: bool,
) -> VerifyOracle {
    let oracle_opts = nose_normalize::NormalizeOptions {
        oracle: true,
        ..*opts
    };
    let per_file: Vec<_> = corpus
        .files
        .par_iter()
        .map(|il| {
            let n = nose_normalize::normalize(il, &corpus.interner, opts);
            // The behavioral ground truth comes from the pre-canonicalization core IL (so a
            // behavior-changing canon can't mask itself), matched to each fully-normalized
            // unit by source span.
            let core = nose_normalize::normalize(il, &corpus.interner, &oracle_opts);
            let mut oracle = VerifyOracle {
                recs: Vec::new(),
                total: 0,
                canon_checked: 0,
                canon_violations: Vec::new(),
                census: Vec::new(),
                census_enabled: census,
                exclusions: VerifyExclusions::default(),
            };
            let func_count = n
                .units
                .iter()
                .filter(|u| n.kind(u.root) == nose_il::NodeKind::Func)
                .count();
            let value_context = (func_count > 1)
                .then(|| nose_normalize::ValueFingerprintContext::new(&n, &corpus.interner));
            // The exact channel's claim surface for this file, from the RAW lowered
            // IL (the same input the detector extracts from), keyed by line span.
            let claim_by_span: std::collections::HashMap<(u32, u32), bool> =
                nose_detect::units_of_file(
                    il,
                    &corpus.interner,
                    &nose_detect::DetectOptions::default(),
                )
                .iter()
                .map(|f| {
                    (
                        (f.start_line, f.end_line),
                        nose_detect::exact_claim_eligible(f),
                    )
                })
                .collect();
            collect_file_verify_recs(
                &n,
                &core,
                value_context.as_ref(),
                &corpus.interner,
                battery,
                &mut oracle,
                &claim_by_span,
            );
            oracle
        })
        .collect();

    let mut oracle = VerifyOracle {
        recs: Vec::new(),
        total: 0,
        canon_checked: 0,
        canon_violations: Vec::new(),
        census: Vec::new(),
        census_enabled: census,
        exclusions: VerifyExclusions::default(),
    };
    for mut file_oracle in per_file {
        oracle.total += file_oracle.total;
        oracle.canon_checked += file_oracle.canon_checked;
        oracle.recs.append(&mut file_oracle.recs);
        oracle.census.append(&mut file_oracle.census);
        oracle
            .canon_violations
            .append(&mut file_oracle.canon_violations);
        if oracle.canon_violations.len() > 20 {
            oracle.canon_violations.truncate(20);
        }
        oracle.exclusions.append(file_oracle.exclusions);
    }
    oracle
}

/// Record one unit's oracle outcome in the exclusion census (no-op unless the
/// `--exclusion-census` instrument is on). `tag_il`/`tag_root` name the subtree
/// the oracle would have interpreted (the core IL when span-matched, else the
/// fully-normalized unit).
fn push_verify_census(
    oracle: &mut VerifyOracle,
    loc: String,
    tag_il: &nose_il::Il,
    tag_root: nose_il::NodeId,
    fp: &[u64],
    reason: &'static str,
) {
    if !oracle.census_enabled {
        return;
    }
    oracle.census.push(verify_census::CensusUnit {
        loc,
        reason,
        fp: fp.to_vec(),
        tags: verify_census::census_tags(tag_il, tag_root),
    });
}

fn collect_file_verify_recs(
    n: &nose_il::Il,
    core: &nose_il::Il,
    value_context: Option<&nose_normalize::ValueFingerprintContext>,
    interner: &Interner,
    battery: &[Vec<nose_normalize::Value>],
    oracle: &mut VerifyOracle,
    // A span the detector never extracted stays CLAIMABLE (fail closed: an
    // unknown unit must not silently exempt itself from the gate).
    claim_by_span: &std::collections::HashMap<(u32, u32), bool>,
) {
    let file_path = &n.meta.path;
    let core_func = func_span_index(core);
    for u in &n.units {
        let root = u.root;
        if n.kind(root) != nose_il::NodeKind::Func {
            continue;
        }
        oracle.total += 1;
        let loc = format!("{}:{}", file_path, n.node(root).span.start_line);
        // The same function in the core IL (by span) — interpret THAT, not `n`.
        let span0 = n.node(root).span;
        let tokens = subtree_node_count(n, root);
        let Some(&core_root) = core_func.get(&(span0.start_byte, span0.end_byte)) else {
            push_verify_census(oracle, loc, n, root, &[], "no-core-span");
            oracle
                .exclusions
                .record(VerifyExclusionReason::CoreMissing, file_path, span0, tokens);
            continue;
        };
        if verify_battery_over_budget(tokens, battery.len()) {
            oracle
                .exclusions
                .record(VerifyExclusionReason::BatteryBail, file_path, span0, tokens);
            push_verify_census(oracle, loc, core, core_root, &[], "battery-bail");
            continue;
        }
        // Soundness is about merges on the VALUE fingerprint. A unit whose value
        // graph is EMPTY (`fn resumed() {}`, or a body the graph captures nothing of)
        // has no value fingerprint to merge on — the detector keys candidates on
        // structure there, never on an empty value multiset — so distinct empty-fp
        // bodies "colliding" is not a product false merge. Exclude empty fingerprints
        // (only those — small non-empty ones stay, so completeness is unaffected).
        // Fingerprint AND pointer-length contracts from ONE value-graph build (the
        // oracle needs both; building twice doubled the per-unit cost). The contract
        // binds n = len(array) so the oracle interprets `f(xs,n)` under the same
        // convention the value graph used to merge it; gated on the contract actually
        // firing, so a non-contract false merge is still exposed by the free battery.
        let (fp, contracts) = match value_context {
            Some(context) => nose_normalize::value_fingerprint_and_contracts_with_context(
                n, root, interner, context,
            ),
            None => nose_normalize::value_fingerprint_and_contracts(n, root, interner),
        };
        if fp.is_empty() {
            push_verify_census(oracle, loc, n, root, &[], "empty-fp");
            oracle.exclusions.record(
                VerifyExclusionReason::EmptyFingerprint,
                file_path,
                span0,
                tokens,
            );
            continue;
        }
        // Run the battery; the unit is interpretable only if every input runs.
        let Some(beh) = run_battery(core, interner, core_root, battery, &contracts) else {
            push_verify_census(oracle, loc, core, core_root, &fp, "battery-bail");
            oracle.exclusions.record(
                VerifyExclusionReason::Uninterpretable,
                file_path,
                span0,
                tokens,
            );
            continue;
        };
        push_verify_census(oracle, loc, core, core_root, &fp, "interpretable");
        // Stricter canon check: the SAME function interpreted on the fully-normalized
        // IL must agree with the core IL on every input — else a canon pass changed
        // behavior. (Only when the full IL is itself fully interpretable on the battery.)
        // Canon preservation is judged on CONCRETE behaviors only: symbolic identity
        // is keyed on syntax, and canonicalization legitimately rewrites syntax, so a
        // Sym-bearing mismatch here is expected, not a behavior change.
        if let Some(full_beh) = run_battery(n, interner, root, battery, &contracts) {
            let concrete = !beh.iter().any(nose_normalize::behavior_has_sym)
                && !full_beh.iter().any(nose_normalize::behavior_has_sym);
            if concrete {
                oracle.canon_checked += 1;
                if full_beh != beh && oracle.canon_violations.len() < 20 {
                    let s = n.node(root).span;
                    oracle
                        .canon_violations
                        .push(format!("{}:{}", file_path, s.start_line));
                }
            }
        }
        let span = n.node(root).span;
        oracle.recs.push(VerifyRec {
            fp,
            beh,
            file: file_path.to_string(),
            start: span.start_line,
            end: span.end_line,
            tokens,
            loc: format!("{}:{}", file_path, span.start_line),
            claimable: claim_by_span
                .get(&(span.start_line, span.end_line))
                .copied()
                .unwrap_or(true),
            domain_sig: param_domain_signature(n, root),
        });
    }
}

/// Stable hash of a unit's declared parameter domains (position-sensitive).
/// Units whose declarations differ are interpreted under different battery
/// coercions and are not behavior-comparable row-for-row.
fn param_domain_signature(il: &nose_il::Il, root: nose_il::NodeId) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for &k in il.children(root) {
        if il.kind(k) == nose_il::NodeKind::Param {
            match nose_semantics::domain_evidence_for_param(il, k) {
                Some(d) => d.hash(&mut h),
                None => 0xD07Fu16.hash(&mut h),
            }
        }
    }
    h.finish()
}

/// Subtree node count — the same size signal the detector gates on, so the
/// value-add evaluator can restrict its gold to meaningful-size units.
fn subtree_node_count(il: &nose_il::Il, root: nose_il::NodeId) -> usize {
    let mut tokens = 0usize;
    let mut stack = vec![root];
    while let Some(x) = stack.pop() {
        tokens += 1;
        stack.extend(il.children(x).iter().copied());
    }
    tokens
}

/// Behavioral ground truth for the value-add evaluator: each interpretable unit with
/// a stable hash of its behavior battery (equal hash ⟺ behaviorally equal on the
/// battery) and whether that behavior is trivial (constant / all-Err — coincidental,
/// not evidence of a real clone). The evaluator groups by behavior to form gold clone
/// pairs, then scores jscpd and nose against them on equal footing.
fn print_verify_json(oracle: &VerifyOracle) -> Result<()> {
    let recs_json: Vec<_> = oracle
        .recs
        .iter()
        .map(|r| {
            serde_json::json!({
                "file": r.file,
                "start_line": r.start,
                "end_line": r.end,
                "tokens": r.tokens,
                "behavior": format!("{:016x}", behavior_hash(&r.beh)),
                "trivial": is_trivial_behavior(&r.beh),
            })
        })
        .collect();
    let excluded_json: Vec<_> = oracle
        .exclusions
        .units
        .iter()
        .map(|u| {
            serde_json::json!({
                "file": u.file,
                "start_line": u.start,
                "end_line": u.end,
                "tokens": u.tokens,
                "reason": u.reason.label(),
            })
        })
        .collect();
    println!(
        "{}",
        serde_json::to_string(&serde_json::json!({
            "units": recs_json,
            "exclusions": {
                "core-missing": oracle.exclusions.core_missing,
                "battery-bail": oracle.exclusions.battery_bail,
                "empty-fingerprint": oracle.exclusions.empty_fingerprint,
                "uninterpretable": oracle.exclusions.uninterpretable,
            },
            "excluded_units": excluded_json,
        }))?
    );
    Ok(())
}

fn print_verify_exclusions(exclusions: &VerifyExclusions) {
    if exclusions.total() == 0 {
        return;
    }
    println!("\nEXCLUSIONS — fail-closed units by reason:");
    println!("  core-missing: {}", exclusions.core_missing);
    println!(
        "  battery-bail: {} (>{} node-rows)",
        exclusions.battery_bail, VERIFY_BATTERY_NODE_ROW_BUDGET
    );
    println!("  empty-fingerprint: {}", exclusions.empty_fingerprint);
    println!("  uninterpretable: {}", exclusions.uninterpretable);
}

/// Soundness: fingerprint-equal ⟹ behavior-equal. Prints the section and returns the
/// HARD false-merge count (the input to the `--max-violations` gate). A disagreement
/// where either behavior carries a symbolic value is reported separately as an
/// ADVISORY lead: symbolic identity is keyed on pre-canon syntax, so a proof-backed
/// canonicalization (AC ordering, distribution) can legitimately make two equivalent
/// units' symbolic traces differ — those need a human look, not a red gate.
fn report_verify_soundness(recs: &[VerifyRec]) -> usize {
    let has_sym = |r: &VerifyRec| r.beh.iter().any(nose_normalize::behavior_has_sym);
    let mut by_fp: std::collections::HashMap<&[u64], Vec<&VerifyRec>> =
        std::collections::HashMap::new();
    for r in recs {
        by_fp.entry(&r.fp).or_default().push(r);
    }
    let mut fp_groups = 0usize;
    let mut violations: Vec<(String, String, usize)> = Vec::new();
    let mut advisory: Vec<(String, String, usize)> = Vec::new();
    let mut lossy: Vec<(String, String, usize)> = Vec::new();
    for members in by_fp.values() {
        if members.len() < 2 {
            continue;
        }
        fp_groups += 1;
        let first = members[0];
        for r in &members[1..] {
            if r.beh != first.beh {
                let diff = r.beh.iter().zip(&first.beh).filter(|(a, b)| a != b).count();
                let rec = (first.loc.clone(), r.loc.clone(), diff);
                if has_sym(first) || has_sym(r) || first.domain_sig != r.domain_sig {
                    advisory.push(rec);
                } else if first.claimable && r.claimable {
                    violations.push(rec);
                } else {
                    lossy.push(rec);
                }
            }
        }
    }
    println!("\nSOUNDNESS — fingerprint-equal ⟹ behavior-equal (exact claim surface):");
    println!("  fingerprint groups (≥2): {fp_groups}");
    let n_violations = violations.len();
    if violations.is_empty() {
        println!("  SOUND: no false merges ✓");
    } else {
        println!("  [!] {n_violations} VIOLATION(S) (false merges):");
        for (a, b, d) in violations.iter().take(20) {
            println!("    {a}  ≡?  {b}   ({d} differing inputs)");
        }
    }
    if !lossy.is_empty() {
        lossy.sort();
        println!(
            "  lossy-fingerprint collisions (outside the exact claim — diagnostics, not gated): {}",
            lossy.len()
        );
        for (a, b, d) in lossy.iter().take(10) {
            println!("    {a}  ≠  {b}   ({d} differing inputs)");
        }
    }
    if !advisory.is_empty() {
        advisory.sort();
        println!(
            "  advisory (symbolic-trace disagreements — review, not gated): {}",
            advisory.len()
        );
        for (a, b, d) in advisory.iter().take(10) {
            println!("    {a}  ≢?  {b}   ({d} differing inputs)");
        }
    }
    n_violations
}

/// Completeness: behavior-equal ⟹ fingerprint-equal (the under-merge / recall
/// direction). Restricted to *non-trivial* behaviors (the return value varies across
/// inputs and isn't uniformly Err/Null) — trivial functions agree coincidentally and
/// aren't evidence of a missed clone. A behavior group split across ≥2 fingerprints
/// is a real Type-4 clone the value graph fails to recognize. Behavior-equal on the
/// battery is necessary-not-sufficient for equivalence, so this is a lower bound on
/// completeness / upper bound on misses — but each surfaced pair is a concrete lead.
fn report_verify_completeness(recs: &[VerifyRec], leads: Option<&std::path::Path>) -> Result<()> {
    let mut by_beh: std::collections::HashMap<&[nose_normalize::Behavior], Vec<&VerifyRec>> =
        std::collections::HashMap::new();
    for r in recs {
        // Concrete behaviors only: symbolic equality says "same opaque operations on
        // equal operands", which is too weak a witness for a MISSED-clone claim (two
        // wrappers calling same-NAMED but different functions would coincide). The
        // under-merge direction keeps its §BC meaning; symbolic coverage serves the
        // soundness direction.
        if !is_trivial_behavior(&r.beh) && !r.beh.iter().any(nose_normalize::behavior_has_sym) {
            by_beh.entry(&r.beh).or_default().push(r);
        }
    }
    let (mut beh_pairs, mut fp_equal_pairs, mut split_groups) = (0usize, 0usize, 0usize);
    // Each surfaced under-merge carries the *max cross-fingerprint vj* in its group: the
    // structural near-ness the behavioral oracle would gate. High vj + behavior-equal =
    // a real structural/loop clone the exact-fingerprint detector misses (e.g. join
    // index-loop vs iterator); low vj + behavior-equal = a coincidental skeleton match
    // (null-guard passthrough) we must NOT merge. This is the two-tier discriminator.
    let mut misses: Vec<(String, String, f64)> = Vec::new();
    let mut near_groups = 0usize; // split groups whose max cross-fp vj ≥ 0.7
    for members in by_beh.values() {
        if members.len() < 2 {
            continue;
        }
        let k = members.len();
        beh_pairs += k * (k - 1) / 2;
        // partition by fingerprint
        let mut by_fp2: std::collections::HashMap<&[u64], Vec<&&VerifyRec>> =
            std::collections::HashMap::new();
        for r in members {
            by_fp2.entry(&r.fp).or_default().push(r);
        }
        for sub in by_fp2.values() {
            let s = sub.len();
            fp_equal_pairs += s * (s - 1) / 2;
        }
        if by_fp2.len() > 1 {
            split_groups += 1;
            let (a, b, vj) = best_split_pair(by_fp2.values().map(|v| *v[0]).collect());
            if vj >= 0.7 {
                near_groups += 1;
            }
            misses.push((a, b, vj));
        }
    }
    // Total order: vj desc, then the two locations — `misses` is collected in `HashMap`
    // iteration order, so ties must break on stable keys for byte-identical output.
    misses.sort_by(|a, b| {
        b.2.partial_cmp(&a.2)
            .unwrap()
            .then(a.0.cmp(&b.0))
            .then(a.1.cmp(&b.1))
    });
    println!("\nCOMPLETENESS — behavior-equal ⟹ fingerprint-equal (non-trivial only):");
    println!(
        "  behavior groups (≥2): {}",
        by_beh.values().filter(|m| m.len() >= 2).count()
    );
    if beh_pairs > 0 {
        println!(
            "  completeness: {fp_equal_pairs}/{beh_pairs} = {:.0}% of behavior-equal pairs also converge",
            100.0 * fp_equal_pairs as f64 / beh_pairs as f64
        );
    }
    println!("  under-merged behavior groups (missed clones): {split_groups}");
    println!(
        "  of which structurally-near (max cross-fp vj ≥ 0.7 → behavior-gated near-match would recover): {near_groups}"
    );
    for (a, b, vj) in misses.iter().take(30) {
        println!("    vj={vj:.2}  {a}  ↮  {b}");
    }
    if let Some(path) = leads {
        write_verify_leads(path, &misses)?;
    }
    Ok(())
}

/// One representative per distinct fingerprint; find the max-vj cross pair.
/// Sort the reps by location so the chosen pair (and so the printed output) is
/// deterministic: `HashMap` iteration is an unspecified order that varies across
/// runs/thread counts, which would otherwise pick a different max-vj pair on ties
/// and break byte-identical output. The pair comes back in canonical orientation
/// (smaller location first) so it reads identically regardless of which rep the
/// scan happened to encounter first.
fn best_split_pair(mut reps: Vec<&VerifyRec>) -> (String, String, f64) {
    reps.sort_by(|a, b| a.loc.cmp(&b.loc));
    let mut best = (0.0f64, reps[0], reps[0]);
    for i in 0..reps.len() {
        for j in (i + 1)..reps.len() {
            let vj = multiset_jaccard_u64(&reps[i].fp, &reps[j].fp);
            if vj >= best.0 {
                best = (vj, reps[i], reps[j]);
            }
        }
    }
    let (a, b) = if best.1.loc <= best.2.loc {
        (best.1.loc.clone(), best.2.loc.clone())
    } else {
        (best.2.loc.clone(), best.1.loc.clone())
    };
    (a, b, best.0)
}

/// D1: export the under-merged pairs as detection leads — oracle-discovered candidates the
/// detection campaign can turn into convergence proposals. Sorted by vj (already), so the
/// strongest (structurally-near AND behavior-equal) come first.
fn write_verify_leads(path: &std::path::Path, misses: &[(String, String, f64)]) -> Result<()> {
    let items: Vec<_> = misses
        .iter()
        .map(|(a, b, vj)| {
            serde_json::json!({ "a": a, "b": b, "vj": vj, "structurally_near": *vj >= 0.7 })
        })
        .collect();
    let near = misses.iter().filter(|(_, _, vj)| *vj >= 0.7).count();
    std::fs::write(
        path,
        serde_json::to_string_pretty(&serde_json::json!({
            "under_merged_pairs": items.len(),
            "structurally_near": near,
            "leads": items,
        }))?,
    )?;
    println!(
        "\nLEADS: wrote {} under-merged pairs ({near} structurally-near) to {}",
        misses.len(),
        path.display()
    );
    Ok(())
}

/// Calibration: P(behavior-equal | value-Jaccard bin). The detector currently
/// trusts only an *exact* fingerprint match (vj = 1.0). This measures how safe it
/// would be to also accept *near* matches — for each vj band, the fraction of pairs
/// that are actually behavior-equal = the precision of accepting at that band. Pairs
/// are sampled by sorting units by fingerprint and comparing each to a window of
/// neighbors (so high-vj pairs are well represented, unlike uniform random pairs).
fn report_verify_calibration(recs: &[VerifyRec]) {
    let mut sorted: Vec<&VerifyRec> = recs.iter().collect();
    sorted.sort_unstable_by(|a, b| a.fp.cmp(&b.fp));
    const BINS: usize = 5; // [.5,.7) [.7,.8) [.8,.9) [.9,1.0) [1.0]
    let mut tot = [0usize; BINS];
    let mut eq = [0usize; BINS];
    let bin = |vj: f64| -> Option<usize> {
        match vj {
            v if v >= 1.0 => Some(4),
            v if v >= 0.9 => Some(3),
            v if v >= 0.8 => Some(2),
            v if v >= 0.7 => Some(1),
            v if v >= 0.5 => Some(0),
            _ => None,
        }
    };
    for (i, a) in sorted.iter().enumerate() {
        for b in sorted.iter().skip(i + 1).take(32) {
            let vj = multiset_jaccard_u64(&a.fp, &b.fp);
            if let Some(bi) = bin(vj) {
                tot[bi] += 1;
                eq[bi] += (a.beh == b.beh) as usize;
            }
        }
    }
    let labels = ["[.5,.7)", "[.7,.8)", "[.8,.9)", "[.9,1.)", "[1.0] "];
    println!("\nCALIBRATION — P(behavior-equal | value-Jaccard) [windowed sample]:");
    println!("  (the detector accepts an exact match [1.0]; this is how safe near-match is)");
    for i in (0..BINS).rev() {
        if tot[i] > 0 {
            println!(
                "  vj {} : {:>5}/{:<5} = {:>3.0}% behavior-equal",
                labels[i],
                eq[i],
                tot[i],
                100.0 * eq[i] as f64 / tot[i] as f64
            );
        }
    }
}

/// Multiset Jaccard over two sorted `u64` vectors (intersection / union by count).
fn multiset_jaccard_u64(a: &[u64], b: &[u64]) -> f64 {
    let (mut i, mut j, mut inter) = (0, 0, 0usize);
    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                inter += 1;
                i += 1;
                j += 1;
            }
        }
    }
    let union = a.len() + b.len() - inter;
    if union == 0 {
        1.0
    } else {
        inter as f64 / union as f64
    }
}

/// Dump each unit's detection features as JSON `{units: [...]}` — the raw value-graph,
/// shape, return and literal fingerprints, before candidate generation/thresholding.
fn cmd_features(
    paths: Vec<PathBuf>,
    min_lines: u32,
    min_tokens: usize,
    no_cfg_norm: bool,
    no_blocks: bool,
) -> Result<()> {
    let refs = paths_as_refs(&paths);
    let corpus = nose_frontend::lower_corpus_many(&refs);
    warn_if_empty(&corpus, &paths);
    let opts = nose_detect::DetectOptions {
        min_lines,
        min_tokens,
        cfg_norm: !no_cfg_norm,
        block_units: !no_blocks,
        ..Default::default()
    };
    let units: Vec<nose_detect::UnitFeat> = corpus
        .files
        .iter()
        .flat_map(|il| nose_detect::units_of_file(il, &corpus.interner, &opts))
        .collect();
    println!(
        "{}",
        serde_json::to_string(&serde_json::json!({ "units": units }))?
    );
    Ok(())
}

fn cmd_ceiling(gold: PathBuf, units: PathBuf, candidates: PathBuf) -> Result<()> {
    let gold_json = std::fs::read_to_string(&gold).with_context(|| format!("reading {gold:?}"))?;
    let units_json =
        std::fs::read_to_string(&units).with_context(|| format!("reading {units:?}"))?;
    let cands_json =
        std::fs::read_to_string(&candidates).with_context(|| format!("reading {candidates:?}"))?;
    let r = nose_eval::ceiling(&gold_json, &units_json, &cands_json)?;
    println!("{}", serde_json::to_string_pretty(&r)?);

    let pct = |n: usize, d: usize| {
        if d == 0 {
            0.0
        } else {
            100.0 * n as f64 / d as f64
        }
    };
    eprintln!(
        "\nrecall funnel ({} units, {} candidate pairs):",
        r.units, r.candidate_pairs
    );
    for (name, s) in [("all", &r.all), ("type4_semantic", &r.type4_semantic)] {
        eprintln!(
            "  {name:<16} gold={:<4} unit-reachable={:<4} ({:.1}%)  candidate-reachable={:<4} ({:.1}%)",
            s.gold,
            s.unit_reachable,
            pct(s.unit_reachable, s.gold),
            s.candidate_reachable,
            pct(s.candidate_reachable, s.gold),
        );
    }
    Ok(())
}

fn cmd_eval(
    gold: PathBuf,
    predictions: PathBuf,
    hard_negatives: Option<PathBuf>,
    corpus: Option<PathBuf>,
) -> Result<()> {
    let gold_json = std::fs::read_to_string(&gold).with_context(|| format!("reading {gold:?}"))?;
    let preds_json = std::fs::read_to_string(&predictions)
        .with_context(|| format!("reading {predictions:?}"))?;
    let hn_json = match &hard_negatives {
        Some(p) => std::fs::read_to_string(p).with_context(|| format!("reading {p:?}"))?,
        None => String::new(),
    };
    let corpus_json = match &corpus {
        Some(p) => std::fs::read_to_string(p).with_context(|| format!("reading {p:?}"))?,
        None => String::new(),
    };

    let report = nose_eval::evaluate(&gold_json, &preds_json, &hn_json, &corpus_json)?;
    println!("{}", serde_json::to_string_pretty(&report)?);

    // headline numbers to stderr
    eprintln!(
        "\npredictions={} gold={}  HN-FP-rate={:.3} ({}/{})",
        report.prediction_count,
        report.gold_count,
        report.hn_fp_rate,
        report.hn_matched,
        report.hn_total
    );
    for (name, m) in &report.slices {
        eprintln!(
            "  slice {name:<26} P={:.4} R={:.4} F1={:.4} (gold {})",
            m.precision, m.recall, m.f1, m.gold
        );
    }
    for m in &report.macro_f1 {
        eprintln!(
            "  macro[{}]  dev_F1={:.4} ({} repos)  heldout_F1={:.4} ({} repos)",
            m.slice, m.dev_f1, m.dev_repos, m.heldout_f1, m.heldout_repos
        );
    }
    Ok(())
}

fn cmd_stats(paths: Vec<PathBuf>, top: usize, json: bool) -> Result<()> {
    let refs = paths_as_refs(&paths);
    let corpus = nose_frontend::lower_corpus_many(&refs);
    warn_if_empty(&corpus, &paths);
    let report = nose_frontend::coverage(&corpus, top);

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }

    println!(
        "files: {}   IL nodes: {}   Raw nodes: {} ({:.3}%)",
        report.files,
        report.total_nodes,
        report.raw_nodes,
        report.raw_ratio * 100.0
    );
    println!("\nper language (worst coverage first):");
    println!(
        "  {:<12} {:>7} {:>10} {:>9} {:>8}",
        "lang", "files", "nodes", "raw", "raw%"
    );
    for l in &report.per_lang {
        println!(
            "  {:<12} {:>7} {:>10} {:>9} {:>7.3}%",
            l.lang,
            l.files,
            l.nodes,
            l.raw_nodes,
            l.raw_ratio * 100.0
        );
    }
    println!("\ntop unhandled constructs (surface kind → Raw):");
    println!("  {:<12} {:<34} {:>8}", "lang", "surface_kind", "count");
    for u in &report.top_unhandled {
        println!("  {:<12} {:<34} {:>8}", u.lang, u.surface_kind, u.count);
    }
    Ok(())
}

fn cmd_capabilities() -> Result<()> {
    let report = CapabilitiesReport::current();
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

struct ScanArgs {
    paths: Vec<PathBuf>,
    top: Option<usize>,
    min_members: Option<usize>,
    min_value: Option<f64>,
    sort: Option<SortKey>,
    config: Option<PathBuf>,
    mode: Vec<ScanMode>,
    show: Vec<ShowView>,
    cache_dir: Option<PathBuf>,
    fail_on: Option<FailOn>,
    baseline: Option<PathBuf>,
    ignore_file: Option<PathBuf>,
    semantic_pack: Vec<PathBuf>,
    write_baseline: bool,
    format: ReportFormat,
    exclude: Vec<String>,
    min_size: Option<usize>,
    min_lines: Option<u32>,
}

struct ChannelDetector {
    name: &'static str,
    detectors: Vec<Box<dyn nose_detect::Detector>>,
}

impl nose_detect::Detector for ChannelDetector {
    fn name(&self) -> &str {
        self.name
    }

    fn score(&self, a: &nose_detect::UnitFeat, b: &nose_detect::UnitFeat) -> f64 {
        self.detectors
            .iter()
            .map(|d| d.score(a, b))
            .fold(0.0, f64::max)
    }
}

/// Lower + detect + rank clone families for a set of paths — the shared core of `scan`
/// and `review` (no cache, baseline, or presentation post-processing).
pub(crate) fn detect_families(
    paths: &[PathBuf],
    exclude: &[String],
    mode: Vec<ScanMode>,
    cfg_mode: Vec<ScanMode>,
    min_tokens: usize,
    min_lines: u32,
) -> Result<Vec<nose_detect::RefactorFamily>> {
    validate_exclude_globs(exclude)?;
    let refs = paths_as_refs(paths);
    let channels = ScanChannels::resolve(mode, cfg_mode)?;
    let opts = scan_detect_options(channels, min_tokens, min_lines);
    let detector = scan_detector(channels, &opts);
    let corpus = nose_frontend::lower_corpus_filtered(&refs, exclude);
    let report = nose_detect::detect(&corpus, &opts, detector.as_ref());
    let mut families = nose_detect::rank_families(&report);
    if channels.abstraction_only() {
        families.retain(|f| f.abstraction_witness.is_some());
    }
    Ok(families)
}

/// Detection options for the resolved scan channels — shared by `scan` and `review`.
fn scan_detect_options(
    channels: ScanChannels,
    min_tokens: usize,
    min_lines: u32,
) -> nose_detect::DetectOptions {
    nose_detect::DetectOptions {
        threshold: channels.threshold(),
        min_lines,
        min_tokens,
        contiguous_min_tokens: min_tokens,
        contiguous_min_lines: min_lines,
        structural: channels.structural(),
        contiguous: channels.syntax,
        // Near also generates VALUE candidates so behaviorally-convergent but shape-divergent
        // pairs (async `.then` ≡ await, impure loop ≡ comprehension) reach the candidate scorer —
        // they share no shape band, so shape-LSH alone would never propose them.
        value_candidates: channels.semantic || channels.near || channels.abstraction,
        shape_candidates: channels.near || channels.abstraction,
        shape_features: channels.near || channels.abstraction,
        abstraction_witnesses: channels.abstraction,
        ..Default::default()
    }
}

fn validate_exclude_globs(exclude: &[String]) -> Result<()> {
    if exclude.is_empty() {
        return Ok(());
    }
    let mut builder = ignore::overrides::OverrideBuilder::new(".");
    for glob in exclude {
        builder
            .add(&format!("!{glob}"))
            .with_context(|| format!("invalid exclude glob {glob:?}"))?;
    }
    builder.build().context("building exclude glob matcher")?;
    Ok(())
}

fn scan_detector(
    channels: ScanChannels,
    opts: &nose_detect::DetectOptions,
) -> Box<dyn nose_detect::Detector> {
    let mut detectors: Vec<Box<dyn nose_detect::Detector>> = Vec::new();
    if channels.semantic {
        detectors.push(Box::new(nose_detect::ExactBehaviorDetector));
    }
    if channels.near || channels.abstraction {
        detectors.push(Box::new(
            nose_detect::StructuralDetector::candidates(opts.jaccard_weight)
                .without_exact_behavior()
                .with_threshold(opts.threshold),
        ));
    }

    match detectors.len() {
        0 => Box::new(nose_detect::CopyPasteDetector),
        1 => detectors.pop().expect("one detector"),
        _ => Box::new(ChannelDetector {
            name: if channels.abstraction && !channels.near {
                "semantic+abstraction"
            } else if channels.abstraction {
                "semantic+near+abstraction"
            } else {
                "semantic+near"
            },
            detectors,
        }),
    }
}

/// Warn (to stderr) when discovery turned up nothing, so a mistyped path or an
/// unsupported tree doesn't masquerade as "no duplication found". Returns true if
/// the corpus is empty (caller may choose to stop early).
fn warn_if_empty(corpus: &Corpus, paths: &[PathBuf]) -> bool {
    if corpus.files.is_empty() {
        warn_no_files(paths);
        return true;
    }
    false
}

/// Render `file` relative to `cwd` when it sits underneath it; otherwise leave it
/// as-is (an absolute path outside cwd is more useful whole than mangled).
fn relativize(file: &str, cwd: &std::path::Path) -> String {
    std::path::Path::new(file)
        .strip_prefix(cwd)
        .ok()
        .and_then(|p| p.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| file.to_string())
}

fn relativize_loc(loc: &mut nose_detect::Loc, cwd: &std::path::Path) {
    loc.file = relativize(&loc.file, cwd);
    if let Some(parent) = &mut loc.enclosing_unit {
        parent.file = relativize(&parent.file, cwd);
        parent.refresh_unit_key();
    }
}

/// Stderr notice that discovery found nothing — so a mistyped path or unsupported
/// tree doesn't masquerade as "no duplication found".
fn warn_no_files(paths: &[PathBuf]) {
    let joined = paths
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    eprintln!(
        "warning: no supported source files found under: {joined}\n  \
         (supported extensions: py/pyi, js/jsx/mjs/cjs, ts/tsx/mts/cts, go, rs, java, c/h, rb, vue/svelte/html/htm)"
    );
}

fn semantic_pack_summary_line(packs: &nose_semantics::SemanticPackSet) -> Option<String> {
    let first_party_count = packs
        .packs()
        .iter()
        .filter(|pack| pack.source == nose_semantics::SemanticPackSource::CompiledFirstParty)
        .count();
    let local = packs
        .packs()
        .iter()
        .filter(|pack| pack.source == nose_semantics::SemanticPackSource::LocalManifest)
        .map(|pack| format!("{}@{} ({})", pack.id, pack.version, pack.influence.as_str()))
        .collect::<Vec<_>>();
    (!local.is_empty()).then(|| {
        format!(
            "semantic packs: {first_party_count} first-party default · {} local opt-in: {}",
            local.len(),
            local.join(", ")
        )
    })
}

fn cmd_scan(args: ScanArgs) -> Result<()> {
    // `--fail-on new` gates on families that are new/changed vs a baseline, so without
    // `--baseline` the gate could never fire — reject the combination instead of silently
    // passing (a CI gate that always succeeds is the worst failure mode).
    if matches!(args.fail_on, Some(FailOn::New)) && args.baseline.is_none() {
        anyhow::bail!(
            "--fail-on new requires --baseline (it gates on families new vs the baseline)"
        );
    }

    let refs = paths_as_refs(&args.paths);
    let settings = resolve_scan_settings(&args)?;
    let opts = scan_detect_options(settings.channels, settings.min_tokens, settings.min_lines);
    let detector = scan_detector(settings.channels, &opts);
    let (report, scope) = scan_report(&args, &refs, &settings.exclude, &opts, detector.as_ref());

    let mut families = nose_detect::rank_families(&report);
    if settings.channels.abstraction_only() {
        families.retain(|f| f.abstraction_witness.is_some());
    }
    families.retain(|f| f.members >= settings.min_members && f.value >= settings.min_value);
    // Show paths relative to the working directory — absolute paths are unreadable
    // in CI logs and reviews, and relative ones are clickable and portable.
    if let Ok(cwd) = std::env::current_dir() {
        for f in &mut families {
            for l in &mut f.locations {
                relativize_loc(l, &cwd);
            }
        }
    }
    weight_shared_lines(&mut families, &refs, &settings.exclude);
    let sort = settings.sort;
    families.sort_by(|a, b| {
        sort.score(b)
            .total_cmp(&sort.score(a))
            // Deterministic tie-breaks: raw value, then first site's location.
            .then(b.value.total_cmp(&a.value))
            .then_with(|| family_anchor(a).cmp(&family_anchor(b)))
    });
    // Baseline: write the current state, or hide already-accepted families so only
    // new/changed duplication is reported and gated.
    if args.write_baseline {
        return write_scan_baseline(&args, &families);
    }
    let baseline_comparison = if let Some(path) = args.baseline.as_ref() {
        let accepted = baseline::load(path)?;
        let comparison = BaselineComparison::new(path, &accepted, &families);
        families.retain(|f| !accepted.keys.contains(&baseline::family_key(f)));
        Some(comparison)
    } else {
        None
    };
    let (families, ignored_families) = partition_ignored(families, settings.ignore_set.as_ref());
    // Computed for EVERY output format: the JSON path used to skip this index,
    // so generated-header families reached agents as `recommended_surface:
    // "default"` while the human report hid them (#224 — the #216 audit's re2c
    // case). One head-read per discovered file, only when families exist.
    let mut families = families;
    let generated_sources = if !families.is_empty() {
        generated_source_index(&refs, &settings.exclude)
    } else {
        std::collections::HashSet::new()
    };
    for f in &mut families {
        for l in &mut f.locations {
            l.looks_generated = generated_sources.contains(&l.file);
        }
    }
    let families = families;

    // `--top 0` means "no limit": show every family (documented in docs/usage.md).
    let limit = if settings.top == 0 {
        usize::MAX
    } else {
        settings.top
    };
    let shown = families.iter().take(limit).collect::<Vec<_>>();
    let reportable_families = families
        .iter()
        .filter(|f| is_default_report_family(f, &generated_sources))
        .collect::<Vec<_>>();
    let shown_reportable = reportable_families
        .iter()
        .take(limit)
        .copied()
        .collect::<Vec<_>>();
    let omitted_note = surface_omission_note(&families, &generated_sources);

    render_scan_report(
        &args,
        &ScanReportView {
            scope: &scope,
            settings: &settings,
            families: &families,
            shown: &shown,
            reportable: &reportable_families,
            shown_reportable: &shown_reportable,
            baseline: baseline_comparison.as_ref(),
            ignored_families: &ignored_families,
            omitted_note: omitted_note.as_deref(),
            generated_sources: &generated_sources,
        },
    )?;
    if args.show.contains(&ShowView::Hotspots)
        && matches!(args.format, ReportFormat::Human | ReportFormat::Markdown)
    {
        print_hotspots_refs(&reportable_families);
    }
    // CI gate: report is already printed; a non-empty (filtered) family set is a
    // failure when --fail-on is set.
    enforce_scan_fail_on(
        &args,
        settings.channels,
        &reportable_families,
        baseline_comparison.as_ref(),
    );
    Ok(())
}

/// The scan settings after layering: CLI flag wins, else config file, else built-in
/// default.
struct ScanSettings {
    top: usize,
    min_members: usize,
    min_value: f64,
    sort: SortKey,
    channels: ScanChannels,
    min_lines: u32,
    min_tokens: usize,
    semantic_packs: nose_semantics::SemanticPackSet,
    exclude: Vec<String>,
    ignore_set: Option<ignores::IgnoreSet>,
}

fn resolve_scan_settings(args: &ScanArgs) -> Result<ScanSettings> {
    let cfg = config::load_scan(args.config.as_deref())?;
    let top = args.top.or(cfg.top).unwrap_or(30);
    let min_members = args.min_members.or(cfg.min_members).unwrap_or(2);
    let min_value = validate_min_value(args.min_value.or(cfg.min_value).unwrap_or(0.0))?;
    let sort = args.sort.or(cfg.sort).unwrap_or(SortKey::Extractability);
    let channels = ScanChannels::resolve(args.mode.clone(), cfg.mode)?;
    let min_lines = args.min_lines.or(cfg.min_lines).unwrap_or(5);
    let min_tokens = args.min_size.or(cfg.min_size).unwrap_or(24);
    let ignore_file = args.ignore_file.clone().or(cfg.ignore_file);
    let mut semantic_pack_paths = cfg.semantic_packs;
    semantic_pack_paths.extend(args.semantic_pack.iter().cloned());
    let semantic_packs = nose_semantics::SemanticPackSet::new_local(&semantic_pack_paths)?;
    // Excludes are additive: config patterns plus any given on the command line.
    let mut exclude = cfg.exclude;
    exclude.extend(args.exclude.iter().cloned());
    validate_exclude_globs(&exclude)?;
    let ignore_set = ignores::load_for_scan(ignore_file.as_deref())?;
    if let Some(ignore_set) = &ignore_set {
        ignore_set.warn_expired();
    }
    Ok(ScanSettings {
        top,
        min_members,
        min_value,
        sort,
        channels,
        min_lines,
        min_tokens,
        semantic_packs,
        exclude,
        ignore_set,
    })
}

/// With --cache-dir, build units per file through the on-disk cache (skips
/// parse/normalize/extract for unchanged files); otherwise lower the whole corpus.
fn scan_report(
    args: &ScanArgs,
    refs: &[&std::path::Path],
    exclude: &[String],
    opts: &nose_detect::DetectOptions,
    detector: &dyn nose_detect::Detector,
) -> (nose_detect::Report, ScanScope) {
    if let Some(dir) = &args.cache_dir {
        let cache::CachedUnits {
            units,
            streams,
            files,
            langs,
        } = time_lower(|| cache::build_units_cached(refs, exclude, opts, dir));
        if files == 0 {
            warn_no_files(&args.paths);
        }
        // The cache path supplies both cached unit features and syntax streams, so every
        // selected scan channel behaves the same as the non-cached path.
        let report = nose_detect::detect_from_units(units, files, &streams, opts, detector).0;
        (report, ScanScope { files, langs })
    } else {
        let corpus = time_lower(|| nose_frontend::lower_corpus_filtered(refs, exclude));
        warn_if_empty(&corpus, &args.paths);
        let scope = ScanScope::from_corpus(&corpus);
        (nose_detect::detect(&corpus, opts, detector), scope)
    }
}

/// Compute the honest shared-line count for each family, before ranking. This layer has
/// source access; the detector deals only in IL.
///
/// `shared_lines` (displayed) is the count of *all* lines invariant across the family
/// — including boilerplate, so it matches what `--show proposal` shows. For *ranking*
/// (`shared_weight`) we separate signal from noise: sum the IDF weight of the
/// substantive lines (non-trivial, and rare across the corpus — a `if err != nil {`
/// that appears in most files contributes ~0), then use that as a **gate** on the
/// full block. A family whose shared lines are all boilerplate/idiom has ~0
/// substantive weight → it scores ~0 however much it "shares"; a family with real
/// shared content is credited for its whole extractable block (boilerplate included).
/// Cross-language families have no shared *source* lines to diff, so they keep
/// `shared_weight = 0` and fall back to the structural estimate in `extractability()`.
/// Only same-language families with ≥2 sites get an honest shared-line count; the
/// rest keep the detector's structural estimate. Computing the corpus line-IDF means
/// re-reading every scanned file, so skip it entirely when no family qualifies (a
/// clean repo, or a run where `--min-value`/`--min-members` filtered everything) —
/// otherwise a quiet scan pays a full second corpus read for nothing.
fn weight_shared_lines(
    families: &mut [nose_detect::RefactorFamily],
    refs: &[&std::path::Path],
    exclude: &[String],
) {
    let needs_shared = |f: &nose_detect::RefactorFamily| f.languages == 1 && f.locations.len() >= 2;
    if !families.iter().any(needs_shared) {
        return;
    }
    let mut lines = FileLineCache::default();
    let idf = corpus_line_idf(refs, exclude, &mut lines);
    for f in families.iter_mut().filter(|f| needs_shared(f)) {
        // Difference evidence comes from the same first readable representative
        // pair the `params` count uses (locations[0] vs the first member that
        // reads), so the two fields stay mutually consistent.
        f.varying_spots = f.locations[1..]
            .iter()
            .find_map(|b| varying_spots_of(&f.locations[0], b, &mut lines))
            .unwrap_or_default();
        if let Some((shared, params)) = shared_lines_of(&f.locations, &mut lines) {
            let substantive: f64 = shared
                .iter()
                .filter(|l| !is_trivial_line(l))
                .map(|l| idf.weight(l))
                .sum();
            // Gate ramps 0→1 as substantive shared content goes 0→2 lines.
            let gate = (substantive / 2.0).clamp(0.0, 1.0);
            f.shared_lines = shared.len() as u32;
            f.shared_weight = shared.len() as f64 * gate;
            f.params = params;
        }
    }
}

fn write_scan_baseline(args: &ScanArgs, families: &[nose_detect::RefactorFamily]) -> Result<()> {
    let path = args
        .baseline
        .as_ref()
        .expect("--write-baseline requires --baseline");
    baseline::write(path, families, family_hint)
        .with_context(|| format!("writing baseline {}", path.display()))?;
    eprintln!(
        "nose: wrote baseline of {} families to {}",
        families.len(),
        path.display()
    );
    Ok(())
}

fn partition_ignored(
    families: Vec<nose_detect::RefactorFamily>,
    ignore_set: Option<&ignores::IgnoreSet>,
) -> (Vec<nose_detect::RefactorFamily>, Vec<IgnoredFamily>) {
    let Some(ignore_set) = ignore_set else {
        return (families, Vec::new());
    };
    let mut active = Vec::with_capacity(families.len());
    let mut ignored_families = Vec::new();
    for family in families {
        if let Some(ignore) = ignore_set.match_family(&family) {
            ignored_families.push(IgnoredFamily { family, ignore });
        } else {
            active.push(family);
        }
    }
    (active, ignored_families)
}

/// Everything the format arms need to render one scan's report.
struct ScanReportView<'a> {
    scope: &'a ScanScope,
    settings: &'a ScanSettings,
    families: &'a [nose_detect::RefactorFamily],
    shown: &'a [&'a nose_detect::RefactorFamily],
    reportable: &'a [&'a nose_detect::RefactorFamily],
    shown_reportable: &'a [&'a nose_detect::RefactorFamily],
    baseline: Option<&'a BaselineComparison>,
    ignored_families: &'a [IgnoredFamily],
    omitted_note: Option<&'a str>,
    generated_sources: &'a std::collections::HashSet<String>,
}

fn render_scan_report(args: &ScanArgs, view: &ScanReportView) -> Result<()> {
    let settings = view.settings;
    match args.format {
        ReportFormat::Json => {
            let json = ScanJsonReport::new(ScanJsonInput {
                scope: view.scope,
                sort: settings.sort,
                top: settings.top,
                families: view.families,
                shown: view.shown,
                baseline: view.baseline,
                ignore_set: settings.ignore_set.as_ref(),
                ignored_families: view.ignored_families,
                semantic_packs: &settings.semantic_packs,
                generated_sources: view.generated_sources,
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
        ReportFormat::Markdown => {
            // Scope line first — tells the reader what was actually scanned (so a small
            // count from `.gitignore`/`--exclude` pruning is visible, not a silent gap).
            println!("{}\n", view.scope.summary());
            if let Some(line) = semantic_pack_summary_line(&settings.semantic_packs) {
                println!("{line}\n");
            }
            print_refactor_markdown(
                view.reportable,
                view.shown_reportable,
                settings.channels,
                view.baseline,
                settings.ignore_set.as_ref(),
                view.ignored_families.len(),
                view.omitted_note,
            );
        }
        ReportFormat::Human => {
            println!("{}", view.scope.summary());
            if let Some(line) = semantic_pack_summary_line(&settings.semantic_packs) {
                println!("{line}");
            }
            if let Some(comparison) = view.baseline {
                println!("{}", comparison.summary.line());
            }
            if let Some(ignore_set) = &settings.ignore_set {
                println!("{}", ignore_set.summary(view.ignored_families.len()).line());
            }
            print_refactor_human(
                view.reportable,
                view.shown_reportable,
                settings.sort,
                settings.channels,
                args.show.contains(&ShowView::Diff),
                args.show.contains(&ShowView::Proposal),
                view.omitted_note,
            )
        }
        ReportFormat::Sarif => println!(
            "{}",
            refactor_sarif(view.shown_reportable, view.reportable.len())?
        ),
    }
    Ok(())
}

fn enforce_scan_fail_on(
    args: &ScanArgs,
    channels: ScanChannels,
    reportable: &[&nose_detect::RefactorFamily],
    baseline_comparison: Option<&BaselineComparison>,
) {
    if let (true, Some(comparison)) = (
        matches!(args.fail_on, Some(FailOn::New)) && !reportable.is_empty(),
        baseline_comparison,
    ) {
        let mut new_families = 0usize;
        let mut changed_families = 0usize;
        for family in reportable {
            match comparison.statuses.get(&baseline::family_key(family)) {
                Some(BaselineStatus::Changed) => changed_families += 1,
                Some(BaselineStatus::New) => new_families += 1,
                None => {}
            }
        }
        let reportable_families = new_families + changed_families;
        eprintln!(
            "\nnose: {} new and {} changed {} found (--fail-on new)",
            new_families,
            changed_families,
            channels.report_label(reportable_families)
        );
        std::process::exit(1);
    }
    if matches!(args.fail_on, Some(FailOn::Any)) && !reportable.is_empty() {
        eprintln!(
            "\nnose: {} {} found (--fail-on any)",
            reportable.len(),
            channels.report_label(reportable.len())
        );
        std::process::exit(1);
    }
}

fn print_hotspots_refs(families: &[&nose_detect::RefactorFamily]) {
    use std::collections::{HashMap, HashSet};
    // module -> (lines residing here that are in a family, distinct families touching it)
    let mut lines: HashMap<&str, u32> = HashMap::new();
    let mut fams: HashMap<&str, HashSet<usize>> = HashMap::new();
    for (fi, f) in families.iter().enumerate() {
        for l in &f.locations {
            let m = std::path::Path::new(&l.file)
                .parent()
                .and_then(|p| p.to_str())
                .unwrap_or("");
            *lines.entry(m).or_insert(0) += l.end_line.saturating_sub(l.start_line) + 1;
            fams.entry(m).or_default().insert(fi);
        }
    }
    if lines.is_empty() {
        return;
    }
    let mut ranked: Vec<(&str, u32, usize)> = lines
        .iter()
        .map(|(m, d)| (*m, *d, fams.get(m).map_or(0, |s| s.len())))
        .collect();
    // Most duplicated lines first; ties by family count, then path for determinism.
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then(b.2.cmp(&a.2)).then(a.0.cmp(b.0)));
    println!("\nduplication hotspots (modules by lines that sit in a clone family):");
    for (m, dup, n) in ranked.iter().take(10) {
        let module = if m.is_empty() { "." } else { m };
        println!("  ~{dup:>5} dup lines · {n:>3} families  {module}");
    }
}

/// Run the corpus discover+parse+lower step, printing its wall time under
/// `NOSE_TIME` (the in-pipeline stages report themselves; this covers the
/// frontend, which usually dominates and is otherwise invisible).
fn time_lower<T>(f: impl FnOnce() -> T) -> T {
    if std::env::var_os("NOSE_TIME").is_none() {
        return f();
    }
    let t0 = std::time::Instant::now();
    let out = f();
    eprintln!(
        "  [time] {:<12} {:>7.1}ms",
        "lower",
        t0.elapsed().as_secs_f64() * 1e3
    );
    out
}

fn total_dup_lines_refs(fs: &[&nose_detect::RefactorFamily]) -> u32 {
    fs.iter().map(|f| f.dup_lines).sum()
}

/// The surface an integration should treat this family as: the ranked
/// `recommended_surface`, except that a family whose every location sits in a
/// generated-header source reports as `generated` — the same families the
/// human report already omits from default output.
fn effective_surface(
    family: &nose_detect::RefactorFamily,
    generated_sources: &std::collections::HashSet<String>,
) -> &'static str {
    if family_all_generated_source(family, generated_sources) {
        "generated"
    } else {
        family.recommended_surface()
    }
}

fn is_default_report_family(
    family: &nose_detect::RefactorFamily,
    generated_sources: &std::collections::HashSet<String>,
) -> bool {
    family.recommended_surface() == "default"
        && !family_all_generated_source(family, generated_sources)
}

fn family_all_generated_source(
    family: &nose_detect::RefactorFamily,
    generated_sources: &std::collections::HashSet<String>,
) -> bool {
    !family.locations.is_empty()
        && family
            .locations
            .iter()
            .all(|loc| generated_sources.contains(&loc.file))
}

fn surface_omission_note(
    families: &[nose_detect::RefactorFamily],
    generated_sources: &std::collections::HashSet<String>,
) -> Option<String> {
    let generated = families
        .iter()
        .filter(|f| {
            f.recommended_surface() == "default"
                && family_all_generated_source(f, generated_sources)
        })
        .count();
    let review = families
        .iter()
        .filter(|f| f.recommended_surface() == "review")
        .count();
    let hidden = families
        .iter()
        .filter(|f| f.recommended_surface() == "hidden")
        .count();
    let debug = families
        .iter()
        .filter(|f| f.recommended_surface() == "debug")
        .count();
    let omitted = generated + review + hidden + debug;
    if omitted == 0 {
        return None;
    }
    if generated == 0 && review == 0 && hidden == 1 && debug == 0 {
        return Some("omitted 1 hidden proof-only family from default output".to_string());
    }
    let mut parts = Vec::new();
    if generated > 0 {
        parts.push(format!("{generated} generated-code"));
    }
    if review > 0 {
        parts.push(format!("{review} review"));
    }
    if hidden > 0 {
        parts.push(format!("{hidden} hidden"));
    }
    if debug > 0 {
        parts.push(format!("{debug} debug"));
    }
    let family_word = if omitted == 1 { "family" } else { "families" };
    Some(format!(
        "omitted {omitted} {family_word} from default output ({})",
        parts.join(", ")
    ))
}

fn generated_source_index(
    refs: &[&std::path::Path],
    exclude: &[String],
) -> std::collections::HashSet<String> {
    let cwd = std::env::current_dir().ok();
    let mut generated = std::collections::HashSet::new();
    for root in refs {
        for (path, _lang) in nose_frontend::discover_paths(root, exclude) {
            if !source_has_generated_header(&path) {
                continue;
            }
            generated.insert(path.clone());
            if let Some(cwd) = &cwd {
                generated.insert(relativize(&path, cwd));
            }
        }
    }
    generated
}

fn source_has_generated_header(file: &str) -> bool {
    let Some(lines) = std::fs::read_to_string(file).ok() else {
        return false;
    };
    lines.lines().take(8).any(is_generated_header_line)
}

fn is_generated_header_line(line: &str) -> bool {
    let line = line.trim().to_ascii_lowercase();
    line.contains("@generated")
        || line.contains("generated by")
        || line.contains("code generated")
        || line.contains("automatically generated")
        || line.contains("auto-generated")
        || line.contains("autogenerated")
        || (line.contains("generated") && line.contains("do not edit"))
}

/// Build a SARIF 2.1.0 document — one result per family, every member site a
/// location so GitHub code-scanning annotates each. The first location is primary;
/// the rest are `relatedLocations`.
/// `shown` is the (possibly `--top`-truncated) slice that gets emitted; `total` is the
/// full active-family count before truncation. A SARIF consumer (GitHub code scanning)
/// otherwise can't tell a truncated upload from a complete one, so the run carries both
/// counts in `properties` and — when families were hidden — a `note` notification telling
/// the reader to pass `--top 0` for the full set.
fn refactor_sarif(shown: &[&nose_detect::RefactorFamily], total: usize) -> Result<String> {
    use serde_json::json;
    let phys = |l: &nose_detect::Loc| {
        json!({
            "physicalLocation": {
                "artifactLocation": { "uri": l.file },
                "region": { "startLine": l.start_line, "endLine": l.end_line }
            }
        })
    };
    let results: Vec<_> = shown
        .iter()
        .map(|f| {
            let msg = format!(
                "{} — {} sites, {} files, ~{} duplicated lines (sim {:.2})",
                family_hint(f),
                f.members,
                f.files,
                f.dup_lines,
                f.mean_score
            );
            json!({
                "ruleId": "duplicate-family",
                "level": "warning",
                "message": { "text": msg },
                "locations": f.locations.first().map(phys).into_iter().collect::<Vec<_>>(),
                "relatedLocations": f.locations.iter().skip(1).map(phys).collect::<Vec<_>>(),
                "properties": { "family_id": baseline::family_id(f) },
            })
        })
        .collect();
    let mut run = json!({
        "tool": { "driver": {
            "name": "nose",
            "informationUri": "https://github.com/",
            "version": env!("CARGO_PKG_VERSION"),
            "rules": [{
                "id": "duplicate-family",
                "name": "DuplicateFamily",
                "shortDescription": { "text": "Duplicated code worth refactoring" }
            }]
        }},
        "results": results,
        "properties": { "total_families": total, "shown_families": shown.len() },
    });
    if shown.len() < total {
        run["invocations"] = json!([{
            "executionSuccessful": true,
            "toolExecutionNotifications": [{
                "level": "note",
                "message": { "text": format!(
                    "Showing {} of {total} clone families (the --top limit). \
                     Pass --top 0 to emit every family.",
                    shown.len()
                ) }
            }]
        }]);
    }
    let doc = json!({
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "version": "2.1.0",
        "runs": [run],
    });
    Ok(serde_json::to_string_pretty(&doc)?)
}

/// Distinct languages in a family, sorted — e.g. `"python, typescript"`. Empty
/// when the family is single-language (caller decides whether to show anything).
fn family_langs(f: &nose_detect::RefactorFamily) -> String {
    if f.languages <= 1 {
        return String::new();
    }
    let mut langs: Vec<&str> = f.locations.iter().map(|l| l.lang.as_str()).collect();
    langs.sort_unstable();
    langs.dedup();
    langs.join(", ")
}

/// A short, fact-grounded refactoring hint for a family — only from signals the
/// report already establishes (a shared symbol name, cross-language spread, the
/// number of modules), never a guess about semantics.
fn family_hint(f: &nose_detect::RefactorFamily) -> String {
    use nose_il::UnitKind;
    // If every named site shares one identifier, it's the same thing copied.
    let mut names = f.locations.iter().filter_map(|l| l.name.as_deref());
    let shared_name = names.next().filter(|first| {
        f.locations.iter().filter(|l| l.name.is_some()).count() == f.members
            && names.all(|n| n == *first)
    });

    let cross = if f.languages > 1 {
        " (cross-language)"
    } else {
        ""
    };
    // The unit that all/most sites are: classes → a base class/mixin; blocks → a
    // method extracted from the repeated region; functions/methods → a helper.
    let all_classes = f.locations.iter().all(|l| l.kind == UnitKind::Class);
    let all_blocks = f.locations.iter().all(|l| l.kind == UnitKind::Block);
    // A computation-poor "class" unit is really a type/interface/enum/schema
    // declaration (lowered to a `Class` skeleton); its refactor is "move to one shared
    // type", not "extract a function with parameters".
    let type_decl = all_classes && f.mean_sem < 12.0;
    let extract = if type_decl {
        "consolidate into one shared type"
    } else if all_classes {
        "extract a shared base class / mixin"
    } else if all_blocks {
        "extract a method from the repeated block"
    } else {
        "extract a helper"
    };

    match (shared_name, f.modules) {
        (Some(name), _) => format!("consolidate `{name}` — {} copies{cross}", f.members),
        (None, m) if m >= 3 && all_classes => {
            format!("repeated across {m} modules — {extract}{cross}")
        }
        (None, m) if m >= 3 => {
            format!("repeated across {m} modules — extract a shared abstraction{cross}")
        }
        (None, m) if m >= 2 => format!("duplicated across {m} modules — {extract}{cross}"),
        (None, _) => format!("local duplication — {extract}{cross}"),
    }
}

fn print_refactor_human(
    all: &[&nose_detect::RefactorFamily],
    shown: &[&nose_detect::RefactorFamily],
    sort: SortKey,
    mode: ScanChannels,
    diff: bool,
    proposal: bool,
    omitted_note: Option<&str>,
) {
    println!(
        "{} {}, ranked by {}  ·  ~{} duplicated lines  (showing {})",
        all.len(),
        mode.report_label(all.len()),
        sort_name(sort),
        total_dup_lines_refs(all),
        shown.len()
    );
    if let Some(note) = omitted_note {
        println!("{note}");
    }
    // Every site is listed (you can't act on a clone you can't see); only pathological
    // fanout is capped, with a pointer to the full machine-readable list.
    const SITE_CAP: usize = 30;
    for (i, f) in shown.iter().enumerate() {
        println!(
            "\n#{}  id {} · {}",
            i + 1,
            baseline::family_id(f),
            family_summary(f)
        );
        println!("    → {}", family_hint(f));
        if let Some(witness) = &f.abstraction_witness {
            println!("    witness {}", abstraction_witness_summary(witness));
        }
        for l in f.locations.iter().take(SITE_CAP) {
            let name = l
                .name
                .as_deref()
                .map(|n| format!("  {n}"))
                .unwrap_or_default();
            // For a partial / sub-DAG clone, point at where the shared computation sits here.
            let shared = match l.shared_subdag {
                Some((s, e)) if (s, e) != (l.start_line, l.end_line) => {
                    format!("  (shared computation: lines {s}-{e})")
                }
                _ => String::new(),
            };
            println!(
                "    {}:{}-{}{}{}",
                l.file, l.start_line, l.end_line, name, shared
            );
        }
        if f.locations.len() > SITE_CAP {
            println!(
                "    … and {} more sites (--format json lists every one)",
                f.locations.len() - SITE_CAP
            );
        }
        if diff && f.locations.len() >= 2 {
            print_member_diff(&f.locations[0], &f.locations[1]);
        }
        if proposal && f.locations.len() >= 2 {
            print_member_proposal(&f.locations[0], &f.locations[1], f.locations.len());
        }
    }
}

/// Print a unified diff between two family members' source — the few lines that
/// differ are what a reviewer needs to judge how cleanly the copies can be merged.
fn print_member_diff(a: &nose_detect::Loc, b: &nose_detect::Loc) {
    let (Some(la), Some(lb)) = (
        read_lines(&a.file, a.start_line, a.end_line),
        read_lines(&b.file, b.start_line, b.end_line),
    ) else {
        return;
    };
    println!(
        "     diff  {}:{}-{}  vs  {}:{}-{}",
        a.file, a.start_line, a.end_line, b.file, b.start_line, b.end_line
    );
    let ar: Vec<&str> = la.iter().map(String::as_str).collect();
    let br: Vec<&str> = lb.iter().map(String::as_str).collect();
    for (tag, line) in line_diff(&ar, &br) {
        println!("       {tag} {line}");
    }
}

/// Synthesize an *extraction proposal* from two representative copies: the lines
/// they share become the body of the shared helper, and each maximal run of differing
/// lines collapses to a `⟨param N⟩` placeholder — i.e. anti-unification at line
/// granularity. Turns "these are similar" into "extract this, parameterize these N
/// spots." The varying spots are the candidate parameters.
///
/// The skeleton is necessarily *pairwise* (the two largest copies). For a family with
/// more copies that's an upper bound: a third copy that diverges further shrinks the
/// truly-shared body and adds parameters, which is why the family's one-line summary —
/// computed as a *majority* intersection across all members — can report fewer shared
/// lines. `members` lets us say so rather than letting the two counts silently disagree.
fn print_member_proposal(a: &nose_detect::Loc, b: &nose_detect::Loc, members: usize) {
    let (Some(la), Some(lb)) = (
        read_lines(&a.file, a.start_line, a.end_line),
        read_lines(&b.file, b.start_line, b.end_line),
    ) else {
        return;
    };
    let ar: Vec<&str> = la.iter().map(String::as_str).collect();
    let br: Vec<&str> = lb.iter().map(String::as_str).collect();
    let (skeleton, shared, params) = anti_unify(&ar, &br);
    let scope = if members > 2 {
        format!(" (of the 2 largest of {members} copies; the rest may share fewer)")
    } else {
        String::new()
    };
    println!(
        "     proposal  extract a shared helper · {shared} shared lines · {params} parameter(s) vary{scope}"
    );
    for line in skeleton.iter().take(40) {
        println!("       │ {line}");
    }
}

/// Anti-unify two line-blocks at line granularity: the lines they share become the
/// body of the extracted helper, and each maximal run of differing lines collapses to
/// one `⟨param N⟩` placeholder (a candidate parameter). Returns the skeleton, the
/// count of shared (invariant) lines, and the parameter count. Shared by the
/// `--show proposal` view and by extractability ranking / honest shared-line reporting.
fn anti_unify(a: &[&str], b: &[&str]) -> (Vec<String>, u32, u32) {
    let diff = line_diff(a, b);
    let mut skeleton: Vec<String> = Vec::new();
    let mut shared = 0u32;
    let mut params = 0u32;
    let mut in_hole = false;
    let mut indent = "";
    for (tag, line) in &diff {
        if *tag == ' ' {
            in_hole = false;
            shared += 1;
            // remember the indentation to align the placeholder
            indent = &line[..line.len() - line.trim_start().len()];
            skeleton.push(line.clone());
        } else if !in_hole {
            in_hole = true;
            params += 1;
            skeleton.push(format!("{indent}⟨param {params}⟩"));
        }
    }
    (skeleton, shared, params)
}

/// The invariant (shared) source lines across a family, plus the parameter count — the
/// honest counterpart to structural similarity. Returns *all* shared lines, including
/// boilerplate (`if err != nil {`, `}`): when a family genuinely shares a block, that
/// boilerplate is part of the helper you'd extract. The caller separates signal from
/// noise by *gating* on the substantive (non-trivial, rare) shared lines — a family
/// that shares only boilerplate scores ~0, while one with real shared content is
/// credited for its whole block (this is what stops idioms from ranking yet still
/// credits a `resolve*()` trio that shares a 13-line skeleton around a few varying args).
///
/// The shared set is intersected over a *majority* of members (up to `MEMBER_CAP`), not
/// just the closest pair — so a diverging copy shrinks the count honestly rather than
/// the flattering pair count overstating `N of M shared`. Parameters come from the first
/// pair that reads (a lower bound on the varying spots). `None` if no pair reads.
fn shared_lines_of(
    locs: &[nose_detect::Loc],
    cache: &mut FileLineCache,
) -> Option<(Vec<String>, u32)> {
    // Lines invariant across the representative pair, plus the parameter count.
    let pair = |a: &nose_detect::Loc, b: &nose_detect::Loc, cache: &mut FileLineCache| {
        let la = cache.slice(&a.file, a.start_line, a.end_line)?;
        let lb = cache.slice(&b.file, b.start_line, b.end_line)?;
        let ar: Vec<&str> = la.iter().map(String::as_str).collect();
        let br: Vec<&str> = lb.iter().map(String::as_str).collect();
        let mut shared = Vec::new();
        let mut params = 0u32;
        let mut in_hole = false;
        for (tag, line) in &line_diff(&ar, &br) {
            if *tag == ' ' {
                in_hole = false;
                let t = line.trim();
                if !t.is_empty() {
                    shared.push(t.to_string());
                }
            } else if !in_hole {
                in_hole = true;
                params += 1;
            }
        }
        Some((shared, params))
    };
    const MEMBER_CAP: usize = 8;
    // Count, per representative-line, how many other members share it. Keep the lines
    // shared by a *majority* (≥60%) of members: honest about a diverging copy (a line
    // only one other member has is dropped) yet robust to a single outlier (a 6-copy
    // family isn't tanked because its 6th copy differs). Parameters come from the first
    // pair (a lower bound on the varying spots).
    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut n_others = 0usize;
    let mut params = 0u32;
    for b in locs.iter().skip(1).take(MEMBER_CAP - 1) {
        let Some((s, p)) = pair(&locs[0], b, cache) else {
            continue;
        };
        // Parameters come from the first pair that actually reads — keyed on
        // `n_others`, not the loop index, so an unreadable representative pair
        // doesn't silently drop the count to zero.
        if n_others == 0 {
            params = p;
        }
        n_others += 1;
        let uniq: std::collections::HashSet<String> = s.into_iter().collect();
        for l in uniq {
            *counts.entry(l).or_insert(0) += 1;
        }
    }
    if n_others == 0 {
        return None;
    }
    let need = ((n_others as f64) * 0.6).ceil().max(1.0) as usize;
    let mut shared: Vec<String> = counts
        .into_iter()
        .filter(|(_, c)| *c >= need)
        .map(|(l, _)| l)
        .collect();
    // Sort to a deterministic order: the caller sums `idf.weight()` over these lines,
    // and float addition isn't associative, so a `HashMap`-iteration order would make
    // `shared_weight` (and, via sort ties, the family order) vary run-to-run and across
    // thread counts — violating the byte-identical-output guarantee.
    shared.sort_unstable();
    Some((shared, params))
}

/// The varying spots between two location line-blocks (#223): each maximal
/// differing run in the line diff becomes one spot carrying both sides' ABSOLUTE
/// source-line ranges and trimmed, length-capped text — so an agent can see WHAT
/// an extracted helper would parameterize (e.g. "every spot is a data literal")
/// without opening files. Same diff the `params` count walks.
fn varying_spots_of(
    a: &nose_detect::Loc,
    b: &nose_detect::Loc,
    cache: &mut FileLineCache,
) -> Option<Vec<nose_detect::VaryingSpot>> {
    const SPOT_CAP: usize = 16;
    const TEXT_CAP: usize = 160;
    let la = cache.slice(&a.file, a.start_line, a.end_line)?;
    let lb = cache.slice(&b.file, b.start_line, b.end_line)?;
    let ar: Vec<&str> = la.iter().map(String::as_str).collect();
    let br: Vec<&str> = lb.iter().map(String::as_str).collect();
    let cap_text = |t: &str| {
        let t = t.trim();
        if t.len() > TEXT_CAP {
            let mut end = TEXT_CAP;
            while !t.is_char_boundary(end) {
                end -= 1;
            }
            format!("{}…", &t[..end])
        } else {
            t.to_string()
        }
    };
    let mut spots: Vec<nose_detect::VaryingSpot> = Vec::new();
    let (mut ai, mut bi) = (0u32, 0u32);
    let mut open = false;
    for (tag, line) in line_diff(&ar, &br) {
        match tag {
            ' ' => {
                open = false;
                ai += 1;
                bi += 1;
            }
            _ => {
                if !open {
                    open = true;
                    if spots.len() >= SPOT_CAP {
                        return Some(spots);
                    }
                    spots.push(nose_detect::VaryingSpot {
                        param: spots.len() as u32 + 1,
                        a_lines: None,
                        b_lines: None,
                        a_text: String::new(),
                        b_text: String::new(),
                    });
                }
                let spot = spots.last_mut().expect("opened above");
                if tag == '-' {
                    let abs = a.start_line + ai;
                    spot.a_lines = Some(match spot.a_lines {
                        None => (abs, abs),
                        Some((s, _)) => (s, abs),
                    });
                    if !spot.a_text.is_empty() {
                        spot.a_text.push(' ');
                    }
                    if spot.a_text.len() <= TEXT_CAP {
                        spot.a_text.push_str(&cap_text(&line));
                    }
                    ai += 1;
                } else {
                    let abs = b.start_line + bi;
                    spot.b_lines = Some(match spot.b_lines {
                        None => (abs, abs),
                        Some((s, _)) => (s, abs),
                    });
                    if !spot.b_text.is_empty() {
                        spot.b_text.push(' ');
                    }
                    if spot.b_text.len() <= TEXT_CAP {
                        spot.b_text.push_str(&cap_text(&line));
                    }
                    bi += 1;
                }
            }
        }
    }
    for s in &mut spots {
        s.a_text = cap_text(&s.a_text);
        s.b_text = cap_text(&s.b_text);
    }
    Some(spots)
}

/// A line with no extractable content on its own: blank, pure delimiters (`}`, `});`,
/// `)`), or a bare control keyword. Sharing one of these between two blocks says
/// nothing about whether they're the same code.
fn is_trivial_line(t: &str) -> bool {
    t.is_empty()
        || t.chars().all(|c| {
            matches!(
                c,
                '{' | '}' | '(' | ')' | '[' | ']' | ';' | ',' | ' ' | '\t'
            )
        })
        || matches!(
            t,
            "return" | "break" | "continue" | "else" | "else {" | "};" | "})" | "});"
        )
}

/// How *idiomatic* (pervasive) each source line is across the scanned corpus, by the
/// fraction of files it appears in. A line in a large fraction of files is a language
/// idiom (`if err != nil {`, a ubiquitous logging call) and earns ~0 weight; a line in
/// few files is specific and earns full weight — so a language idiom, however often it's
/// literally duplicated, can't rank as an extractable refactor, with no hardcoded
/// idiom list. The floor is generous (`LO`): ordinary cross-file duplication — the very
/// thing we want to surface — keeps full weight; only genuinely pervasive lines are
/// docked. This matters on small repos, where naive IDF would penalize everything.
struct LineIdf {
    df: std::collections::HashMap<String, u32>,
    n_files: f64,
}

impl LineIdf {
    fn weight(&self, line: &str) -> f64 {
        if self.n_files <= 1.0 {
            return 1.0; // single-file corpus: no frequency signal
        }
        let frac = self.df.get(line).copied().unwrap_or(1) as f64 / self.n_files;
        const LO: f64 = 0.25; // ≤25% of files: specific → full weight
        const HI: f64 = 0.60; // ≥60% of files: pervasive idiom → no weight
        ((HI - frac) / (HI - LO)).clamp(0.0, 1.0)
    }
}

/// Build the [`LineIdf`] by reading every scanned file once (through `cache`, which the
/// per-family diffs then reuse) and counting, per trimmed non-trivial line, how many
/// distinct files contain it.
fn corpus_line_idf(
    refs: &[&std::path::Path],
    exclude: &[String],
    cache: &mut FileLineCache,
) -> LineIdf {
    let mut df: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    let mut n_files = 0u32;
    for root in refs {
        for (path, _lang) in nose_frontend::discover_paths(root, exclude) {
            let Some(all) = cache.whole(&path) else {
                continue;
            };
            n_files += 1;
            let mut seen = std::collections::HashSet::new();
            for l in all {
                let t = l.trim();
                if !is_trivial_line(t) && seen.insert(t.to_string()) {
                    *df.entry(t.to_string()).or_insert(0) += 1;
                }
            }
        }
    }
    LineIdf {
        df,
        n_files: n_files.max(1) as f64,
    }
}

/// Deterministic ranking tie-break: a family's first site `(file, start line)`.
fn family_anchor(f: &nose_detect::RefactorFamily) -> (String, u32) {
    f.locations
        .first()
        .map(|l| (l.file.clone(), l.start_line))
        .unwrap_or_default()
}

/// Memoizes file contents (split into lines) so ranking many families that touch the
/// same files reads each file at most once. `None` for files that fail to read.
#[derive(Default)]
struct FileLineCache(std::collections::HashMap<String, Option<Vec<String>>>);

impl FileLineCache {
    /// All lines of `file`, reading and caching on first touch. `None` if unreadable.
    fn whole(&mut self, file: &str) -> Option<&[String]> {
        self.0
            .entry(file.to_string())
            .or_insert_with(|| {
                std::fs::read_to_string(file)
                    .ok()
                    .map(|t| t.lines().map(str::to_string).collect())
            })
            .as_deref()
    }

    /// Lines `start..=end` (1-based) of `file`.
    fn slice(&mut self, file: &str, start: u32, end: u32) -> Option<Vec<String>> {
        let all = self.whole(file)?;
        let (s, e) = (
            start.saturating_sub(1) as usize,
            (end as usize).min(all.len()),
        );
        (s < e).then(|| all[s..e].to_vec())
    }
}

/// Read lines `start..=end` (1-based) of `file` as raw strings.
fn read_lines(file: &str, start: u32, end: u32) -> Option<Vec<String>> {
    let text = std::fs::read_to_string(file).ok()?;
    let lines: Vec<&str> = text.lines().collect();
    let (s, e) = (
        start.saturating_sub(1) as usize,
        (end as usize).min(lines.len()),
    );
    (s < e).then(|| lines[s..e].iter().map(|l| l.to_string()).collect())
}

/// Minimal LCS line diff → `(' '|'-'|'+', line)`. Caps each side so the O(n·m)
/// table stays small on large members (the differing lines are what matter).
fn line_diff(a: &[&str], b: &[&str]) -> Vec<(char, String)> {
    const CAP: usize = 120;
    let a = &a[..a.len().min(CAP)];
    let b = &b[..b.len().min(CAP)];
    let (n, m) = (a.len(), b.len());
    let mut dp = vec![vec![0u16; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[i][j] = if a[i] == b[j] {
                dp[i + 1][j + 1] + 1
            } else {
                dp[i + 1][j].max(dp[i][j + 1])
            };
        }
    }
    let mut out = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if a[i] == b[j] {
            out.push((' ', a[i].to_string()));
            i += 1;
            j += 1;
        } else if dp[i + 1][j] >= dp[i][j + 1] {
            out.push(('-', a[i].to_string()));
            i += 1;
        } else {
            out.push(('+', b[j].to_string()));
            j += 1;
        }
    }
    out.extend(a[i..].iter().map(|l| ('-', l.to_string())));
    out.extend(b[j..].iter().map(|l| ('+', l.to_string())));
    out
}

fn print_refactor_markdown(
    all: &[&nose_detect::RefactorFamily],
    shown: &[&nose_detect::RefactorFamily],
    mode: ScanChannels,
    baseline: Option<&BaselineComparison>,
    ignore_set: Option<&ignores::IgnoreSet>,
    ignored_families: usize,
    omitted_note: Option<&str>,
) {
    println!("# {}\n", mode.markdown_title());
    println!(
        "{} families · ~{} duplicated lines · showing top {}\n",
        all.len(),
        total_dup_lines_refs(all),
        shown.len()
    );
    if let Some(note) = omitted_note {
        println!("{note}\n");
    }
    if let Some(comparison) = baseline {
        println!("{}\n", comparison.summary.line());
    }
    if let Some(ignore_set) = ignore_set {
        println!("{}\n", ignore_set.summary(ignored_families).line());
    }
    for (i, f) in shown.iter().enumerate() {
        let xlang = match family_langs(f) {
            s if s.is_empty() => String::new(),
            s => format!(" · cross-language: {s}"),
        };
        println!(
            "## {}. `{}` — {} sites, {} files, {} modules — ~{} dup lines ({}){}",
            i + 1,
            baseline::family_id(f),
            f.members,
            f.files,
            f.modules,
            f.dup_lines,
            similarity_cell(f),
            xlang
        );
        println!("\n*{}*\n", family_hint(f));
        if let Some(witness) = &f.abstraction_witness {
            println!("_witness: {}_\n", abstraction_witness_summary(witness));
        }
        for l in &f.locations {
            let name = l
                .name
                .as_deref()
                .map(|n| format!(" `{n}`"))
                .unwrap_or_default();
            println!("- `{}:{}-{}`{}", l.file, l.start_line, l.end_line, name);
        }
        println!();
    }
}

struct DetectArgs {
    paths: Vec<PathBuf>,
    min_lines: u32,
    min_tokens: usize,
    threshold: Option<f64>,
    candidates: bool,
    minhash_k: usize,
    bands: usize,
    no_cfg_norm: bool,
    dce: bool,
    no_blocks: bool,
    out: Option<PathBuf>,
    summary: bool,
    bench_schema: bool,
    repos_root: Option<PathBuf>,
    dump: Option<PathBuf>,
}

fn cmd_detect(args: DetectArgs) -> Result<()> {
    let refs = paths_as_refs(&args.paths);
    let corpus = time_lower(|| nose_frontend::lower_corpus_many(&refs));
    warn_if_empty(&corpus, &args.paths);

    let opts = nose_detect::DetectOptions {
        min_lines: args.min_lines,
        min_tokens: args.min_tokens,
        threshold: args
            .threshold
            .unwrap_or(if args.candidates { 0.70 } else { 0.86 }),
        minhash_k: args.minhash_k,
        bands: args.bands,
        cfg_norm: !args.no_cfg_norm,
        dce: args.dce,
        block_units: !args.no_blocks,
        ..Default::default()
    };
    let detector = if args.candidates {
        nose_detect::StructuralDetector::candidates(opts.jaccard_weight)
            .with_threshold(opts.threshold)
    } else {
        nose_detect::StructuralDetector::strict(opts.jaccard_weight).with_threshold(opts.threshold)
    };

    // Diagnostic dump: units + candidates + predictions to a directory.
    if let Some(dir) = &args.dump {
        let root = args
            .repos_root
            .as_ref()
            .context("--dump requires --repos-root")?;
        let (report, dump) = nose_detect::detect_with_dump(&corpus, &opts, &detector);
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;

        let units: Vec<nose_eval::UnitRegion> = dump
            .units
            .iter()
            .map(|u| match map_to_repo(root, &u.path) {
                // keep index alignment for unmappable units with a sentinel repo
                Some((repo, file)) => nose_eval::UnitRegion {
                    repo,
                    file,
                    start_line: u.start_line,
                    end_line: u.end_line,
                },
                None => nose_eval::UnitRegion {
                    repo: String::new(),
                    file: u.path.clone(),
                    start_line: u.start_line,
                    end_line: u.end_line,
                },
            })
            .collect();
        std::fs::write(
            dir.join("units.json"),
            serde_json::to_string(&nose_eval::UnitsDump { units })?,
        )?;
        std::fs::write(
            dir.join("candidates.json"),
            serde_json::to_string(&nose_eval::CandidatesDump {
                candidates: dump.candidates,
            })?,
        )?;
        let preds = to_benchmark_predictions(&report, root);
        std::fs::write(dir.join("predictions.json"), serde_json::to_string(&preds)?)?;
        eprintln!(
            "dump → {}: {} units, {} candidate pairs, {} predictions",
            dir.display(),
            report.metrics.units,
            report.metrics.candidate_pairs,
            preds.duplicates.len()
        );
        return Ok(());
    }

    let report = nose_detect::detect(&corpus, &opts, &detector);

    if args.bench_schema {
        let root = args
            .repos_root
            .as_ref()
            .context("--bench-schema requires --repos-root")?;
        let preds = to_benchmark_predictions(&report, root);
        let json = serde_json::to_string_pretty(&preds)?;
        match &args.out {
            Some(path) => {
                std::fs::write(path, json).with_context(|| format!("writing {}", path.display()))?
            }
            None => println!("{json}"),
        }
        eprintln!(
            "emitted {} benchmark-schema predictions",
            preds.duplicates.len()
        );
        return Ok(());
    }

    if args.summary {
        print_summary(&report);
        return Ok(());
    }

    let json = serde_json::to_string_pretty(&report)?;
    match args.out {
        Some(path) => {
            std::fs::write(&path, json).with_context(|| format!("writing {}", path.display()))?
        }
        None => println!("{json}"),
    }
    Ok(())
}

/// Map an absolute/scanned path to `(repo_id, repo_relative_path)` given the root
/// whose immediate children are repo ids.
fn map_to_repo(root: &std::path::Path, path: &str) -> Option<(String, String)> {
    let rel = std::path::Path::new(path).strip_prefix(root).ok()?;
    let mut comps = rel.components();
    let repo = comps.next()?.as_os_str().to_str()?.to_string();
    let relpath = comps.as_path().to_str()?.to_string();
    if relpath.is_empty() {
        return None;
    }
    Some((repo, relpath))
}

/// Convert nose's clone pairs into benchmark predictions (repo id +
/// repo-relative paths, 1-based lines, `nose_semantic` channel).
fn to_benchmark_predictions(
    report: &nose_detect::Report,
    root: &std::path::Path,
) -> nose_eval::Predictions {
    let mut duplicates = Vec::new();
    for d in &report.duplicates {
        let (lrepo, lfile) = match map_to_repo(root, &d.left.file) {
            Some(x) => x,
            None => continue,
        };
        let (rrepo, rfile) = match map_to_repo(root, &d.right.file) {
            Some(x) => x,
            None => continue,
        };
        duplicates.push(nose_eval::PredPair {
            repo: Some(lrepo.clone()),
            channel: Some("nose_semantic".to_string()),
            score: Some(d.score),
            left: nose_eval::PredRegion {
                repo: Some(lrepo),
                file: lfile,
                start_line: d.left.start_line,
                end_line: d.left.end_line,
                symbol: d.left.name.clone(),
            },
            right: nose_eval::PredRegion {
                repo: Some(rrepo),
                file: rfile,
                start_line: d.right.start_line,
                end_line: d.right.end_line,
                symbol: d.right.name.clone(),
            },
        });
    }
    nose_eval::Predictions {
        schema_version: "0.1.0".to_string(),
        tool: "nose".to_string(),
        duplicates,
    }
}

fn print_summary(report: &nose_detect::Report) {
    let m = &report.metrics;
    eprintln!(
        "nose [{}]: {} files, {} units, {} candidate pairs, {} accepted, {} clone groups",
        report.detector, m.files, m.units, m.candidate_pairs, m.accepted_pairs, m.groups
    );
    for (n, g) in report.groups.iter().enumerate() {
        println!(
            "\nGroup {} (score {:.3}, {} members):",
            n + 1,
            g.score,
            g.members.len()
        );
        for mem in &g.members {
            let name = mem.name.as_deref().unwrap_or("");
            println!(
                "  {}:{}-{}  [{}] {}",
                mem.file, mem.start_line, mem.end_line, mem.lang, name
            );
        }
    }
}

fn cmd_il(path: PathBuf, format: Format, normalized: bool, no_cfg_norm: bool) -> Result<()> {
    let path_str = path.to_string_lossy().to_string();
    let lang = Lang::from_path(&path_str)
        .with_context(|| format!("unsupported file extension: {path_str}"))?;
    let src = std::fs::read(&path).with_context(|| format!("reading {path_str}"))?;
    let interner = Interner::new();
    let raw = nose_frontend::lower_source(FileId(0), &path_str, &src, lang, &interner)?;
    let il = if normalized {
        let opts = nose_normalize::NormalizeOptions {
            cfg_norm: !no_cfg_norm,
            ..Default::default()
        };
        nose_normalize::normalize(&raw, &interner, &opts)
    } else {
        raw
    };

    match format {
        Format::Sexpr => {
            println!("{}", il.to_sexpr(il.root, &interner));
        }
        Format::Json => {
            println!("{}", serde_json::to_string_pretty(&il)?);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use nose_detect::{LineSpan, Loc, LocInit, RefactorFamily};

    fn fam(langs: usize, modules: usize, names: &[Option<&str>]) -> RefactorFamily {
        fam_kind(langs, modules, names, nose_il::UnitKind::Function)
    }

    fn fam_kind(
        langs: usize,
        modules: usize,
        names: &[Option<&str>],
        kind: nose_il::UnitKind,
    ) -> RefactorFamily {
        let locations = names
            .iter()
            .enumerate()
            .map(|(i, n)| {
                Loc::new(LocInit {
                    file: format!("m{i}/f.rs"),
                    source_span: LineSpan::new(1, 10),
                    lang: "rust".into(),
                    kind,
                    name: n.map(|s| s.to_string()),
                    sem: 50,
                    span_tokens: 50,
                })
            })
            .collect();
        RefactorFamily {
            value: 1.0,
            members: names.len(),
            files: names.len(),
            modules,
            languages: langs,
            mean_score: 0.9,
            mean_lines: 10,
            dup_lines: 10,
            shared_lines: 0,
            params: 0,
            shared_weight: 0.0,
            locations,
            mean_sem: 50.0,
            scope: "prod",
            discount: 1.0,
            abstraction_witness: None,
            witness: None,
            varying_spots: Vec::new(),
            semantic_laws: Vec::new(),
        }
    }

    #[test]
    fn verify_battery_budget_is_node_row_bounded() {
        assert!(
            !verify_battery_over_budget(2_000, 192),
            "the documented 2k x 192-row boundary stays inside the verify budget"
        );
        assert!(
            verify_battery_over_budget(2_001, 192),
            "one node beyond the boundary fails closed as battery-bail"
        );
        assert!(
            !verify_battery_over_budget(6_000, 1),
            "large units are allowed when the battery is tiny"
        );
    }

    #[test]
    fn shared_lines_params_come_from_first_successful_pair() {
        use std::io::Write;
        // The representative pair can be unreadable while a *later* pair reads fine
        // (e.g. a deleted/edited file among the family members). The parameter count
        // must then come from the first pair that actually reads — not be dropped
        // just because the readable pair wasn't iteration 0.
        let dir = std::env::temp_dir().join(format!("nose_slo_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let write = |name: &str, body: &str| {
            let p = dir.join(name);
            std::fs::File::create(&p)
                .unwrap()
                .write_all(body.as_bytes())
                .unwrap();
            p.to_string_lossy().to_string()
        };
        let f0 = write("a.rs", "AAA\nshared1\nshared2\n");
        let f2 = write("c.rs", "BBB\nshared1\nshared2\n");
        let missing = dir.join("missing.rs").to_string_lossy().to_string();

        let mk = |file: String| {
            Loc::new(LocInit {
                file,
                source_span: LineSpan::new(1, 3),
                lang: "rust".into(),
                kind: nose_il::UnitKind::Function,
                name: None,
                sem: 50,
                span_tokens: 50,
            })
        };
        // locs[1] (the first compared pair) is unreadable; locs[2] reads and differs
        // from the representative by one parameter line.
        let locs = vec![mk(f0), mk(missing), mk(f2)];
        let mut cache = FileLineCache(std::collections::HashMap::new());
        let (shared, params) = shared_lines_of(&locs, &mut cache).expect("a later pair reads");

        assert!(
            shared.contains(&"shared1".to_string()),
            "shared lines extracted: {shared:?}"
        );
        assert_eq!(
            params, 1,
            "params must come from the first successful pair, not iteration 0"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn hint_shared_name_consolidates() {
        let f = fam(1, 3, &[Some("series"), Some("series"), Some("series")]);
        assert_eq!(family_hint(&f), "consolidate `series` — 3 copies");
    }

    #[test]
    fn hint_cross_language_is_flagged() {
        let f = fam(2, 2, &[Some("parse"), Some("parse")]);
        assert!(family_hint(&f).ends_with("(cross-language)"));
    }

    #[test]
    fn hint_mixed_names_falls_back_to_spread() {
        let f = fam(1, 3, &[Some("replace"), Some("replaceOrAppend"), None]);
        assert_eq!(
            family_hint(&f),
            "repeated across 3 modules — extract a shared abstraction"
        );
    }

    #[test]
    fn hint_local_duplication() {
        let f = fam(1, 1, &[None, None]);
        assert_eq!(family_hint(&f), "local duplication — extract a helper");
    }

    #[test]
    fn hint_class_family_suggests_base_class() {
        let f = fam_kind(1, 3, &[None, None, None], nose_il::UnitKind::Class);
        assert_eq!(
            family_hint(&f),
            "repeated across 3 modules — extract a shared base class / mixin"
        );
    }

    #[test]
    fn hint_block_family_suggests_method() {
        let f = fam_kind(1, 1, &[None, None], nose_il::UnitKind::Block);
        assert_eq!(
            family_hint(&f),
            "local duplication — extract a method from the repeated block"
        );
    }
}
