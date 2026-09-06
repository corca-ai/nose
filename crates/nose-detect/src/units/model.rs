use crate::abstraction;
use crate::fragment::{FragmentKind, ProofFacts};
use nose_il::{Lang, UnitKind, UnitOrigin};
use nose_semantics::ValueLaw;

/// A unit ready for comparison. Self-contained (owns its features and location)
/// so the detector can flatten units from many files into one vector. All feature
/// vectors are content-derived hashes (interner-independent), so a `UnitFeat` is
/// portable across runs — which is what lets the CLI cache it by source-content hash.
#[derive(serde::Deserialize)]
pub struct UnitFeat {
    pub path: String,
    pub lang: Lang,
    pub kind: UnitKind,
    #[serde(default, skip_serializing_if = "UnitOrigin::is_unknown")]
    pub origin: UnitOrigin,
    pub name: Option<String>,
    pub start_line: u32,
    pub end_line: u32,
    pub source_region: Option<nose_il::SourceRegion>,
    /// Shared original bytes; restored from the per-file stream on cache hits.
    #[serde(skip)]
    pub source_document: Option<std::sync::Arc<nose_il::SourceDocument>>,
    pub token_count: usize,
    /// Sorted multiset of local shape hashes (syntactic structure).
    pub shapes: Vec<u64>,
    /// MinHash signature over `shapes`, used by the Type-3 near-duplicate channel.
    pub shape_minhash: Vec<u64>,
    /// Sorted multiset of value-graph (GVN) hashes — the semantic substrate that
    /// is invariant to temporaries, statement order, and common-subexpression
    /// duplication.
    pub value: Vec<u64>,
    /// Coordinate-free replay of occurrence-salted values; never used for detection.
    pub review_value: Option<nose_normalize::ReviewValueFingerprint>,
    /// MinHash signature for candidate generation (over the value graph when
    /// available, else shapes).
    pub minhash: Vec<u64>,
    /// Pre-order node-tag sequence, for alignment scoring.
    pub linear: Vec<u64>,
    /// Compact normalized-IL tree roles used only to prove a source-connected mapped witness
    /// for an existing near-candidate seed. Empty when shape features are disabled or the unit
    /// exceeds the bounded witness representation.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) connected_tokens: Vec<crate::connected::MappedToken>,
    /// Pre-order typed tokens used only by the experimental abstraction witness layer.
    ///
    /// Unlike `linear`, this keeps a value-sensitive literal tag so a pair that differs
    /// only by `0` vs `1` can be explained as one literal hole without weakening the
    /// exact semantic fingerprint.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) abstraction_tokens: Vec<abstraction::WitnessToken>,
    /// Sorted multiset of literal (`Const`) value hashes. A high `lits/value`
    /// ratio marks a "data-table" unit (constant-dominated, e.g. a locale map),
    /// where the constants must match for a clone.
    pub lits: Vec<u64>,
    /// Sorted multiset of RETURN-sink value hashes — what the unit returns. True
    /// clones return the same computed values; used to demote near-identical units
    /// that differ only in their result (`<` vs `<=`, an extra effect).
    pub returns: Vec<u64>,
    /// The unit's value-graph build produced exactly one `Return` sink and nothing
    /// irreversible (no effects, throws, or breaks; loop iteration `Cond` guards are
    /// allowed and listed in [`cond_sinks`](Self::cond_sinks)) — the unit computes ONE
    /// value, so `returns[0]` plus the guards is its entire behavior. The
    /// reinvented-helper containment channel keys on this.
    #[serde(default)]
    pub pure_single_return: bool,
    /// Sorted guard-value hashes of the unit's loop `Cond` sinks. A containment match
    /// against this unit as a helper must find every one of these in the container too
    /// (same iteration scheme), not just the return value.
    #[serde(default)]
    pub cond_sinks: Vec<u64>,
    /// The build relied on a pointer-length contract (a free-param loop bound assumed to
    /// be `len(array)`), which drops the bound from the value hash — so the return hash
    /// does not faithfully determine the value. Makes the unit ineligible as a
    /// containment helper (coevo series 6, S3-3).
    #[serde(default)]
    pub used_length_contract: bool,
    /// Return-sink hashes of every SAME-FILE function this unit provably calls
    /// (`CallTarget::DirectFunction` evidence), sorted+deduped. A containment match on
    /// one of these hashes is the unit *using* a helper, not reinventing it.
    #[serde(default)]
    pub called_helper_returns: Vec<u64>,
    /// The unit's heavy sub-DAG ANCHORS (sub-computations of ≥ `ANCHOR_MIN_WEIGHT` value-nodes),
    /// sorted/deduped by hash. Two units sharing a rare anchor share an extractable common
    /// sub-computation — a partial / sub-DAG clone that whole-unit Jaccard misses. Each carries
    /// its weight (to RANK the shared sub-DAG by size) and the source line range (to SHOW where
    /// the shared computation lives).
    #[serde(default)]
    pub anchors: Vec<nose_normalize::Anchor>,
    /// The unit sits inside an inline test module (`mod tests` / `mod test`) —
    /// Rust keeps tests inside production files, so a path heuristic alone tags
    /// their scaffolding `prod` (#226).
    #[serde(default)]
    pub in_test_module: bool,
    /// Pack-facing value laws that actually rewrote or bridged this unit's value graph.
    #[serde(default)]
    pub semantic_laws: Vec<ValueLaw>,
    /// Query-local, dependency-backed protocol evidence for the external near lane.
    /// Empty in cached units and every no-pack or exact-only run.
    #[serde(default)]
    pub semantic_pack_near_protocols: Vec<nose_semantics::SemanticPackNearProtocol>,
    /// Whether the value fingerprint is safe to use as a strict semantic proof.
    ///
    /// `semantic` mode must not report units whose fingerprint passed through lossy
    /// lowering (`Raw`, abstract literals such as JS regex, or opaque calls). Those
    /// units can still participate in `near` via structural scoring.
    #[serde(default)]
    pub exact_safe: bool,
    /// The exact-fragment classification, when this unit is a sub-function fragment.
    ///
    /// `None` for ordinary function/method/class/block units; `Some(_)` names *why* the
    /// fragment is an exact semantic clone (its [`FragmentKind`], with a stable
    /// [`reason_code`](FragmentKind::reason_code)). Surfaced so reporting and ranking can
    /// separate proof fragments from actionable refactors without re-deriving the shape.
    #[serde(default)]
    pub fragment_kind: Option<FragmentKind>,
    /// What the recognizer proved about this fragment at acceptance time. Present iff
    /// [`fragment_kind`](Self::fragment_kind) is `Some`.
    #[serde(default)]
    pub proof_facts: Option<ProofFacts>,
}

