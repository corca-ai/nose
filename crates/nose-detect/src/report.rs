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

use crate::{AbstractionWitness, Group, Loc, Report};
use nose_semantics::{value_law_provenance, ValueLawProvenance};
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
    /// Invariant (shared) source lines between the two representative copies — the
    /// body of the helper you'd extract. The honest counterpart to `mean_score`: two
    /// copies can be `sim 1.00` structurally yet share few literal lines (a dispatch
    /// skeleton wrapping divergent bodies). Computed at the presentation layer (needs
    /// source); `0` until then, or for cross-language families (no shared lines).
    pub shared_lines: u32,
    /// Number of varying spots between the two representative copies — the parameters
    /// the extracted helper would take. High param count ⇒ a costlier, uglier extract.
    /// Computed alongside `shared_lines`; `0` until then.
    pub params: u32,
    /// The same shared-line measure as `shared_lines`, but unrounded *and* weighted by
    /// how specific each line is (a pervasive idiom contributes ~0) — the value the
    /// ranking actually uses. Kept as a float so families don't collapse into integer
    /// ties (which would let the raw-volume tie-break re-dominate the order).
    /// `shared_lines` is just this rounded, for display. `0.0` until computed.
    pub shared_weight: f64,
    /// The duplicated sites, largest first.
    pub locations: Vec<Loc>,
    /// Mean value-graph size across members (low → computation-poor type/data def).
    pub mean_sem: f64,
    /// Where the duplication lives: `"prod"`, `"test"` (all sites in test code), or
    /// `"mixed"` (logic duplicated *across* the test boundary — ranked normally
    /// because it's a real leak, unlike intentional test scaffolding).
    pub scope: &'static str,
    /// Refactor-worthiness discount in `(0, 1]` from `refactor_discount` — demotes
    /// all-type-definition and all-generated families. Applied to *both* `value` and
    /// `extractability` so the default ranking honors it too.
    pub discount: f64,
    /// Experimental weak-claim witness for `abstraction` mode. This records a typed
    /// template and caveats for near families that share structure but differ by one
    /// supported literal leaf position. It is not a semantic-equivalence proof.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub abstraction_witness: Option<AbstractionWitness>,
    /// Pack-facing semantic laws that influenced this family-level value fingerprint.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub semantic_laws: Vec<ValueLawProvenance>,
    /// WHY the members merged — the agent-facing equivalence witness (#222): an
    /// exact value-graph proof, a shared heavy sub-DAG, a token-identical
    /// copy-paste run, or structural similarity with its value/shape components.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub witness: Option<crate::EquivalenceWitness>,
    /// WHAT differs between the two representative copies (#223): each varying
    /// spot the extracted helper would parameterize, with absolute line ranges
    /// and truncated text per side. Same provenance as `params` (the first
    /// readable representative pair); empty until the presentation layer reads
    /// source, and for cross-language families.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub varying_spots: Vec<VaryingSpot>,
}

/// One varying spot between a family's two representative copies — the hole an
/// extracted helper would parameterize. Sides may be one-sided (a pure
/// insertion/deletion run has lines in only one copy).
#[derive(Clone, serde::Serialize)]
pub struct VaryingSpot {
    /// 1-based parameter index, matching the family's `params` count.
    pub param: u32,
    /// Absolute `[start, end]` source lines of this spot in the FIRST
    /// representative copy (the family's `locations[0]`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub a_lines: Option<(u32, u32)>,
    /// Absolute `[start, end]` source lines in the second representative copy.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub b_lines: Option<(u32, u32)>,
    /// Trimmed, length-capped text of the spot in the first copy.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub a_text: String,
    /// Trimmed, length-capped text of the spot in the second copy.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub b_text: String,
}

