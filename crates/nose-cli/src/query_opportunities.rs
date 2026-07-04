use crate::baseline;
use rustc_hash::{FxHashMap, FxHashSet};
use std::cmp::Ordering;

pub(crate) fn total_dup_lines_refs(fs: &[&nose_detect::RefactorFamily]) -> u32 {
    fs.iter().map(|f| f.dup_lines).sum()
}

/// Overlap grouping (issues #263/#264): families whose members are
/// overlapping slices of the same source regions are one refactoring
/// *opportunity*, not several. The primary (best-ranked) family keeps its
/// numbered entry; its slices fold into a one-line note under it and carry
/// `overlap_primary_id` in JSON. Grouping is presentation policy: every
/// family stays in JSON, baselines, ignores, and `--fail-on` exactly as
/// before.
#[derive(Default)]
pub(crate) struct OpportunityGroups {
    /// Slice family id → its primary's family id.
    pub(crate) primary_of: FxHashMap<String, String>,
    /// Primary family id → slice family ids, in rank order.
    pub(crate) slices_of: FxHashMap<String, Vec<String>>,
}

impl OpportunityGroups {
    /// Group `families` (already in rank order — the first family of a group
    /// is its primary). Two families join when at least two distinct member
    /// pairs overlap on the same file by ≥ half of the shorter span: one
    /// shared region can be coincidence, two parallel shared regions are the
    /// same opportunity sliced (the craken-cli shape — six families, two
    /// insights). Conservative by construction: 2-member families must
    /// overlap on *both* members.
    pub(crate) fn from_ranked(families: &[&nose_detect::RefactorFamily]) -> Self {
        // A file listing implausibly many families would make candidate
        // generation quadratic; skip it rather than risk query speed.
        const PER_FILE_CAP: usize = 200;
        let mut by_file: FxHashMap<&str, FileOpportunityBucket> = FxHashMap::default();
        let mut family_files: Vec<Vec<&str>> = Vec::with_capacity(families.len());
        for (i, f) in families.iter().enumerate() {
            let mut files: Vec<&str> = f.locations.iter().map(|l| l.file.as_str()).collect();
            files.sort_unstable();
            files.dedup();
            for &file in &files {
                by_file.entry(file).or_default().families.push(i);
            }
            for loc in &f.locations {
                by_file
                    .entry(loc.file.as_str())
                    .or_default()
                    .intervals
                    .push(MemberInterval {
                        family: i,
                        start: loc.start_line,
                        end: loc.end_line,
                    });
            }
            family_files.push(files);
        }
        let capped_files: FxHashSet<&str> = by_file
            .iter()
            .filter_map(|(&file, bucket)| (bucket.families.len() <= PER_FILE_CAP).then_some(file))
            .collect();
        // Keep the old cap semantics (a pair needs at least one capped shared file), but
        // only run the full greedy overlap check for pairs that have two possible member
        // overlaps anywhere. The greedy pass below remains the behavioral authority.
        let mut candidates: Vec<(usize, usize)> = overlapping_candidate_counts(&by_file)
            .into_iter()
            .filter_map(|(pair @ (i, j), count)| {
                (count >= 2 && share_capped_file(&family_files[i], &family_files[j], &capped_files))
                    .then_some(pair)
            })
            .collect();
        candidates.sort_unstable();
        // Union-find keyed so each set's root is its smallest (best-ranked)
        // index — that root is the opportunity's primary.
        let mut parent: Vec<usize> = (0..families.len()).collect();
        fn find(parent: &mut [usize], mut x: usize) -> usize {
            while parent[x] != x {
                parent[x] = parent[parent[x]];
                x = parent[x];
            }
            x
        }
        for (i, j) in candidates {
            if overlapping_member_pairs(families[i], families[j]) >= 2 {
                let (ri, rj) = (find(&mut parent, i), find(&mut parent, j));
                let (lo, hi) = (ri.min(rj), ri.max(rj));
                parent[hi] = lo;
            }
        }
        let mut groups = Self::default();
        for i in 0..families.len() {
            let root = find(&mut parent, i);
            if root != i {
                let primary = baseline::family_id(families[root]);
                let slice = baseline::family_id(families[i]);
                groups.primary_of.insert(slice.clone(), primary.clone());
                groups.slices_of.entry(primary).or_default().push(slice);
            }
        }
        groups
    }

