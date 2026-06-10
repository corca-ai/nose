//! `nose-eval` — measure Type-4 detection against an audited gold set.
//!
//! Self-contained implementation of the audited benchmark methodology:
//! line-span IoU partial credit, pair-order-invariant `pair_overlap`,
//! maximum-weight bipartite matching (see the `matching` module), per-slice
//! precision/recall/F1, repo-macro F1 split by dev/held-out (generalization
//! gate), and a hard-negative false-positive rate (precision guard). Precision
//! is depressed by the gold being a *sample* (not exhaustive), so recall and
//! F1-deltas above the noise floor are the real signal.

mod matching;
mod schema;

pub use schema::{CandidatesDump, PredPair, PredRegion, Predictions, UnitRegion, UnitsDump};

use rustc_hash::{FxHashMap, FxHashSet};
use schema::*;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Clone)]
struct EvalRegion {
    repo: String,
    file: String,
    start: u32,
    end: u32,
}

#[derive(Clone)]
struct EvalPair {
    repo: String,
    left: EvalRegion,
    right: EvalRegion,
    kind: Option<String>,
    clone_type: Option<String>,
}

fn region_iou(a: &EvalRegion, b: &EvalRegion) -> f64 {
    if a.repo != b.repo || a.file != b.file {
        return 0.0;
    }
    let ostart = a.start.max(b.start);
    let oend = a.end.min(b.end);
    if oend < ostart {
        return 0.0;
    }
    let overlap = (oend - ostart + 1) as f64;
    let ustart = a.start.min(b.start);
    let uend = a.end.max(b.end);
    let union = (uend - ustart + 1) as f64;
    overlap / union
}

/// Pair-order-invariant overlap of a prediction pair against a gold pair.
fn pair_overlap(p: &EvalPair, g: &EvalPair) -> f64 {
    let aligned = region_iou(&p.left, &g.left).min(region_iou(&p.right, &g.right));
    let swapped = region_iou(&p.left, &g.right).min(region_iou(&p.right, &g.left));
    aligned.max(swapped)
}

#[derive(Serialize)]
pub struct SliceMetric {
    pub gold: usize,
    pub predictions: usize,
    pub weighted_tp: f64,
    pub precision: f64,
    pub recall: f64,
    pub f1: f64,
}

#[derive(Serialize)]
pub struct MacroMetric {
    pub slice: String,
    pub dev_f1: f64,
    pub heldout_f1: f64,
    pub dev_repos: usize,
    pub heldout_repos: usize,
}

#[derive(Serialize)]
pub struct Report {
    pub prediction_count: usize,
    pub gold_count: usize,
    pub slices: BTreeMap<String, SliceMetric>,
    pub macro_f1: Vec<MacroMetric>,
    pub hn_fp_rate: f64,
    pub hn_matched: usize,
    pub hn_total: usize,
    /// Precision computed on the judged pool only (honest denominator on a
    /// non-exhaustive gold). `None` when the gold ships no pool.
    pub pool_precision: Option<f64>,
    /// Predictions that overlap some pooled (judged) pair.
    pub pool_judged: usize,
    /// Of those, the ones whose matched pool pair was labeled a true clone.
    pub pool_clone_hits: usize,
}

fn metric(preds: &[EvalPair], golds: &[EvalPair]) -> SliceMetric {
    let wtp = matched_weight(preds, golds);
    let precision = if preds.is_empty() {
        0.0
    } else {
        wtp / preds.len() as f64
    };
    let recall = if golds.is_empty() {
        0.0
    } else {
        wtp / golds.len() as f64
    };
    let f1 = if precision + recall > 0.0 {
        2.0 * precision * recall / (precision + recall)
    } else {
        0.0
    };
    SliceMetric {
        gold: golds.len(),
        predictions: preds.len(),
        weighted_tp: wtp,
        precision,
        recall,
        f1,
    }
}