impl RefactorFamily {
    /// How cleanly this family extracts into one shared helper — the default ranking.
    /// Where `value` ranks by raw duplicated *volume* (and so over-rewards a big block
    /// whose copies actually share little), extractability ranks by the lines you'd
    /// truly remove: the *invariant* lines, dampened by the number of parameters the
    /// helper would need. A 400-line dispatcher sharing 9 lines across 30 varying spots
    /// sinks below a 20-line pair sharing 18 lines with one parameter.
    ///
    /// The honest extractable size depends on whether the copies are comparable as text:
    /// - **same-language** (`languages == 1`): `shared_lines` is the truth. If it is 0,
    ///   the family shares no invariant lines — a structural-only match (a language
    ///   idiom like `if err != nil { return err }`, or two unrelated type literals with
    ///   the same shape). There is *nothing to extract*, so the score is 0. This is the
    ///   key correction over volume ranking, which floated these to the top with a
    ///   misleading `sim 1.00`.
    /// - **cross-language** (`languages > 1`): there are no shared *source* lines to
    ///   diff, yet these are real Type-4 clones — fall back to the structural estimate
    ///   `mean_lines × mean_score` so cross-language spread still ranks on its merits.
    ///
    /// Two cleanliness factors scale the result: a **parameter penalty** (each varying
    /// spot widens the helper signature) and, for same-language families, **tightness**
    /// — the fraction of each copy that is invariant (`shared_lines / rep_lines`).
    /// Tightness is what separates `15/15` (extract the whole thing) from `22/384`
    /// (extract a 22-line helper and leave 360 unique lines at every site — barely a
    /// dedup); absolute shared lines alone can't tell them apart.
    pub fn extractability(&self) -> f64 {
        let (extract_lines, tightness) = if self.languages > 1 {
            // cross-language: there are no shared *source* lines to diff, so we can
            // neither weight out idioms nor measure tightness. Require *substance*
            // instead — a tiny cross-language "clone" (a few lines) is almost always a
            // shared idiom (an error check, a one-liner), and we can't verify it line by
            // line — whereas a real cross-language abstraction is a substantial routine.
            // Confidence ramps from 0 at ≤3 lines to 1 at ≥9, standing in for tightness.
            let confidence = ((self.mean_lines as f64 - 3.0) / 6.0).clamp(0.0, 1.0);
            (self.mean_lines as f64 * self.mean_score, confidence)
        } else {
            let rep = self
                .locations
                .first()
                .map_or(self.mean_lines, span_lines)
                .max(1) as f64;
            (self.shared_weight, (self.shared_weight / rep).min(1.0))
        };
        // Each extra parameter makes the helper's signature wider and the call sites
        // noisier; 1–2 is clean, a dozen means the "shared" code is mostly scaffolding.
        let param_penalty = 1.0 / (1.0 + 0.5 * self.params as f64);
        extract_lines
            * effective_copies(self.members)
            * spread(self.files, self.modules, self.languages)
            * param_penalty
            * tightness
            * self.discount
            * self.default_surface_weight()
    }

    /// Product placement for the default scan/review/debug surfaces. This is a
    /// presentation/ranking decision, not detector semantics: exact fragments remain
    /// present in `--top 0` JSON even when their default ranking is dampened.
    pub fn recommended_surface(&self) -> &'static str {
        let fragment_sites = self.locations.iter().filter(|l| l.is_fragment).count();
        let all_generated = self.locations.iter().all(is_generated_loc);
        let high_fanout = self.members >= 8;
        if all_generated
            || self.mean_lines <= 1
            || (fragment_sites > 0 && high_fanout && self.mean_lines <= 3)
        {
            return "hidden";
        }
        if fragment_sites == 0 {
            return "default";
        }
        // Mixed whole-unit + fragment families stay default: the enclosing unit is already a
        // product-sized candidate, and the fragment locations serve as supporting evidence.
        // The tiny/high-fanout proof-only cases above are deliberately not promoted by
        // this escape hatch.
        if fragment_sites < self.locations.len() {
            return "default";
        }

        let all_test = self.locations.iter().all(is_test_loc);
        let all_have_enclosing = self.locations.iter().all(|l| l.enclosing_unit.is_some());
        let has_effect_or_body = self.locations.iter().any(|l| {
            matches!(
                l.fragment_kind,
                Some(
                    crate::FragmentKind::LoopEffect
                        | crate::FragmentKind::IndexAssignEffect
                        | crate::FragmentKind::SelfFieldAssign
                        | crate::FragmentKind::SelfFieldBody
                )
            )
        });
        let has_guard_or_exit = self.locations.iter().any(|l| {
            matches!(
                l.fragment_kind,
                Some(
                    crate::FragmentKind::ConditionalGuard
                        | crate::FragmentKind::DirectReturn
                        | crate::FragmentKind::DirectThrow
                )
            )
        });
        let tiny_test_scaffold = all_test
            && all_have_enclosing
            && (self.mean_lines <= 3 || (has_effect_or_body && self.mean_lines <= 4));

