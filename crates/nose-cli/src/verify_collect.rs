use crate::legacy_prelude::*;
use crate::verify_admission::{exact_admission_rejection, ExactAdmissionRejectionDiagnostic};

/// One record per interpretable unit.
pub(super) struct VerifyRec {
    pub(super) fp: Vec<u64>,
    pub(super) beh: Vec<nose_normalize::Behavior>,
    pub(super) file: String,
    pub(super) start: u32,
    pub(super) end: u32,
    pub(super) tokens: usize,
    pub(super) loc: String,
    /// Can the exact `semantic` channel ever claim this unit (strict-exact-safe
    /// and above the degenerate-fingerprint floor)? Scopes the HARD gate.
    pub(super) claimable: bool,
    /// Diagnostics-only reason for a unit that cannot enter the exact semantic
    /// claim surface. This does not participate in the product admission gate.
    pub(super) admission_rejection: Option<ExactAdmissionRejectionDiagnostic>,
    /// Hash of the unit's declared parameter domains. The oracle binds battery
    /// rows under declared-type coercion, so two units are battery-COMPARABLE
    /// only when their declarations agree; a disagreement across different
    /// declarations is an advisory lead, not a hard violation.
    pub(super) domain_sig: u64,
    /// Index into `corpus.files` and the CORE-IL root, so `--falsify` can re-normalize the
    /// file (deterministically) and re-interpret this unit on search-generated inputs (#317).
    pub(super) file_idx: usize,
    pub(super) core_root: nose_il::NodeId,
}

#[derive(Clone, Copy)]
pub(super) enum VerifyExclusionReason {
    CoreMissing,
    BatteryBail,
    EmptyFingerprint,
    Uninterpretable,
    /// #244 fail-closed: the unit forked on more symbolic If/ternary sites than
    /// the per-execution exploration cap allows.
    PathBail,
}

impl VerifyExclusionReason {
    pub(super) fn label(self) -> &'static str {
        match self {
            VerifyExclusionReason::CoreMissing => "core-missing",
            VerifyExclusionReason::BatteryBail => "battery-bail",
            VerifyExclusionReason::EmptyFingerprint => "empty-fingerprint",
            VerifyExclusionReason::Uninterpretable => "uninterpretable",
            VerifyExclusionReason::PathBail => "path-bail",
        }
    }
}

pub(super) struct VerifyExcludedUnit {
    pub(super) reason: VerifyExclusionReason,
    pub(super) file: String,
    pub(super) start: u32,
    pub(super) end: u32,
    pub(super) tokens: usize,
}

#[derive(Default)]
pub(super) struct VerifyExclusions {
    pub(super) core_missing: usize,
    pub(super) battery_bail: usize,
    pub(super) empty_fingerprint: usize,
    pub(super) uninterpretable: usize,
    pub(super) path_bail: usize,
    pub(super) units: Vec<VerifyExcludedUnit>,
}

impl VerifyExclusions {
    fn record(
        &mut self,
        reason: VerifyExclusionReason,
        file: &str,
        span: nose_il::Span,
        tokens: usize,
    ) {
        match reason {
            VerifyExclusionReason::CoreMissing => self.core_missing += 1,
            VerifyExclusionReason::BatteryBail => self.battery_bail += 1,
            VerifyExclusionReason::EmptyFingerprint => self.empty_fingerprint += 1,
            VerifyExclusionReason::Uninterpretable => self.uninterpretable += 1,
            VerifyExclusionReason::PathBail => self.path_bail += 1,
        }
        self.units.push(VerifyExcludedUnit {
            reason,
            file: file.to_string(),
            start: span.start_line,
            end: span.end_line,
            tokens,
        });
    }

    fn append(&mut self, other: VerifyExclusions) {
        self.core_missing += other.core_missing;
        self.battery_bail += other.battery_bail;
        self.empty_fingerprint += other.empty_fingerprint;
        self.uninterpretable += other.uninterpretable;
        self.path_bail += other.path_bail;
        self.units.extend(other.units);
    }

