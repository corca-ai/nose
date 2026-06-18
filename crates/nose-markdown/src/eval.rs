//! Stage-0 measurement spine (#436): pairwise scoring + golden-set evaluation.
//!
//! The detector produces families; to measure quality against a labeled golden we expose the raw
//! per-candidate-pair relation scores (pre-threshold) so the harness can sweep thresholds for
//! PR-AUC (the survey's primary metric — ROC is optimistic under near-dup class imbalance).
//!
//! The golden is a FROZEN, committed artifact of labeled pairs. Per epic #435 it is built with no
//! human in the loop: construction-truth/provenance anchors (certain) + an LLM panel for the
//! ambiguous real pairs, self-calibrated against the anchors. This module is label-source-agnostic;
//! it just consumes `{a, b, label}` pairs and scores the detector against them.

use crate::fingerprint::{self, Fingerprint};
use crate::unit::{self, Unit};
use crate::verify::CorpusModel;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A reference to one unit span (file + 1-based inclusive line range).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Ref {
    pub path: String,
    pub start: u32,
    pub end: u32,
}

impl Ref {
    fn of(u: &Unit) -> Ref {
        Ref {
            path: u.path.clone(),
            start: u.start_line,
            end: u.end_line,
        }
    }
}

/// A scored candidate pair (relation score = TF-IDF cosine, rescued by containment).
#[derive(Clone, Debug, Serialize)]
pub struct ScoredPair {
    pub a: Ref,
    pub b: Ref,
    pub score: f64,
    pub containment: f64,
    pub commonness: f64,
}

/// A scored pair plus the raw text of each side — emitted by `--dump-pairs` for golden building.
#[derive(Clone, Debug, Serialize)]
pub struct DumpPair {
    #[serde(flatten)]
    pub scored: ScoredPair,
    pub text_a: String,
    pub text_b: String,
}

