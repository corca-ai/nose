//! `nose` — semantic IL transpiler + Type-4 clone detector CLI.

mod baseline;
mod cache;
mod config;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use nose_il::{Corpus, FileId, Interner, Lang};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "nose",
    version,
    about = "Find refactoring candidates and semantic (Type-4) code clones",
    long_about = "nose lowers each language into one normalized IL and finds duplicated code.\n\
                  • `nose scan <paths>` — ranked architecture/design-level refactoring candidates\n\
                  • `nose stats <paths>`    — IL lowering coverage\n\
                  • `nose il <file>`        — inspect the IL"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
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
    /// Detect Type-4 clones across one or more source trees (raw pairs/groups).
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
        /// Acceptance threshold in `[0,1]`. Defaults: 0.86 (strict behavioral mode),
        /// 0.70 (--candidates mode). Lower for recall, raise for precision.
        #[arg(long)]
        threshold: Option<f64>,
        /// Refactoring-candidate mode: disable the behavioral-precision gates and
        /// default the threshold to 0.70. Surfaces near-duplicate FAMILIES (locale
        /// classes, comparison operators, sync/async wrappers) for human review —
        /// ~99% review-worthy. Use the default (strict) mode for behavioral clones.
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
    /// Find architecture/design-level refactoring candidates, ranked by how much
    /// duplication they represent. Runs candidate mode and groups clones into
    /// families (e.g. a set of near-identical classes across modules), sorted by
    /// refactoring value (removable lines × similarity × cross-module spread).
    Scan {
        /// Paths to source files or directories (recursively scanned).
        #[arg(required = true)]
        paths: Vec<PathBuf>,
        /// How many top families to show. [default: 30]
        #[arg(long)]
        top: Option<usize>,
        /// Only families with at least this many duplicated sites. [default: 2]
        #[arg(long)]
        min_members: Option<usize>,
        /// Hide families whose refactoring value is below this (noise floor on
        /// large repos). 0 shows everything. [default: 0]
        #[arg(long)]
        min_value: Option<f64>,
        /// Read defaults from this config file (else `nose.toml`/`.nose.toml`).
        #[arg(long, value_name = "FILE")]
        config: Option<PathBuf>,
        /// After the report, print duplication hotspots — directories/modules
        /// ranked by total duplicated lines across families (architecture view).
        #[arg(long)]
        hotspots: bool,
        /// Cache per-file analysis under this directory. Re-runs reuse the cache for
        /// unchanged files (keyed by content hash), skipping parse/normalize/extract
        /// — much faster on repeated invocations (CI, pre-commit, iterating).
        #[arg(long, value_name = "DIR")]
        cache_dir: Option<PathBuf>,
        /// Exit with a non-zero status if any family is reported (after the filters
        /// above). Turns `scan` into a CI gate: e.g.
        /// `nose scan src --min-value 300 --min-members 3 --fail`.
        #[arg(long)]
        fail: bool,
        /// Baseline file of already-accepted families. Families recorded here are
        /// hidden from the report and don't trip `--fail`, so a run flags only *new*
        /// duplication — the way to adopt on a codebase that already has clones.
        #[arg(long, value_name = "FILE")]
        baseline: Option<PathBuf>,
        /// Write the current families to the `--baseline` file (accept today's state)
        /// and exit, instead of reporting.
        #[arg(long, requires = "baseline")]
        write_baseline: bool,
        /// Acceptance threshold in `[0,1]`. [default: 0.70]
        #[arg(long)]
        threshold: Option<f64>,
        /// Output format.
        #[arg(long, default_value = "human")]
        format: ReportFormat,
        /// Use strict behavioral-clone mode instead of candidate mode.
        #[arg(long)]
        strict: bool,
        /// Show each family's source inline (human format) as a unified diff between
        /// its two representative members — both copies and exactly what differs.
        #[arg(long)]
        diff: bool,
        /// Show an extraction proposal per family (human format): the shared skeleton
        /// of two copies with the varying spots collapsed to ⟨param N⟩ placeholders —
        /// i.e. what to extract and how many parameters it needs.
        #[arg(long)]
        proposal: bool,
        /// Skip files matching a gitignore-style glob (repeatable), e.g.
        /// `--exclude tests --exclude 'vendor/**' --exclude '**/*.generated.ts'`.
        /// (.gitignore is already respected automatically.)
        #[arg(long)]
        exclude: Vec<String>,
        /// Ignore units smaller than this (structural size in IL nodes) — the single
        /// minimum-size knob. [default: 24]
        #[arg(long)]
        min_tokens: Option<usize>,
        /// Disable the contiguous copy-paste channel (the Type-1/2 floor that finds
        /// duplicated runs regardless of unit boundaries). Leaves only the structural
        /// (design-level) families. On by default.
        #[arg(long)]
        no_contiguous: bool,
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
    },
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum Format {
    Sexpr,
    Json,
}

