//! Pipeline orchestrator: units → Stage 1 candidates → Stage 2 verify/rank → Stage 3 witness,
//! clustered into ranked near-duplicate families with orthogonal evidence fields.
//!
//! Honesty (epic #435): a family carries a relation tier + score, a span witness, and the
//! orthogonal evidence the user filters on (commonness, removable, files). It never asserts
//! "same meaning" or "worth removing".

use crate::fingerprint::{self, Fingerprint};
use crate::unit::{self, Unit, UnitKind};
use crate::verify::CorpusModel;
use crate::witness::{self, Span};

#[derive(Clone, Debug, serde::Serialize)]
pub struct Member {
    pub path: String,
    pub start_line: u32,
    pub end_line: u32,
    pub kind: UnitKind,
    pub heading: Option<String>,
}

/// A duplicated span together with the two files it was found in (the representative pair).
#[derive(Clone, Debug, serde::Serialize)]
pub struct WitnessRef {
    pub a_path: String,
    pub b_path: String,
    #[serde(flatten)]
    pub span: Span,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct Family {
    /// Relation tier: `exact | near-high | near-med | near-low | partial`.
    pub tier: &'static str,
    /// Mean pairwise relation score in 0..=1.
    pub score: f64,
    pub members: Vec<Member>,
    pub files: usize,
    /// Estimated removable lines if single-sourced: (members-1) * mean span lines.
    pub removable: u32,
    /// Orthogonal evidence: mean DF-fraction of the shared content (high ⇒ ubiquitous boilerplate).
    pub commonness: f64,
    /// Whether all members are normalized-identical (a true exact-render claim).
    pub exact: bool,
    /// A large, multi-file cluster: a repeated section *skeleton* (a template), not a per-instance
    /// clone. Reported separately so a templated-doc blob does not masquerade as one clone family.
    pub template: bool,
    /// Representative duplicated span (from the highest-scoring member pair), with its files.
    pub witness: Option<WitnessRef>,
}

pub struct Options {
    /// Minimum normalized prose words for a unit to participate (suppresses trivial fragments).
    pub min_words: usize,
    /// Relation acceptance threshold on the TF-IDF/containment score.
    pub threshold: f64,
    /// Edges below this score do not *merge* clusters (they avoid weak transitive chaining into
    /// mega-families); they still corroborate a family formed by stronger edges.
    pub cluster_threshold: f64,
    /// A pair must share at least this many char-gram shingles to be accepted — a match-substance
    /// floor that drops thin overlaps without requiring identical lines (so reworded near-dups,
    /// which have no identical line but share many grams, are preserved).
    pub min_shared_grams: usize,
    /// A containment-rescued (small-in-large) pair must have a contiguous span witness of at least
    /// this many lines — proof of a real shared block, not scattered-vocabulary coincidence.
    pub min_containment_witness: u32,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            min_words: 8,
            threshold: 0.5,
            cluster_threshold: 0.7,
            min_shared_grams: 24,
            min_containment_witness: 2,
        }
    }
}

/// A cluster this large and spread this wide is treated as a repeated template skeleton.
const TEMPLATE_MIN_MEMBERS: usize = 8;
const TEMPLATE_MIN_FILES: usize = 4;

struct UnionFind {
    parent: Vec<usize>,
}
impl UnionFind {
    fn new(n: usize) -> Self {
        UnionFind {
            parent: (0..n).collect(),
        }
    }
    fn find(&mut self, x: usize) -> usize {
        let mut r = x;
        while self.parent[r] != r {
            r = self.parent[r];
        }
        let mut c = x;
        while self.parent[c] != r {
            let n = self.parent[c];
            self.parent[c] = r;
            c = n;
        }
        r
    }
    fn union(&mut self, a: usize, b: usize) {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra != rb {
            self.parent[ra.max(rb)] = ra.min(rb);
        }
    }
}

/// Stage-2 acceptance for one candidate pair: returns the relation score if the pair clears the
/// threshold, the match-substance floor, and (for size-disparate containment matches) the
/// contiguous-witness gate — `None` otherwise. Shared by `detect` and the synthetic recall
/// benchmark so they can never drift apart.
pub(crate) fn accept_pair(
    units: &[Unit],
    fps: &[Fingerprint],
    model: &CorpusModel,
    i: usize,
    j: usize,
    opts: &Options,
) -> Option<f64> {
    let cos = model.tfidf_cosine(&fps[i], &fps[j]);
    let cont = fingerprint::containment(&fps[i].shingles, &fps[j].shingles);
    // TF-IDF is the primary relation score; containment rescues small-in-large.
    let rel = if cont >= 0.8 && cont > cos { cont } else { cos };
    let shared = fingerprint::shared_grams(&fps[i].shingles, &fps[j].shingles);
    if rel < opts.threshold || shared < opts.min_shared_grams {
        return None;
    }
    // Genuine SMALL-IN-LARGE (a real size disparity) only: a small unit's grams are "contained"
    // in a large one even by mere common-morpheme coincidence between unrelated docs. Require a
    // real CONTIGUOUS shared block (line witness) before trusting it. Same-size high-overlap pairs
    // are left to the score above, so reworded near-dups are unaffected.
    let (la, lb) = (fps[i].shingles.len(), fps[j].shingles.len());
    let small_in_large = la.min(lb) * 2 < la.max(lb) && cont >= 0.8 && cont > cos;
    if small_in_large {
        let has_block = witness::witness(&units[i], &units[j])
            .is_some_and(|w| w.matched_lines >= opts.min_containment_witness);
        if !has_block {
            return None;
        }
    }
    Some(rel)
}