/// One labeled golden pair (`label = true` ⇒ the two spans are a near-duplicate relation).
#[derive(Clone, Debug, Deserialize)]
pub struct GoldPair {
    pub a: Ref,
    pub b: Ref,
    pub label: bool,
    #[serde(default)]
    pub source: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Golden {
    pub pairs: Vec<GoldPair>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Metrics {
    pub gold_pairs: usize,
    pub positives: usize,
    pub negatives: usize,
    pub scored_candidates: usize,
    pub roc_auc: f64,
    pub pr_auc: f64,
    pub r_at_p95: f64,
    pub r_at_p99: f64,
    /// Of gold positives, the fraction that appear at all in the candidate set (Stage-1 ceiling).
    pub candidate_recall: f64,
}

fn build(
    docs: &[(String, String)],
    min_words: usize,
) -> (Vec<Unit>, Vec<Fingerprint>, CorpusModel) {
    let mut units: Vec<Unit> = Vec::new();
    for (path, src) in docs {
        for u in unit::split_units(path, src) {
            if u.prose_words() >= min_words {
                units.push(u);
            }
        }
    }
    let fps: Vec<Fingerprint> = units.iter().map(Fingerprint::of).collect();
    let model = CorpusModel::fit(&fps);
    (units, fps, model)
}

fn rel_score(model: &CorpusModel, fps: &[Fingerprint], i: usize, j: usize) -> (f64, f64, f64) {
    let cos = model.tfidf_cosine(&fps[i], &fps[j]);
    let cont = fingerprint::containment(&fps[i].shingles, &fps[j].shingles);
    let common = model.commonness(&fps[i], &fps[j]);
    let rel = if cont >= 0.8 && cont > cos { cont } else { cos };
    (rel, cont, common)
}

/// Score every candidate pair (no threshold) for the given corpus.
pub fn score_pairs(docs: &[(String, String)], min_words: usize) -> Vec<ScoredPair> {
    let (units, fps, model) = build(docs, min_words);
    fingerprint::candidate_pairs(&fps)
        .into_iter()
        .map(|(i, j)| {
            let (score, containment, commonness) = rel_score(&model, &fps, i, j);
            ScoredPair {
                a: Ref::of(&units[i]),
                b: Ref::of(&units[j]),
                score,
                containment,
                commonness,
            }
        })
        .collect()
}

/// Like `score_pairs` but carries the raw unit text — for building the golden labeling set.
pub fn dump_pairs(docs: &[(String, String)], min_words: usize) -> Vec<DumpPair> {
    let (units, fps, model) = build(docs, min_words);
    fingerprint::candidate_pairs(&fps)
        .into_iter()
        .map(|(i, j)| {
            let (score, containment, commonness) = rel_score(&model, &fps, i, j);
            DumpPair {
                scored: ScoredPair {
                    a: Ref::of(&units[i]),
                    b: Ref::of(&units[j]),
                    score,
                    containment,
                    commonness,
                },
                text_a: units[i].raw.clone(),
                text_b: units[j].raw.clone(),
            }
        })
        .collect()
}

fn key(a: &Ref, b: &Ref) -> (Ref, Ref) {
    if a <= b {
        (a.clone(), b.clone())
    } else {
        (b.clone(), a.clone())
    }
}

/// Evaluate the detector's candidate scores against a labeled golden.
pub fn evaluate(scored: &[ScoredPair], golden: &Golden) -> Metrics {
    let mut score_of: HashMap<(Ref, Ref), f64> = HashMap::new();
    for p in scored {
        score_of.insert(key(&p.a, &p.b), p.score);
    }
    let mut xs: Vec<f64> = Vec::new();
    let mut ys: Vec<u8> = Vec::new();
    let (mut pos, mut neg, mut cand_hit_pos, mut total_pos) = (0usize, 0usize, 0usize, 0usize);
    for g in &golden.pairs {
        let s = score_of.get(&key(&g.a, &g.b)).copied().unwrap_or(0.0);
        xs.push(s);
        ys.push(g.label as u8);
        if g.label {
            pos += 1;
            total_pos += 1;
            if score_of.contains_key(&key(&g.a, &g.b)) {
                cand_hit_pos += 1;
            }
        } else {
            neg += 1;
        }
    }
    Metrics {
        gold_pairs: golden.pairs.len(),
        positives: pos,
        negatives: neg,
        scored_candidates: scored.len(),
        roc_auc: roc_auc(&xs, &ys),
        pr_auc: average_precision(&xs, &ys),
        r_at_p95: recall_at_precision(&xs, &ys, 0.95),
        r_at_p99: recall_at_precision(&xs, &ys, 0.99),
        candidate_recall: if total_pos == 0 {
            f64::NAN
        } else {
            cand_hit_pos as f64 / total_pos as f64
        },
    }
}

// --- ranking metrics (ported from the validated survey harness) ---

fn roc_auc(scores: &[f64], labels: &[u8]) -> f64 {
    let mut idx: Vec<usize> = (0..scores.len()).collect();
    idx.sort_by(|&a, &b| scores[a].partial_cmp(&scores[b]).unwrap());
    // average ranks with tie handling
    let mut ranks = vec![0.0f64; scores.len()];
    let mut i = 0;
    while i < idx.len() {
        let mut j = i;
        while j + 1 < idx.len() && scores[idx[j + 1]] == scores[idx[i]] {
            j += 1;
        }
        let avg = (i + j) as f64 / 2.0 + 1.0;
        for k in i..=j {
            ranks[idx[k]] = avg;
        }
        i = j + 1;
    }
    let npos = labels.iter().filter(|&&l| l == 1).count();
    let nneg = labels.len() - npos;
    if npos == 0 || nneg == 0 {
        return f64::NAN;
    }
    let sum_pos: f64 = ranks
        .iter()
        .zip(labels)
        .filter(|(_, &l)| l == 1)
        .map(|(r, _)| *r)
        .sum();
    (sum_pos - npos as f64 * (npos as f64 + 1.0) / 2.0) / (npos as f64 * nneg as f64)
}

fn average_precision(scores: &[f64], labels: &[u8]) -> f64 {
    let mut idx: Vec<usize> = (0..scores.len()).collect();
    idx.sort_by(|&a, &b| scores[b].partial_cmp(&scores[a]).unwrap());
    let npos = labels.iter().filter(|&&l| l == 1).count();
    if npos == 0 {
        return f64::NAN;
    }
    let (mut tp, mut fp, mut ap, mut prev_recall) = (0.0, 0.0, 0.0, 0.0);
    for &i in &idx {
        if labels[i] == 1 {
            tp += 1.0;
        } else {
            fp += 1.0;
        }
        let recall = tp / npos as f64;
        let precision = tp / (tp + fp);
        ap += (recall - prev_recall) * precision;
        prev_recall = recall;
    }
    ap
}

fn recall_at_precision(scores: &[f64], labels: &[u8], min_prec: f64) -> f64 {
    let mut idx: Vec<usize> = (0..scores.len()).collect();
    idx.sort_by(|&a, &b| scores[b].partial_cmp(&scores[a]).unwrap());
    let npos = labels.iter().filter(|&&l| l == 1).count();
    if npos == 0 {
        return f64::NAN;
    }
    let (mut tp, mut fp, mut best) = (0.0, 0.0, 0.0);
    for &i in &idx {
        if labels[i] == 1 {
            tp += 1.0;
        } else {
            fp += 1.0;
        }
        let precision = tp / (tp + fp);
        let recall = tp / npos as f64;
        if precision >= min_prec && recall > best {
            best = recall;
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perfect_separation_scores_one() {
        let xs = [0.9, 0.8, 0.2, 0.1];
        let ys = [1u8, 1, 0, 0];
        assert!((roc_auc(&xs, &ys) - 1.0).abs() < 1e-9);
        assert!((average_precision(&xs, &ys) - 1.0).abs() < 1e-9);
        assert!((recall_at_precision(&xs, &ys, 0.95) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn evaluate_against_golden() {
        let body =
            "# Install\n\nDownload the release binary and put it on your PATH, then run the \
                    version subcommand to confirm the installation completed without any errors.";
        let other =
            "# Topic\n\nQuantum entanglement correlates distant particles so a measurement \
                     on one instantly constrains the outcome of the other across vast distances.";
        let docs = vec![
            ("a.md".to_string(), body.to_string()),
            ("b.md".to_string(), body.to_string()),
            ("c.md".to_string(), other.to_string()),
        ];
        let scored = score_pairs(&docs, 8);
        // a/b are a true dup; (a,c) is a true non-dup.
        let g = Golden {
            pairs: vec![
                GoldPair {
                    a: Ref {
                        path: "a.md".into(),
                        start: 1,
                        end: 3,
                    },
                    b: Ref {
                        path: "b.md".into(),
                        start: 1,
                        end: 3,
                    },
                    label: true,
                    source: "construction".into(),
                },
                GoldPair {
                    a: Ref {
                        path: "a.md".into(),
                        start: 1,
                        end: 3,
                    },
                    b: Ref {
                        path: "c.md".into(),
                        start: 1,
                        end: 3,
                    },
                    label: false,
                    source: "construction".into(),
                },
            ],
        };
        let m = evaluate(&scored, &g);
        assert_eq!(m.positives, 1);
        assert_eq!(m.negatives, 1);
        assert!((m.roc_auc - 1.0).abs() < 1e-9, "roc={}", m.roc_auc);
        assert!((m.candidate_recall - 1.0).abs() < 1e-9);
    }

    /// Precision regression gate on the committed multi-domain docs golden (the representative,
    /// harder corpus). Floors set conservatively below observed (PR-AUC 0.944, cand-recall 1.0).
    #[test]
    fn docs_golden_precision_floor() {
        let base = concat!(env!("CARGO_MANIFEST_DIR"), "/../../bench/markdown");
        let golden: Golden = serde_json::from_str(
            &std::fs::read_to_string(format!("{base}/golden.docs.v1.json")).unwrap(),
        )
        .unwrap();
        let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(format!("{base}/corpus-docs"))
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("md"))
            .collect();
        paths.sort();
        let docs: Vec<(String, String)> = paths
            .iter()
            .map(|p| {
                let fname = p.file_name().unwrap().to_str().unwrap();
                (
                    format!("bench/markdown/corpus-docs/{fname}"),
                    std::fs::read_to_string(p).unwrap(),
                )
            })
            .collect();
        let m = evaluate(&score_pairs(&docs, 8), &golden);
        eprintln!(
            "docs golden: pr_auc={:.4} roc={:.4} r@p95={:.3} cand_recall={}",
            m.pr_auc, m.roc_auc, m.r_at_p95, m.candidate_recall
        );
        assert!(
            m.candidate_recall >= 0.95,
            "candidate recall regressed: {}",
            m.candidate_recall
        );
        assert!(
            m.pr_auc >= 0.88,
            "multi-domain PR-AUC regressed: {}",
            m.pr_auc
        );
    }
}