// Cache payloads use MessagePack's compact struct representation. Serialize
// every field so defaults in the middle of the record cannot shift later
// values; the cache stage schema owns compatibility across field changes.
impl serde::Serialize for UnitFeat {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;

        let mut state = serializer.serialize_struct("UnitFeat", 30)?;
        state.serialize_field("path", &self.path)?;
        state.serialize_field("lang", &self.lang)?;
        state.serialize_field("kind", &self.kind)?;
        state.serialize_field("origin", &CompactUnitOrigin(self.origin))?;
        state.serialize_field("name", &self.name)?;
        state.serialize_field("start_line", &self.start_line)?;
        state.serialize_field("end_line", &self.end_line)?;
        state.serialize_field("source_region", &self.source_region)?;
        state.serialize_field("token_count", &self.token_count)?;
        state.serialize_field("shapes", &self.shapes)?;
        state.serialize_field("shape_minhash", &self.shape_minhash)?;
        state.serialize_field("value", &self.value)?;
        state.serialize_field("review_value", &self.review_value)?;
        state.serialize_field("minhash", &self.minhash)?;
        state.serialize_field("linear", &self.linear)?;
        state.serialize_field("connected_tokens", &self.connected_tokens)?;
        state.serialize_field("abstraction_tokens", &self.abstraction_tokens)?;
        state.serialize_field("lits", &self.lits)?;
        state.serialize_field("returns", &self.returns)?;
        state.serialize_field("pure_single_return", &self.pure_single_return)?;
        state.serialize_field("cond_sinks", &self.cond_sinks)?;
        state.serialize_field("used_length_contract", &self.used_length_contract)?;
        state.serialize_field("called_helper_returns", &self.called_helper_returns)?;
        state.serialize_field("anchors", &self.anchors)?;
        state.serialize_field("in_test_module", &self.in_test_module)?;
        state.serialize_field("semantic_laws", &self.semantic_laws)?;
        state.serialize_field(
            "semantic_pack_near_protocols",
            &self.semantic_pack_near_protocols,
        )?;
        state.serialize_field("exact_safe", &self.exact_safe)?;
        state.serialize_field("fragment_kind", &self.fragment_kind)?;
        state.serialize_field("proof_facts", &self.proof_facts)?;
        state.end()
    }
}

struct CompactUnitOrigin(UnitOrigin);

impl serde::Serialize for CompactUnitOrigin {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeTuple;

        let mut tuple = serializer.serialize_tuple(7)?;
        tuple.serialize_element(&self.0.domains)?;
        tuple.serialize_element(&self.0.subkind)?;
        tuple.serialize_element(&self.0.body_kind)?;
        tuple.serialize_element(&self.0.source_granularity)?;
        tuple.serialize_element(&self.0.region_kind)?;
        tuple.serialize_element(&self.0.container_kind)?;
        tuple.serialize_element(&self.0.evidence_flags)?;
        tuple.end()
    }
}

pub(crate) fn abstraction_family_witness<'a>(
    members: impl IntoIterator<Item = &'a UnitFeat>,
) -> Option<crate::AbstractionWitness> {
    let units = members
        .into_iter()
        .map(|unit| (unit.lang, unit.kind, unit.abstraction_tokens.as_slice()))
        .collect::<Vec<_>>();
    abstraction::family_witness(&units)
}
