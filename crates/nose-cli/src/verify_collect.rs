use crate::oracle_gate::{
    func_span_index, run_battery, run_battery_diagnostic, verify_battery_over_budget,
};
use crate::verify_admission::{
    exact_admission_rejection_with_context, runtime_boundary_rejection_diagnostic_with_context,
    AdmissionContext, ExactAdmissionRejectionDiagnostic,
};
use crate::verify_census;
use nose_il::{Corpus, Interner, Lang};
use rayon::prelude::*;

mod census;
use census::{census_outcome, push_verify_census, synthetic_blocker, CensusLocation};
mod support;
use support::{
    admission_rejection_for_rec, oracle_exclusion_diagnostic, param_domain_signature,
    subtree_node_count, unit_value_fingerprint_and_contracts,
};

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
    /// and above the degenerate-fingerprint floor, after the product's default
    /// extraction gates)? Scopes the HARD gate.
    pub(super) claimable: bool,
    /// Product extraction admission, recorded separately from exact eligibility so
    /// offline reports can explain why a safe, rich fingerprint is still out of scope.
    pub(super) product_admission: &'static str,
    /// Whether this unit participated in the concrete core-vs-canonical behavior check.
    pub(super) canon_exposed: bool,
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
    pub(super) diagnostic: Option<ExactAdmissionRejectionDiagnostic>,
}

