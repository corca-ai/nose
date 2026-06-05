//! `nose` — multi-language code clone detector CLI.

mod baseline;
mod cache;
mod config;
mod ignores;
mod review;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use nose_il::{Corpus, FileId, Interner, Lang};
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
        #[arg(long)]
        threshold: Option<f64>,
        /// Candidate mode: disable the behavioral-precision gates and default the
        /// threshold to 0.70. Surfaces near-duplicate FAMILIES (locale classes,
        /// comparison operators, sync/async wrappers) for human review. Use the
        /// default strict path for behavioral-clone research runs.
        #[arg(long)]
        candidates: bool,
        /// MinHash signature length (LSH).
        #[arg(long, default_value_t = 128)]
        minhash_k: usize,
        /// LSH band count (more bands → catches lower-similarity candidates).
        #[arg(long, default_value_t = 32)]
        bands: usize,
        /// Disable control-flow normalization (ablation).
        #[arg(long)]
        no_cfg_norm: bool,
        /// Enable dead-code / dead-assignment elimination (experimental).
        #[arg(long)]
        dce: bool,
        /// Disable sub-function block units (loops/ifs/try). Blocks are ON by
        /// default — they lift recall and pool-precision by catching fragment-level
        /// clones; `--no-blocks` reverts to function/method/class units only.
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
        #[arg(long)]
        min_value: Option<f64>,
        /// Rank families by: `extractability` (how cleanly it folds into one helper —
        /// the default), `value` (raw duplicated volume), or `sites` (most copies).
        #[arg(long)]
        sort: Option<SortKey>,
        /// Read defaults from this config file (else `nose.toml`/`.nose.toml`).
        #[arg(long, value_name = "FILE")]
        config: Option<PathBuf>,
        /// Detection channels to run. Omit for `syntax,semantic`. If present, this
        /// replaces the default; pass a comma-list or repeat it, e.g.
        /// `--mode syntax,near` or `--mode syntax --mode semantic`. The `near` channel
        /// takes an optional acceptance threshold inline: `--mode near:0.8` (default 0.70).
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
        /// hidden from the report and don't trip `--fail`, so a run flags only *new*
        /// duplication — the way to adopt on a codebase that already has clones.
        #[arg(long, value_name = "FILE")]
        baseline: Option<PathBuf>,
        /// Structured ignore file for intentionally suppressed families. Defaults
        /// to `nose.ignore.json` when that file exists.
        #[arg(long, value_name = "FILE")]
        ignore_file: Option<PathBuf>,
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
        /// Disable sub-function block units (loops/ifs/try).
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
        /// behavior,trivial}]}` (the oracle's behavioral ground truth) instead of the
        /// soundness/completeness report. Used by the value-add evaluator.
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

/// One `--mode` channel. `near` carries its own acceptance threshold (`near:0.8`),
/// so there is no separate `--threshold` flag to mis-combine.
#[derive(Clone, Copy, PartialEq, serde::Deserialize)]
#[serde(try_from = "String")]
enum ScanMode {
    /// CPD-style syntax copy-paste runs (the Type-1/2 floor).
    Syntax,
    /// Exact value-fingerprint Type-4 semantic clones.
    Semantic,
    /// Fuzzy Type-3 near-duplicate candidates, with an optional acceptance threshold.
    Near(Option<f64>),
}

