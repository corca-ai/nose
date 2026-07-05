use crate::path_utils::{paths_as_refs, require_paths_exist, warn_if_empty};
use anyhow::{Context, Result};
use nose_il::Corpus;
use rayon::prelude::*;
use std::path::PathBuf;

/// Dump each unit's detection features as JSON `{units: [...]}` — the raw value-graph,
/// shape, return and literal fingerprints, before candidate generation/thresholding.
pub(super) fn cmd_features(
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

/// #391 prevalence probe: walk every function unit, build its value graph with the opaque
/// census on, and aggregate per IL construct how many `Opaque` fallbacks were minted (the
/// value-graph analog of `stats`'s lowering `Raw`). `total_fallback` opaques are full coverage
/// gaps (the construct could not be modeled at all); the rest are partial/semantic opaques.
pub(super) fn cmd_value_census(paths: Vec<PathBuf>, no_cfg_norm: bool) -> Result<()> {
    use std::collections::{BTreeMap, HashSet};
    let refs = paths_as_refs(&paths);
    let corpus = nose_frontend::lower_corpus_many(&refs);
    warn_if_empty(&corpus, &paths);
    let opts = nose_normalize::NormalizeOptions {
        cfg_norm: !no_cfg_norm,
        ..Default::default()
    };
    // Build the value graph (the dominant cost) for every unit in parallel, each file producing a
    // local census, then reduce. The census API builds a fresh per-unit graph, so this is safe.
    type Census = (
        BTreeMap<(String, bool), u64>,
        BTreeMap<(String, bool), u64>,
        u64,
        u64,
    );
    let (opaque_nodes, affected_units, total_units, units_with_opaque): Census = corpus
        .files
        .par_iter()
        .map(|il| {
            let n = nose_normalize::normalize(il, &corpus.interner, &opts);
            let mut on: BTreeMap<(String, bool), u64> = BTreeMap::new();
            let mut au: BTreeMap<(String, bool), u64> = BTreeMap::new();
            let (mut tu, mut uo): (u64, u64) = (0, 0);
            for u in &n.units {
                if n.kind(u.root) != nose_il::NodeKind::Func {
                    continue; // count function-level units only (blocks would double-count)
                }
                tu += 1;
                let census =
                    nose_normalize::value_graph_opaque_census(&n, u.root, &corpus.interner);
                if !census.is_empty() {
                    uo += 1;
                }
                let mut seen: HashSet<(String, bool)> = HashSet::new();
                for (kind, total, count) in census {
                    let key = (format!("{kind:?}"), total);
                    *on.entry(key.clone()).or_insert(0) += u64::from(count);
                    if seen.insert(key.clone()) {
                        *au.entry(key).or_insert(0) += 1;
                    }
                }
            }
            (on, au, tu, uo)
        })
        .reduce(
            || (BTreeMap::new(), BTreeMap::new(), 0, 0),
            |mut a, b| {
                for (k, v) in b.0 {
                    *a.0.entry(k).or_insert(0) += v;
                }
                for (k, v) in b.1 {
                    *a.1.entry(k).or_insert(0) += v;
                }
                (a.0, a.1, a.2 + b.2, a.3 + b.3)
            },
        );
    let mut by_construct: Vec<_> = opaque_nodes
        .iter()
        .map(|((kind, total), n)| {
            serde_json::json!({
                "construct": kind,
                "total_fallback": total,
                "opaque_nodes": n,
                "units": affected_units.get(&(kind.clone(), *total)).copied().unwrap_or(0),
            })
        })
        .collect::<Vec<_>>();
    by_construct.sort_by(|a, b| {
        b["opaque_nodes"]
            .as_u64()
            .cmp(&a["opaque_nodes"].as_u64())
            .then_with(|| a["construct"].as_str().cmp(&b["construct"].as_str()))
    });
    println!(
        "{}",
        serde_json::to_string(&serde_json::json!({
            "files": corpus.files.len(),
            "function_units": total_units,
            "units_with_opaque": units_with_opaque,
            "by_construct": by_construct,
        }))?
    );
    Ok(())
}

pub(super) fn cmd_ceiling(gold: PathBuf, units: PathBuf, candidates: PathBuf) -> Result<()> {
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

pub(super) fn cmd_eval(
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

pub(super) fn cmd_stats(paths: Vec<PathBuf>, top: usize, json: bool) -> Result<()> {
    require_paths_exist(&paths)?;
    let refs = paths_as_refs(&paths);
    let corpus = nose_frontend::lower_corpus_many(&refs);
    warn_if_empty(&corpus, &paths);
    let report = nose_frontend::coverage(&corpus, top);

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }

    let gap_raw = report.raw_nodes.saturating_sub(report.boundary_raw);
    println!(
        "files: {}   IL nodes: {}   Raw nodes: {} ({:.3}%)   = {} lowering-gap + {} intentional-boundary",
        report.files,
        report.total_nodes,
        report.raw_nodes,
        report.raw_ratio * 100.0,
        gap_raw,
        report.boundary_raw,
    );
    println!("\nper language (worst coverage first):");
    println!(
        "  {:<12} {:>7} {:>10} {:>9} {:>8} {:>9}",
        "lang", "files", "nodes", "raw", "raw%", "gap"
    );
    for l in &report.per_lang {
        println!(
            "  {:<12} {:>7} {:>10} {:>9} {:>7.3}% {:>9}",
            l.lang,
            l.files,
            l.nodes,
            l.raw_nodes,
            l.raw_ratio * 100.0,
            l.raw_nodes.saturating_sub(l.boundary_raw),
        );
    }
    println!("\ntop unhandled constructs (surface kind → Raw; `boundary` = by-design, not a gap):");
    println!(
        "  {:<12} {:<34} {:>8}  kind",
        "lang", "surface_kind", "count"
    );
    for u in &report.top_unhandled {
        let kind = if u.boundary { "boundary" } else { "gap" };
        println!(
            "  {:<12} {:<34} {:>8}  {kind}",
            u.lang, u.surface_kind, u.count
        );
    }
    Ok(())
}

#[derive(serde::Serialize)]
struct GapImpactReport {
    files: usize,
    total_nodes: usize,
    raw_nodes: usize,
    lowering_gap_raw: usize,
    intentional_boundary_raw: usize,
    rows: Vec<GapImpactRow>,
}

#[derive(serde::Serialize)]
struct GapImpactRow {
    lang: String,
    surface_kind: String,
    raw_count: usize,
    files: usize,
    units: usize,
    unit_nodes: u64,
    unit_lines: u64,
    repos: usize,
    actionability_score: f64,
    kind: String,
    top_repos: Vec<RepoCount>,
    samples: Vec<String>,
}

#[derive(serde::Serialize)]
struct RepoCount {
    repo: String,
    count: usize,
}

#[derive(Default)]
struct GapImpactAcc {
    raw_count: usize,
    files: std::collections::BTreeSet<String>,
    units: std::collections::BTreeSet<(String, u32)>,
    unit_nodes: u64,
    unit_lines: u64,
    repos: std::collections::BTreeSet<String>,
    repo_counts: std::collections::BTreeMap<String, usize>,
    samples: std::collections::BTreeSet<String>,
}

pub(super) fn cmd_gap_impact(paths: Vec<PathBuf>, top: usize, json: bool) -> Result<()> {
    require_paths_exist(&paths)?;
    let refs = paths_as_refs(&paths);
    let corpus = nose_frontend::lower_corpus_many(&refs);
    warn_if_empty(&corpus, &paths);

    let coverage = nose_frontend::coverage(&corpus, usize::MAX);
    let rows = top_gap_impact_rows(&corpus, top);
    let report = GapImpactReport {
        files: coverage.files,
        total_nodes: coverage.total_nodes,
        raw_nodes: coverage.raw_nodes,
        lowering_gap_raw: coverage.raw_nodes.saturating_sub(coverage.boundary_raw),
        intentional_boundary_raw: coverage.boundary_raw,
        rows,
    };
    emit_gap_impact_report(&report, json)
}

fn collect_gap_impact_accs(
    corpus: &Corpus,
) -> std::collections::BTreeMap<(String, String), GapImpactAcc> {
    use nose_il::{NodeId, Span};
    use std::collections::{BTreeMap, HashMap};

    let mut rows: BTreeMap<(String, String), GapImpactAcc> = BTreeMap::new();
    for il in &corpus.files {
        let lang = il.meta.lang.name().to_string();
        let path = il.meta.path.clone();
        let repo = repo_name(&path);
        let unit_spans: Vec<(NodeId, Span)> = il
            .units
            .iter()
            .map(|unit| (unit.root, il.node(unit.root).span))
            .collect();
        let mut unit_size_cache: HashMap<u32, (usize, u32)> = HashMap::new();

        for node in &il.nodes {
            let Some(raw) = nose_frontend::raw_node_surface(&corpus.interner, node) else {
                continue;
            };
            if raw.boundary {
                continue;
            }

            let key = (lang.clone(), raw.surface_kind);
            let acc = rows.entry(key).or_default();
            acc.raw_count += 1;
            acc.files.insert(path.clone());
            acc.repos.insert(repo.clone());
            *acc.repo_counts.entry(repo.clone()).or_default() += 1;
            if acc.samples.len() < 16 {
                acc.samples.insert(format!(
                    "{}:{}-{}",
                    path, node.span.start_line, node.span.end_line
                ));
            }

            if let Some(unit) = nearest_unit(node.span, &unit_spans) {
                let unit_key = (path.clone(), unit.0);
                if acc.units.insert(unit_key) {
                    let (nodes, lines) = *unit_size_cache.entry(unit.0).or_insert_with(|| {
                        (
                            subtree_node_count(il, unit),
                            il.node(unit).span.line_count(),
                        )
                    });
                    acc.unit_nodes += nodes as u64;
                    acc.unit_lines += lines as u64;
                }
            }
        }
    }
    rows
}

fn top_gap_impact_rows(corpus: &Corpus, top: usize) -> Vec<GapImpactRow> {
    let mut rows: Vec<GapImpactRow> = collect_gap_impact_accs(corpus)
        .into_iter()
        .map(gap_impact_row)
        .collect();
    rows.sort_by(|a, b| {
        b.actionability_score
            .total_cmp(&a.actionability_score)
            .then_with(|| b.raw_count.cmp(&a.raw_count))
            .then_with(|| a.lang.cmp(&b.lang))
            .then_with(|| a.surface_kind.cmp(&b.surface_kind))
    });
    rows.truncate(top);
    rows
}

fn gap_impact_row(((lang, surface_kind), acc): ((String, String), GapImpactAcc)) -> GapImpactRow {
    let files = acc.files.len();
    let units = acc.units.len();
    let repos = acc.repos.len();
    let score = actionability_score(files, units, acc.unit_lines, acc.raw_count);
    let mut top_repos: Vec<RepoCount> = acc
        .repo_counts
        .into_iter()
        .map(|(repo, count)| RepoCount { repo, count })
        .collect();
    top_repos.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.repo.cmp(&b.repo)));
    top_repos.truncate(5);
    let kind = if surface_kind == "ERROR" {
        "parser-error"
    } else {
        "lowering-gap"
    };
    GapImpactRow {
        lang,
        surface_kind,
        raw_count: acc.raw_count,
        files,
        units,
        unit_nodes: acc.unit_nodes,
        unit_lines: acc.unit_lines,
        repos,
        actionability_score: score,
        kind: kind.to_string(),
        top_repos,
        samples: acc.samples.into_iter().take(5).collect(),
    }
}

