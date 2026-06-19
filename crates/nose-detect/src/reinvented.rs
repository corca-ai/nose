use crate::{report, units::UnitFeat};
use nose_il::UnitKind;
use serde::Serialize;
use std::collections::HashMap;

/// One reinvented-helper containment finding: `container` computes, as an interior
/// sub-DAG, exactly the whole body of the pure single-return `helper` — WITHOUT calling
/// it. The actionable fix is the inverse of extract-method: replace the matched lines
/// with a call to the existing helper.
///
/// The claim is exact-grade and one-sided: both units pass the strict exact gate, and
/// an equal value-graph node hash is the same hash-consed canonical-structure guarantee
/// the exact channel rides — the container's sub-computation and the helper's body are
/// the same computation, not merely similar. Units that *call* a helper with this
/// behavior (directly, or via a behaviorally-equal twin) are excluded — calling is the
/// fix, not the smell.
#[derive(Serialize, Clone)]
pub struct ReinventedHelper {
    pub helper_file: String,
    pub helper_name: Option<String>,
    pub helper_start_line: u32,
    pub helper_end_line: u32,
    /// The helper is defined in test code. A production container must not be told to
    /// call it directly; rehome/extract a production helper first.
    pub helper_in_test: bool,
    pub container_file: String,
    pub container_name: Option<String>,
    pub container_start_line: u32,
    pub container_end_line: u32,
    /// Source lines INSIDE the container where the helper's computation lives.
    pub site_start_line: u32,
    pub site_end_line: u32,
    /// The site is APPROXIMATE — the matched computation is a synthesized loop fold with
    /// no precise source span, so the site is the whole container range and includes
    /// computation beyond the helper's. Consumers must not mechanically replace these
    /// exact lines (coevo series 6, S3-1).
    pub site_approximate: bool,
    /// The container is a TEST file. Such findings are judgment-deep (§2b): the
    /// "reinvention" is often intentional — a test asserts the helper's expected value as
    /// a literal (calling it would be circular), or duplicates fixture setup. Kept in
    /// `--show reinvented` / JSON, excluded from the bare-default surface; a field audit
    /// (2026-06-13) measured them as the dominant non-actionable class.
    pub container_in_test: bool,
    /// Value-graph size of the reinvented computation — the ranking key.
    pub weight: u32,
}

/// Minimum value-graph size for a helper to participate in containment matching —
/// below this, "reinventing" it is idiom-sized noise (`x + 1` helpers), not a finding.
const REINVENTED_HELPER_MIN_NODES: usize = 8;

/// Minimum SOURCE size (pre-normalization tokens) for a containment helper. Value-graph
/// weight alone cannot tell a compressed loop (a whole accumulator loop canonicalizes to
/// a ~4-node `Reduce` — semantically rich) from a one-line delegation idiom
/// (`self._print(expr.args[0])` — w7 but trivial to re-type); the helper's source size
/// is the honest "is calling it actually better than writing it" proxy. Measured on
/// sympy: the delegation-idiom noise band sits at ≤12 tokens, real helpers at ≥25.
const REINVENTED_HELPER_MIN_TOKENS: usize = 20;

/// Compute the reinvented-helper containment findings over the extracted units: join
/// each eligible helper's single return-sink hash against every other unit's sub-DAG
/// anchors (which carry interior source spans). Deterministic: helpers bucket by hash
/// with a path/line-ordered representative, and the output is sorted by weight then
/// location.
pub fn reinvented_helpers(units: &[UnitFeat]) -> Vec<ReinventedHelper> {
    let mut by_hash: HashMap<u64, Vec<usize>> = HashMap::new();
    for (i, u) in units.iter().enumerate() {
        if matches!(u.kind, UnitKind::Function | UnitKind::Method)
            && u.fragment_kind.is_none()
            && u.exact_safe
            && u.pure_single_return
            && !u.used_length_contract
            && u.returns.len() == 1
            && u.value.len() >= REINVENTED_HELPER_MIN_NODES
            && u.token_count >= REINVENTED_HELPER_MIN_TOKENS
        {
            by_hash.entry(u.returns[0]).or_default().push(i);
        }
    }
    if by_hash.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<ReinventedHelper> = Vec::new();
    for (ci, c) in units.iter().enumerate() {
        if !matches!(c.kind, UnitKind::Function | UnitKind::Method)
            || c.fragment_kind.is_some()
            || !c.exact_safe
        {
            continue;
        }
        for anchor in &c.anchors {
            let Some(helpers) = by_hash.get(&anchor.hash) else {
                continue;
            };
            // The container already obtains this computation BY CALLING a helper.
            if c.called_helper_returns.binary_search(&anchor.hash).is_ok() {
                continue;
            }
            // The container must also carry the helper's loop guards (same iteration
            // scheme) — matching a fold while iterating differently is not containment.
            let guards_present = |h: &UnitFeat| {
                h.cond_sinks
                    .iter()
                    .all(|g| c.value.binary_search(g).is_ok())
            };
            // Deterministic representative among behaviorally-equal helpers; the
            // container must be strictly bigger than the helper (an equal-size match
            // is the exact channel's whole-unit clone, not a containment).
            let rep = helpers
                .iter()
                .copied()
                .filter(|&h| {
                    h != ci && c.value.len() > units[h].value.len() && guards_present(&units[h])
                })
                .min_by_key(|&h| (&units[h].path, units[h].start_line));
            let Some(h) = rep else { continue };
            let h = &units[h];
            // A matched anchor with a REAL (non-zero) source span must lie inside the
            // container's own line range. A span outside it means the shared computation
            // textually lives in a DIFFERENT function that this unit merely inlined
            // (generalized inlining splices a callee's value graph in) — the unit is a
            // transitive CALLER, not a reinventor, and `called_helper_returns` (one call
            // level deep) does not catch a two-hop chain. Reject (coevo series 6, S3-2).
            // A synthesized loop-fold anchor carries no span (line 0); it falls back to
            // the container range and is the faithful flagship case, so it is allowed.
            let real_span = anchor.line_start != 0;
            let inside = anchor.line_start >= c.start_line && anchor.line_end <= c.end_line;
            if real_span && !inside {
                continue;
            }
            // Loop-exit nodes (a synthesized `Reduce`) may carry no source span; fall
            // back to the container's own range rather than reporting line 0. When this
            // fallback fires the site is the WHOLE container (approximate — the matched
            // computation is a sub-part), flagged for honest consumer reporting.
            let (site_start, site_end, site_approximate) = if anchor.line_start == 0 {
                (c.start_line, c.end_line, true)
            } else {
                (anchor.line_start, anchor.line_end, false)
            };
            out.push(ReinventedHelper {
                helper_file: h.path.clone(),
                helper_name: h.name.clone(),
                helper_start_line: h.start_line,
                helper_end_line: h.end_line,
                helper_in_test: report::is_test_path(&h.path),
                container_file: c.path.clone(),
                container_name: c.name.clone(),
                container_start_line: c.start_line,
                container_end_line: c.end_line,
                site_start_line: site_start,
                site_end_line: site_end,
                site_approximate,
                container_in_test: report::is_test_path(&c.path),
                weight: anchor.weight,
            });
        }
    }
    out.sort_by(|a, b| {
        b.weight
            .cmp(&a.weight)
            .then_with(|| {
                (&a.container_file, a.container_start_line)
                    .cmp(&(&b.container_file, b.container_start_line))
            })
            .then_with(|| {
                (&a.helper_file, a.helper_start_line).cmp(&(&b.helper_file, b.helper_start_line))
            })
    });
    out
}