    pub(super) fn total(&self) -> usize {
        self.core_missing
            + self.battery_bail
            + self.empty_fingerprint
            + self.uninterpretable
            + self.path_bail
    }
}

/// The oracle's interpretation pass: every interpretable unit's record, plus the
/// CANON PRESERVATION tallies — a stricter, pair-free soundness check: does the full
/// normalization pipeline preserve each unit's behavior vs the pre-canon core IL? A
/// mismatch is a behavior-changing canon bug, even if no corpus twin collides with it.
pub(super) struct VerifyOracle {
    pub(super) recs: Vec<VerifyRec>,
    pub(super) total: usize,
    pub(super) canon_checked: usize,
    pub(super) canon_violations: Vec<String>,
    /// Per-unit census records (outcome + construct tags), populated only when
    /// the `--exclusion-census` instrument is requested.
    pub(super) census: Vec<verify_census::CensusUnit>,
    census_enabled: bool,
    pub(super) exclusions: VerifyExclusions,
}

pub(super) fn collect_verify_recs(
    corpus: &Corpus,
    opts: &nose_normalize::NormalizeOptions,
    battery: &[Vec<nose_normalize::Value>],
    census: bool,
) -> VerifyOracle {
    let oracle_opts = nose_normalize::NormalizeOptions {
        oracle: true,
        ..*opts
    };
    let per_file: Vec<_> = corpus
        .files
        .par_iter()
        .enumerate()
        .map(|(file_idx, il)| {
            let n = nose_normalize::normalize(il, &corpus.interner, opts);
            // The behavioral ground truth comes from the pre-canonicalization core IL (so a
            // behavior-changing canon can't mask itself), matched to each fully-normalized
            // unit by source span.
            let core = nose_normalize::normalize(il, &corpus.interner, &oracle_opts);
            let mut oracle = VerifyOracle {
                recs: Vec::new(),
                total: 0,
                canon_checked: 0,
                canon_violations: Vec::new(),
                census: Vec::new(),
                census_enabled: census,
                exclusions: VerifyExclusions::default(),
            };
            let func_count = n
                .units
                .iter()
                .filter(|u| n.kind(u.root) == nose_il::NodeKind::Func)
                .count();
            let value_context = (func_count > 1)
                .then(|| nose_normalize::ValueFingerprintContext::new(&n, &corpus.interner));
            let exact_safe_roots: Vec<_> = n
                .units
                .iter()
                .filter_map(|unit| {
                    let root = unit.root;
                    (n.kind(root) == nose_il::NodeKind::Func
                        && !verify_battery_over_budget(subtree_node_count(&n, root), battery.len()))
                    .then_some(root)
                })
                .collect();
            let exact_safe_by_span =
                nose_detect::exact_safe_roots_by_span(&n, &corpus.interner, &exact_safe_roots);
            collect_file_verify_recs(
                &n,
                &core,
                value_context.as_ref(),
                &corpus.interner,
                battery,
                &mut oracle,
                &exact_safe_by_span,
                file_idx,
            );
            oracle
        })
        .collect();

    let mut oracle = VerifyOracle {
        recs: Vec::new(),
        total: 0,
        canon_checked: 0,
        canon_violations: Vec::new(),
        census: Vec::new(),
        census_enabled: census,
        exclusions: VerifyExclusions::default(),
    };
    for mut file_oracle in per_file {
        oracle.total += file_oracle.total;
        oracle.canon_checked += file_oracle.canon_checked;
        oracle.recs.append(&mut file_oracle.recs);
        oracle.census.append(&mut file_oracle.census);
        oracle
            .canon_violations
            .append(&mut file_oracle.canon_violations);
        if oracle.canon_violations.len() > 20 {
            oracle.canon_violations.truncate(20);
        }
        oracle.exclusions.append(file_oracle.exclusions);
    }
    oracle
}