fn emit_gap_impact_report(report: &GapImpactReport, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }

    println!(
        "files: {}   IL nodes: {}   Raw nodes: {}   = {} lowering-gap + {} intentional-boundary",
        report.files,
        report.total_nodes,
        report.raw_nodes,
        report.lowering_gap_raw,
        report.intentional_boundary_raw,
    );
    println!("\ntop lowering-gap impact candidates:");
    println!(
        "  {:<10} {:<30} {:>8} {:>7} {:>7} {:>11} {:>10} {:>9} {:<13}  top_repos",
        "lang",
        "surface_kind",
        "raw",
        "files",
        "units",
        "unit_nodes",
        "unit_lines",
        "score",
        "kind"
    );
    for row in &report.rows {
        let repos = row
            .top_repos
            .iter()
            .map(|repo| format!("{}:{}", repo.repo, repo.count))
            .collect::<Vec<_>>()
            .join(",");
        println!(
            "  {:<10} {:<30} {:>8} {:>7} {:>7} {:>11} {:>10} {:>9.1} {:<13}  {}",
            row.lang,
            row.surface_kind,
            row.raw_count,
            row.files,
            row.units,
            row.unit_nodes,
            row.unit_lines,
            row.actionability_score,
            row.kind,
            repos
        );
    }
    Ok(())
}

