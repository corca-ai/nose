//! Research instrument for the oracle-completeness campaign: which IL constructs
//! keep units OUT of the interpreter oracle, and how much fingerprint-merge mass
//! is therefore unverified.
//!
//! `nose verify --exclusion-census <path>` records one [`CensusUnit`] per
//! counted function unit — its oracle outcome, its value fingerprint, and the
//! raw construct tags present in the subtree the oracle would interpret. The
//! report then derives, per construct tag, how many units carrying it were
//! excluded and how many fingerprint-equal pairs are unverified because at
//! least one side was excluded.
//!
//! The census deliberately records raw tags (node kinds, builtin names, literal
//! retention classes) for BOTH interpretable and excluded units rather than a
//! hard-coded "unsupported construct" list: the interpreter's handled set
//! drifts (experiments §BF), so the discriminating constructs are derived from
//! the two populations at analysis time instead of asserted here.

use anyhow::Result;
use nose_il::{Il, NodeId, NodeKind, Payload};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

/// One counted function unit's census record.
pub(crate) struct CensusUnit {
    pub(crate) loc: String,
    /// `"interpretable"`, or the exclusion reason: `"battery-bail"`,
    /// `"empty-fp"`, `"no-core-span"`.
    pub(crate) reason: &'static str,
    /// Value fingerprint (empty for `"empty-fp"` units).
    pub(crate) fp: Vec<u64>,
    /// Sorted construct tags present in the interpreted subtree.
    pub(crate) tags: Vec<String>,
}

impl CensusUnit {
    fn excluded(&self) -> bool {
        self.reason != "interpretable"
    }
}

/// The construct tags present in `root`'s subtree, sorted and deduplicated.
///
/// Tag vocabulary: `kind:<NodeKind>` for every node kind, refined for the two
/// payload-sensitive kinds — calls become `builtin:<Builtin>` / `call:named` /
/// `call:cid` / `call:other`, and literals tag only their *unretained* classes
/// (`lit:unretained:<class>`), since retained literal values are interpretable.
pub(crate) fn census_tags(il: &Il, root: NodeId) -> Vec<String> {
    let mut tags: HashSet<String> = HashSet::new();
    let mut stack = vec![root];
    while let Some(x) = stack.pop() {
        let node = il.node(x);
        match node.kind {
            NodeKind::Call => {
                tags.insert(match node.payload {
                    Payload::Builtin(b) => format!("builtin:{b:?}"),
                    Payload::Name(_) => "call:named".to_string(),
                    Payload::Cid(_) => "call:cid".to_string(),
                    _ => "call:other".to_string(),
                });
            }
            NodeKind::Lit => match node.payload {
                Payload::LitInt(_) | Payload::LitBool(_) | Payload::LitStr(_) => {}
                Payload::Lit(c) => {
                    tags.insert(format!("lit:unretained:{c:?}"));
                }
                _ => {
                    tags.insert("lit:other".to_string());
                }
            },
            k => {
                tags.insert(format!("kind:{k:?}"));
            }
        }
        stack.extend(il.children(x).iter().copied());
    }
    let mut v: Vec<String> = tags.into_iter().collect();
    v.sort();
    v
}

#[derive(serde::Serialize)]
struct TagRow {
    tag: String,
    interpretable_units: usize,
    excluded_units: usize,
    /// Fingerprint-equal pairs with ≥1 excluded side, attributed to every tag
    /// present in an excluded member of the group (multi-attributed by design —
    /// a pair is unverified because of *all* the constructs that kept its
    /// members out, and the ranking question is "which construct, if covered,
    /// touches the most unverified mass").
    unverified_pairs: usize,
    example_excluded: Vec<String>,
}

#[derive(serde::Serialize)]
struct CensusReport {
    units_total: usize,
    interpretable_units: usize,
    excluded_by_reason: BTreeMap<String, usize>,
    /// Fingerprint-equal pair mass: `verified` pairs had both sides interpreted
    /// by the oracle; `unverified` pairs carry no behavioral check at all.
    merge_pairs: MergePairs,
    /// Per-construct rows, sorted by unverified mass (the campaign order).
    tags: Vec<TagRow>,
}

#[derive(serde::Serialize)]
struct MergePairs {
    total: usize,
    verified: usize,
    unverified: usize,
}

fn pairs(n: usize) -> usize {
    n * (n - 1) / 2
}

