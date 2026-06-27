use crate::legacy_prelude::*;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "nose",
    version,
    about = "nose finds duplication in code and docs.\n\
             nose finds; you judge. Run `nose query <path>` to explore.",
    long_about = "nose finds duplication in code and docs.\n\
                  nose finds; you judge. Run `nose query <path>` to explore."
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) cmd: Cmd,
}

#[derive(Subcommand)]
pub(crate) enum Cmd {
    /// Research interface for raw unit clone pairs/groups.
    /// Hidden: `query` is the user-facing command; `detect` is the strict/research
    /// and benchmark interface (`--bench-schema`, `--dump`, …).
    #[command(hide = true)]
    Detect {
        /// Paths to source files or directories (recursively analyzed).
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
        /// comparison operators, sync/async wrappers) for human triage. Use the
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
    /// Analyze a path and explore duplication families.
    ///
    /// nose finds duplication in code and docs.
    ///
    /// nose finds; you judge. Filter, group, sort, or open families to explore.
    #[command(
        override_usage = "nose query <path> [terms...] [OPTIONS]\n       nose query --root <path> --root <path> [terms...] [OPTIONS]"
    )]
    Query {
        /// Additional root path to analyze; repeat for multi-root queries.
        #[arg(short = 'r', long = "root", value_name = "PATH")]
        roots: Vec<PathBuf>,
        /// Legacy form: first bare argument is the root path and the rest are query
        /// terms. With --root/-r, every bare argument is a query term.
        #[arg(value_name = "PATH_OR_TERM")]
        positionals: Vec<String>,
        /// Output format (`human`, `json`, `markdown`, or `sarif`).
        #[arg(long, default_value = "human")]
        format: ReportFormat,
        /// Detection channels to run; omit for `syntax,semantic,near`. Pass a comma-list
        /// or repeat the flag; fuzzy channels take an inline threshold (`near:0.8`).
        #[arg(long, value_delimiter = ',')]
        mode: Vec<DetectionMode>,
        /// Ignore units smaller than this size, in IL tokens (the unit's node count). [default: 24]
        #[arg(long)]
        min_size: Option<usize>,
        /// Advanced: also require this many source lines (most uses only need --min-size). [default: 5]
        #[arg(long, hide = true)]
        min_lines: Option<u32>,
        /// Hide families whose refactoring value is below this (noise floor on large repos).
        #[arg(long, value_parser = parse_min_value)]
        min_value: Option<f64>,
        /// Keep only families with at least this many duplicated copies. [default: 2]
        #[arg(long)]
        min_members: Option<usize>,
        /// Skip paths matching a gitignore-style glob (repeatable). (.gitignore is already respected.)
        #[arg(long)]
        exclude: Vec<String>,
        /// Cache per-file analysis under this directory; re-runs reuse it for unchanged files.
        #[arg(long, value_name = "DIR")]
        cache_dir: Option<PathBuf>,
        /// Structured-ignore file for suppressed families; auto-read `nose.ignore.json` when present.
        #[arg(long, value_name = "FILE")]
        ignore_file: Option<PathBuf>,
        /// Local semantic-pack v0 manifest file or directory to load (repeatable; explicit opt-in).
        #[arg(long = "semantic-pack", value_name = "FILE_OR_DIR")]
        semantic_pack: Vec<PathBuf>,
        /// Read defaults from this config file (else `nose.toml`/`.nose.toml`).
        #[arg(long, value_name = "FILE")]
        config: Option<PathBuf>,
        /// CI gate — exit non-zero when default-surface families are reported: `any`, or
        /// `new` (only new/changed vs `--baseline`).
        #[arg(long, value_name = "WHAT")]
        fail_on: Option<FailOn>,
        /// Accepted-baseline file: hide already-recorded families so only new/changed
        /// duplication is shown and gated.
        #[arg(long, value_name = "FILE")]
        baseline: Option<PathBuf>,
        /// Write the current families to `--baseline` (accept today's state) and exit.
        #[arg(long, requires = "baseline")]
        write_baseline: bool,
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
        /// Paths to source files or directories (recursively analyzed).
        #[arg(required = true)]
        paths: Vec<PathBuf>,
        /// How many top unhandled surface kinds to list.
        #[arg(long, default_value_t = 30)]
        top: usize,
        /// Output format (`human` or `json`) — the same `--format` contract as `query` and `il`.
        #[arg(long, value_enum, default_value_t = StatsFormat::Human)]
        format: StatsFormat,
    },
    /// Rank remaining lowering gaps by affected files/units, not just Raw count.
    /// (Hidden — research.)
    #[command(hide = true)]
    GapImpact {
        /// Paths to source files or directories (recursively analyzed).
        #[arg(required = true)]
        paths: Vec<PathBuf>,
        /// How many surface rows to list.
        #[arg(long, default_value_t = 30)]
        top: usize,
        /// Output format (`human` or `json`).
        #[arg(long, value_enum, default_value_t = StatsFormat::Human)]
        format: StatsFormat,
    },
    /// Dump the IL for a source file — debug why two snippets do or don't converge.
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
    /// Emit the machine-readable capability contract for integrations.
    Capabilities,
    /// Validate semantic-pack v0 manifests and declared conformance fixtures.
    #[command(name = "semantic-pack")]
    SemanticPack {
        #[command(subcommand)]
        cmd: SemanticPackCmd,
    },
    /// Dump per-unit detection features (value-graph / shape / return fingerprints)
    /// as JSON — the raw signal, before candidate generation or thresholding. Lets a
    /// convergence evaluator measure representation-convergence and behavioral-separation
    /// directly on the fingerprints, free of gate/threshold/cluster confounds.
    /// (Hidden — research.)
    #[command(hide = true)]
    Features {
        /// Paths to source files or directories (recursively analyzed).
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
    /// Value-graph coverage census (#391 prevalence probe): per IL construct, how many
    /// `Opaque` fallbacks the value graph mints — the value-graph analog of `stats`'s lowering
    /// `Raw` ratio. Ranks which constructs the fingerprint cannot model (map reads, dynamic
    /// dispatch, …) by unproven mass. JSON only. (Hidden — research.)
    #[command(hide = true)]
    ValueCensus {
        /// Paths to source files or directories (recursively analyzed).
        #[arg(required = true)]
        paths: Vec<PathBuf>,
        /// Disable control-flow normalization (ablation).
        #[arg(long)]
        no_cfg_norm: bool,
    },
    /// Soundness oracle: verify that value-fingerprint-equal units actually compute
    /// the same thing. Interprets each function on a battery of inputs and reports any
    /// fingerprint-equal pair whose behavior differs (a false merge — the cardinal sin
    /// of a clone detector). Also reports completeness. (Hidden — research.)
    #[command(hide = true)]
    Verify {
        /// Paths to source files or directories (recursively analyzed).
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
        /// Write a local recall-loss diagnostics report. The artifact combines
        /// soundness gate numbers, oracle under-merges, fail-closed exclusions,
        /// and structured exact-admission rejections. It never sends data remotely
        /// and omits raw source snippets by default.
        #[arg(long)]
        recall_loss_report: Option<PathBuf>,
        /// Write a JSON census of the units the oracle could NOT interpret — exclusion
        /// reasons, the construct tags they carry, and how much fingerprint-merge mass
        /// is unverified per construct. The instrument for ranking oracle-coverage work
        /// by unverified-merge mass instead of by guess.
        #[arg(long)]
        exclusion_census: Option<PathBuf>,
        /// Run the falsification-driven distinguishing-input SEARCH (#317) IN ADDITION to the
        /// fixed battery: for each fingerprint-equal group the battery found equal, search a
        /// value-kind-rich input domain (two distinct strings/lists, int32-wrapping ints, float
        /// magnitudes, mined constants) for a row that distinguishes two members — a false merge
        /// the fixed battery's input STARVATION missed. Offline/opt-in; reports the search delta.
        #[arg(long)]
        falsify: bool,
    },
    /// EXPERIMENTAL Type-4 benchmark harness (research tool, not a stable interface).
    ///
    /// Measures a behavioral-equivalence ACCEPTANCE gate: groups interpretable units
    /// by their behavior on an input battery (two units are "accepted" iff identical
    /// on every input) and, against a Type-4 manifest, reports the recall it recovers
    /// BEYOND exact-fingerprint matching and the hard-negative false merges it would
    /// introduce. `--battery wide` is the larger bounded input domain.
    #[command(hide = true)]
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
pub(crate) enum SemanticPackCmd {
    /// Check local semantic-pack v0 manifests for structural conformance.
    Check {
        /// Semantic-pack manifest file or directory of direct `*.json` manifests.
        #[arg(required = true, value_name = "FILE_OR_DIR")]
        paths: Vec<PathBuf>,
        /// Output format.
        #[arg(long, default_value = "human")]
        format: semantic_pack::CheckFormat,
    },
    /// Report builtin semantic-pack adoption gates for optional/default lanes.
    #[command(name = "adoption-gates")]
    AdoptionGates {
        /// Output format.
        #[arg(long, default_value = "human")]
        format: semantic_pack::AdoptionGateFormat,
    },
    /// Report semantic-pack compatibility policy and fail-closed gates.
    Compatibility {
        /// Output format.
        #[arg(long, default_value = "human")]
        format: semantic_pack::CompatibilityFormat,
    },
    /// Report builtin semantic-pack declarations, conformance refs, and coverage gaps.
    Inventory {
        /// Output format.
        #[arg(long, default_value = "human")]
        format: semantic_pack::InventoryFormat,
    },
}

#[derive(Clone, Copy, PartialEq, clap::ValueEnum)]
pub(crate) enum BatteryKind {
    Standard,
    Wide,
}

#[derive(Clone, Copy, clap::ValueEnum)]
pub(crate) enum Format {
    Sexpr,
    Json,
}

#[derive(Clone, Copy, PartialEq, Default, clap::ValueEnum)]
pub(crate) enum StatsFormat {
    /// Human-readable coverage table.
    #[default]
    Human,
    /// Machine-readable JSON.
    Json,
}

pub(crate) struct QueryArgs {
    pub(crate) paths: Vec<PathBuf>,
    pub(crate) min_members: Option<usize>,
    pub(crate) min_value: Option<f64>,
    pub(crate) sort: Option<SortKey>,
    pub(crate) config: Option<PathBuf>,
    pub(crate) mode: Vec<DetectionMode>,
    pub(crate) cache_dir: Option<PathBuf>,
    pub(crate) fail_on: Option<FailOn>,
    pub(crate) baseline: Option<PathBuf>,
    pub(crate) ignore_file: Option<PathBuf>,
    pub(crate) semantic_pack: Vec<PathBuf>,
    pub(crate) write_baseline: bool,
    pub(crate) format: ReportFormat,
    pub(crate) exclude: Vec<String>,
    pub(crate) min_size: Option<usize>,
    pub(crate) min_lines: Option<u32>,
    pub(crate) scope: ScopeFilter,
}

/// `--scope`: which test-boundary side of the report to keep. An explicit
/// consumer choice (issue #264 asked to read production findings first), not a
/// worthiness call — the rubric's "location never excuses duplication" governs
/// labels, while this governs what one invocation displays and gates on.
#[derive(Clone, Copy, PartialEq, Default, clap::ValueEnum)]
pub(crate) enum ScopeFilter {
    /// Everything (the default).
    #[default]
    All,
    /// Drop all-test families; keep prod and mixed (a test↔prod leak is prod's
    /// problem).
    Prod,
    /// Only all-test families (e.g. hunting scaffolding to consolidate).
    Test,
}

impl ScopeFilter {
    pub(crate) fn keeps(self, family: &nose_detect::RefactorFamily) -> bool {
        match self {
            ScopeFilter::All => true,
            ScopeFilter::Prod => family.scope != "test",
            ScopeFilter::Test => family.scope == "test",
        }
    }
}