/// Record one unit's oracle outcome in the exclusion census (no-op unless the
/// `--exclusion-census` instrument is on). `tag_il`/`tag_root` name the subtree
/// the oracle would have interpreted (the core IL when span-matched, else the
/// fully-normalized unit).
fn push_verify_census(
    oracle: &mut VerifyOracle,
    loc: String,
    tag_il: &nose_il::Il,
    tag_root: nose_il::NodeId,
    fp: &[u64],
    reason: &'static str,
) {
    if !oracle.census_enabled {
        return;
    }
    oracle.census.push(verify_census::CensusUnit {
        loc,
        reason,
        fp: fp.to_vec(),
        tags: verify_census::census_tags(tag_il, tag_root),
    });
}

/// Did a canon pass change a unit's behavior? True iff some battery row's full-IL behavior
/// is not equivalent to the core-IL behavior. Equivalence (`behavior_equiv`) treats two
/// ABORTING runs (both `ret == Err`) as equal regardless of the effects recorded before the
/// abort: an erroring execution has no observable result (the input is out of the unit's
/// domain), and reordering operations before a guaranteed trap is behavior-preserving.
/// Without this, impossible inputs (an int bound to an array param of a multi-array-param C
/// routine like `fe25519_add`, #369) manufacture spurious violations. `Ok→Err`, `Err→Ok`,
/// and differing successful results still trip (the `ret`s differ, or both are non-`Err` and
/// compared in full).
fn canon_changed_behavior(
    core: &[nose_normalize::Behavior],
    full: &[nose_normalize::Behavior],
) -> bool {
    core.len() != full.len()
        || core
            .iter()
            .zip(full)
            .any(|(c, f)| !nose_normalize::behavior_equiv(c, f))
}