impl std::str::FromStr for ScanMode {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, String> {
        let s = s.trim();
        match s {
            "syntax" => Ok(ScanMode::Syntax),
            "semantic" => Ok(ScanMode::Semantic),
            "near" => Ok(ScanMode::Near(None)),
            _ => {
                if let Some(t) = s.strip_prefix("near:") {
                    let v: f64 = t
                        .parse()
                        .map_err(|_| format!("invalid near threshold in `{s}`"))?;
                    if !(0.0..=1.0).contains(&v) {
                        return Err(format!("near threshold must be in [0,1], got {v}"));
                    }
                    Ok(ScanMode::Near(Some(v)))
                } else {
                    Err(format!(
                        "unknown mode `{s}` (expected syntax, semantic, near, or near:T)"
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

fn parse_scan_mode(s: &str) -> std::result::Result<ScanMode, String> {
    s.parse()
}

#[derive(Clone, Copy)]
struct ScanChannels {
    syntax: bool,
    semantic: bool,
    near: bool,
    /// The `near:T` threshold, if one was given in the mode spec.
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
            threshold: None,
        };
        for mode in selected {
            match mode {
                ScanMode::Syntax => channels.syntax = true,
                ScanMode::Semantic => channels.semantic = true,
                ScanMode::Near(t) => {
                    channels.near = true;
                    if t.is_some() {
                        channels.threshold = t;
                    }
                }
            }
        }
        if !channels.syntax && !channels.semantic && !channels.near {
            anyhow::bail!("--mode must include at least one of syntax, semantic, or near");
        }
        Ok(channels)
    }

    fn structural(self) -> bool {
        self.semantic || self.near
    }

    fn report_label(self, count: usize) -> &'static str {
        let singular = count == 1;
        match (self.syntax, self.semantic, self.near, singular) {
            (true, false, false, true) => "syntax clone family",
            (true, false, false, false) => "syntax clone families",
            (false, true, false, true) => "semantic clone family",
            (false, true, false, false) => "semantic clone families",
            (false, false, true, true) => "near-duplicate family",
            (false, false, true, false) => "near-duplicate families",
            (_, _, _, true) => "clone family",
            (_, _, _, false) => "clone families",
        }
    }

    fn markdown_title(self) -> &'static str {
        match (self.syntax, self.semantic, self.near) {
            (true, false, false) => "Syntax Clone Families",
            (false, true, false) => "Semantic Clone Families",
            (false, false, true) => "Near-Duplicate Families",
            _ => "Clone Families",
        }
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
                stable: vec!["capabilities", "il", "review", "scan", "stats"],
            },
            schemas: CapabilitiesSchemas {
                capabilities: vec![CAPABILITIES_SCHEMA_VERSION],
                scan_json: vec![SCAN_JSON_SCHEMA_VERSION],
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
                    "sort",
                    "top",
                ],
                capabilities: scan_capability_flags(),
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
struct ScanJsonRanking {
    sort: &'static str,
    total_families: usize,
    shown_families: usize,
    limit: Option<usize>,
}

#[derive(serde::Serialize)]
struct ScanJsonFamily<'a> {
    family_id: String,
    #[serde(flatten)]
    family: &'a nose_detect::RefactorFamily,
    #[serde(skip_serializing_if = "Option::is_none")]
    baseline_status: Option<&'static str>,
}

#[derive(serde::Serialize)]
struct ScanJsonIgnoredFamily<'a> {
    family_id: String,
    #[serde(flatten)]
    family: &'a nose_detect::RefactorFamily,
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
            ranking: ScanJsonRanking {
                sort: input.sort.json_name(),
                total_families: input.families.len(),
                shown_families: input.shown.len(),
                limit: (input.top != 0).then_some(input.top),
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
            let current_members: std::collections::HashSet<_> =
                baseline::member_keys(family).into_iter().collect();
            if baseline
                .entries
                .iter()
                .filter(|entry| !current_keys.contains(&entry.key))
                .any(|entry| {
                    !entry.members.is_empty()
                        && entry.members.iter().any(|m| current_members.contains(m))
                })
            {
                changed_current.insert(key);
                for entry in baseline
                    .entries
                    .iter()
                    .filter(|entry| !current_keys.contains(&entry.key))
                {
                    if !entry.members.is_empty()
                        && entry.members.iter().any(|m| current_members.contains(m))
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
        Cmd::Detect {
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
        } => cmd_detect(DetectArgs {
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
        }),
        Cmd::Eval {
            gold,
            predictions,
            hard_negatives,
            corpus,
        } => cmd_eval(gold, predictions, hard_negatives, corpus),
        Cmd::Scan {
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
            write_baseline,
            format,
            exclude,
            min_size,
            min_lines,
        } => cmd_scan(ScanArgs {
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
            write_baseline,
            format,
            exclude,
            min_size,
            min_lines,
        }),
        Cmd::Review {
            paths,
            base,
            mode,
            min_size,
            min_lines,
            exclude,
            config,
            format,
            top,
            fail,
        } => {
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
                format,
                top,
                fail,
            })
        }
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
        } => cmd_verify(paths, no_cfg_norm, json, max_violations, leads),
        Cmd::BehavioralGate {
            paths,
            manifest,
            battery,
        } => cmd_behavioral_gate(paths, manifest, battery),
    }
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
    use nose_normalize::{run_unit, Behavior, NormalizeOptions, Value};
    use std::collections::hash_map::DefaultHasher;
    use std::collections::{HashMap, HashSet};
    use std::hash::{Hash, Hasher};

    let refs: Vec<&std::path::Path> = paths.iter().map(|p| p.as_path()).collect();
    let corpus = nose_frontend::lower_corpus_many(&refs);
    warn_if_empty(&corpus, &paths);
    let opts = NormalizeOptions::default();
    let oracle_opts = NormalizeOptions {
        oracle: true,
        ..opts
    };
    let battery = match battery_kind {
        BatteryKind::Standard => verify_battery(&verify_probes(&corpus)),
        BatteryKind::Wide => wide_battery(&verify_probes(&corpus)),
    };