/// Run the full pipeline over `(path, source)` documents.
pub fn detect(docs: &[(String, String)], opts: &Options) -> Vec<Family> {
    // Stage 0: units (filter to prose-bearing units above the trivial floor).
    let mut units: Vec<Unit> = Vec::new();
    for (path, src) in docs {
        for u in unit::split_units(path, src) {
            if u.prose_words() >= opts.min_words {
                units.push(u);
            }
        }
    }
    if units.len() < 2 {
        return Vec::new();
    }

    let fps: Vec<Fingerprint> = units.iter().map(Fingerprint::of).collect();
    let model = CorpusModel::fit(&fps);

    // Stage 1 → Stage 2: score every candidate pair, accept above threshold.
    let cands = fingerprint::candidate_pairs(&fps);
    let mut accepted: Vec<(usize, usize, f64)> = Vec::new();
    for (i, j) in cands {
        if let Some(rel) = accept_pair(&units, &fps, &model, i, j, opts) {
            accepted.push((i, j, rel));
        }
    }
    if accepted.is_empty() {
        return Vec::new();
    }

    // Cluster on STRONG edges only (>= cluster_threshold): weak edges in
    // [threshold, cluster_threshold) corroborate but never chain a mega-family together.
    let mut uf = UnionFind::new(units.len());
    for &(i, j, s) in &accepted {
        if s >= opts.cluster_threshold {
            uf.union(i, j);
        }
    }
    let mut groups: std::collections::BTreeMap<usize, Vec<usize>> =
        std::collections::BTreeMap::new();
    for idx in 0..units.len() {
        let r = uf.find(idx);
        groups.entry(r).or_default().push(idx);
    }

    let mut families: Vec<Family> = groups
        .values()
        .filter_map(|members| build_family(members, &units, &fps, &model, &accepted))
        .collect();

    // Rank: real per-instance families before templated blobs; then confidence-weighted
    // removable (so a low-cohesion blob can't top the list on raw size alone).
    families.sort_by(|a, b| {
        let key = |f: &Family| (f.removable as f64) * f.score;
        a.template
            .cmp(&b.template)
            .then(key(b).partial_cmp(&key(a)).unwrap())
            .then(b.members.len().cmp(&a.members.len()))
            .then(a.members[0].path.cmp(&b.members[0].path))
    });
    families
}

