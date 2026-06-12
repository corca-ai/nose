use super::*;

/// A heavy sub-DAG anchor: a shared sub-computation's structural `hash`, its `weight` (sub-DAG
/// size), and the source line range (`line_start..=line_end`) of the IL subtree that produced it —
/// so a partial / sub-DAG clone can report WHERE the shared computation lives in each unit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Anchor {
    pub hash: u64,
    pub weight: u32,
    pub line_start: u32,
    pub line_end: u32,
}

/// A unit's heavy sub-DAG anchors, sorted/deduped by hash.
pub type Anchors = Vec<Anchor>;

/// One value-graph build's fingerprints: `(value, literal, return)` hash multisets plus the
/// heavy sub-DAG [`Anchors`].
pub type FingerprintBundle = (Vec<u64>, Vec<u64>, Vec<u64>, Anchors);
/// [`FingerprintBundle`] plus value-law provenance and the unit's containment sink
/// profile — the final pair is `(pure single return, cond guard hashes)`: whether the
/// build's sinks are exactly one `Return` plus iteration `Cond` guards (no effects,
/// throws, or breaks), and the sorted guard-value hashes of those `Cond` sinks. The
/// reinvented-helper containment channel keys on both.
pub type FingerprintLawBundle = (
    Vec<u64>,
    Vec<u64>,
    Vec<u64>,
    Anchors,
    Vec<ValueLaw>,
    (bool, Vec<u64>),
);

/// Public entry: the value-graph fingerprint of the unit rooted at `root`
/// (sorted multiset of `u64` value hashes). Equivalent computations → equal
/// multisets.
pub fn value_fingerprint(il: &Il, root: NodeId, interner: &Interner) -> Vec<u64> {
    value_fingerprint_lits(il, root, interner).0
}

/// Like [`value_fingerprint`], but also returns (1) the sorted multiset of literal
/// (`Const`) value hashes — for "data-table" detection — and (2) the sorted multiset
/// of RETURN-sink value hashes — what the unit actually computes/returns, for a
/// return-signature match (true clones return the same values).
pub fn value_fingerprint_lits(
    il: &Il,
    root: NodeId,
    interner: &Interner,
) -> (Vec<u64>, Vec<u64>, Vec<u64>) {
    let mut b = Builder::new(il, interner);
    b.build_unit(root);
    b.fingerprint_lits()
}

/// The default minimum sub-computation size (in value-nodes) for a node to be an extractable
/// anchor. Below this a shared sub-DAG is a common idiom (`x+1`, `len(xs)`), not a refactor.
/// The #248 sweep (experiments §BW) measured the §BJ 8–20 band: floor 8 gains real
/// worthy-recall (+0.9pp held-out, flat P@10, default-surface families CONSOLIDATE on
/// corpus repos) but floods the near-only gate surface with small families (nose's own
/// dup-gate 24 → 73) — recall-positive, burden-heavy. The default stays 20; recall-first
/// consumers can set `NOSE_ANCHOR_MIN_WEIGHT=8`.
pub const ANCHOR_MIN_WEIGHT: u32 = 20;