    // One interpretable record per generated source file (each holds exactly one function).
    struct U {
        fp: Vec<u64>,
        beh_hash: u64,
        trivial: bool,
    }
    let mut units: HashMap<String, U> = HashMap::new();

    for il in &corpus.files {
        let n = nose_normalize::normalize(il, &corpus.interner, &opts);
        let core = nose_normalize::normalize(il, &corpus.interner, &oracle_opts);
        let mut core_func: HashMap<(u32, u32), nose_il::NodeId> = HashMap::new();
        let mut stk = vec![core.root];
        while let Some(x) = stk.pop() {
            if core.kind(x) == nose_il::NodeKind::Func {
                let s = core.node(x).span;
                core_func.entry((s.start_byte, s.end_byte)).or_insert(x);
            }
            stk.extend(core.children(x).iter().copied());
        }
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
            let mut beh: Vec<Behavior> = Vec::with_capacity(battery.len());
            let mut ok = true;
            for inputs in &battery {
                let row = apply_contracts(inputs, &contracts);
                match run_unit(&core, core_root, &row) {
                    Some(b) => beh.push(b),
                    None => {
                        ok = false;
                        break;
                    }
                }
            }
            if !ok {
                continue;
            }
            // Trivial behavior (constant / all-Err) is coincidental, never evidence of a
            // clone — exclude it from behavioral merging (matches the verify completeness gate).
            let distinct: HashSet<&Value> = beh.iter().map(|b| &b.ret).collect();
            let trivial = distinct.len() < 2
                || beh
                    .iter()
                    .all(|b| matches!(b.ret, Value::Null | Value::Err));
            let mut h = DefaultHasher::new();
            beh.hash(&mut h);
            units.insert(
                manifest_key(&il.meta.path),
                U {
                    fp,
                    beh_hash: h.finish(),
                    trivial,
                },
            );
        }
    }

    // Cross-reference the manifest's labeled pairs.
    #[derive(serde::Deserialize)]
    struct Side {
        path: String,
    }
    #[derive(serde::Deserialize)]
    struct Item {
        left: Side,
        right: Side,
        semantic_status: String,
        split: String,
    }
    #[derive(serde::Deserialize)]
    struct Manifest {
        items: Vec<Item>,
    }
    let m: Manifest = serde_json::from_str(&std::fs::read_to_string(&manifest)?)?;

    // Tally, restricted to pairs where BOTH units are interpretable (the slice this gate
    // can speak to). For each: did exact-fingerprint merge them? did the behavioral gate?
    struct Tally {
        pairs: usize,
        fp_merge: usize,
        beh_merge: usize,
        beh_only: usize, // behavioral merge that fingerprint missed (the leap value / cost)
    }
    impl Tally {
        fn new() -> Self {
            Tally {
                pairs: 0,
                fp_merge: 0,
                beh_merge: 0,
                beh_only: 0,
            }
        }
    }
    let mut pos = Tally::new();
    let mut neg = Tally::new();
    let mut pos_heldout = 0usize;
    let mut pos_heldout_beh_only = 0usize;
    let mut uninterp_pairs = 0usize;

