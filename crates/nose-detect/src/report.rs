//! Turn raw clone groups into ranked **refactoring opportunities**.
//!
//! For architecture/design-level refactoring, what matters is not "these two
//! functions are similar" but "this structure repeats across the codebase — extract
//! an abstraction." So we rank *families* (clone groups) by a refactoring-value
//! score that rewards:
//!   - **how much code** could be removed (`dup_lines` ≈ (members−1) × mean span),
//!   - **how clean** the extraction is (mean similarity),
//!   - **design-level spread** — a family spanning many files / modules signals a
//!     missing abstraction, weighted above a local copy-paste.

use crate::{Group, Loc, Report};
use serde::Serialize;
use std::path::Path;

/// A ranked refactoring opportunity: one clone family plus its design metrics.
#[derive(Serialize, Clone)]
pub struct RefactorFamily {
    /// Ranking score (higher = more worth refactoring). See `refactor_value`.
    pub value: f64,
    /// Number of duplicated sites.
    pub members: usize,
    /// Distinct files the family spans.
    pub files: usize,
    /// Distinct directories/modules the family spans (design-level spread).
    pub modules: usize,
    /// Distinct languages (cross-language family).
    pub languages: usize,
    /// Mean pairwise similarity within the family.
    pub mean_score: f64,
    /// Mean lines per member.
    pub mean_lines: u32,
    /// Lines that could be removed by extracting one shared copy
    /// (≈ `(members − 1) × mean_lines`).
    pub dup_lines: u32,
    /// The duplicated sites, largest first.
    pub locations: Vec<Loc>,
    /// Mean value-graph size across members (low → computation-poor type/data def).
    pub mean_sem: f64,
    /// Where the duplication lives: `"prod"`, `"test"` (all sites in test code), or
    /// `"mixed"` (logic duplicated *across* the test boundary — ranked normally
    /// because it's a real leak, unlike intentional test scaffolding).
    pub scope: &'static str,
}

/// The directory ("module") a file lives in — the design-level grouping key.
fn module_of(file: &str) -> &str {
    Path::new(file)
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or("")
}

fn span_lines(l: &Loc) -> u32 {
    l.end_line.saturating_sub(l.start_line) + 1
}

/// Fraction of `b`'s lines that lie inside `a` (both in the same file). Used to
/// collapse a site that is contained in — or near-identical to — a larger one.
fn overlap_frac(a: &Loc, b: &Loc) -> f64 {
    let start = a.start_line.max(b.start_line);
    let end = a.end_line.min(b.end_line);
    if end < start {
        return 0.0;
    }
    (end - start + 1) as f64 / span_lines(b).max(1) as f64
}

/// Compute the refactoring-value score for a family's metrics.
///
/// `dup_lines` is the backbone (how much code disappears). Mean similarity scales
/// it (a 0.7-similar family needs more manual work than a 0.99 one). The design
/// multiplier rewards spread: cross-module duplication is a missing abstraction;
/// cross-language is a notable (if harder) design signal.
fn refactor_value(
    mean_lines: u32,
    members: usize,
    mean_score: f64,
    files: usize,
    modules: usize,
    languages: usize,
) -> f64 {
    // Removable code grows with the number of copies, but with DIMINISHING returns:
    // the first few dedups capture the design win, whereas a fragment repeated across
    // hundreds of sites is almost always an idiom / generated / boilerplate pattern
    // (a Javadoc nav block, test scaffolding), not an extractable abstraction. So the
    // copy count is linear up to a small knee, then square-root-dampened — fanout no
    // longer rewards the ranking *linearly* (a 400-copy family is ~20× a 2-copy one,
    // not 400×). The reported `dup_lines` stays the honest `mean_lines × (members−1)`;
    // only this ranking score is dampened.
    let copies = members.saturating_sub(1) as f64;
    const KNEE: f64 = 6.0;
    let effective_copies = if copies <= KNEE {
        copies
    } else {
        KNEE + (copies - KNEE).sqrt()
    };
    let spread = 1.0
        + 0.30 * (files.min(8) as f64 - 1.0).max(0.0)
        + 0.50 * (modules.min(6) as f64 - 1.0).max(0.0)
        + 0.50 * (languages as f64 - 1.0).max(0.0);
    mean_lines as f64 * effective_copies * mean_score * spread
}