        if tiny_test_scaffold {
            // The fragment-quality audit found these are usually correct but too often
            // test arrange/assert or fixture-constructor substrate to be review items.
            "hidden"
        } else if high_fanout && self.mean_lines <= 3 {
            "hidden"
        } else if has_effect_or_body {
            // Receiver/effect-bearing fragments are usually synchronization hazards first.
            // Promote only substantial, cross-file production fragments to default; keep
            // test/generated-looking and tiny forms in review/hidden.
            if !all_test && self.mean_lines >= 12 && self.files >= 2 && self.modules >= 2 {
                "default"
            } else if self.mean_lines <= 3 && all_have_enclosing {
                "hidden"
            } else {
                "review"
            }
        } else if has_guard_or_exit {
            if self.mean_lines >= 12 && self.files >= 2 && !all_test {
                "default"
            } else if self.mean_lines <= 3 {
                "hidden"
            } else {
                "review"
            }
        } else if self.mean_lines <= 8 {
            "review"
        } else {
            "default"
        }
    }

    fn default_surface_weight(&self) -> f64 {
        match self.recommended_surface() {
            "default" => 1.0,
            "review" => 0.35,
            "hidden" => 0.05,
            "debug" => 0.02,
            _ => 1.0,
        }
    }

    /// Divergent-edit **hazard**: how likely this family is to be edited inconsistently
    /// (one copy changed, the siblings missed) and cause a bug. A *severity* axis,
    /// orthogonal to `extractability` (which is about *fixability*). This is the formula
    /// calibrated against mined ground truth — a leave-one-repo-out evaluation over a
    /// 12-repo / 8-language corpus of real divergent edits (G1) and bug-linked ones (G2);
    /// see `eval/hazard/RESULTS.md` and `docs/hazard-ranking.md`.
    ///
    /// The data overturned the intuitive design: semantic-fingerprint *size* (`mean_sem`)
    /// is anti-predictive (typical divergences are in smaller families); source-**line**
    /// span is the real magnitude signal. The terms, with their learned weight signs:
    ///   - `mean_lines` (+) — edit surface; the more lines, the more chances to diverge.
    ///   - `spread` (+) — cross-directory dispersion; far-apart copies are missed more.
    ///   - `invisibility` (+) — `1 − tightness`: copies that share little *text* despite
    ///     being a matched (semantically equivalent) family are the ones a developer can't
    ///     see, so won't update. This is the **inverse** of extractability's `tightness`
    ///     term. Identical copies still carry some hazard, so it floors at 0.3 rather
    ///     than 0.
    ///   - `scope_weight` — a divergence in prod is costlier than in tests.
    ///
    /// (`mean_sem`, `members`, and `params` were tested and dropped — anti-predictive,
    /// redundant, or sign-unstable noise. No `discount` term: the calibration corpus had
    /// negligible vendored/generated code, so it is omitted to keep the score faithful to
    /// the measured formula.)
    pub fn hazard(&self) -> f64 {
        // tightness = invariant fraction; 0 for cross-language (no shared source lines),
        // which makes invisibility maximal — exactly where the data says hazard is highest.
        let tightness = (self.shared_weight / (self.mean_lines.max(1) as f64)).clamp(0.0, 1.0);
        let invisibility = 0.3 + 0.7 * (1.0 - tightness);
        let scope_weight = match self.scope {
            "test" => 0.25,
            "mixed" => 0.5,
            _ => 1.0, // prod
        };
        self.mean_lines as f64
            * spread(self.files, self.modules, self.languages)
            * invisibility
            * scope_weight
    }
}

/// The directory ("module") a file lives in — the design-level grouping key.
fn module_of(file: &str) -> &str {
    Path::new(file)
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or("")
}

fn span_lines(l: &Loc) -> u32 {
    l.span_lines
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
    mean_lines as f64 * effective_copies(members) * mean_score * spread(files, modules, languages)
}

/// Copies, dampened. Removable code grows with the number of copies, but with
/// DIMINISHING returns: the first few dedups capture the design win, whereas a
/// fragment repeated across hundreds of sites is almost always an idiom / generated
/// / boilerplate pattern (a Javadoc nav block, test scaffolding), not an extractable
/// abstraction. So the copy count is linear up to a small knee, then
/// square-root-dampened — fanout no longer rewards the ranking *linearly* (a 400-copy
/// family is ~20× a 2-copy one, not 400×). The reported `dup_lines` stays the honest
/// `mean_lines × (members−1)`; only the ranking scores are dampened.
fn effective_copies(members: usize) -> f64 {
    let copies = members.saturating_sub(1) as f64;
    const KNEE: f64 = 6.0;
    if copies <= KNEE {
        copies
    } else {
        KNEE + (copies - KNEE).sqrt()
    }
}

/// Design-spread multiplier: cross-module duplication is a missing abstraction;
/// cross-language is a notable (if harder) design signal.
fn spread(files: usize, modules: usize, languages: usize) -> f64 {
    1.0 + 0.30 * (files.min(8) as f64 - 1.0).max(0.0)
        + 0.50 * (modules.min(6) as f64 - 1.0).max(0.0)
        + 0.50 * (languages as f64 - 1.0).max(0.0)
}

/// Is this site test code, by the usual path / unit-name conventions? Conservative:
/// only well-known markers, so production code is never misclassified as a test.
/// Public so presentation layers can scope-guard per-location advice (e.g. never
/// recommend calling a test helper from production copies).
pub fn is_test_loc(l: &Loc) -> bool {
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
    path_test || name_test || l.in_test_module
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
        ".d.ts", // TS ambient declarations: not extractable refactor targets (often generated)
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

/// The distinct keys of `locs` under `key`, sorted. Collect-then-`sort_unstable`+`dedup` is
/// the family-stat idiom for counting distinct files / modules / languages.
fn distinct_by<'a>(locs: &'a [Loc], key: impl Fn(&'a Loc) -> &'a str) -> Vec<&'a str> {
    let mut v: Vec<&'a str> = locs.iter().map(key).collect();
    v.sort_unstable();
    v.dedup();
    v
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

    let files = distinct_by(&locs, |l| l.file.as_str());
    let modules = distinct_by(&locs, |l| module_of(&l.file));
    let langs = distinct_by(&locs, |l| l.lang.as_str());

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

    let discount = refactor_discount(all_class, mean_sem, all_generated);
    let value = refactor_value(
        mean_lines,
        members,
        group.score,
        files.len(),
        modules.len(),
        langs.len(),
    ) * discount;
    RefactorFamily {
        value,
        members,
        files: files.len(),
        modules: modules.len(),
        languages: langs.len(),
        mean_score: group.score,
        mean_lines,
        dup_lines,
        // Filled in at the presentation layer (needs source access).
        shared_lines: 0,
        params: 0,
        shared_weight: 0.0,
        locations: locs,
        mean_sem,
        scope,
        discount,
        abstraction_witness: group.abstraction_witness.clone(),
        witness: group.witness.clone(),
        varying_spots: Vec::new(),
        semantic_laws: group
            .semantic_laws
            .iter()
            .filter_map(|&law| value_law_provenance(law))
            .collect(),
    }
}