/// Build one ranked family from a cluster of unit indices, or `None` if it is not a real
/// (≥2-member, ≥1-accepted-pair) family.
fn build_family(
    members: &[usize],
    units: &[Unit],
    fps: &[Fingerprint],
    model: &CorpusModel,
    accepted: &[(usize, usize, f64)],
) -> Option<Family> {
    if members.len() < 2 {
        return None;
    }
    let memberset: std::collections::HashSet<usize> = members.iter().copied().collect();
    let inpairs: Vec<&(usize, usize, f64)> = accepted
        .iter()
        .filter(|(i, j, _)| memberset.contains(i) && memberset.contains(j))
        .collect();
    if inpairs.is_empty() {
        return None;
    }
    let score = inpairs.iter().map(|p| p.2).sum::<f64>() / inpairs.len() as f64;
    let commonness = inpairs
        .iter()
        .map(|&&(i, j, _)| model.commonness(&fps[i], &fps[j]))
        .sum::<f64>()
        / inpairs.len() as f64;
    let exact = members
        .iter()
        .all(|&m| units[m].norm == units[members[0]].norm);
    let rep = inpairs
        .iter()
        .max_by(|a, b| a.2.partial_cmp(&b.2).unwrap())
        .unwrap();
    let wit = witness::witness(&units[rep.0], &units[rep.1]).map(|span| WitnessRef {
        a_path: units[rep.0].path.clone(),
        b_path: units[rep.1].path.clone(),
        span,
    });

    let mut fam_members: Vec<Member> = members
        .iter()
        .map(|&m| Member {
            path: units[m].path.clone(),
            start_line: units[m].start_line,
            end_line: units[m].end_line,
            kind: units[m].kind,
            heading: units[m].heading.clone(),
        })
        .collect();
    fam_members
        .sort_by(|a, b| (a.path.as_str(), a.start_line).cmp(&(b.path.as_str(), b.start_line)));

    let files = fam_members
        .iter()
        .map(|m| m.path.as_str())
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    let mean_lines = fam_members
        .iter()
        .map(|m| (m.end_line - m.start_line + 1) as u64)
        .sum::<u64>()
        / fam_members.len() as u64;
    let removable = (fam_members.len() as u32 - 1) * mean_lines as u32;

    let mostly_containment = score < 0.5 || wit.is_none();
    let tier = if exact {
        "exact"
    } else if score >= 0.9 {
        "near-high"
    } else if score >= 0.7 {
        "near-med"
    } else if mostly_containment {
        "partial"
    } else {
        "near-low"
    };

    // A large cluster spread across many files is a repeated section skeleton (template), not a
    // per-instance clone — reported separately so it doesn't masquerade as one clone family.
    let template =
        !exact && fam_members.len() >= TEMPLATE_MIN_MEMBERS && files >= TEMPLATE_MIN_FILES;

    Some(Family {
        tier,
        score,
        members: fam_members,
        files,
        removable,
        commonness,
        exact,
        template,
        witness: wit,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(p: &str, s: &str) -> (String, String) {
        (p.to_string(), s.to_string())
    }

    #[test]
    fn finds_exact_copy_family() {
        let body =
            "# Install\n\nDownload the binary from the releases page and place it on your PATH. \
                    Then run the version command to confirm the installation succeeded correctly.";
        let docs = vec![doc("a.md", body), doc("b.md", body), doc("c.md", body)];
        let fams = detect(&docs, &Options::default());
        assert_eq!(fams.len(), 1);
        let f = &fams[0];
        assert!(f.exact);
        assert_eq!(f.tier, "exact");
        assert_eq!(f.members.len(), 3);
        assert_eq!(f.files, 3);
        assert!(f.witness.is_some());
    }

    #[test]
    fn finds_near_dup_not_unrelated() {
        let a =
            "# Guide\n\nThe configuration loader reads the settings file at startup and validates \
                 every field before the server begins accepting incoming network connections.";
        let b = "# Guide\n\nThe configuration loader reads the settings file on startup and validates \
                 each field before the service starts accepting incoming network connections today.";
        let c = "# Other\n\nQuantum entanglement lets distant particles share correlated measurement \
                 outcomes instantaneously across arbitrarily large separating spatial distances now.";
        let fams = detect(
            &[doc("a.md", a), doc("b.md", b), doc("c.md", c)],
            &Options::default(),
        );
        assert_eq!(fams.len(), 1, "only a/b should form a family");
        assert_eq!(fams[0].members.len(), 2);
        assert!(!fams[0].exact);
        assert!(fams[0].score > 0.5);
    }

    #[test]
    fn deterministic_output() {
        let body = "# T\n\nA reasonably long paragraph of prose that should be detected as a clear \
                    duplicate when copied verbatim into several different documentation files here.";
        let docs = vec![doc("a.md", body), doc("b.md", body)];
        let a = detect(&docs, &Options::default());
        let b = detect(&docs, &Options::default());
        assert_eq!(
            serde_json::to_string(&a).unwrap(),
            serde_json::to_string(&b).unwrap()
        );
    }

    #[test]
    fn large_multifile_cluster_is_flagged_template() {
        // 8 near-identical (not byte-identical) sections across 8 files → a repeated skeleton.
        let docs: Vec<(String, String)> = (0..8)
            .map(|i| {
                doc(
                    &format!("f{i}.md"),
                    &format!(
                        "# Endpoint\n\nThe service validates the request payload and then writes the \
                         record to the primary datastore before returning a confirmation number {i}."
                    ),
                )
            })
            .collect();
        let fams = detect(&docs, &Options::default());
        assert_eq!(fams.len(), 1);
        assert!(
            fams[0].template,
            "8 members across 8 files should be a template"
        );
        assert!(!fams[0].exact);
    }

    #[test]
    fn thin_overlap_is_not_a_family() {
        // Two docs sharing only a short generic phrase (below the min-shared-grams floor).
        let a =
            "# A\n\nThis document explains the alpha subsystem and its asynchronous queue drains.";
        let b = "# B\n\nThis document explains the beta module and its synchronous retular cache loads.";
        let fams = detect(&[doc("a.md", a), doc("b.md", b)], &Options::default());
        assert!(
            fams.is_empty(),
            "thin shared phrase must not form a family: {fams:?}"
        );
    }

    #[test]
    fn genuine_small_in_large_still_detected() {
        // A real multi-line block pasted into a larger doc must still be found (P3 must not
        // break true small-in-large — only scattered-vocabulary containment is dropped).
        let block = "The configuration loader reads the settings file at startup.\n\
                     It validates every required field before continuing.\n\
                     Then the server begins accepting incoming network connections.";
        let small = format!("# Note\n\n{block}");
        let large = format!(
            "# Host\n\nAn unrelated introduction about entirely different matters here.\n\n\
             {block}\n\nAn unrelated conclusion about other separate topics here too."
        );
        let fams = detect(
            &[doc("small.md", &small), doc("large.md", &large)],
            &Options::default(),
        );
        assert_eq!(fams.len(), 1, "pasted block should be found: {fams:?}");
        assert!(fams[0].witness.is_some());
    }
}