#[derive(Clone, Copy, PartialEq, clap::ValueEnum)]
enum ReportFormat {
    /// Ranked, human-readable terminal report.
    Human,
    /// Machine-readable JSON (one object per family).
    Json,
    /// Markdown report (for PRs / issues / docs).
    Markdown,
    /// SARIF 2.1.0 (GitHub code-scanning / PR annotations).
    Sarif,
}

/// Stack size for the worker pool and the main worker thread. Lowering/normalization
/// walk the syntax tree recursively, so a pathologically deep file (minified bundle,
/// generated code) can need a deep stack — far more than the default ~2 MB (rayon
/// worker) or ~8 MB (main). Sized generously so nose never crashes on real repos.
/// Virtual only; pages commit lazily. See `deeply_nested_file_does_not_overflow`.
const STACK_SIZE: usize = 1024 * 1024 * 1024;

fn main() -> Result<()> {
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

fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Il {
            path,
            format,
            normalized,
            no_cfg_norm,
        } => cmd_il(path, format, normalized, no_cfg_norm),
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
            config,
            hotspots,
            cache_dir,
            fail,
            baseline,
            write_baseline,
            threshold,
            format,
            strict,
            diff,
            proposal,
            exclude,
            min_tokens,
            no_contiguous,
        } => cmd_scan(ScanArgs {
            paths,
            top,
            min_members,
            min_value,
            config,
            hotspots,
            cache_dir,
            fail,
            baseline,
            write_baseline,
            threshold,
            format,
            strict,
            diff,
            proposal,
            exclude,
            min_tokens,
            no_contiguous,
        }),
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
        } => cmd_verify(paths, no_cfg_norm, json),
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
    let mut probes: Vec<Value> = top(strs, 16).into_iter().map(|h| Value::Str(vec![h])).collect();
    probes.extend(top(ints, 16).into_iter().map(Value::Int));
    probes
}