#[derive(Clone, Copy)]
struct RuntimeDiagnosticSource<'a> {
    il: &'a nose_il::Il,
    root: Option<nose_il::NodeId>,
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
    fn record_core_missing(&mut self, file: &str, span: nose_il::Span, tokens: usize) {
        self.record(VerifyExclusionReason::CoreMissing, file, span, tokens, None);
    }

    fn record_battery_bail(&mut self, file: &str, span: nose_il::Span, tokens: usize) {
        self.record(VerifyExclusionReason::BatteryBail, file, span, tokens, None);
    }

    fn record_empty_fingerprint(&mut self, file: &str, span: nose_il::Span, tokens: usize) {
        self.record(
            VerifyExclusionReason::EmptyFingerprint,
            file,
            span,
            tokens,
            None,
        );
    }

    fn record(
        &mut self,
        reason: VerifyExclusionReason,
        file: &str,
        span: nose_il::Span,
        tokens: usize,
        diagnostic: Option<ExactAdmissionRejectionDiagnostic>,
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
            diagnostic,
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
    let admission_context = AdmissionContext::from_corpus(corpus);
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
                        && (census
                            || !verify_battery_over_budget(
                                subtree_node_count(&n, root),
                                battery.len(),
                            )))
                    .then_some(root)
                })
                .collect();
            let exact_safe_by_span =
                nose_detect::exact_safe_roots_by_span(&n, &corpus.interner, &exact_safe_roots);
            collect_file_verify_recs(
                il,
                &n,
                &core,
                value_context.as_ref(),
                &corpus.interner,
                battery,
                &mut oracle,
                &exact_safe_by_span,
                file_idx,
                &admission_context,
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

#[allow(clippy::too_many_lines)]
#[allow(clippy::too_many_arguments)]
fn collect_file_verify_recs(
    raw: &nose_il::Il,
    n: &nose_il::Il,
    core: &nose_il::Il,
    value_context: Option<&nose_normalize::ValueFingerprintContext>,
    interner: &Interner,
    battery: &[Vec<nose_normalize::Value>],
    oracle: &mut VerifyOracle,
    exact_safe_by_span: &std::collections::HashMap<(u32, u32), bool>,
    file_idx: usize,
    admission_context: &AdmissionContext,
) {
    let file_path = &n.meta.path;
    let raw_func = func_span_index(raw);
    let core_func = func_span_index(core);
    for u in &n.units {
        let root = u.root;
        if n.kind(root) != nose_il::NodeKind::Func {
            continue;
        }
        oracle.total += 1;
        let root_span = n.node(root).span;
        let census_location = CensusLocation {
            unique: format!(
                "{}:{}-{}@{}-{}",
                file_path,
                root_span.start_line,
                root_span.end_line,
                root_span.start_byte,
                root_span.end_byte
            ),
            verify: format!("{}:{}", file_path, root_span.start_line),
        };
        // The same function in the core IL (by span) — interpret THAT, not `n`.
        let span0 = n.node(root).span;
        let tokens = subtree_node_count(n, root);
        let raw_source = RuntimeDiagnosticSource {
            il: raw,
            root: raw_func.get(&(span0.start_byte, span0.end_byte)).copied(),
        };
        // Compute the product fingerprint and exact-claim eligibility before
        // oracle admission so the census can distinguish valuable fail-closed
        // mass from units the exact channel could never claim.
        let (fp, contracts) =
            unit_value_fingerprint_and_contracts(value_context, n, root, interner);
        let exact_safe = exact_safe_by_span
            .get(&(span0.start_byte, span0.end_byte))
            .copied()
            .unwrap_or(true);
        let product_admission = nose_detect::default_product_unit_admission(
            raw,
            n,
            nose_detect::ProductUnitAdmissionInput {
                root,
                kind: u.kind,
                origin: u.origin,
                tokens,
                exact_safe,
                value_len: fp.len(),
            },
        );
        let claimable = product_admission.admitted()
            && nose_detect::exact_claim_eligible_parts(exact_safe, fp.len());
        let Some(&core_root) = core_func.get(&(span0.start_byte, span0.end_byte)) else {
            let blocker = synthetic_blocker("il", "il.core-span", "kind:Func");
            let outcome = census_outcome(
                "no-core-span",
                exact_safe,
                product_admission.label(),
                claimable,
                None,
                Some(blocker),
            );
            push_verify_census(oracle, &census_location, n, root, &fp, outcome);
            oracle
                .exclusions
                .record_core_missing(file_path, span0, tokens);
            continue;
        };
        if verify_battery_over_budget(tokens, battery.len()) {
            oracle
                .exclusions
                .record_battery_bail(file_path, span0, tokens);
            let blocker = synthetic_blocker("budget", "budget.oracle-cost", "kind:Func");
            let outcome = census_outcome(
                "battery-bail",
                exact_safe,
                product_admission.label(),
                claimable,
                None,
                Some(blocker),
            );
            push_verify_census(oracle, &census_location, core, core_root, &fp, outcome);
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
        if fp.is_empty() {
            let blocker = synthetic_blocker("value", "value.empty-fingerprint", "kind:Func");
            let outcome = census_outcome(
                "empty-fp",
                exact_safe,
                product_admission.label(),
                claimable,
                None,
                Some(blocker),
            );
            push_verify_census(oracle, &census_location, n, root, &fp, outcome);
            oracle
                .exclusions
                .record_empty_fingerprint(file_path, span0, tokens);
            continue;
        }
        // Run the battery; the unit is interpretable only if every input runs.
        let beh = match run_battery_diagnostic(core, interner, core_root, battery, &contracts) {
            Ok(behaviors) => behaviors,
            Err(blocker) => {
                let path_cap = blocker.capability_id == "budget.symbolic-branch-sites";
                let (census_reason, reason) = if path_cap {
                    ("path-bail", VerifyExclusionReason::PathBail)
                } else {
                    ("battery-bail", VerifyExclusionReason::Uninterpretable)
                };
                let diagnostic = oracle_exclusion_diagnostic(
                    reason,
                    raw_source,
                    n,
                    interner,
                    root,
                    admission_context,
                );
                let outcome = census_outcome(
                    census_reason,
                    exact_safe,
                    product_admission.label(),
                    claimable,
                    diagnostic.as_ref(),
                    Some(blocker),
                );
                push_verify_census(oracle, &census_location, core, core_root, &fp, outcome);
                oracle
                    .exclusions
                    .record(reason, file_path, span0, tokens, diagnostic);
                continue;
            }
        };
        let outcome = census_outcome(
            "interpretable",
            exact_safe,
            product_admission.label(),
            claimable,
            None,
            None,
        );
        push_verify_census(oracle, &census_location, core, core_root, &fp, outcome);
        // Stricter canon check: the SAME function interpreted on the fully-normalized
        // IL must agree with the core IL on every input — else a canon pass changed
        // behavior. (Only when the full IL is itself fully interpretable on the battery.)
        // Canon preservation is judged on CONCRETE behaviors only: symbolic identity
        // is keyed on syntax, and canonicalization legitimately rewrites syntax, so a
        // Sym-bearing mismatch here is expected, not a behavior change.
        let mut full_path_cap = false;
        let mut canon_exposed = false;
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
                canon_exposed = true;
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
        let admission_rejection = admission_rejection_for_rec(
            n,
            interner,
            root,
            exact_safe,
            fp.len(),
            admission_context,
            raw_source,
        );
        oracle.recs.push(VerifyRec {
            fp,
            beh,
            file: file_path.to_string(),
            start: span.start_line,
            end: span.end_line,
            tokens,
            loc: census_location.verify,
            claimable,
            product_admission: product_admission.label(),
            canon_exposed,
            admission_rejection,
            domain_sig: param_domain_signature(n, root),
            file_idx,
            core_root,
        });
    }
}