/// Total matched weight of the optimal prediction↔gold assignment.
fn matched_weight(preds: &[EvalPair], golds: &[EvalPair]) -> f64 {
    if preds.is_empty() || golds.is_empty() {
        return 0.0;
    }
    // index gold by (repo,file) of either region for sparse candidate lookup
    let mut idx: FxHashMap<(&str, &str), Vec<usize>> = FxHashMap::default();
    for (gi, g) in golds.iter().enumerate() {
        idx.entry((g.left.repo.as_str(), g.left.file.as_str()))
            .or_default()
            .push(gi);
        idx.entry((g.right.repo.as_str(), g.right.file.as_str()))
            .or_default()
            .push(gi);
    }
    let mut edges = Vec::new();
    for (pi, p) in preds.iter().enumerate() {
        let mut cand: FxHashSet<usize> = FxHashSet::default();
        for key in [
            (p.left.repo.as_str(), p.left.file.as_str()),
            (p.right.repo.as_str(), p.right.file.as_str()),
        ] {
            if let Some(v) = idx.get(&key) {
                cand.extend(v.iter().copied());
            }
        }
        for gi in cand {
            let w = pair_overlap(p, &golds[gi]);
            if w > 0.0 {
                edges.push((pi, gi, w));
            }
        }
    }
    matching::max_weight_matching(preds.len(), golds.len(), &edges)
        .iter()
        .map(|&(_, _, w)| w)
        .sum()
}

fn is_type4(g: &EvalPair) -> bool {
    g.clone_type.as_deref() == Some("type4_semantic")
}

fn is_production(g: &EvalPair) -> bool {
    g.kind
        .as_deref()
        .is_some_and(|k| k.starts_with("production_"))
}

fn is_structural(g: &EvalPair) -> bool {
    matches!(
        g.kind.as_deref(),
        Some("production_mirror")
            | Some("production_structural")
            | Some("production_async_sync")
            | Some("production_cross_version")
    )
}

fn is_test_example(g: &EvalPair) -> bool {
    g.kind
        .as_deref()
        .is_some_and(|k| k.contains("test") || k.contains("example"))
}

/// Run the full evaluation. Each argument is the *content* of the corresponding
/// JSON file (corpus may be empty `""` to skip the macro split).
pub fn evaluate(
    gold_json: &str,
    preds_json: &str,
    hn_json: &str,
    corpus_json: &str,
) -> anyhow::Result<Report> {
    let gold: Gold = serde_json::from_str(gold_json)?;
    let preds: Predictions = serde_json::from_str(preds_json)?;

    let golds: Vec<EvalPair> = gold.duplicates.iter().map(eval_gold).collect();
    // keep only the semantic channel (default when omitted)
    let pred_pairs: Vec<EvalPair> = preds
        .duplicates
        .iter()
        .filter(|p| p.channel.as_deref().unwrap_or("nose_semantic") == "nose_semantic")
        .map(eval_pred)
        .collect();

    // --- slices ---
    let mut slices = BTreeMap::new();
    type SlicePred = fn(&EvalPair) -> bool;
    let slice_defs: [(&str, SlicePred); 5] = [
        ("all", |_| true),
        ("type4_semantic", is_type4),
        ("production", is_production),
        ("near_exact_or_structural", is_structural),
        ("test_example", is_test_example),
    ];
    for (name, pred) in slice_defs {
        let sub: Vec<EvalPair> = golds.iter().filter(|g| pred(g)).cloned().collect();
        slices.insert(name.to_string(), metric(&pred_pairs, &sub));
    }

    // --- macro F1 (dev vs held-out) for `all` and `type4_semantic` ---
    let splits = parse_splits(corpus_json);
    let mut macro_f1 = Vec::new();
    for (sname, sfilter) in [("all", None), ("type4_semantic", Some(()))] {
        let gsub: Vec<EvalPair> = golds
            .iter()
            .filter(|g| sfilter.is_none() || is_type4(g))
            .cloned()
            .collect();
        let (dev_f1, dev_n) = macro_over_split(&pred_pairs, &gsub, &splits, "dev");
        let (held_f1, held_n) = macro_over_split(&pred_pairs, &gsub, &splits, "heldout");
        macro_f1.push(MacroMetric {
            slice: sname.to_string(),
            dev_f1,
            heldout_f1: held_f1,
            dev_repos: dev_n,
            heldout_repos: held_n,
        });
    }

    // --- hard-negative false-positive rate ---
    let (hn_matched, hn_total) = if hn_json.is_empty() {
        (0, 0)
    } else {
        let hn: HardNegatives = serde_json::from_str(hn_json)?;
        let total = hn.hard_negatives.len();
        let mut matched = 0;
        for h in &hn.hard_negatives {
            let hp = eval_hn(h);
            // a prediction "fires" on this non-clone if it overlaps it well
            if pred_pairs.iter().any(|p| pair_overlap(p, &hp) >= 0.5) {
                matched += 1;
            }
        }
        (matched, total)
    };
    let hn_fp_rate = if hn_total == 0 {
        0.0
    } else {
        hn_matched as f64 / hn_total as f64
    };

    // --- pool-aware precision (honest denominator on a non-exhaustive gold) ---
    let (pool_judged, pool_clone_hits) = pool_counts(&gold, &pred_pairs);
    let pool_precision = (pool_judged > 0).then(|| pool_clone_hits as f64 / pool_judged as f64);

    Ok(Report {
        prediction_count: pred_pairs.len(),
        gold_count: golds.len(),
        slices,
        macro_f1,
        hn_fp_rate,
        hn_matched,
        hn_total,
        pool_precision,
        pool_judged,
        pool_clone_hits,
    })
}

