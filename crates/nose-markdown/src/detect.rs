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
    /// Representative duplicated span (from the highest-scoring member pair), with its files.
    pub witness: Option<WitnessRef>,
}

pub struct Options {
    /// Minimum normalized prose words for a unit to participate (suppresses trivial fragments).
    pub min_words: usize,
    /// Relation acceptance threshold on the TF-IDF/containment score.
    pub threshold: f64,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            min_words: 8,
            threshold: 0.5,
        }
    }
}

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
        let cos = model.tfidf_cosine(&fps[i], &fps[j]);
        let cont = fingerprint::containment(&fps[i].shingles, &fps[j].shingles);
        // TF-IDF is the primary relation score; containment rescues small-in-large.
        let rel = if cont >= 0.8 && cont > cos { cont } else { cos };
        if rel >= opts.threshold {
            accepted.push((i, j, rel));
        }
    }
    if accepted.is_empty() {
        return Vec::new();
    }

    // Cluster by transitive closure.
    let mut uf = UnionFind::new(units.len());
    for &(i, j, _) in &accepted {
        uf.union(i, j);
    }
    let mut groups: std::collections::BTreeMap<usize, Vec<usize>> =
        std::collections::BTreeMap::new();
    for idx in 0..units.len() {
        let r = uf.find(idx);
        // only keep indices that participate in at least one accepted pair
        groups.entry(r).or_default().push(idx);
    }

    let mut families: Vec<Family> = groups
        .values()
        .filter_map(|members| build_family(members, &units, &fps, &model, &accepted))
        .collect();

    // Rank: removable lines first, then score, then members; deterministic tie-break by path.
    families.sort_by(|a, b| {
        b.removable
            .cmp(&a.removable)
            .then(b.score.partial_cmp(&a.score).unwrap())
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

    Some(Family {
        tier,
        score,
        members: fam_members,
        files,
        removable,
        commonness,
        exact,
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
}