/// The effective anchor weight floor: `ANCHOR_MIN_WEIGHT` unless the research
/// knob `NOSE_ANCHOR_MIN_WEIGHT` overrides it (#248 — the §BJ 8–20 band sweep).
/// A research surface like `NOSE_ANCHOR_SCORE*`, not a product setting.
pub fn anchor_min_weight() -> u32 {
    static V: OnceLock<u32> = OnceLock::new();
    *V.get_or_init(|| {
        std::env::var("NOSE_ANCHOR_MIN_WEIGHT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(ANCHOR_MIN_WEIGHT)
    })
}

/// Heavy sub-DAG anchor hashes of a unit — see `Builder::anchors`. Two units sharing a (rare)
/// anchor share an extractable sub-computation: a partial / sub-DAG clone.
pub fn value_anchors(il: &Il, root: NodeId, interner: &Interner) -> Anchors {
    let mut b = Builder::new(il, interner);
    b.build_unit(root);
    b.anchors(anchor_min_weight())
}

/// `value_fingerprint_lits` plus the unit's heavy sub-DAG anchors, all from ONE value-graph
/// build (anchors share the build, so adding them is free vs. fingerprinting alone).
pub fn value_fingerprint_lits_anchors(
    il: &Il,
    root: NodeId,
    interner: &Interner,
) -> FingerprintBundle {
    let (v, l, r, a, _, _) = value_fingerprint_lits_anchors_laws(il, root, interner);
    (v, l, r, a)
}

/// `value_fingerprint_lits_anchors` plus pack-facing value-law provenance for laws that
/// actually rewrote or bridged the unit's value graph.
pub fn value_fingerprint_lits_anchors_laws(
    il: &Il,
    root: NodeId,
    interner: &Interner,
) -> FingerprintLawBundle {
    let mut b = Builder::new(il, interner);
    b.build_unit(root);
    finish_fingerprint_law_bundle(b)
}

/// Context-shared variant of [`value_fingerprint_lits_anchors`].
pub fn value_fingerprint_lits_anchors_with_context(
    il: &Il,
    root: NodeId,
    interner: &Interner,
    context: &ValueFingerprintContext,
) -> FingerprintBundle {
    let (v, l, r, a, _, _) =
        value_fingerprint_lits_anchors_laws_with_context(il, root, interner, context);
    (v, l, r, a)
}

/// Context-shared variant of [`value_fingerprint_lits_anchors_laws`].
pub fn value_fingerprint_lits_anchors_laws_with_context(
    il: &Il,
    root: NodeId,
    interner: &Interner,
    context: &ValueFingerprintContext,
) -> FingerprintLawBundle {
    let mut b = Builder::new(il, interner).with_context(context);
    b.build_unit_with_context(root, Some(context));
    finish_fingerprint_law_bundle(b)
}

/// The default minimum sub-DAG weight for a CONTAINMENT anchor — the granularity the
/// reinvented-helper channel matches helper bodies at. Much smaller than the
/// near-channel [`ANCHOR_MIN_WEIGHT`]: loop canonicalization COMPRESSES a whole
/// accumulator loop to a ~4-node `Reduce`, so a meaningful helper body can be tiny in
/// value nodes; the helper side is independently floored on unit size. The near channel
/// re-filters to its own floor at consumption.
pub const CONTAINMENT_ANCHOR_MIN_WEIGHT: u32 = 4;

/// The effective anchor COLLECTION floor: anchors are gathered once per unit at the
/// finer of the two consumer floors (`NOSE_REINVENTED_MIN_WEIGHT` overrides the
/// containment floor for research sweeps).
pub fn containment_anchor_min_weight() -> u32 {
    static V: OnceLock<u32> = OnceLock::new();
    *V.get_or_init(|| {
        std::env::var("NOSE_REINVENTED_MIN_WEIGHT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(CONTAINMENT_ANCHOR_MIN_WEIGHT)
    })
}

fn finish_fingerprint_law_bundle(mut b: Builder<'_>) -> FingerprintLawBundle {
    let (v, l, r) = b.fingerprint_lits();
    let a = b.anchors(containment_anchor_min_weight().min(anchor_min_weight()));
    b.value_laws.sort_unstable();
    b.value_laws.dedup();
    let sink_profile = b.sink_profile();
    (v, l, r, a, b.value_laws, sink_profile)
}

pub fn value_fingerprint_lits_with_context(
    il: &Il,
    root: NodeId,
    interner: &Interner,
    context: &ValueFingerprintContext,
) -> (Vec<u64>, Vec<u64>, Vec<u64>) {
    let mut b = Builder::new(il, interner).with_context(context);
    b.build_unit_with_context(root, Some(context));
    b.fingerprint_lits()
}

/// The pointer-length contracts the unit relied on to converge: deduped, sorted
/// `(array_param_pos, length_param_pos)` pairs. The behavioral oracle binds
/// `args[length_pos] = len(args[array_pos])` for each, so it interprets the unit under the
/// SAME `n = len(array)` convention the value graph used to merge it. Empty when none.
pub fn value_fingerprint_contracts(il: &Il, root: NodeId, interner: &Interner) -> Vec<(u32, u32)> {
    value_fingerprint_and_contracts(il, root, interner).1
}

/// Both the value fingerprint AND the pointer-length contracts from a SINGLE build — the
/// behavioral oracle needs both per unit, and building the value graph twice (once for each)
/// doubled the per-unit cost.
pub fn value_fingerprint_and_contracts(
    il: &Il,
    root: NodeId,
    interner: &Interner,
) -> (Vec<u64>, Vec<(u32, u32)>) {
    let mut b = Builder::new(il, interner);
    b.build_unit(root);
    finish_fingerprint_contracts(b)
}

/// Context-shared variant of [`value_fingerprint_and_contracts`].
pub fn value_fingerprint_and_contracts_with_context(
    il: &Il,
    root: NodeId,
    interner: &Interner,
    context: &ValueFingerprintContext,
) -> (Vec<u64>, Vec<(u32, u32)>) {
    let mut b = Builder::new(il, interner).with_context(context);
    b.build_unit_with_context(root, Some(context));
    finish_fingerprint_contracts(b)
}

fn finish_fingerprint_contracts(mut b: Builder<'_>) -> (Vec<u64>, Vec<(u32, u32)>) {
    let fp = b.fingerprint_lits().0;
    b.contracts.sort_unstable();
    b.contracts.dedup();
    (fp, b.contracts)
}