/// Count judged predictions and true-clone hits against the precision pool.
/// Each prediction is matched to its best-overlapping judged pool pair (≥0.5);
/// a matched prediction is "judged" and counts toward precision iff that pair
/// was a true clone. Predictions overlapping no pool pair are simply unjudged.
fn pool_counts(gold: &Gold, pred_pairs: &[EvalPair]) -> (usize, usize) {
    let (mut pool_judged, mut pool_clone_hits) = (0usize, 0usize);
    if !gold.pool.is_empty() {
        let pool: Vec<(EvalPair, bool)> = gold
            .pool
            .iter()
            .map(|pp| {
                let mk = |t: &(String, String, u32, u32)| EvalRegion {
                    repo: t.0.clone(),
                    file: t.1.clone(),
                    start: t.2,
                    end: t.3,
                };
                (
                    EvalPair {
                        repo: pp.left.0.clone(),
                        left: mk(&pp.left),
                        right: mk(&pp.right),
                        kind: None,
                        clone_type: None,
                    },
                    pp.clone,
                )
            })
            .collect();
        for p in pred_pairs {
            let best = pool
                .iter()
                .map(|(pe, lbl)| (pair_overlap(p, pe), *lbl))
                .filter(|&(ov, _)| ov >= 0.5)
                .max_by(|a, b| a.0.total_cmp(&b.0));
            if let Some((_, lbl)) = best {
                pool_judged += 1;
                pool_clone_hits += lbl as usize;
            }
        }
    }
    (pool_judged, pool_clone_hits)
}

// ---------------------------------------------------------------------------
// ceiling: split recall loss across unit-extraction / candidate-gen / scoring
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct CeilingSlice {
    pub gold: usize,
    /// gold pairs where BOTH regions are covered by an extracted unit
    pub unit_reachable: usize,
    /// of those, pairs whose two units form an LSH candidate
    pub candidate_reachable: usize,
}

#[derive(Serialize)]
pub struct CeilingReport {
    pub units: usize,
    pub candidate_pairs: usize,
    pub all: CeilingSlice,
    pub type4_semantic: CeilingSlice,
}

/// A unit "covers" a region if, in the same repo+file, ≥50% of the region's lines
/// lie inside the unit.
fn coverage_fraction(unit: &UnitRegion, r: &EvalRegion) -> f64 {
    if unit.repo != r.repo || unit.file != r.file {
        return 0.0;
    }
    let ostart = unit.start_line.max(r.start);
    let oend = unit.end_line.min(r.end);
    if oend < ostart {
        return 0.0;
    }
    let overlap = (oend - ostart + 1) as f64;
    let region_len = (r.end - r.start + 1) as f64;
    overlap / region_len
}