    pub(crate) fn is_slice(&self, family: &nose_detect::RefactorFamily) -> bool {
        self.primary_of.contains_key(&baseline::family_id(family))
    }

    pub(crate) fn slices(&self, family: &nose_detect::RefactorFamily) -> Option<&[String]> {
        self.slices_of
            .get(&baseline::family_id(family))
            .map(Vec::as_slice)
    }
}

#[derive(Default)]
struct FileOpportunityBucket {
    families: Vec<usize>,
    intervals: Vec<MemberInterval>,
}

#[derive(Clone, Copy)]
struct MemberInterval {
    family: usize,
    start: u32,
    end: u32,
}

fn overlapping_candidate_counts(
    by_file: &FxHashMap<&str, FileOpportunityBucket>,
) -> FxHashMap<(usize, usize), u8> {
    let mut counts: FxHashMap<(usize, usize), u8> = FxHashMap::default();
    for bucket in by_file.values() {
        let mut intervals = bucket.intervals.clone();
        intervals.sort_unstable_by_key(|iv| (iv.start, iv.end, iv.family));
        for left in 0..intervals.len() {
            let a = intervals[left];
            for &b in &intervals[left + 1..] {
                if b.start > a.end {
                    break;
                }
                if a.family == b.family || !intervals_half_overlap(a, b) {
                    continue;
                }
                let key = (a.family.min(b.family), a.family.max(b.family));
                let count = counts.entry(key).or_insert(0);
                if *count < 2 {
                    *count += 1;
                }
            }
        }
    }
    counts
}

fn intervals_half_overlap(a: MemberInterval, b: MemberInterval) -> bool {
    let lo = a.start.max(b.start);
    let hi = a.end.min(b.end);
    if lo > hi {
        return false;
    }
    let overlap = hi - lo + 1;
    let len_a = a.end - a.start + 1;
    let len_b = b.end - b.start + 1;
    overlap * 2 >= len_a.min(len_b)
}

fn share_capped_file(a: &[&str], b: &[&str], capped_files: &FxHashSet<&str>) -> bool {
    let (mut i, mut j) = (0, 0);
    while let (Some(&fa), Some(&fb)) = (a.get(i), b.get(j)) {
        match fa.cmp(fb) {
            Ordering::Less => i += 1,
            Ordering::Greater => j += 1,
            Ordering::Equal => {
                if capped_files.contains(fa) {
                    return true;
                }
                i += 1;
                j += 1;
            }
        }
    }
    false
}

/// Greedy one-to-one count of member pairs that overlap on the same file by
/// at least half of the shorter span.
fn overlapping_member_pairs(
    a: &nose_detect::RefactorFamily,
    b: &nose_detect::RefactorFamily,
) -> usize {
    let mut used = vec![false; b.locations.len()];
    let mut pairs = 0;
    for la in &a.locations {
        for (j, lb) in b.locations.iter().enumerate() {
            if used[j] || la.file != lb.file {
                continue;
            }
            let lo = la.start_line.max(lb.start_line);
            let hi = la.end_line.min(lb.end_line);
            if lo > hi {
                continue;
            }
            let overlap = hi - lo + 1;
            let len_a = la.end_line - la.start_line + 1;
            let len_b = lb.end_line - lb.start_line + 1;
            if overlap * 2 >= len_a.min(len_b) {
                used[j] = true;
                pairs += 1;
                break;
            }
        }
    }
    pairs
}