/// Is this site test code, by the usual path / unit-name conventions? Conservative:
/// only well-known markers, so production code is never misclassified as a test.
fn is_test_loc(l: &Loc) -> bool {
    let p = l.file.to_ascii_lowercase();
    let path_test = p.contains("/test/")
        || p.contains("/tests/")
        || p.contains("/__tests__/")
        || p.contains("/spec/")
        || p.starts_with("test/")
        || p.starts_with("tests/")
        || p.ends_with("_test.go")
        || p.ends_with("conftest.py")
        || ["_test.", ".test.", ".spec.", "_spec."]
            .iter()
            .any(|m| p.contains(m))
        || file_stem(&p).starts_with("test_");
    let name_test = l
        .name
        .as_deref()
        .is_some_and(|n| n.starts_with("Test") || n.starts_with("test_"));
    path_test || name_test
}

fn file_stem(path: &str) -> &str {
    let file = path.rsplit('/').next().unwrap_or(path);
    file.split('.').next().unwrap_or(file)
}

/// Is this site vendored / generated / third-party code — not the maintainer's to
/// dedupe? Conservative, well-known markers only. On the labelset, families all of
/// whose sites match this were 0/12 worthy.
fn is_generated_loc(l: &Loc) -> bool {
    let p = l.file.to_ascii_lowercase();
    [
        "vendor/",
        "third_party/",
        "third-party/",
        "/deps/",
        "node_modules/",
        "/dist/",
        "/build/",
        ".min.",
        ".pb.",
        "_pb2",
        ".g.dart",
        "generated/",
        "/gen/",
        ".generated.",
    ]
    .iter()
    .any(|m| p.contains(m))
}

/// Below this mean value-graph size, an all-`Class` family is a field-only type
/// definition (a record/enum/DTO), not shared behavior — see the dogfood review.
const TYPEDEF_SEM: f64 = 12.0;

/// Refactor-worthiness discount in `(0, 1]`, applied after `refactor_value`.
/// Discounts families a reviewer reliably dismisses, without dropping them:
///   - **value-poor type definitions** — `Class` families matching only on field
///     shape, no behavior to extract;
///   - **vendored / generated code** — not the maintainer's to dedupe (0/12 worthy
///     on the labelset).
///
/// Note: test-code duplication is *not* discounted — it's a real smell, ranked like
/// any other; `scope` stays a context tag, not a penalty.
///
/// Disable with `NOSE_NO_REFACTOR_DISCOUNT=1` (used for A/B measurement).
fn refactor_discount(all_class: bool, mean_sem: f64, all_generated: bool) -> f64 {
    if std::env::var_os("NOSE_NO_REFACTOR_DISCOUNT").is_some() {
        return 1.0;
    }
    let mut q = 1.0;
    if all_class && mean_sem < TYPEDEF_SEM {
        q *= 0.25;
    }
    if all_generated {
        q *= 0.1;
    }
    q
}