    for it in &m.items {
        let (lk, rk) = (manifest_key(&it.left.path), manifest_key(&it.right.path));
        let (Some(lu), Some(ru)) = (units.get(&lk), units.get(&rk)) else {
            uninterp_pairs += 1;
            continue;
        };
        let positive = it.semantic_status == "equivalent";
        let t = if positive { &mut pos } else { &mut neg };
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
                pos_heldout_beh_only += 1;
            }
        }
        if positive && it.split == "heldout" {
            pos_heldout += 1;
        }
    }

    let kind = match battery_kind {
        BatteryKind::Standard => "standard (leap 2)",
        BatteryKind::Wide => "wide (leap 3)",
    };
    println!("=== behavioral-equivalence acceptance gate — battery: {kind} ===");
    println!("battery rows: {}", battery.len());
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
    Ok(())
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
) -> Result<()> {
    use nose_normalize::{run_unit, Behavior, NormalizeOptions, Value};
    use std::collections::HashMap;
    use std::hash::{Hash, Hasher};

    let refs: Vec<&std::path::Path> = paths.iter().map(|p| p.as_path()).collect();
    let corpus = nose_frontend::lower_corpus_many(&refs);
    warn_if_empty(&corpus, &paths);
    let opts = NormalizeOptions {
        cfg_norm: !no_cfg_norm,
        ..Default::default()
    };
    // Mine the literals the corpus branches on, to probe value-keyed branches. The
    // battery is identical for every unit (a function uses only its first `arity`
    // inputs), so behavior vectors are always length-comparable.
    let battery = verify_battery(&verify_probes(&corpus));

    // One record per interpretable unit.
    struct Rec {
        fp: Vec<u64>,
        beh: Vec<Behavior>,
        file: String,
        start: u32,
        end: u32,
        tokens: usize,
        loc: String,
    }
    let mut recs: Vec<Rec> = Vec::new();
    let mut total = 0usize;
    // CANON PRESERVATION: a stricter, pair-free soundness check — does the full
    // normalization pipeline preserve each unit's behavior vs the pre-canon core IL? A
    // mismatch is a behavior-changing canon bug, even if no corpus twin collides with it.
    let mut canon_checked = 0usize;
    let mut canon_violations: Vec<String> = Vec::new();

    let oracle_opts = NormalizeOptions {
        oracle: true,
        ..opts
    };
    for il in &corpus.files {
        let n = nose_normalize::normalize(il, &corpus.interner, &opts);
        // The behavioral ground truth comes from the pre-canonicalization core IL (so a
        // behavior-changing canon can't mask itself), matched to each fully-normalized
        // unit by source span. Walk the core IL once, indexing every Func by its byte span.
        let core = nose_normalize::normalize(il, &corpus.interner, &oracle_opts);
        let mut core_func: HashMap<(u32, u32), nose_il::NodeId> = HashMap::new();
        {
            let mut stk = vec![core.root];
            while let Some(x) = stk.pop() {
                if core.kind(x) == nose_il::NodeKind::Func {
                    let s = core.node(x).span;
                    core_func.entry((s.start_byte, s.end_byte)).or_insert(x);
                }
                stk.extend(core.children(x).iter().copied());
            }
        }
        for u in &n.units {
            let root = u.root;
            if n.kind(root) != nose_il::NodeKind::Func {
                continue;
            }
            total += 1;
            // The same function in the core IL (by span) — interpret THAT, not `n`.
            let span0 = n.node(root).span;
            let Some(&core_root) = core_func.get(&(span0.start_byte, span0.end_byte)) else {
                continue;
            };
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
            let (fp, contracts) =
                nose_normalize::value_fingerprint_and_contracts(&n, root, &corpus.interner);
            if fp.is_empty() {
                continue;
            }
            // Run the battery; the unit is interpretable only if every input runs.
            let mut beh = Vec::new();
            let mut ok = true;
            for inputs in &battery {
                let row = apply_contracts(inputs, &contracts);
                match run_unit(&core, core_root, &row) {
                    Some(b) => beh.push(b),
                    None => {
                        ok = false;
                        break;
                    }
                }
            }
            if !ok {
                continue;
            }
            // Stricter canon check: the SAME function interpreted on the fully-normalized
            // IL must agree with the core IL on every input — else a canon pass changed
            // behavior. (Only when the full IL is itself fully interpretable on the battery.)
            {
                let mut full_beh = Vec::with_capacity(battery.len());
                let mut full_ok = true;
                for inputs in &battery {
                    let row = apply_contracts(inputs, &contracts);
                    match run_unit(&n, root, &row) {
                        Some(b) => full_beh.push(b),
                        None => {
                            full_ok = false;
                            break;
                        }
                    }
                }
                if full_ok {
                    canon_checked += 1;
                    if full_beh != beh && canon_violations.len() < 20 {
                        let s = n.node(root).span;
                        canon_violations.push(format!("{}:{}", il.meta.path, s.start_line));
                    }
                }
            }
            let span = n.node(root).span;
            // Subtree node count — the same size signal the detector gates on, so the
            // value-add evaluator can restrict its gold to meaningful-size units.
            let mut tokens = 0usize;
            let mut stack = vec![root];
            while let Some(x) = stack.pop() {
                tokens += 1;
                stack.extend(n.children(x).iter().copied());
            }
            recs.push(Rec {
                fp,
                beh,
                file: il.meta.path.clone(),
                start: span.start_line,
                end: span.end_line,
                tokens,
                loc: format!("{}:{}", il.meta.path, span.start_line),
            });
        }
    }

    // Behavioral ground truth for the value-add evaluator: each interpretable unit with
    // a stable hash of its behavior battery (equal hash ⟺ behaviorally equal on the
    // battery) and whether that behavior is trivial (constant / all-Err — coincidental,
    // not evidence of a real clone). The evaluator groups by behavior to form gold clone
    // pairs, then scores jscpd and nose against them on equal footing.
    if json {
        let recs_json: Vec<_> = recs
            .iter()
            .map(|r| {
                let mut h = std::collections::hash_map::DefaultHasher::new();
                r.beh.hash(&mut h);
                let distinct: std::collections::HashSet<&Value> =
                    r.beh.iter().map(|b| &b.ret).collect();
                let trivial = distinct.len() < 2
                    || r.beh
                        .iter()
                        .all(|b| matches!(b.ret, Value::Null | Value::Err));
                serde_json::json!({
                    "file": r.file,
                    "start_line": r.start,
                    "end_line": r.end,
                    "tokens": r.tokens,
                    "behavior": format!("{:016x}", h.finish()),
                    "trivial": trivial,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({ "units": recs_json }))?
        );
        return Ok(());
    }

    println!("=== value-graph oracle (soundness + completeness) ===");
    println!(
        "units: {total} total, {} interpretable ({} excluded)",
        recs.len(),
        total - recs.len()
    );

    // --- Canon preservation: full-normalize behavior must equal pre-canon core behavior. ---
    println!("\nCANON PRESERVATION — normalization preserves behavior:");
    println!("  units checked (interpretable both ways): {canon_checked}");
    if canon_violations.is_empty() {
        println!("  PRESERVED: every canon-changed unit computes the same thing ✓");
    } else {
        println!(
            "  [!] {} unit(s) whose behavior CHANGED under canonicalization:",
            canon_violations.len()
        );
        for loc in &canon_violations {
            println!("    {loc}");
        }
    }

    // --- Soundness: fingerprint-equal ⟹ behavior-equal. ---
    let mut by_fp: HashMap<&[u64], Vec<&Rec>> = HashMap::new();
    for r in &recs {
        by_fp.entry(&r.fp).or_default().push(r);
    }
    let mut fp_groups = 0usize;
    let mut violations: Vec<(String, String, usize)> = Vec::new();
    for members in by_fp.values() {
        if members.len() < 2 {
            continue;
        }
        fp_groups += 1;
        let first = members[0];
        for r in &members[1..] {
            if r.beh != first.beh {
                let diff = r.beh.iter().zip(&first.beh).filter(|(a, b)| a != b).count();
                violations.push((first.loc.clone(), r.loc.clone(), diff));
            }
        }
    }
    println!("\nSOUNDNESS — fingerprint-equal ⟹ behavior-equal:");
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

    // --- Completeness: behavior-equal ⟹ fingerprint-equal (the under-merge / recall
    // direction). Restricted to *non-trivial* behaviors (the return value varies across
    // inputs and isn't uniformly Err/Null) — trivial functions agree coincidentally and
    // aren't evidence of a missed clone. A behavior group split across ≥2 fingerprints
    // is a real Type-4 clone the value graph fails to recognize. Behavior-equal on the
    // battery is necessary-not-sufficient for equivalence, so this is a lower bound on
    // completeness / upper bound on misses — but each surfaced pair is a concrete lead.
    let trivial = |beh: &[Behavior]| -> bool {
        let distinct: std::collections::HashSet<&Value> = beh.iter().map(|b| &b.ret).collect();
        distinct.len() < 2
            || beh
                .iter()
                .all(|b| matches!(b.ret, Value::Null | Value::Err))
    };
    let mut by_beh: HashMap<&[Behavior], Vec<&Rec>> = HashMap::new();
    for r in &recs {
        if !trivial(&r.beh) {
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
        let mut by_fp2: HashMap<&[u64], Vec<&&Rec>> = HashMap::new();
        for r in members {
            by_fp2.entry(&r.fp).or_default().push(r);
        }
        for sub in by_fp2.values() {
            let s = sub.len();
            fp_equal_pairs += s * (s - 1) / 2;
        }
        if by_fp2.len() > 1 {
            split_groups += 1;
            // one representative per distinct fingerprint; find the max-vj cross pair.
            // Sort the reps by location so the chosen pair (and so the printed output) is
            // deterministic: `by_fp2.values()` iterates a `HashMap` in an unspecified order
            // that varies across runs/thread counts, which would otherwise pick a different
            // max-vj pair on ties and break byte-identical output.
            let mut reps: Vec<&&Rec> = by_fp2.values().map(|v| v[0]).collect();
            reps.sort_by(|a, b| a.loc.cmp(&b.loc));
            let mut best = (0.0f64, &reps[0], &reps[0]);
            for i in 0..reps.len() {
                for j in (i + 1)..reps.len() {
                    let vj = multiset_jaccard_u64(&reps[i].fp, &reps[j].fp);
                    if vj >= best.0 {
                        best = (vj, &reps[i], &reps[j]);
                    }
                }
            }
            if best.0 >= 0.7 {
                near_groups += 1;
            }
            // Canonical orientation (smaller location first) so the pair reads identically
            // regardless of which rep the scan happened to encounter first.
            let (a, b) = if best.1.loc <= best.2.loc {
                (best.1.loc.clone(), best.2.loc.clone())
            } else {
                (best.2.loc.clone(), best.1.loc.clone())
            };
            misses.push((a, b, best.0));
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
    // D1: export the under-merged pairs as detection leads — oracle-discovered candidates the
    // detection campaign can turn into convergence proposals. Sorted by vj (already), so the
    // strongest (structurally-near AND behavior-equal) come first.
    if let Some(path) = &leads {
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
    }

    // --- Calibration: P(behavior-equal | value-Jaccard bin). The detector currently
    // trusts only an *exact* fingerprint match (vj = 1.0). This measures how safe it
    // would be to also accept *near* matches — for each vj band, the fraction of pairs
    // that are actually behavior-equal = the precision of accepting at that band. Pairs
    // are sampled by sorting units by fingerprint and comparing each to a window of
    // neighbors (so high-vj pairs are well represented, unlike uniform random pairs).
    let mut sorted: Vec<&Rec> = recs.iter().collect();
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
    // CI soundness gate: fail if false merges exceed the budget. The independent oracle thus
    // becomes a permanent regression gate on the detection campaign — a new canon that
    // introduces a real false merge pushes the count over budget here.
    if let Some(budget) = max_violations {
        if n_violations > budget {
            anyhow::bail!("verify gate: {n_violations} false merges exceed the budget of {budget}");
        }
        println!("\nGATE: {n_violations} ≤ {budget} false merges — OK ✓");
    }
    Ok(())
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
    let refs: Vec<&std::path::Path> = paths.iter().map(|p| p.as_path()).collect();
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
    let refs: Vec<&std::path::Path> = paths.iter().map(|p| p.as_path()).collect();
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
    let refs: Vec<&std::path::Path> = paths.iter().map(|p| p.as_path()).collect();
    let channels = ScanChannels::resolve(mode, cfg_mode)?;
    let threshold = if channels.near {
        channels.threshold.unwrap_or(0.70)
    } else {
        1.0
    };
    let opts = nose_detect::DetectOptions {
        threshold,
        min_lines,
        min_tokens,
        contiguous_min_tokens: min_tokens,
        contiguous_min_lines: min_lines,
        structural: channels.structural(),
        contiguous: channels.syntax,
        value_candidates: channels.semantic,
        shape_candidates: channels.near,
        shape_features: channels.near,
        ..Default::default()
    };
    let detector = scan_detector(channels, &opts);
    let corpus = nose_frontend::lower_corpus_filtered(&refs, exclude);
    let report = nose_detect::detect(&corpus, &opts, detector.as_ref());
    Ok(nose_detect::rank_families(&report))
}

fn scan_detector(
    channels: ScanChannels,
    opts: &nose_detect::DetectOptions,
) -> Box<dyn nose_detect::Detector> {
    let mut detectors: Vec<Box<dyn nose_detect::Detector>> = Vec::new();
    if channels.semantic {
        detectors.push(Box::new(nose_detect::ExactBehaviorDetector));
    }
    if channels.near {
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
            name: "semantic+near",
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
         (supported extensions: py, js, ts/tsx, go, rs, java, c/h, rb)"
    );
}

fn cmd_scan(args: ScanArgs) -> Result<()> {
    let refs: Vec<&std::path::Path> = args.paths.iter().map(|p| p.as_path()).collect();

    // Resolve each setting: CLI flag wins, else config file, else built-in default.
    let cfg = config::load_scan(args.config.as_deref())?;
    let top = args.top.or(cfg.top).unwrap_or(30);
    let min_members = args.min_members.or(cfg.min_members).unwrap_or(2);
    let min_value = args.min_value.or(cfg.min_value).unwrap_or(0.0);
    let sort = args.sort.or(cfg.sort).unwrap_or(SortKey::Extractability);
    let channels = ScanChannels::resolve(args.mode, cfg.mode)?;
    let threshold = if channels.near {
        channels.threshold.unwrap_or(0.70)
    } else {
        1.0
    };
    let min_lines = args.min_lines.or(cfg.min_lines).unwrap_or(5);
    let min_tokens = args.min_size.or(cfg.min_size).unwrap_or(24);
    let ignore_file = args.ignore_file.or(cfg.ignore_file);
    // Excludes are additive: config patterns plus any given on the command line.
    let mut exclude = cfg.exclude;
    exclude.extend(args.exclude.iter().cloned());
    let ignore_set = ignores::load_for_scan(ignore_file.as_deref())?;
    if let Some(ignore_set) = &ignore_set {
        ignore_set.warn_expired();
    }

    let opts = nose_detect::DetectOptions {
        threshold,
        min_lines,
        min_tokens,
        contiguous_min_tokens: min_tokens,
        contiguous_min_lines: min_lines,
        structural: channels.structural(),
        contiguous: channels.syntax,
        value_candidates: channels.semantic,
        shape_candidates: channels.near,
        shape_features: channels.near,
        ..Default::default()
    };
    let detector = scan_detector(channels, &opts);

    // With --cache-dir, build units per file through the on-disk cache (skips
    // parse/normalize/extract for unchanged files); otherwise lower the whole corpus.
    let (report, scope) = if let Some(dir) = &args.cache_dir {
        let cache::CachedUnits {
            units,
            streams,
            files,
            langs,
        } = time_lower(|| cache::build_units_cached(&refs, &exclude, &opts, dir));
        if files == 0 {
            warn_no_files(&args.paths);
        }
        // The cache path supplies both cached unit features and syntax streams, so every
        // selected scan channel behaves the same as the non-cached path.
        let report =
            nose_detect::detect_from_units(units, files, &streams, &opts, detector.as_ref()).0;
        (report, ScanScope { files, langs })
    } else {
        let corpus = time_lower(|| nose_frontend::lower_corpus_filtered(&refs, &exclude));
        warn_if_empty(&corpus, &args.paths);
        let scope = ScanScope::from_corpus(&corpus);
        (
            nose_detect::detect(&corpus, &opts, detector.as_ref()),
            scope,
        )
    };

    let mut families = nose_detect::rank_families(&report);
    families.retain(|f| f.members >= min_members && f.value >= min_value);
    // Show paths relative to the working directory — absolute paths are unreadable
    // in CI logs and reviews, and relative ones are clickable and portable.
    if let Ok(cwd) = std::env::current_dir() {
        for f in &mut families {
            for l in &mut f.locations {
                l.file = relativize(&l.file, &cwd);
            }
        }
    }
    // Compute the honest shared-line count for each family, then rank. This layer has
    // source access; the detector deals only in IL.
    //
    // `shared_lines` (displayed) is the count of *all* lines invariant across the family
    // — including boilerplate, so it matches what `--proposal` shows. For *ranking*
    // (`shared_weight`) we separate signal from noise: sum the IDF weight of the
    // substantive lines (non-trivial, and rare across the corpus — a `if err != nil {`
    // that appears in most files contributes ~0), then use that as a **gate** on the
    // full block. A family whose shared lines are all boilerplate/idiom has ~0
    // substantive weight → it scores ~0 however much it "shares"; a family with real
    // shared content is credited for its whole extractable block (boilerplate included).
    // Cross-language families have no shared *source* lines to diff, so they keep
    // `shared_weight = 0` and fall back to the structural estimate in `extractability()`.
    // Only same-language families with ≥2 sites get an honest shared-line count; the
    // rest keep the detector's structural estimate. Computing the corpus line-IDF means
    // re-reading every scanned file, so skip it entirely when no family qualifies (a
    // clean repo, or a run where `--min-value`/`--min-members` filtered everything) —
    // otherwise a quiet scan pays a full second corpus read for nothing.
    let needs_shared = |f: &nose_detect::RefactorFamily| f.languages == 1 && f.locations.len() >= 2;
    if families.iter().any(needs_shared) {
        let mut lines = FileLineCache::default();
        let idf = corpus_line_idf(&refs, &exclude, &mut lines);
        for f in families.iter_mut().filter(|f| needs_shared(f)) {
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
        let path = args
            .baseline
            .as_ref()
            .expect("--write-baseline requires --baseline");
        baseline::write(path, &families, family_hint)
            .with_context(|| format!("writing baseline {}", path.display()))?;
        eprintln!(
            "nose: wrote baseline of {} families to {}",
            families.len(),
            path.display()
        );
        return Ok(());
    }
    let baseline_comparison = args.baseline.as_ref().map(|path| {
        let accepted = baseline::load(path);
        let comparison = BaselineComparison::new(path, &accepted, &families);
        families.retain(|f| !accepted.keys.contains(&baseline::family_key(f)));
        comparison
    });
    let mut ignored_families = Vec::new();
    if let Some(ignore_set) = &ignore_set {
        let mut active = Vec::with_capacity(families.len());
        for family in families {
            if let Some(ignore) = ignore_set.match_family(&family) {
                ignored_families.push(IgnoredFamily { family, ignore });
            } else {
                active.push(family);
            }
        }
        families = active;
    }

    // `--top 0` means "no limit": show every family (documented in docs/usage.md).
    let limit = if top == 0 { usize::MAX } else { top };
    let shown = families.iter().take(limit).collect::<Vec<_>>();

    match args.format {
        ReportFormat::Json => {
            let json = ScanJsonReport::new(ScanJsonInput {
                scope: &scope,
                sort,
                top,
                families: &families,
                shown: &shown,
                baseline: baseline_comparison.as_ref(),
                ignore_set: ignore_set.as_ref(),
                ignored_families: &ignored_families,
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
        ReportFormat::Markdown => {
            // Scope line first — tells the reader what was actually scanned (so a small
            // count from `.gitignore`/`--exclude` pruning is visible, not a silent gap).
            println!("{}\n", scope.summary());
            print_refactor_markdown(
                &families,
                &shown,
                channels,
                baseline_comparison.as_ref(),
                ignore_set.as_ref(),
                ignored_families.len(),
            );
        }
        ReportFormat::Human => {
            println!("{}", scope.summary());
            if let Some(comparison) = &baseline_comparison {
                println!("{}", comparison.summary.line());
            }
            if let Some(ignore_set) = &ignore_set {
                println!("{}", ignore_set.summary(ignored_families.len()).line());
            }
            print_refactor_human(
                &families,
                &shown,
                sort,
                channels,
                args.show.contains(&ShowView::Diff),
                args.show.contains(&ShowView::Proposal),
            )
        }
        ReportFormat::Sarif => println!("{}", refactor_sarif(&shown, families.len())?),
    }
    if args.show.contains(&ShowView::Hotspots)
        && matches!(args.format, ReportFormat::Human | ReportFormat::Markdown)
    {
        print_hotspots(&families);
    }
    // CI gate: report is already printed; a non-empty (filtered) family set is a
    // failure when --fail is set.
    if let (true, Some(comparison)) = (
        matches!(args.fail_on, Some(FailOn::New)) && !families.is_empty(),
        baseline_comparison.as_ref(),
    ) {
        let mut new_families = 0usize;
        let mut changed_families = 0usize;
        for family in &families {
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
    if matches!(args.fail_on, Some(FailOn::Any)) && !families.is_empty() {
        eprintln!(
            "\nnose: {} {} found (--fail)",
            families.len(),
            channels.report_label(families.len())
        );
        std::process::exit(1);
    }
    Ok(())
}

/// Duplication hotspots: directories/modules ranked by how many of *their own*
/// lines participate in a clone family. Each family site contributes its span to
/// the module it physically lives in (and each distinct family touching the module
/// is tallied), so a module's score reflects the duplicated code actually sitting
/// there — not a family-wide figure smeared across every directory it spans. An
/// architecture-level view of *where* the duplication concentrates.
fn print_hotspots(families: &[nose_detect::RefactorFamily]) {
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

fn total_dup_lines(fs: &[nose_detect::RefactorFamily]) -> u32 {
    fs.iter().map(|f| f.dup_lines).sum()
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
    all: &[nose_detect::RefactorFamily],
    shown: &[&nose_detect::RefactorFamily],
    sort: SortKey,
    mode: ScanChannels,
    diff: bool,
    proposal: bool,
) {
    println!(
        "{} {}, ranked by {}  ·  ~{} duplicated lines  (showing {})",
        all.len(),
        mode.report_label(all.len()),
        sort_name(sort),
        total_dup_lines(all),
        shown.len()
    );
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
        for l in f.locations.iter().take(SITE_CAP) {
            let name = l
                .name
                .as_deref()
                .map(|n| format!("  {n}"))
                .unwrap_or_default();
            println!("    {}:{}-{}{}", l.file, l.start_line, l.end_line, name);
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
/// `--proposal` view and by extractability ranking / honest shared-line reporting.
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
/// pair (a lower bound on the varying spots). `None` if the first two spans can't read.
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
    for (i, b) in locs.iter().skip(1).take(MEMBER_CAP - 1).enumerate() {
        let Some((s, p)) = pair(&locs[0], b, cache) else {
            continue;
        };
        if i == 0 {
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
    all: &[nose_detect::RefactorFamily],
    shown: &[&nose_detect::RefactorFamily],
    mode: ScanChannels,
    baseline: Option<&BaselineComparison>,
    ignore_set: Option<&ignores::IgnoreSet>,
    ignored_families: usize,
) {
    println!("# {}\n", mode.markdown_title());
    println!(
        "{} families · ~{} duplicated lines · showing top {}\n",
        all.len(),
        total_dup_lines(all),
        shown.len()
    );
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
    let refs: Vec<&std::path::Path> = args.paths.iter().map(|p| p.as_path()).collect();
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
    use nose_detect::{Loc, RefactorFamily};

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
            .map(|(i, n)| Loc {
                file: format!("m{i}/f.rs"),
                start_line: 1,
                end_line: 10,
                lang: "rust".into(),
                kind,
                name: n.map(|s| s.to_string()),
                sem: 50,
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
        }
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