/// Distinct languages in a family, sorted — e.g. `"python, typescript"`. Empty
/// when the family is single-language (caller decides whether to show anything).
pub(crate) fn family_langs(f: &nose_detect::RefactorFamily) -> String {
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
/// number of directories), never a guess about semantics.
pub(crate) fn family_hint(f: &nose_detect::RefactorFamily) -> String {
    use nose_il::UnitKind;
    // Exactly one member is a whole named function/method while every other
    // member is an inline block or fragment: the family itself proves the
    // inline copies compute what the existing helper computes (issue #263's
    // local-`clamp` case) — the action is "call it", not "extract a second
    // one". Stronger and safer than a fresh extraction, so it wins the hint.
    let named_units: Vec<&nose_detect::Loc> = f
        .locations
        .iter()
        .filter(|l| {
            matches!(l.kind, UnitKind::Function | UnitKind::Method)
                && l.name.is_some()
                && !l.is_fragment
        })
        .collect();
    let inline_copies = f
        .locations
        .iter()
        .filter(|l| l.kind == UnitKind::Block || l.is_fragment)
        .count();
    // Coevo C2 guards: never point production copies at a helper that lives
    // in test code (tests may call prod, not the reverse), and never
    // recommend calling into a generated file (not the maintainer's API).
    if let [helper] = named_units[..] {
        let helper_callable =
            !helper.looks_generated && (f.scope == "test" || !nose_detect::is_test_loc(helper));
        if helper_callable && inline_copies >= 1 && inline_copies == f.locations.len() - 1 {
            let name = helper.name.as_deref().unwrap_or("the helper");
            let sites = if inline_copies == 1 {
                "1 site reimplements".to_string()
            } else {
                format!("{inline_copies} sites reimplement")
            };
            let base = format!(
                "{sites} `{name}` — call the existing helper ({})",
                helper.file
            );
            // Series 2: many varying spots mean the copies diverge from the
            // helper — the early return must not bypass the caution.
            return if f.params >= HIGH_PARAM_SPOTS && f.languages == 1 {
                format!(
                    "{base} — high-parameter ({} varying spots): verify the \
                     copies really match the helper before swapping in calls",
                    f.params
                )
            } else {
                base
            };
        }
    }

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
    // A computation-poor "class" unit is really a type/interface/enum/schema
    // declaration (lowered to a `Class` skeleton); its refactor is "move to one shared
    // type", not "extract a function with parameters".
    let type_decl = all_classes && f.mean_sem < 12.0;
    let extract = if let Some(origin_hint) = origin_extract_hint(f) {
        origin_hint
    } else if type_decl {
        "consolidate into one shared type"
    } else if all_classes {
        "extract a shared base class / mixin"
    } else if all_blocks {
        "extract a method from the repeated block"
    } else {
        "extract a helper"
    };

    let base = match (shared_name, f.modules) {
        (Some(name), _) => format!("consolidate `{name}` — {} copies{cross}", f.members),
        (None, m) if m >= 3 && all_classes => {
            format!("repeated across {m} directories — {extract}{cross}")
        }
        (None, m) if m >= 3 => {
            format!("repeated across {m} directories — extract a shared abstraction{cross}")
        }
        (None, m) if m >= 2 => format!("duplicated across {m} directories — {extract}{cross}"),
        (None, _) => format!("local duplication — {extract}{cross}"),
    };
    // "Extract a method" overclaims when the helper would take many parameters
    // (issue #264 hit 6–16 varying spots): keep the fact-grounded action but
    // flag the readability price instead of asserting a clean extraction.
    if f.params >= HIGH_PARAM_SPOTS && f.languages == 1 {
        return format!(
            "{base} — high-parameter ({} varying spots): divergence readability; \
             a smaller helper for the invariant core may fit better",
            f.params
        );
    }
    // Test-scope duplication is a real smell, but Arrange/Act/Assert setup is often
    // duplicated on purpose — extracting it can hide each scenario's intent (issue
    // #264). Flag that triage caveat without asserting a verdict; the worthy
    // fixture-vs-scaffold call is the reader's (and is not feature-decidable — see the
    // default-surface-noise-audit). `mixed` (a test↔prod leak) is a real extract, no caveat.
    if f.scope == "test" {
        return format!(
            "{base} — test scaffolding: consolidate only a genuinely shared fixture/helper, \
             not per-scenario setup"
        );
    }
    base
}

struct OriginFactSummary {
    all_style: bool,
    all_markup: bool,
    all_preprocessor: bool,
    all_type_contract: bool,
    any_implementation_type: bool,
    all_implementation_type: bool,
    all_imperative: bool,
    all_interface_trait_protocol: bool,
    all_class: bool,
    all_declaration_only: bool,
    all_declarative_denotation: bool,
    any_mixed_body: bool,
    any_implementation_body: bool,
    every_named_copy_same_symbol: bool,
}

impl OriginFactSummary {
    fn from_family(f: &nose_detect::RefactorFamily) -> Option<Self> {
        use nose_il::{UnitBodyKind, UnitDomain, UnitSubkind};

        if f.locations.iter().all(|loc| loc.origin.is_unknown()) {
            return None;
        }

        let all_have_domain = |domain| f.locations.iter().all(|loc| loc.origin.has_domain(domain));
        let any_have_domain = |domain| f.locations.iter().any(|loc| loc.origin.has_domain(domain));
        let all_subkind = |subkind| f.locations.iter().all(|loc| loc.origin.subkind == subkind);
        let all_body = |body_kind| {
            f.locations
                .iter()
                .all(|loc| loc.origin.body_kind == body_kind)
        };
        let any_body = |body_kind| {
            f.locations
                .iter()
                .any(|loc| loc.origin.body_kind == body_kind)
        };
        let mut names = f.locations.iter().filter_map(|loc| loc.name.as_deref());
        let every_named_copy_same_symbol = if let Some(first) = names.next() {
            f.locations.iter().filter(|loc| loc.name.is_some()).count() == f.members
                && names.all(|name| name == first)
        } else {
            false
        };

        Some(Self {
            all_style: all_have_domain(UnitDomain::Style),
            all_markup: all_have_domain(UnitDomain::Markup),
            all_preprocessor: all_have_domain(UnitDomain::Preprocessor),
            all_type_contract: all_have_domain(UnitDomain::TypeContract),
            any_implementation_type: any_have_domain(UnitDomain::ImplementationType),
            all_implementation_type: all_have_domain(UnitDomain::ImplementationType),
            all_imperative: all_have_domain(UnitDomain::Imperative),
            all_interface_trait_protocol: all_subkind(UnitSubkind::InterfaceTraitProtocol),
            all_class: all_subkind(UnitSubkind::Class),
            all_declaration_only: all_body(UnitBodyKind::DeclarationOnly),
            all_declarative_denotation: all_body(UnitBodyKind::DeclarativeDenotation),
            any_mixed_body: any_body(UnitBodyKind::Mixed),
            any_implementation_body: any_body(UnitBodyKind::Implementation),
            every_named_copy_same_symbol,
        })
    }
}

fn origin_extract_hint(f: &nose_detect::RefactorFamily) -> Option<&'static str> {
    let facts = OriginFactSummary::from_family(f)?;

    if facts.all_style {
        return Some(
            "merge selectors or move the declarations to a shared class/token if these elements should be coupled",
        );
    }
    if facts.all_markup {
        return Some("share a component/template only if the data shape matches");
    }
    if facts.all_preprocessor {
        return Some("divergence macro expansion and conditional context before sharing");
    }
    if facts.all_type_contract && !facts.any_implementation_type {
        if facts.all_interface_trait_protocol {
            return Some("consolidate one shared interface/protocol contract");
        }
        return Some("consolidate one shared type/API contract");
    }
    if facts.all_type_contract && facts.any_implementation_type {
        return Some(
            "consolidate the type contract; divergence whether shared behavior should move too",
        );
    }
    if facts.all_implementation_type {
        if facts.all_class && (facts.any_implementation_body || facts.any_mixed_body) {
            return Some("extract a shared base class / mixin");
        }
        return Some("consolidate shared type implementation");
    }
    if facts.all_imperative {
        return Some("extract a helper");
    }
    None
}