fn family_of(group: &Group) -> RefactorFamily {
    // Collapse co-located units to one refactoring site. Block extraction yields a
    // function unit *and* inner blocks that overlap it, and near-identical spans can
    // differ by a line; all of these are one place to refactor, not several. Keep the
    // largest enclosing span per file and drop anything that substantially overlaps it.
    let mut locs = group.members.clone();
    // Largest span first (within a file) so the enclosing unit wins.
    locs.sort_by(|a, b| {
        a.file
            .cmp(&b.file)
            .then_with(|| span_lines(b).cmp(&span_lines(a)))
            .then_with(|| a.start_line.cmp(&b.start_line))
    });
    let mut kept: Vec<Loc> = Vec::with_capacity(locs.len());
    for l in locs {
        let subsumed = kept
            .iter()
            .any(|k| k.file == l.file && overlap_frac(k, &l) >= 0.5);
        if !subsumed {
            kept.push(l);
        }
    }
    let mut locs = kept;
    locs.sort_by_key(|b| std::cmp::Reverse(span_lines(b)));
    let members = locs.len();
    let total_lines: u32 = locs.iter().map(span_lines).sum();
    let mean_lines = if members > 0 {
        total_lines / members as u32
    } else {
        0
    };
    let dup_lines = mean_lines * (members.saturating_sub(1) as u32);

    let mut files: Vec<&str> = locs.iter().map(|l| l.file.as_str()).collect();
    files.sort_unstable();
    files.dedup();
    let mut modules: Vec<&str> = locs.iter().map(|l| module_of(&l.file)).collect();
    modules.sort_unstable();
    modules.dedup();
    let mut langs: Vec<&str> = locs.iter().map(|l| l.lang.as_str()).collect();
    langs.sort_unstable();
    langs.dedup();

    let mean_sem = if members > 0 {
        locs.iter().map(|l| l.sem as f64).sum::<f64>() / members as f64
    } else {
        0.0
    };
    let n_test = locs.iter().filter(|l| is_test_loc(l)).count();
    let scope = if n_test == 0 {
        "prod"
    } else if n_test == members {
        "test"
    } else {
        "mixed"
    };
    let all_class = locs.iter().all(|l| l.kind == nose_il::UnitKind::Class);
    let all_generated = locs.iter().all(is_generated_loc);

    let value = refactor_value(
        mean_lines,
        members,
        group.score,
        files.len(),
        modules.len(),
        langs.len(),
    ) * refactor_discount(all_class, mean_sem, all_generated);
    RefactorFamily {
        value,
        members,
        files: files.len(),
        modules: modules.len(),
        languages: langs.len(),
        mean_score: group.score,
        mean_lines,
        dup_lines,
        locations: locs,
        mean_sem,
        scope,
    }
}

/// Rank a detection report's groups as refactoring opportunities, highest value
/// first. Trivial families (a single pair of tiny fragments) sink to the bottom.
pub fn rank_families(report: &Report) -> Vec<RefactorFamily> {
    let mut fams: Vec<RefactorFamily> = report.groups.iter().map(family_of).collect();
    fams.sort_by(|a, b| b.value.total_cmp(&a.value));
    // Drop families subsumed by a higher-value one: block-unit extraction can make a
    // family of inner blocks (e.g. a shared loop body) alongside the family of the
    // enclosing functions that contain it — the same regions, reported twice. Keep
    // the larger (already first by value), drop the contained one.
    let mut kept: Vec<RefactorFamily> = Vec::with_capacity(fams.len());
    for f in fams {
        if !kept.iter().any(|k| subsumes(k, &f)) {
            kept.push(f);
        }
    }
    kept
}