/// Rank a detection report's groups as refactoring opportunities, highest value
/// first. Trivial families (a single pair of tiny fragments) sink to the bottom.
pub fn rank_families(report: &Report) -> Vec<RefactorFamily> {
    let mut fams: Vec<RefactorFamily> = report
        .groups
        .iter()
        .map(family_of)
        // Drop families living entirely in generated / vendored / ambient-declaration
        // files (`vendor/`, `.min.`, `*.d.ts`, `// Generated`-style paths): you don't
        // refactor code a tool regenerates. A *partly*-generated family is kept — that's
        // a real leak of hand-written logic into generated output.
        .filter(|f| !f.locations.iter().all(is_generated_loc))
        // A raw group can collapse to one reported site when all of its matches are
        // overlapping windows in the same file. That is not a clone family, and it must
        // not subsume a real multi-site family before the CLI's min-member gate drops it.
        .filter(|f| f.members >= 2)
        .collect();
    // Dedup overlapping families by total span, LARGEST FIRST, so the most complete
    // family of a region is the one kept and the sub-blocks inside it are dropped. (A
    // value/extractability order would keep whichever scored highest — often a sub-block
    // — leaving its enclosing family *also* in the list: the same OAuth test-server or
    // design-verifier function reported as several overlapping entries. The caller
    // re-sorts the survivors by the chosen key, so this order only governs dedup.)
    // Sort LARGEST span first (then value), with a min-location final tie-break so this is a TOTAL
    // order: families that tie on span AND value still sort deterministically by source position,
    // not by upstream group-iteration order. Without it, two equal-span/value families could be
    // deduped in either direction depending on map-iteration order, making the kept set (and so the
    // dup-gate count) sensitive to incidental ordering. (The gate is otherwise fully deterministic:
    // FxHash and IEEE `+−×÷`/`sqrt` are platform-independent, so CI and local agree exactly.)
    fams.sort_by(|a, b| {
        total_span(b)
            .cmp(&total_span(a))
            .then(b.value.total_cmp(&a.value))
            .then_with(|| family_min_loc(a).cmp(&family_min_loc(b)))
    });
    // Keep a family unless an already-kept (larger) one subsumes it. `subsumes(k, f)`
    // requires `k` to have a site in the file of *every* `f` site — so any subsumer must
    // touch `f`'s first file. Index kept families by the files they touch and test only
    // those candidates, instead of scanning all kept families (the dedup was O(families²)
    // — ~0.13s on guava's ~6k families). Same result: every possible subsumer is in the
    // candidate set, and `subsumes` returns false for the rest.
    let mut kept: Vec<RefactorFamily> = Vec::with_capacity(fams.len());
    let mut by_file: rustc_hash::FxHashMap<String, Vec<usize>> = rustc_hash::FxHashMap::default();
    for f in fams {
        let first_file = f.locations.first().map(|l| l.file.as_str()).unwrap_or("");
        let subsumed = by_file
            .get(first_file)
            .is_some_and(|idxs| idxs.iter().any(|&ki| subsumes(&kept[ki], &f)));
        if !subsumed {
            let ki = kept.len();
            for l in &f.locations {
                by_file.entry(l.file.clone()).or_default().push(ki);
            }
            kept.push(f);
        }
    }
    kept
}

/// Total source lines a family spans across all its sites — its "size" for dedup.
fn total_span(f: &RefactorFamily) -> u32 {
    f.locations.iter().map(span_lines).sum()
}

/// A family's lexicographically smallest site `(file, start_line, end_line)` — a stable identity
/// used only as the final dedup-sort tie-break, so equal-span/value families order deterministically
/// by source position rather than by incidental group-iteration order.
fn family_min_loc(f: &RefactorFamily) -> Option<(&str, u32, u32)> {
    f.locations
        .iter()
        .map(|l| (l.file.as_str(), l.start_line, l.end_line))
        .min()
}