fn nearest_unit(
    raw_span: nose_il::Span,
    units: &[(nose_il::NodeId, nose_il::Span)],
) -> Option<nose_il::NodeId> {
    units
        .iter()
        .filter(|(_, span)| span_contains(*span, raw_span))
        .min_by_key(|(root, span)| (span.end_byte.saturating_sub(span.start_byte), root.0))
        .map(|(root, _)| *root)
}

fn span_contains(outer: nose_il::Span, inner: nose_il::Span) -> bool {
    outer.file == inner.file
        && outer.start_byte <= inner.start_byte
        && outer.end_byte >= inner.end_byte
}

fn subtree_node_count(il: &nose_il::Il, root: nose_il::NodeId) -> usize {
    let mut count = 0usize;
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        count += 1;
        stack.extend_from_slice(il.children(node));
    }
    count
}

fn actionability_score(files: usize, units: usize, unit_lines: u64, raw_count: usize) -> f64 {
    let breadth = (files as f64).ln_1p();
    let unit_signal = units as f64;
    let line_signal = (unit_lines as f64 + 1.0).ln();
    let raw_signal = (raw_count as f64 + 1.0).ln();
    ((unit_signal * breadth) + line_signal + raw_signal) * 10.0
}

fn repo_name(path: &str) -> String {
    let marker = "bench/repos/";
    if let Some(rest) = path
        .strip_prefix(marker)
        .or_else(|| path.find(marker).map(|idx| &path[idx + marker.len()..]))
    {
        return rest
            .split('/')
            .next()
            .filter(|name| !name.is_empty())
            .unwrap_or("<unknown>")
            .to_string();
    }
    std::path::Path::new(path)
        .components()
        .next()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .unwrap_or_else(|| "<unknown>".to_string())
}