/// Does family `outer` subsume `inner` — i.e. every `inner` site sits inside some
/// `outer` site in the same file? Then `inner` is a sub-structure already covered.
fn subsumes(outer: &RefactorFamily, inner: &RefactorFamily) -> bool {
    if outer.locations.len() < inner.locations.len() {
        return false;
    }
    inner.locations.iter().all(|i| {
        outer
            .locations
            .iter()
            .any(|o| o.file == i.file && o.start_line <= i.start_line && o.end_line >= i.end_line)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Metrics, Report};
    use nose_il::UnitKind::{Class, Function};

    fn loc(file: &str, s: u32, e: u32, lang: &str) -> Loc {
        Loc {
            file: file.into(),
            start_line: s,
            end_line: e,
            lang: lang.into(),
            kind: Function,
            name: None,
            sem: 50,
        }
    }
    /// A site with explicit kind / value-graph size / name (for discount tests).
    fn loc_k(file: &str, s: u32, e: u32, kind: nose_il::UnitKind, sem: usize) -> Loc {
        Loc {
            file: file.into(),
            start_line: s,
            end_line: e,
            lang: "rust".into(),
            kind,
            name: None,
            sem,
        }
    }
    fn report(groups: Vec<Group>) -> Report {
        Report {
            tool: "nose",
            version: "test",
            detector: "structural".into(),
            duplicates: vec![],
            groups,
            metrics: Metrics {
                files: 0,
                units: 0,
                candidate_pairs: 0,
                accepted_pairs: 0,
                groups: 0,
            },
        }
    }

    #[test]
    fn dedups_colocated_units() {
        // a function and an inner block with the same span = one site
        let g = Group {
            score: 1.0,
            members: vec![
                loc("a.rs", 1, 20, "rust"),
                loc("a.rs", 1, 20, "rust"),
                loc("b.rs", 1, 20, "rust"),
            ],
        };
        let f = &rank_families(&report(vec![g]))[0];
        assert_eq!(
            f.members, 2,
            "co-located identical spans collapse to one site"
        );
        assert_eq!(f.files, 2);
    }

    #[test]
    fn subsumed_family_is_dropped() {
        // An outer family of two functions, and an inner family of blocks contained
        // within them (same regions, reported twice) — only the outer survives.
        let outer = Group {
            score: 0.9,
            members: vec![loc("a.rs", 10, 40, "rust"), loc("b.rs", 10, 40, "rust")],
        };
        let inner = Group {
            score: 1.0,
            members: vec![loc("a.rs", 15, 25, "rust"), loc("b.rs", 15, 25, "rust")],
        };
        let fams = rank_families(&report(vec![inner, outer]));
        assert_eq!(fams.len(), 1, "the contained family should be dropped");
        assert_eq!(
            fams[0].mean_lines, 31,
            "the surviving family is the outer one"
        );
    }

    #[test]
    fn collapses_overlapping_and_nested_sites() {
        // A function (247-273) and an inner block (259-271), plus a near-identical
        // off-by-one span (143-167 vs 144-167) all collapse to their enclosing site.
        let g = Group {
            score: 0.9,
            members: vec![
                loc("seg.py", 247, 273, "python"), // function
                loc("seg.py", 259, 271, "python"), // inner block — contained
                loc("seg.py", 276, 304, "python"), // a distinct second function
                loc("seg.py", 290, 302, "python"), // inner block — contained
                loc("con.py", 143, 167, "python"),
                loc("con.py", 144, 167, "python"), // off-by-one near-duplicate
            ],
        };
        let f = &rank_families(&report(vec![g]))[0];
        assert_eq!(
            f.members, 3,
            "two functions in seg.py + one region in con.py = 3 sites"
        );
        assert_eq!(f.files, 2);
    }

    #[test]
    fn keeps_adjacent_distinct_sites() {
        // Adjacent but non-overlapping regions are genuinely separate sites.
        let g = Group {
            score: 0.9,
            members: vec![
                loc("p.py", 714, 762, "python"),
                loc("p.py", 763, 794, "python"),
                loc("p.py", 795, 818, "python"),
            ],
        };
        let f = &rank_families(&report(vec![g]))[0];
        assert_eq!(
            f.members, 3,
            "adjacent non-overlapping regions stay distinct"
        );
    }

    #[test]
    fn dup_lines_and_module_spread() {
        // 3 copies of a ~10-line unit across 3 modules
        let g = Group {
            score: 0.9,
            members: vec![
                loc("x/a.rs", 1, 10, "rust"),
                loc("y/b.rs", 1, 10, "rust"),
                loc("z/c.rs", 1, 10, "rust"),
            ],
        };
        let f = &rank_families(&report(vec![g]))[0];
        assert_eq!(f.members, 3);
        assert_eq!(f.modules, 3);
        assert_eq!(f.mean_lines, 10);
        assert_eq!(f.dup_lines, 20, "(members-1) * mean_lines");
    }

    #[test]
    fn ranks_by_value_design_level_first() {
        // big cross-module family should outrank a small local pair
        let big = Group {
            score: 0.8,
            members: (0..10)
                .map(|i| loc(&format!("m{i}/f.rs"), 1, 30, "rust"))
                .collect(),
        };
        let small = Group {
            score: 1.0,
            members: vec![loc("p/a.rs", 1, 6, "rust"), loc("p/b.rs", 1, 6, "rust")],
        };
        let fams = rank_families(&report(vec![small, big]));
        assert!(
            fams[0].members == 10,
            "the large cross-module family ranks first"
        );
        assert!(fams[0].value > fams[1].value);
    }

    #[test]
    fn cross_language_bonus() {
        let mono = Group {
            score: 0.9,
            members: vec![loc("a.py", 1, 10, "python"), loc("b.py", 1, 10, "python")],
        };
        let cross = Group {
            score: 0.9,
            members: vec![
                loc("a.py", 1, 10, "python"),
                loc("b.ts", 1, 10, "typescript"),
            ],
        };
        let fm = family_of(&mono);
        let fc = family_of(&cross);
        assert_eq!(fc.languages, 2);
        assert!(
            fc.value > fm.value,
            "cross-language family is weighted higher"
        );
    }

    #[test]
    fn test_code_duplication_is_not_discounted() {
        // Duplication in tests is a real smell too — a test-only family with the same
        // metrics as a prod family gets the same value (only tagged, not penalised).
        let prod = Group {
            score: 1.0,
            members: vec![
                loc("src/a.rs", 1, 30, "rust"),
                loc("src/b.rs", 1, 30, "rust"),
            ],
        };
        let test = Group {
            score: 1.0,
            members: vec![
                loc("tests/a.rs", 1, 30, "rust"),
                loc("tests/b.rs", 1, 30, "rust"),
            ],
        };
        let fp = family_of(&prod);
        let ft = family_of(&test);
        assert_eq!(ft.scope, "test");
        assert_eq!(fp.scope, "prod");
        assert_eq!(
            ft.value, fp.value,
            "test-code duplication is ranked like any other (tag only, no penalty)"
        );
    }

    #[test]
    fn mixed_test_prod_is_not_discounted() {
        // Logic duplicated *across* the test boundary is a real smell — keep it.
        // Use a test *name* marker (not a test path) so the two families share the
        // same module/file/spread metrics and differ only in scope.
        let test_named = Loc {
            name: Some("test_thing".into()),
            ..loc("src/b.rs", 1, 30, "rust")
        };
        let mixed = Group {
            score: 1.0,
            members: vec![loc("src/a.rs", 1, 30, "rust"), test_named],
        };
        let pure = Group {
            score: 1.0,
            members: vec![
                loc("src/a.rs", 1, 30, "rust"),
                loc("src/b.rs", 1, 30, "rust"),
            ],
        };
        let fmixed = family_of(&mixed);
        let fpure = family_of(&pure);
        assert_eq!(fmixed.scope, "mixed");
        assert_eq!(
            fmixed.value, fpure.value,
            "test↔prod duplication is not discounted"
        );
    }

    #[test]
    fn value_poor_typedef_class_is_discounted() {
        // A field-only type definition (low value-graph) matches on shape alone.
        let typedef = Group {
            score: 1.0,
            members: vec![
                loc_k("src/a.rs", 1, 30, Class, 5),
                loc_k("src/b.rs", 1, 30, Class, 5),
            ],
        };
        // A behavior-rich class of the same size is a genuine candidate.
        let rich = Group {
            score: 1.0,
            members: vec![
                loc_k("src/c.rs", 1, 30, Class, 80),
                loc_k("src/d.rs", 1, 30, Class, 80),
            ],
        };
        let ftd = family_of(&typedef);
        let frich = family_of(&rich);
        assert!(
            frich.value > ftd.value,
            "value-poor type-def class is discounted below a behavior-rich one"
        );
        // A function family of the same low sem is NOT a type-def → not discounted.
        let func = Group {
            score: 1.0,
            members: vec![
                loc_k("src/e.rs", 1, 30, Function, 5),
                loc_k("src/f.rs", 1, 30, Function, 5),
            ],
        };
        assert!(
            family_of(&func).value > ftd.value,
            "the type-def discount applies only to all-Class families"
        );
    }

    #[test]
    fn vendored_family_is_discounted() {
        // All sites in vendored/generated paths → not the maintainer's to dedupe.
        let vendored = Group {
            score: 1.0,
            members: vec![
                loc("a/vendor/x.go", 1, 30, "go"),
                loc("b/vendor/y.go", 1, 30, "go"),
            ],
        };
        let owned = Group {
            score: 1.0,
            members: vec![loc("src/x.go", 1, 30, "go"), loc("src/y.go", 1, 30, "go")],
        };
        assert!(
            family_of(&owned).value > family_of(&vendored).value,
            "vendored duplication is discounted below owned-code duplication"
        );
    }
}