fn build_report(units: &[CensusUnit]) -> CensusReport {
    let mut excluded_by_reason: BTreeMap<String, usize> = BTreeMap::new();
    let mut tag_interp: HashMap<&str, usize> = HashMap::new();
    let mut tag_excl: HashMap<&str, usize> = HashMap::new();
    let mut tag_examples: HashMap<&str, Vec<&str>> = HashMap::new();
    for u in units {
        if u.excluded() {
            *excluded_by_reason.entry(u.reason.to_string()).or_default() += 1;
        }
        for t in &u.tags {
            if u.excluded() {
                *tag_excl.entry(t).or_default() += 1;
                tag_examples.entry(t).or_default().push(&u.loc);
            } else {
                *tag_interp.entry(t).or_default() += 1;
            }
        }
    }

    let mut by_fp: HashMap<&[u64], Vec<&CensusUnit>> = HashMap::new();
    for u in units {
        if !u.fp.is_empty() {
            by_fp.entry(&u.fp).or_default().push(u);
        }
    }
    let mut merge = MergePairs {
        total: 0,
        verified: 0,
        unverified: 0,
    };
    let mut tag_unverified: HashMap<&str, usize> = HashMap::new();
    for group in by_fp.values() {
        if group.len() < 2 {
            continue;
        }
        let interp = group.iter().filter(|u| !u.excluded()).count();
        let (total, verified) = (pairs(group.len()), pairs(interp));
        merge.total += total;
        merge.verified += verified;
        let unverified = total - verified;
        if unverified == 0 {
            continue;
        }
        merge.unverified += unverified;
        let mut group_tags: HashSet<&str> = HashSet::new();
        for u in group.iter().filter(|u| u.excluded()) {
            group_tags.extend(u.tags.iter().map(String::as_str));
        }
        for t in group_tags {
            *tag_unverified.entry(t).or_default() += unverified;
        }
    }

    let all_tags: HashSet<&str> = tag_interp.keys().chain(tag_excl.keys()).copied().collect();
    let mut rows: Vec<TagRow> = all_tags
        .into_iter()
        .map(|t| {
            let mut examples: Vec<&str> = tag_examples.get(t).cloned().unwrap_or_default();
            examples.sort_unstable();
            examples.truncate(3);
            TagRow {
                tag: t.to_string(),
                interpretable_units: tag_interp.get(t).copied().unwrap_or(0),
                excluded_units: tag_excl.get(t).copied().unwrap_or(0),
                unverified_pairs: tag_unverified.get(t).copied().unwrap_or(0),
                example_excluded: examples.into_iter().map(str::to_string).collect(),
            }
        })
        .collect();
    rows.sort_by(|a, b| {
        b.unverified_pairs
            .cmp(&a.unverified_pairs)
            .then(b.excluded_units.cmp(&a.excluded_units))
            .then(a.tag.cmp(&b.tag))
    });

    CensusReport {
        units_total: units.len(),
        interpretable_units: units.iter().filter(|u| !u.excluded()).count(),
        excluded_by_reason,
        merge_pairs: merge,
        tags: rows,
    }
}

/// Write the exclusion-census JSON report. Deterministic: every list is sorted
/// on stable keys, so the file is byte-identical across runs and thread counts.
pub(crate) fn write_report(path: &Path, units: &[CensusUnit]) -> Result<()> {
    let report = build_report(units);
    std::fs::write(path, serde_json::to_string_pretty(&report)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use nose_il::{Builtin, FileId, FileMeta, IlBuilder, Lang, Span};

    fn unit(loc: &str, reason: &'static str, fp: Vec<u64>, tags: &[&str]) -> CensusUnit {
        CensusUnit {
            loc: loc.to_string(),
            reason,
            fp,
            tags: tags.iter().map(|t| t.to_string()).collect(),
        }
    }

    #[test]
    fn report_counts_unverified_merge_mass_per_tag() {
        // fp group {a,b,c}: a interpretable, b/c excluded → 3 pairs, 0 verified.
        // fp group {d,e}: both interpretable → 1 pair, verified.
        let units = vec![
            unit("a.py:1", "interpretable", vec![1, 2], &["kind:Loop"]),
            unit(
                "b.py:1",
                "battery-bail",
                vec![1, 2],
                &["kind:Loop", "call:named"],
            ),
            unit("c.py:9", "battery-bail", vec![1, 2], &["builtin:Len"]),
            unit("d.py:1", "interpretable", vec![3], &["kind:Loop"]),
            unit("e.py:1", "interpretable", vec![3], &["kind:Loop"]),
            unit("f.py:1", "empty-fp", vec![], &["kind:Raw"]),
        ];
        let r = build_report(&units);
        assert_eq!(r.units_total, 6);
        assert_eq!(r.interpretable_units, 3);
        assert_eq!(r.excluded_by_reason["battery-bail"], 2);
        assert_eq!(r.excluded_by_reason["empty-fp"], 1);
        assert_eq!(r.merge_pairs.total, 4);
        assert_eq!(r.merge_pairs.verified, 1);
        assert_eq!(r.merge_pairs.unverified, 3);
        let row = |tag: &str| r.tags.iter().find(|t| t.tag == tag).unwrap();
        // All 3 unverified pairs touch an excluded member carrying each tag.
        assert_eq!(row("call:named").unverified_pairs, 3);
        assert_eq!(row("builtin:Len").unverified_pairs, 3);
        assert_eq!(row("kind:Loop").unverified_pairs, 3);
        assert_eq!(row("kind:Loop").interpretable_units, 3);
        assert_eq!(row("kind:Loop").excluded_units, 1);
        assert_eq!(row("kind:Raw").unverified_pairs, 0);
        assert_eq!(row("call:named").example_excluded, vec!["b.py:1"]);
    }

    #[test]
    fn census_tags_refine_calls_and_skip_retained_literals() {
        let sp = Span::synthetic(FileId(0));
        let mut b = IlBuilder::new(FileId(0));
        let s = b.add(NodeKind::Lit, Payload::LitStr(0xABCD), sp, &[]);
        let call = b.add(NodeKind::Call, Payload::Builtin(Builtin::Len), sp, &[s]);
        let ret = b.add(NodeKind::Return, Payload::None, sp, &[call]);
        let func = b.add(NodeKind::Func, Payload::None, sp, &[ret]);
        let il = b.finish(
            func,
            FileMeta {
                path: "census.rs".into(),
                lang: Lang::Rust,
            },
            Vec::new(),
            Vec::new(),
        );
        let tags = census_tags(&il, func);
        assert!(tags.contains(&"builtin:Len".to_string()));
        assert!(tags.contains(&"kind:Return".to_string()));
        assert!(tags.contains(&"kind:Func".to_string()));
        // The retained string literal contributes no tag.
        assert!(!tags.iter().any(|t| t.starts_with("lit:")));
        // The call node is refined, never a bare kind tag.
        assert!(!tags.contains(&"kind:Call".to_string()));
    }
}