#[allow(clippy::too_many_arguments)]
fn collect_file_verify_recs(
    n: &nose_il::Il,
    core: &nose_il::Il,
    value_context: Option<&nose_normalize::ValueFingerprintContext>,
    interner: &Interner,
    battery: &[Vec<nose_normalize::Value>],
    oracle: &mut VerifyOracle,
    exact_safe_by_span: &std::collections::HashMap<(u32, u32), bool>,
    file_idx: usize,
) {
    let file_path = &n.meta.path;
    let core_func = func_span_index(core);
    for u in &n.units {
        let root = u.root;
        if n.kind(root) != nose_il::NodeKind::Func {
            continue;
        }
        oracle.total += 1;
        let loc = format!("{}:{}", file_path, n.node(root).span.start_line);
        // The same function in the core IL (by span) — interpret THAT, not `n`.
        let span0 = n.node(root).span;
        let tokens = subtree_node_count(n, root);
        let Some(&core_root) = core_func.get(&(span0.start_byte, span0.end_byte)) else {
            push_verify_census(oracle, loc, n, root, &[], "no-core-span");
            oracle
                .exclusions
                .record(VerifyExclusionReason::CoreMissing, file_path, span0, tokens);
            continue;
        };
        if verify_battery_over_budget(tokens, battery.len()) {
            oracle
                .exclusions
                .record(VerifyExclusionReason::BatteryBail, file_path, span0, tokens);
            push_verify_census(oracle, loc, core, core_root, &[], "battery-bail");
            continue;
        }
        // Soundness is about merges on the VALUE fingerprint. A unit whose value
        // graph is EMPTY (`fn resumed() {}`, or a body the graph captures nothing of)
        // has no value fingerprint to merge on — the detector keys candidates on
        // structure there, never on an empty value multiset — so distinct empty-fp
        // bodies "colliding" is not a product false merge. Exclude empty fingerprints
        // (only those — small non-empty ones stay, so completeness is unaffected).
        // Fingerprint AND pointer-length contracts from ONE value-graph build (the
        // oracle needs both; building twice doubled the per-unit cost). The contract
        // binds n = len(array) so the oracle interprets `f(xs,n)` under the same
        // convention the value graph used to merge it; gated on the contract actually
        // firing, so a non-contract false merge is still exposed by the free battery.
        let (fp, contracts) = match value_context {
            Some(context) => nose_normalize::value_fingerprint_and_contracts_with_context(
                n, root, interner, context,
            ),
            None => nose_normalize::value_fingerprint_and_contracts(n, root, interner),
        };
        if fp.is_empty() {
            push_verify_census(oracle, loc, n, root, &[], "empty-fp");
            oracle.exclusions.record(
                VerifyExclusionReason::EmptyFingerprint,
                file_path,
                span0,
                tokens,
            );
            continue;
        }
        // Run the battery; the unit is interpretable only if every input runs.
        let mut path_cap = false;
        let Some(beh) = run_battery(
            core,
            interner,
            core_root,
            battery,
            &contracts,
            &mut path_cap,
        ) else {
            let (census_reason, reason) = if path_cap {
                ("path-bail", VerifyExclusionReason::PathBail)
            } else {
                ("battery-bail", VerifyExclusionReason::Uninterpretable)
            };
            push_verify_census(oracle, loc, core, core_root, &fp, census_reason);
            oracle.exclusions.record(reason, file_path, span0, tokens);
            continue;
        };
        push_verify_census(oracle, loc, core, core_root, &fp, "interpretable");
        // Stricter canon check: the SAME function interpreted on the fully-normalized
        // IL must agree with the core IL on every input — else a canon pass changed
        // behavior. (Only when the full IL is itself fully interpretable on the battery.)
        // Canon preservation is judged on CONCRETE behaviors only: symbolic identity
        // is keyed on syntax, and canonicalization legitimately rewrites syntax, so a
        // Sym-bearing mismatch here is expected, not a behavior change.
        let mut full_path_cap = false;
        if let Some(full_beh) =
            run_battery(n, interner, root, battery, &contracts, &mut full_path_cap)
        {
            // Path-explored behaviors always carry the Sym assume markers, so the
            // concrete-only filter below also keeps canon preservation away from
            // path alignment questions (canonicalization may merge or split the
            // very branches exploration forks on).
            let concrete = !beh.iter().any(nose_normalize::behavior_has_sym)
                && !full_beh.iter().any(nose_normalize::behavior_has_sym);
            if concrete {
                oracle.canon_checked += 1;
                if canon_changed_behavior(&beh, &full_beh) && oracle.canon_violations.len() < 20 {
                    let s = n.node(root).span;
                    oracle
                        .canon_violations
                        .push(format!("{}:{}", file_path, s.start_line));
                }
            }
        }
        let span = n.node(root).span;
        let exact_safe = exact_safe_by_span
            .get(&(span.start_line, span.end_line))
            .copied()
            .unwrap_or(true);
        let claimable = nose_detect::exact_claim_eligible_parts(exact_safe, fp.len());
        let admission_rejection =
            exact_admission_rejection(n, interner, root, exact_safe, fp.len());
        oracle.recs.push(VerifyRec {
            fp,
            beh,
            file: file_path.to_string(),
            start: span.start_line,
            end: span.end_line,
            tokens,
            loc: format!("{}:{}", file_path, span.start_line),
            claimable,
            admission_rejection,
            domain_sig: param_domain_signature(n, root),
            file_idx,
            core_root,
        });
    }
}

/// Stable hash of a unit's declared parameter domains (position-sensitive).
/// Units whose declarations differ are interpreted under different battery
/// coercions and are not behavior-comparable row-for-row.
fn param_domain_signature(il: &nose_il::Il, root: nose_il::NodeId) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for &k in il.children(root) {
        if il.kind(k) == nose_il::NodeKind::Param {
            match nose_semantics::domain_evidence_for_param(il, k) {
                Some(d) => d.hash(&mut h),
                None => 0xD07Fu16.hash(&mut h),
            }
        }
    }
    h.finish()
}

/// Subtree node count — the same size signal the detector gates on, so the
/// value-add evaluator can restrict its gold to meaningful-size units.
fn subtree_node_count(il: &nose_il::Il, root: nose_il::NodeId) -> usize {
    let mut tokens = 0usize;
    let mut stack = vec![root];
    while let Some(x) = stack.pop() {
        tokens += 1;
        stack.extend(il.children(x).iter().copied());
    }
    tokens
}