fn cmd_verify(paths: Vec<PathBuf>, no_cfg_norm: bool, json: bool) -> Result<()> {
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
            let fp = nose_normalize::value_fingerprint(&n, root, &corpus.interner);
            if fp.is_empty() {
                continue;
            }
            // Run the battery; the unit is interpretable only if every input runs.
            let mut beh = Vec::new();
            let mut ok = true;
            for inputs in &battery {
                match run_unit(&core, core_root, inputs) {
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
                    match run_unit(&n, root, inputs) {
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
                        canon_violations
                            .push(format!("{}:{}", il.meta.path, s.start_line));
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
                    || r.beh.iter().all(|b| matches!(b.ret, Value::Null | Value::Err));
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
        println!("{}", serde_json::to_string(&serde_json::json!({ "units": recs_json }))?);
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
    if violations.is_empty() {
        println!("  SOUND: no false merges ✓");
    } else {
        println!("  [!] {} VIOLATION(S) (false merges):", violations.len());
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
            let reps: Vec<&&Rec> = by_fp2.values().map(|v| v[0]).collect();
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
            misses.push((best.1.loc.clone(), best.2.loc.clone(), best.0));
        }
    }
    misses.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());
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

struct ScanArgs {
    paths: Vec<PathBuf>,
    top: Option<usize>,
    min_members: Option<usize>,
    min_value: Option<f64>,
    config: Option<PathBuf>,
    hotspots: bool,
    cache_dir: Option<PathBuf>,
    fail: bool,
    baseline: Option<PathBuf>,
    write_baseline: bool,
    threshold: Option<f64>,
    format: ReportFormat,
    strict: bool,
    diff: bool,
    proposal: bool,
    exclude: Vec<String>,
    min_tokens: Option<usize>,
    no_contiguous: bool,
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
    let threshold = args.threshold.or(cfg.threshold).unwrap_or(0.70);
    let min_lines = cfg.min_lines.unwrap_or(5);
    let min_tokens = args.min_tokens.or(cfg.min_tokens).unwrap_or(24);
    // Excludes are additive: config patterns plus any given on the command line.
    let mut exclude = cfg.exclude;
    exclude.extend(args.exclude.iter().cloned());

    let opts = nose_detect::DetectOptions {
        threshold,
        min_lines,
        min_tokens,
        // The contiguous copy-paste channel runs for `scan` (the Type-1/2 floor
        // a token detector catches); it stays off for the strict/gold `detect` path.
        contiguous: !args.no_contiguous,
        ..Default::default()
    };
    let detector = if args.strict {
        nose_detect::StructuralDetector::strict(opts.jaccard_weight).with_threshold(opts.threshold)
    } else {
        nose_detect::StructuralDetector::candidates(opts.jaccard_weight)
            .with_threshold(opts.threshold)
    };

    // With --cache-dir, build units per file through the on-disk cache (skips
    // parse/normalize/extract for unchanged files); otherwise lower the whole corpus.
    let report = if let Some(dir) = &args.cache_dir {
        let (units, files) = time_lower(|| cache::build_units_cached(&refs, &exclude, &opts, dir));
        if files == 0 {
            warn_no_files(&args.paths);
        }
        nose_detect::detect_from_units(units, files, &opts, &detector).0
    } else {
        let corpus = time_lower(|| nose_frontend::lower_corpus_filtered(&refs, &exclude));
        warn_if_empty(&corpus, &args.paths);
        nose_detect::detect(&corpus, &opts, &detector)
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
    // Baseline: write the current state, or hide already-accepted families so only
    // new duplication is reported and gated.
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
    if let Some(path) = &args.baseline {
        let known = baseline::load(path);
        families.retain(|f| !known.contains(&baseline::family_key(f)));
    }

    let shown = families.iter().take(top).collect::<Vec<_>>();

    match args.format {
        ReportFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&shown)?);
        }
        ReportFormat::Markdown => print_refactor_markdown(&families, &shown),
        ReportFormat::Human => print_refactor_human(&families, &shown, args.diff, args.proposal),
        ReportFormat::Sarif => println!("{}", refactor_sarif(&shown)?),
    }
    if args.hotspots && matches!(args.format, ReportFormat::Human | ReportFormat::Markdown) {
        print_hotspots(&families);
    }
    // CI gate: report is already printed; a non-empty (filtered) family set is a
    // failure when --fail is set.
    if args.fail && !families.is_empty() {
        eprintln!(
            "\nnose: {} refactoring {} found (--fail)",
            families.len(),
            if families.len() == 1 {
                "family"
            } else {
                "families"
            }
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
fn refactor_sarif(families: &[&nose_detect::RefactorFamily]) -> Result<String> {
    use serde_json::json;
    let phys = |l: &nose_detect::Loc| {
        json!({
            "physicalLocation": {
                "artifactLocation": { "uri": l.file },
                "region": { "startLine": l.start_line, "endLine": l.end_line }
            }
        })
    };
    let results: Vec<_> = families
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
            })
        })
        .collect();
    let doc = json!({
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
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
        }]
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
    let extract = if all_classes {
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
    diff: bool,
    proposal: bool,
) {
    println!(
        "{} refactoring candidate families  ·  ~{} duplicated lines  (showing {})",
        all.len(),
        total_dup_lines(all),
        shown.len()
    );
    for (i, f) in shown.iter().enumerate() {
        let xlang = if f.languages > 1 {
            format!(" · {} langs ({})", f.languages, family_langs(f))
        } else {
            String::new()
        };
        // Tag non-production families: `test` (intentional scaffolding, discounted)
        // and `mixed` (logic duplicated across the test boundary — a real smell).
        let scope = match f.scope {
            "test" => " · test",
            "mixed" => " · test↔prod",
            _ => "",
        };
        println!(
            "\n#{:<3} value {:>7.0}  ·  {} sites · {} files · {} modules{}{} · sim {:.2} · ~{} dup lines",
            i + 1,
            f.value,
            f.members,
            f.files,
            f.modules,
            xlang,
            scope,
            f.mean_score,
            f.dup_lines
        );
        println!("     → {}", family_hint(f));
        for l in f.locations.iter().take(6) {
            let name = l
                .name
                .as_deref()
                .map(|n| format!("  {n}"))
                .unwrap_or_default();
            println!("     {}:{}-{}{}", l.file, l.start_line, l.end_line, name);
        }
        if f.locations.len() > 6 {
            println!("     … +{} more", f.locations.len() - 6);
        }
        if diff && f.locations.len() >= 2 {
            print_member_diff(&f.locations[0], &f.locations[1]);
        }
        if proposal && f.locations.len() >= 2 {
            print_member_proposal(&f.locations[0], &f.locations[1]);
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
fn print_member_proposal(a: &nose_detect::Loc, b: &nose_detect::Loc) {
    let (Some(la), Some(lb)) = (
        read_lines(&a.file, a.start_line, a.end_line),
        read_lines(&b.file, b.start_line, b.end_line),
    ) else {
        return;
    };
    let ar: Vec<&str> = la.iter().map(String::as_str).collect();
    let br: Vec<&str> = lb.iter().map(String::as_str).collect();
    let diff = line_diff(&ar, &br);
    // Collapse each maximal run of differing lines into one parameter placeholder.
    let mut skeleton: Vec<String> = Vec::new();
    let mut params = 0usize;
    let mut in_hole = false;
    let mut indent = "";
    for (tag, line) in &diff {
        if *tag == ' ' {
            in_hole = false;
            // remember the indentation to align the placeholder
            indent = &line[..line.len() - line.trim_start().len()];
            skeleton.push(line.clone());
        } else if !in_hole {
            in_hole = true;
            params += 1;
            skeleton.push(format!("{indent}⟨param {params}⟩"));
        }
    }
    let common = diff.iter().filter(|(t, _)| *t == ' ').count();
    println!(
        "     proposal  extract a shared helper · {common} shared lines · {params} parameter(s) vary"
    );
    for line in skeleton.iter().take(40) {
        println!("       │ {line}");
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
) {
    println!("# Refactoring candidates\n");
    println!(
        "{} families · ~{} duplicated lines · showing top {}\n",
        all.len(),
        total_dup_lines(all),
        shown.len()
    );
    for (i, f) in shown.iter().enumerate() {
        let xlang = match family_langs(f) {
            s if s.is_empty() => String::new(),
            s => format!(" · cross-language: {s}"),
        };
        println!(
            "## {}. {} sites, {} files, {} modules — ~{} dup lines (sim {:.2}){}",
            i + 1,
            f.members,
            f.files,
            f.modules,
            f.dup_lines,
            f.mean_score,
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
            locations,
            mean_sem: 50.0,
            scope: "prod",
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
