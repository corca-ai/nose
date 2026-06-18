//! Synthetic recall-vs-edit-ratio benchmark (#443).
//!
//! The golden set gates PRECISION; nothing gated RECALL. This benchmark injects controlled edits
//! into distinct base prose blocks at known ratios and measures how reliably the end-to-end
//! pipeline (candidate generation + Stage-2 acceptance) still recovers each base↔edited pair.
//! Deterministic (seeded), self-contained (committed base corpus embedded at compile time), and
//! wired into a test floor so a future change that silently sacrifices recall fails CI.

use crate::detect::{accept_pair, Options};
use crate::fingerprint::{self, Fingerprint};
use crate::unit::{self, Unit};
use crate::verify::CorpusModel;
use std::collections::HashSet;

const BASE_CORPUS: &str = include_str!("../../../bench/markdown/synth_base.md");

/// Deterministic LCG — no global RNG state, so the benchmark is byte-reproducible.
struct Lcg(u64);
impl Lcg {
    fn new(seed: u64) -> Self {
        Lcg(seed ^ 0x9E37_79B9_7F4A_7C15)
    }
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            ((self.next() >> 33) as usize) % n
        }
    }
}

/// The distinct base prose bodies (one per `##` section of the committed base corpus; the intro
/// `#` section is skipped).
pub fn base_blocks() -> Vec<String> {
    unit::split_units("synth_base.md", BASE_CORPUS)
        .into_iter()
        .filter(|u| u.heading.as_deref() != Some("Synthetic recall base corpus"))
        .map(|u| {
            // keep the body paragraph, drop the heading line
            u.raw
                .lines()
                .skip(1)
                .collect::<Vec<_>>()
                .join("\n")
                .trim()
                .to_string()
        })
        .filter(|b| b.split_whitespace().count() >= 25)
        .collect()
}

/// Replace `ratio` of the words with random words drawn from `vocab` (simulating edits).
fn reword(text: &str, ratio: f64, vocab: &[&str], rng: &mut Lcg) -> String {
    let mut words: Vec<String> = text.split_whitespace().map(|w| w.to_string()).collect();
    if words.is_empty() || vocab.is_empty() {
        return text.to_string();
    }
    let k = ((words.len() as f64) * ratio).round() as usize;
    for _ in 0..k {
        let i = rng.below(words.len());
        words[i] = vocab[rng.below(vocab.len())].to_string();
    }
    words.join(" ")
}

fn one_unit(body: &str) -> Unit {
    // each synthetic doc is a single section → exactly one unit
    unit::split_units("d.md", &format!("# B\n\n{body}"))
        .into_iter()
        .next()
        .expect("non-empty doc yields a unit")
}

/// Recall (fraction of base↔edited pairs recovered end-to-end) at each edit ratio.
pub fn recall_curve(ratios: &[f64]) -> Vec<(f64, f64)> {
    let bases = base_blocks();
    let n = bases.len();
    let vocab_owned: Vec<String> = bases
        .iter()
        .flat_map(|b| b.split_whitespace())
        .filter(|w| !w.is_empty() && w.chars().all(|c| c.is_alphanumeric()))
        .map(|w| w.to_string())
        .collect();
    let vocab: Vec<&str> = vocab_owned.iter().map(|s| s.as_str()).collect();
    let opts = Options::default();

    ratios
        .iter()
        .map(|&r| {
            let mut rng = Lcg::new(0xBEEF_u64.wrapping_add((r * 1000.0) as u64));
            // units: base_0..base_{n-1}, then edit_0..edit_{n-1} (same order ⇒ pair i ↔ n+i)
            let mut units: Vec<Unit> = bases.iter().map(|b| one_unit(b)).collect();
            for b in &bases {
                let e = reword(b, r, &vocab, &mut rng);
                units.push(one_unit(&e));
            }
            let fps: Vec<Fingerprint> = units.iter().map(Fingerprint::of).collect();
            let model = CorpusModel::fit(&fps);
            let cands: HashSet<(usize, usize)> =
                fingerprint::candidate_pairs(&fps).into_iter().collect();
            let hits = (0..n)
                .filter(|&i| {
                    cands.contains(&(i, n + i))
                        && accept_pair(&units, &fps, &model, i, n + i, &opts).is_some()
                })
                .count();
            (r, hits as f64 / n as f64)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_corpus_is_distinct_and_loaded() {
        let bases = base_blocks();
        assert!(
            bases.len() >= 12,
            "expected a dozen+ base blocks, got {}",
            bases.len()
        );
        // distinct: no two base blocks should themselves be a detected duplicate.
        let docs: Vec<(String, String)> = bases
            .iter()
            .enumerate()
            .map(|(i, b)| (format!("b{i}.md"), format!("# B\n\n{b}")))
            .collect();
        let fams = crate::detect(&docs, &Options::default());
        assert!(
            fams.is_empty(),
            "base blocks must be mutually distinct, got {fams:?}"
        );
    }

    #[test]
    fn recall_curve_meets_floor() {
        let ratios = [0.0, 0.1, 0.2, 0.35, 0.5];
        let curve = recall_curve(&ratios);
        for (r, rec) in &curve {
            eprintln!("edit_ratio {r:.2} -> recall {rec:.3}");
        }
        let get = |r: f64| curve.iter().find(|(x, _)| (x - r).abs() < 1e-9).unwrap().1;
        // Regression floors over the MEANINGFUL range (set conservatively below observed). The
        // perturbation is *scattered random* word replacement — harsher than real editing, so
        // these are lower bounds. recall@0.5 is the adversarial worst case (scattered edits at
        // that rate shatter nearly every char-5-gram); it is reported above for trend-tracking,
        // not asserted, as it sits at the detection floor and would make a brittle gate.
        assert!(
            get(0.0) >= 0.99,
            "exact copies must be recovered: {}",
            get(0.0)
        );
        assert!(get(0.1) >= 0.95, "recall@0.1 = {}", get(0.1));
        assert!(get(0.2) >= 0.85, "recall@0.2 = {}", get(0.2));
        assert!(get(0.35) >= 0.65, "recall@0.35 = {}", get(0.35));
        assert!(
            get(0.5) <= get(0.35),
            "recall must be monotonic non-increasing in edit ratio"
        );
    }

    #[test]
    fn recall_curve_is_deterministic() {
        assert_eq!(recall_curve(&[0.2, 0.5]), recall_curve(&[0.2, 0.5]));
    }
}