pub(crate) fn proposal_action_label(f: &nose_detect::RefactorFamily) -> &'static str {
    use nose_il::UnitKind;

    if let Some(origin_hint) = origin_extract_hint(f) {
        return match origin_hint {
            "extract a helper" => "extract a shared helper",
            other => other,
        };
    }
    let all_classes = f.locations.iter().all(|loc| loc.kind == UnitKind::Class);
    let all_blocks = f.locations.iter().all(|loc| loc.kind == UnitKind::Block);
    let type_decl = all_classes && f.mean_sem < 12.0;
    if type_decl {
        "consolidate into one shared type"
    } else if all_classes {
        "extract a shared base class / mixin"
    } else if all_blocks {
        "extract a method from the repeated block"
    } else {
        "extract a shared helper"
    }
}

pub(crate) fn hint_reasons(f: &nose_detect::RefactorFamily) -> Vec<String> {
    let Some(facts) = OriginFactSummary::from_family(f) else {
        return Vec::new();
    };

    let mut reasons = Vec::new();
    if facts.all_type_contract {
        if facts.all_interface_trait_protocol {
            reasons.push(format!(
                "all copies are {} interface/protocol contracts",
                family_language_label(f)
            ));
        } else {
            reasons.push("all copies are type/API contract regions".to_string());
        }
    } else if facts.all_implementation_type {
        reasons.push("all copies are behavior-bearing type implementation regions".to_string());
    } else if facts.all_style {
        reasons.push("all copies are declarative style rules".to_string());
    } else if facts.all_markup {
        reasons.push("all copies are rendered markup/template regions".to_string());
    } else if facts.all_preprocessor {
        reasons.push("all copies are macro/preprocessor regions".to_string());
    } else if facts.all_imperative {
        reasons.push("all copies are imperative callable regions".to_string());
    }

    if facts.all_declaration_only {
        reasons.push("no implementation body was found".to_string());
    } else if facts.all_declarative_denotation {
        reasons
            .push("the duplicate is a declaration/denotation, not an imperative body".to_string());
    } else if facts.any_mixed_body {
        reasons.push("some copied regions mix declarations with reusable behavior".to_string());
    } else if facts.any_implementation_body {
        reasons.push("an implementation body was found".to_string());
    }

    if facts.every_named_copy_same_symbol {
        reasons.push("every copy has the same symbol name".to_string());
    }
    reasons
}

fn family_language_label(f: &nose_detect::RefactorFamily) -> String {
    let mut langs = f
        .locations
        .iter()
        .map(|loc| loc.lang.as_str())
        .collect::<Vec<_>>();
    langs.sort_unstable();
    langs.dedup();
    if langs.len() == 1 {
        language_label(langs[0]).to_string()
    } else {
        "cross-language".to_string()
    }
}

fn language_label(lang: &str) -> &'static str {
    match lang {
        "css" => "CSS",
        "go" => "Go",
        "html" => "HTML",
        "javascript" => "JavaScript",
        "typescript" => "TypeScript",
        "rust" => "Rust",
        "swift" => "Swift",
        "java" => "Java",
        "python" => "Python",
        "ruby" => "Ruby",
        "c" => "C",
        "vue" => "Vue",
        "svelte" => "Svelte",
        _ => "same-language",
    }
}

/// At this many varying spots an extraction stops being clean (issue #264's
/// triage experience: 6+ spots read as scenario-shaped, not helper-shaped).
const HIGH_PARAM_SPOTS: u32 = 6;