/// Does family `outer` subsume `inner` — i.e. every `inner` site lands on (mostly
/// inside) some `outer` site in the same file? Then `inner` reports the same regions
/// already covered. This catches two cases the field eval flagged as double-counting:
/// strict containment (a shared loop body reported alongside the enclosing functions),
/// and **window-shifted overlap** — the contiguous channel finding the same run at a
/// few different start lines, surfacing as several near-identical families. Requiring
/// the bulk (≥60%) of each inner site to fall in an outer site collapses both without
/// merging genuinely distinct code (which would need >60% line overlap to qualify).
fn subsumes(outer: &RefactorFamily, inner: &RefactorFamily) -> bool {
    // No site-count guard: a single large outer site can cover several smaller inner
    // sites, so requiring `outer.len() >= inner.len()` wrongly kept those (double-counted)
    // inner families. Coverage alone — every inner site ≥60% inside some same-file outer
    // site — is the criterion. The caller only ever asks whether a larger-span (kept)
    // family subsumes a smaller one, so this can't collapse genuinely distinct code.
    const COVER: f64 = 0.60;
    inner.locations.iter().all(|i| {
        outer
            .locations
            .iter()
            .any(|o| o.file == i.file && overlap_frac(o, i) >= COVER)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EnclosingUnit, LineSpan, LocInit, Metrics, Report};
    use nose_il::UnitKind::{Class, Function};

    fn loc(file: &str, s: u32, e: u32, lang: &str) -> Loc {
        Loc::new(LocInit {
            file: file.into(),
            source_span: LineSpan::new(s, e),
            lang: lang.into(),
            kind: Function,
            name: None,
            sem: 50,
            span_tokens: 50,
        })
    }
    /// A site with explicit kind / value-graph size / name (for discount tests).
    fn loc_k(file: &str, s: u32, e: u32, kind: nose_il::UnitKind, sem: usize) -> Loc {
        Loc::new(LocInit {
            file: file.into(),
            source_span: LineSpan::new(s, e),
            lang: "rust".into(),
            kind,
            name: None,
            sem,
            span_tokens: sem,
        })
    }

    fn fragment_loc(file: &str, s: u32, e: u32) -> Loc {
        fragment_loc_k(file, s, e, crate::FragmentKind::ConditionalGuard)
    }

    fn fragment_loc_k(file: &str, s: u32, e: u32, fragment_kind: crate::FragmentKind) -> Loc {
        Loc {
            is_fragment: true,
            fragment_kind: Some(fragment_kind),
            reason_code: Some(fragment_kind.reason_code()),
            ..Loc::new(LocInit {
                file: file.into(),
                source_span: LineSpan::new(s, e),
                lang: "rust".into(),
                kind: nose_il::UnitKind::Block,
                name: None,
                sem: 50,
                span_tokens: 50,
            })
        }
    }

    fn test_fragment_loc_k(file: &str, s: u32, e: u32, fragment_kind: crate::FragmentKind) -> Loc {
        let mut loc = fragment_loc_k(file, s, e, fragment_kind);
        loc.enclosing_unit = Some(EnclosingUnit {
            file: file.into(),
            start_line: s.saturating_sub(5).max(1),
            end_line: e + 5,
            kind: Function,
            name: Some("test_scaffold".into()),
            unit_key: format!(
                "{file}:Function:{}-{}:test_scaffold",
                s.saturating_sub(5).max(1),
                e + 5
            ),
        });
        loc
    }

    /// A family with the given locations and metrics, other fields at neutral values.
    fn fam(
        value: f64,
        mean_lines: u32,
        shared: u32,
        params: u32,
        locs: Vec<Loc>,
    ) -> RefactorFamily {
        RefactorFamily {
            value,
            members: locs.len(),
            files: locs
                .iter()
                .map(|l| &l.file)
                .collect::<std::collections::HashSet<_>>()
                .len(),
            modules: 1,
            languages: locs
                .iter()
                .map(|l| &l.lang)
                .collect::<std::collections::HashSet<_>>()
                .len()
                .max(1),
            mean_score: 1.0,
            mean_lines,
            dup_lines: mean_lines,
            shared_lines: shared,
            params,
            shared_weight: shared as f64,
            locations: locs,
            mean_sem: 50.0,
            scope: "prod",
            discount: 1.0,
            abstraction_witness: None,
            witness: None,
            varying_spots: Vec::new(),
            semantic_laws: Vec::new(),
        }
    }

    #[test]
    fn extractability_ranks_tight_over_bloated() {
        // A: a big block whose copies share little (a dispatch skeleton over divergent
        // bodies) — high raw `value`, few shared lines, many params. B: a small tight
        // pair. Extractability must rank B above A even though A's `value` is larger.
        let bloated = fam(
            5000.0,
            200,
            9,
            22,
            vec![loc("a.rs", 1, 200, "rs"), loc("b.rs", 1, 200, "rs")],
        );
        let tight = fam(
            200.0,
            22,
            18,
            1,
            vec![loc("c.rs", 1, 22, "rs"), loc("d.rs", 1, 22, "rs")],
        );
        assert!(
            bloated.value > tight.value,
            "old ranking favors the bloated block"
        );
        assert!(
            tight.extractability() > bloated.extractability(),
            "extractability favors the cleanly-extractable pair"
        );
    }

    #[test]
    fn hazard_inverts_extractability_on_text_similarity() {
        // The defining contract: same size and copy count, differing only in how much
        // text the copies share. `tight` is near-identical; `divergent` is the same
        // behavior with little shared text (an invisible sibling). Hazard must rank the
        // divergent one higher (it's the dangerous one), and extractability the tight one
        // — the two axes are *opposed* on the text-similarity dimension.
        let tight = fam(
            0.0,
            30,
            27,
            0,
            vec![loc("a.rs", 1, 30, "rs"), loc("b.rs", 1, 30, "rs")],
        );
        let divergent = fam(
            0.0,
            30,
            3,
            0,
            vec![loc("c.rs", 1, 30, "rs"), loc("d.rs", 1, 30, "rs")],
        );
        assert!(
            divergent.hazard() > tight.hazard(),
            "hazard ranks the syntactically-divergent (invisible) family higher"
        );
        assert!(
            tight.extractability() > divergent.extractability(),
            "extractability ranks the tight family higher — the axes are opposed"
        );
    }

    #[test]
    fn hazard_surfaces_cross_language() {
        // Cross-language families have no shared source lines (shared_weight 0), so
        // invisibility maxes out — the sibling is truly invisible. Hazard surfaces them,
        // where extractability can barely rank them.
        let xlang = fam(
            0.0,
            30,
            0,
            0,
            vec![loc("a.py", 1, 30, "py"), loc("b.ts", 1, 30, "ts")],
        );
        let tight_same = fam(
            0.0,
            30,
            27,
            0,
            vec![loc("a.rs", 1, 30, "rs"), loc("b.rs", 1, 30, "rs")],
        );
        assert!(
            xlang.hazard() > tight_same.hazard(),
            "an invisible cross-language sibling outranks a tight same-language pair"
        );
    }

    #[test]
    fn hazard_demotes_test_scope() {
        let prod = fam(
            0.0,
            30,
            3,
            0,
            vec![loc("a.rs", 1, 30, "rs"), loc("b.rs", 1, 30, "rs")],
        );
        let mut test = fam(
            0.0,
            30,
            3,
            0,
            vec![loc("a.rs", 1, 30, "rs"), loc("b.rs", 1, 30, "rs")],
        );
        test.scope = "test";
        assert!(
            prod.hazard() > test.hazard(),
            "a divergence in prod outranks the same divergence in tests"
        );
    }

    #[test]
    fn extractability_falls_back_without_shared() {
        // Cross-language families have no shared *source* lines (shared_lines = 0); the
        // fallback keeps them ranked on structural similarity × volume, not at zero.
        let xlang = fam(
            0.0,
            40,
            0,
            0,
            vec![
                loc("a.py", 1, 40, "python"),
                loc("a.ts", 1, 40, "typescript"),
            ],
        );
        assert!(xlang.extractability() > 0.0);
    }

    #[test]
    fn subsumes_collapses_window_shift() {
        // A block family whose sites are a few-line-shifted window of a larger family's
        // sites (the contiguous channel finding the same run at different starts) is
        // subsumed — the field-eval double-counting case.
        let outer = fam(
            10.0,
            7,
            7,
            0,
            vec![loc("a.rs", 11, 17, "rs"), loc("b.rs", 10, 14, "rs")],
        );
        let inner = fam(
            10.0,
            10,
            10,
            0,
            vec![loc("a.rs", 8, 17, "rs"), loc("b.rs", 7, 14, "rs")],
        );
        assert!(subsumes(&outer, &inner), "≥60%-overlapping sites collapse");
        let distinct = fam(
            10.0,
            11,
            11,
            0,
            vec![loc("a.rs", 40, 50, "rs"), loc("b.rs", 40, 50, "rs")],
        );
        assert!(
            !subsumes(&outer, &distinct),
            "non-overlapping family is kept"
        );
    }

    #[test]
    fn subsumes_when_one_outer_site_covers_several_inner_sites() {
        // A larger family with FEWER but bigger sites can still cover an inner family's
        // MORE numerous smaller sites — every inner site lands inside an outer one, so it
        // is double-counting and must be subsumed. (A site-count early-out used to reject
        // this, leaving both families in the report.)
        let outer = fam(
            10.0,
            100,
            100,
            0,
            vec![loc("a.rs", 1, 100, "rs"), loc("b.rs", 1, 100, "rs")],
        );
        let inner = fam(
            10.0,
            30,
            30,
            0,
            vec![
                loc("a.rs", 10, 40, "rs"),
                loc("a.rs", 60, 90, "rs"),
                loc("b.rs", 20, 50, "rs"),
            ],
        );
        assert!(
            subsumes(&outer, &inner),
            "one big outer site may cover several inner sites"
        );
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
            semantic_laws: Vec::new(),
            abstraction_witness: None,
            witness: None,
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
            semantic_laws: Vec::new(),
            abstraction_witness: None,
            witness: None,
        };
        let inner = Group {
            score: 1.0,
            members: vec![loc("a.rs", 15, 25, "rust"), loc("b.rs", 15, 25, "rust")],
            semantic_laws: Vec::new(),
            abstraction_witness: None,
            witness: None,
        };
        let fams = rank_families(&report(vec![inner, outer]));
        assert_eq!(fams.len(), 1, "the contained family should be dropped");
        assert_eq!(
            fams[0].mean_lines, 31,
            "the surviving family is the outer one"
        );
    }

    #[test]
    fn single_site_family_does_not_subsume_reportable_family() {
        // A contiguous-channel group can be one long same-file window after
        // `family_of` coalesces overlapping matches. It is not itself reportable, so
        // it must not hide the exact semantic method family it covers.
        let syntax_window = Group {
            score: 1.0,
            members: vec![loc("Param.java", 1589, 1611, "java")],
            semantic_laws: Vec::new(),
            abstraction_witness: None,
            witness: None,
        };
        let semantic_methods = Group {
            score: 1.0,
            members: vec![
                loc("Param.java", 1589, 1593, "java"),
                loc("Param.java", 1595, 1599, "java"),
                loc("Param.java", 1601, 1605, "java"),
                loc("Param.java", 1607, 1611, "java"),
            ],
            semantic_laws: Vec::new(),
            abstraction_witness: None,
            witness: None,
        };

        let fams = rank_families(&report(vec![syntax_window, semantic_methods]));

        assert_eq!(fams.len(), 1);
        assert_eq!(
            fams[0].members, 4,
            "the reportable semantic family should survive"
        );
        assert!(fams[0]
            .locations
            .iter()
            .any(|loc| loc.start_line <= 1592 && loc.end_line >= 1592));
    }

    #[test]
    fn dedup_order_is_independent_of_group_input_order() {
        // Two families that tie on span AND value but live in different files. The dedup sort's
        // min-location tie-break must order them deterministically by source position, NOT by
        // group-iteration order — otherwise the kept set (and so the dup-gate count) would be
        // sensitive to incidental ordering.
        let mk = |f1: &'static str, f2: &'static str| Group {
            score: 1.0,
            members: vec![loc(f1, 1, 20, "rust"), loc(f2, 1, 20, "rust")],
            semantic_laws: Vec::new(),
            abstraction_witness: None,
            witness: None,
        };
        let keys = |groups| {
            rank_families(&report(groups))
                .iter()
                .map(|f| family_min_loc(f).map(|(file, s, e)| (file.to_string(), s, e)))
                .collect::<Vec<_>>()
        };
        let forward = keys(vec![mk("a.rs", "a2.rs"), mk("b.rs", "b2.rs")]);
        let reversed = keys(vec![mk("b.rs", "b2.rs"), mk("a.rs", "a2.rs")]);
        assert_eq!(
            forward, reversed,
            "dedup result must not depend on group input order"
        );
        assert_eq!(
            forward[0].as_ref().unwrap().0,
            "a.rs",
            "tied families order by source position",
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
            semantic_laws: Vec::new(),
            abstraction_witness: None,
            witness: None,
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
            semantic_laws: Vec::new(),
            abstraction_witness: None,
            witness: None,
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
            semantic_laws: Vec::new(),
            abstraction_witness: None,
            witness: None,
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
            semantic_laws: Vec::new(),
            abstraction_witness: None,
            witness: None,
        };
        let small = Group {
            score: 1.0,
            members: vec![loc("p/a.rs", 1, 6, "rust"), loc("p/b.rs", 1, 6, "rust")],
            semantic_laws: Vec::new(),
            abstraction_witness: None,
            witness: None,
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
            semantic_laws: Vec::new(),
            abstraction_witness: None,
            witness: None,
        };
        let cross = Group {
            score: 0.9,
            members: vec![
                loc("a.py", 1, 10, "python"),
                loc("b.ts", 1, 10, "typescript"),
            ],
            semantic_laws: Vec::new(),
            abstraction_witness: None,
            witness: None,
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
            semantic_laws: Vec::new(),
            abstraction_witness: None,
            witness: None,
        };
        let test = Group {
            score: 1.0,
            members: vec![
                loc("tests/a.rs", 1, 30, "rust"),
                loc("tests/b.rs", 1, 30, "rust"),
            ],
            semantic_laws: Vec::new(),
            abstraction_witness: None,
            witness: None,
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
            semantic_laws: Vec::new(),
            abstraction_witness: None,
            witness: None,
        };
        let pure = Group {
            score: 1.0,
            members: vec![
                loc("src/a.rs", 1, 30, "rust"),
                loc("src/b.rs", 1, 30, "rust"),
            ],
            semantic_laws: Vec::new(),
            abstraction_witness: None,
            witness: None,
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
    fn pack_facing_laws_become_family_provenance_only_when_registered() {
        let proven = Group {
            score: 1.0,
            members: vec![
                loc("src/a.py", 1, 30, "python"),
                loc("src/b.py", 1, 30, "python"),
            ],
            semantic_laws: vec![nose_semantics::ValueLaw::NumericFactorDistribution],
            abstraction_witness: None,
            witness: None,
        };
        let family = family_of(&proven);
        assert_eq!(family.semantic_laws.len(), 1);
        assert_eq!(
            family.semantic_laws[0].pack_id,
            nose_semantics::FIRST_PARTY_VALUE_LAW_PACK_ID
        );
        assert_eq!(
            family.semantic_laws[0].law_id,
            "value-graph.factor-distribute.numeric-common-factor"
        );
        assert_eq!(
            family.semantic_laws[0].proof_obligation_id,
            "normalize.value_graph.factor_distribute"
        );

        let internal_only = Group {
            score: 1.0,
            members: vec![
                loc("src/c.py", 1, 30, "python"),
                loc("src/d.py", 1, 30, "python"),
            ],
            semantic_laws: vec![nose_semantics::ValueLaw::AddCommutativity],
            abstraction_witness: None,
            witness: None,
        };
        assert!(
            family_of(&internal_only).semantic_laws.is_empty(),
            "historical internal value-law gates must not be over-reported as pack-facing laws"
        );
    }

    #[test]
    fn tiny_all_fragment_family_is_hidden_and_downranked() {
        let whole = fam(
            10.0,
            2,
            2,
            0,
            vec![loc("src/a.rs", 1, 2, "rust"), loc("src/b.rs", 1, 2, "rust")],
        );
        let fragment = fam(
            10.0,
            2,
            2,
            0,
            vec![
                fragment_loc("src/a.rs", 3, 4),
                fragment_loc("src/b.rs", 3, 4),
            ],
        );

        assert_eq!(whole.recommended_surface(), "default");
        assert_eq!(fragment.recommended_surface(), "hidden");
        assert!(
            fragment.extractability() < whole.extractability(),
            "tiny exact fragments remain present but should not outrank whole-unit candidates"
        );
    }

    #[test]
    fn tiny_mixed_fragment_family_is_hidden() {
        let mixed = fam(
            10.0,
            1,
            0,
            0,
            vec![
                loc("src/a.rs", 1, 3, "rust"),
                fragment_loc_k("src/b.rs", 9, 9, crate::FragmentKind::ExprEffect),
                fragment_loc_k("src/c.rs", 12, 12, crate::FragmentKind::ExprEffect),
            ],
        );

        assert_eq!(
            mixed.recommended_surface(),
            "hidden",
            "one whole-unit site must not promote a one-line proof fragment family"
        );
    }

    #[test]
    fn fragment_surface_uses_kind_scope_and_spread() {
        let review_effect = fam(
            10.0,
            6,
            6,
            0,
            vec![
                fragment_loc_k("src/a.rs", 1, 6, crate::FragmentKind::LoopEffect),
                fragment_loc_k("src/b.rs", 1, 6, crate::FragmentKind::LoopEffect),
            ],
        );
        assert_eq!(
            review_effect.recommended_surface(),
            "review",
            "medium effect fragments are synchronization hazards before default refactors"
        );

        let tiny_test_expr = fam(
            10.0,
            3,
            3,
            0,
            vec![
                test_fragment_loc_k("tests/a.py", 1, 3, crate::FragmentKind::ExprEffect),
                test_fragment_loc_k("tests/b.py", 1, 3, crate::FragmentKind::ExprEffect),
            ],
        );
        assert_eq!(
            tiny_test_expr.recommended_surface(),
            "hidden",
            "tiny test-only expression-effect scaffolding stays diagnostic-only"
        );

        let tiny_test_self_field = fam(
            10.0,
            4,
            3,
            1,
            vec![
                test_fragment_loc_k("tests/A.java", 10, 13, crate::FragmentKind::SelfFieldBody),
                test_fragment_loc_k("tests/B.java", 20, 23, crate::FragmentKind::SelfFieldBody),
            ],
        );
        assert_eq!(
            tiny_test_self_field.recommended_surface(),
            "hidden",
            "small test fixture constructor bodies stay out of review output"
        );

        let medium_test_expr = fam(
            10.0,
            8,
            7,
            0,
            vec![
                test_fragment_loc_k("tests/a.py", 10, 17, crate::FragmentKind::ExprEffect),
                test_fragment_loc_k("tests/b.py", 20, 27, crate::FragmentKind::ExprEffect),
            ],
        );
        assert_eq!(
            medium_test_expr.recommended_surface(),
            "review",
            "larger test setup fragments can still be useful review context"
        );

        let default_guard = fam(
            10.0,
            14,
            14,
            0,
            vec![
                fragment_loc("src/a.rs", 1, 14),
                fragment_loc("src/b.rs", 1, 14),
            ],
        );
        assert_eq!(
            default_guard.recommended_surface(),
            "default",
            "substantial cross-file guard fragments can be default candidates"
        );

        let generated = fam(
            10.0,
            20,
            20,
            0,
            vec![
                fragment_loc("target/generated/a.rs", 1, 20),
                fragment_loc("target/generated/b.rs", 1, 20),
            ],
        );
        assert_eq!(
            generated.recommended_surface(),
            "hidden",
            "generated-looking exact fragments stay out of default output"
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
            semantic_laws: Vec::new(),
            abstraction_witness: None,
            witness: None,
        };
        // A behavior-rich class of the same size is a genuine candidate.
        let rich = Group {
            score: 1.0,
            members: vec![
                loc_k("src/c.rs", 1, 30, Class, 80),
                loc_k("src/d.rs", 1, 30, Class, 80),
            ],
            semantic_laws: Vec::new(),
            abstraction_witness: None,
            witness: None,
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
            semantic_laws: Vec::new(),
            abstraction_witness: None,
            witness: None,
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
            semantic_laws: Vec::new(),
            abstraction_witness: None,
            witness: None,
        };
        let owned = Group {
            score: 1.0,
            members: vec![loc("src/x.go", 1, 30, "go"), loc("src/y.go", 1, 30, "go")],
            semantic_laws: Vec::new(),
            abstraction_witness: None,
            witness: None,
        };
        assert!(
            family_of(&owned).value > family_of(&vendored).value,
            "vendored duplication is discounted below owned-code duplication"
        );
    }
}