pub fn ceiling(
    gold_json: &str,
    units_json: &str,
    candidates_json: &str,
) -> anyhow::Result<CeilingReport> {
    let gold: Gold = serde_json::from_str(gold_json)?;
    let units: UnitsDump = serde_json::from_str(units_json)?;
    let cands: CandidatesDump = serde_json::from_str(candidates_json)?;

    // index units by (repo, file)
    let mut idx: FxHashMap<(&str, &str), Vec<usize>> = FxHashMap::default();
    for (ui, u) in units.units.iter().enumerate() {
        idx.entry((u.repo.as_str(), u.file.as_str()))
            .or_default()
            .push(ui);
    }
    let cand_set: FxHashSet<(u32, u32)> = cands
        .candidates
        .iter()
        .map(|&(a, b)| if a <= b { (a, b) } else { (b, a) })
        .collect();

    let cover = |r: &EvalRegion| -> Option<usize> {
        let list = idx.get(&(r.repo.as_str(), r.file.as_str()))?;
        list.iter()
            .copied()
            .map(|ui| (ui, coverage_fraction(&units.units[ui], r)))
            .filter(|&(_, f)| f >= 0.5)
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(ui, _)| ui)
    };

    let mut all = CeilingSlice {
        gold: 0,
        unit_reachable: 0,
        candidate_reachable: 0,
    };
    let mut t4 = CeilingSlice {
        gold: 0,
        unit_reachable: 0,
        candidate_reachable: 0,
    };

    for g in &gold.duplicates {
        let p = eval_gold(g);
        let lu = cover(&p.left);
        let ru = cover(&p.right);
        let unit_ok = matches!((lu, ru), (Some(a), Some(b)) if a != b);
        let cand_ok = unit_ok && {
            let (a, b) = (lu.unwrap() as u32, ru.unwrap() as u32);
            let key = if a <= b { (a, b) } else { (b, a) };
            cand_set.contains(&key)
        };
        let t4flag = is_type4(&p);
        all.gold += 1;
        all.unit_reachable += unit_ok as usize;
        all.candidate_reachable += cand_ok as usize;
        if t4flag {
            t4.gold += 1;
            t4.unit_reachable += unit_ok as usize;
            t4.candidate_reachable += cand_ok as usize;
        }
    }

    Ok(CeilingReport {
        units: units.units.len(),
        candidate_pairs: cands.candidates.len(),
        all,
        type4_semantic: t4,
    })
}

fn macro_over_split(
    preds: &[EvalPair],
    golds: &[EvalPair],
    splits: &FxHashMap<String, String>,
    split: &str,
) -> (f64, usize) {
    // repos in this split that have at least one gold pair
    let mut repos: FxHashSet<&str> = FxHashSet::default();
    for g in golds {
        if splits.get(&g.repo).map(|s| s.as_str()) == Some(split) {
            repos.insert(g.repo.as_str());
        }
    }
    if repos.is_empty() {
        return (0.0, 0);
    }
    let mut sum = 0.0;
    for repo in &repos {
        let gsub: Vec<EvalPair> = golds.iter().filter(|g| g.repo == *repo).cloned().collect();
        let psub: Vec<EvalPair> = preds.iter().filter(|p| p.repo == *repo).cloned().collect();
        sum += metric(&psub, &gsub).f1;
    }
    (sum / repos.len() as f64, repos.len())
}

fn parse_splits(corpus_json: &str) -> FxHashMap<String, String> {
    let mut m = FxHashMap::default();
    if let Ok(c) = serde_json::from_str::<Corpus>(corpus_json) {
        for r in c.repositories {
            if let Some(s) = r.split {
                m.insert(r.id, s);
            }
        }
    }
    m
}

fn eval_gold(g: &GoldPair) -> EvalPair {
    EvalPair {
        repo: g.repo.clone(),
        left: region(&g.left, &g.repo),
        right: region(&g.right, &g.repo),
        kind: g.kind.clone(),
        clone_type: g.clone_type.clone(),
    }
}

fn eval_pred(p: &PredPair) -> EvalPair {
    let repo = p
        .repo
        .clone()
        .or_else(|| p.left.repo.clone())
        .unwrap_or_default();
    EvalPair {
        repo: repo.clone(),
        left: pred_region(&p.left, &repo),
        right: pred_region(&p.right, &repo),
        kind: None,
        clone_type: None,
    }
}

fn eval_hn(h: &HardNeg) -> EvalPair {
    EvalPair {
        repo: h.repo.clone(),
        left: region(&h.left, &h.repo),
        right: region(&h.right, &h.repo),
        kind: None,
        clone_type: None,
    }
}

/// Build an [`EvalRegion`] from any schema region (`Region`/`PredRegion` share the field
/// names): take the region's own repo or fall back to the pair's repo. A macro rather than a
/// generic so it works over the two distinct schema types without a `RegionLike` trait.
macro_rules! to_eval_region {
    ($r:expr, $repo:expr) => {
        EvalRegion {
            repo: $r.repo.clone().unwrap_or_else(|| $repo.to_string()),
            file: $r.file.clone(),
            start: $r.start_line,
            end: $r.end_line,
        }
    };
}

fn region(r: &Region, repo: &str) -> EvalRegion {
    to_eval_region!(r, repo)
}

fn pred_region(r: &PredRegion, repo: &str) -> EvalRegion {
    to_eval_region!(r, repo)
}
