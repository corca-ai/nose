//! Source identity and conservative cross-snapshot correspondence. Equal
//! content is an equivalence class; correspondence never proves edit history.

mod candidate_index;
pub mod evolution;
mod reconcile;
mod review_evidence;
mod snapshot;
pub use reconcile::{reconcile, ChangeKind, Correspondence, Reconciliation};
pub use snapshot::{RegionRecord, RegionSnapshot};

use crate::{Loc, RefactorFamily, UnitFeat};
use nose_il::ContentDigest;

mod encoding;
pub(crate) use encoding::digest;

type AnalysisInputs<'a> = (
    &'a [u64],
    &'a [u64],
    &'a [u64],
    bool,
    &'a Option<crate::fragment::ProofFacts>,
    &'a [nose_semantics::ValueLaw],
);

fn analysis_inputs(unit: &UnitFeat) -> AnalysisInputs<'_> {
    let (values, returns, cond_sinks) = unit
        .review_value
        .as_ref()
        .map_or((&unit.value, &unit.returns, &unit.cond_sinks), |review| {
            (&review.values, &review.returns, &review.cond_sinks)
        });
    (
        values,
        returns,
        cond_sinks,
        unit.exact_safe,
        &unit.proof_facts,
        &unit.semantic_laws,
    )
}

pub(crate) fn unit_analysis_key(unit: &UnitFeat) -> ContentDigest {
    digest(b"nose.region-analysis/v1", &analysis_inputs(unit))
}

/// Reuse a group member's digest only for identical complete serialized inputs.
pub(crate) struct AnalysisKeyReference<'a> {
    inputs: AnalysisInputs<'a>,
    raw_values: bool,
    digest: ContentDigest,
}

impl<'a> AnalysisKeyReference<'a> {
    pub(crate) fn new(unit: &'a UnitFeat) -> Self {
        Self {
            inputs: analysis_inputs(unit),
            raw_values: unit.review_value.is_none(),
            digest: unit_analysis_key(unit),
        }
    }

    /// `equal_values` must come from evidence that raw unit values are equal.
    /// Selected review overrides and all other serialized inputs remain independent.
    pub(crate) fn key_for(&self, unit: &UnitFeat, equal_values: bool) -> ContentDigest {
        let inputs = analysis_inputs(unit);
        let known_values = equal_values && self.raw_values && unit.review_value.is_none();
        debug_assert!(!known_values || self.inputs.0 == inputs.0);
        if (known_values || self.inputs.0 == inputs.0)
            && self.inputs.1 == inputs.1
            && self.inputs.2 == inputs.2
            && self.inputs.3 == inputs.3
            && self.inputs.4 == inputs.4
            && self.inputs.5 == inputs.5
        {
            self.digest
        } else {
            unit_analysis_key(unit)
        }
    }
}

/// Pathless source/region signature, shared by identical occurrences. Byte
/// offsets belong to the address, not to this signature.
pub fn region_key(loc: &Loc) -> Option<ContentDigest> {
    let source = loc.source_region.as_ref()?;
    Some(digest(
        b"nose.region-content/v1",
        &(
            &loc.lang,
            loc.kind,
            loc.origin,
            loc.is_fragment,
            loc.fragment_kind,
            loc.reason_code,
            source.content_digest,
        ),
    ))
}

/// A many-to-one key for a multiset of region contents and detector evidence.
/// Missing byte provenance makes the whole key unavailable. This is never a
/// durable occurrence id or permission to transfer a disposition to all copies.
pub fn review_key(family: &RefactorFamily) -> Option<ContentDigest> {
    if !review_evidence::has_complete_pack_members(family) {
        return None;
    }
    let mut members = family
        .locations
        .iter()
        .map(member_review_key)
        .collect::<Option<Vec<_>>>()?;
    let mut edges = Vec::with_capacity(family.direct_edges.len());
    for edge in family.direct_edges.iter() {
        let mut ends = [
            *members.get(edge.left as usize)?,
            *members.get(edge.right as usize)?,
        ];
        ends.sort();
        edges.push((ends, edge.witness_kind));
    }
    edges.sort();
    members.sort(); // Multiplicity is intentional.
    let mut laws = family.semantic_laws.clone();
    laws.sort();
    let key = digest(
        b"nose.review-content/v1",
        &(
            members,
            edges,
            (
                family.witness.as_ref()?.kind(),
                family.witness.as_ref()?.value_nodes(),
            ),
            laws,
        ),
    );
    Some(match &family.abstraction_witness {
        Some(witness) => digest(
            b"nose.review-abstraction/v1",
            &(key, review_evidence::abstraction_key(witness)),
        ),
        None => key,
    })
}

fn member_review_key(loc: &Loc) -> Option<ContentDigest> {
    let shared = match loc.shared_subdag {
        Some((start, end)) if start >= loc.start_line && end <= loc.end_line && start <= end => {
            Some((start - loc.start_line, end - loc.start_line))
        }
        Some(_) => None,
        None => None,
    };
    let mut key = digest(
        b"nose.review-member/v1",
        &(region_key(loc)?, loc.analysis_digest, shared),
    );
    // Inlined callee anchors can lie outside the caller. Bind their actual
    // selected bytes rather than treating an absolute line as caller-relative.
    if loc.shared_subdag.is_some() && shared.is_none() {
        key = digest(
            b"nose.review-shared-source/v1",
            &(key, loc.shared_source_region.as_ref()?.content_digest),
        );
    }
    if !loc.semantic_pack_near.is_empty() || !loc.semantic_pack_external_exact.is_empty() {
        key = digest(
            b"nose.review-pack-member/v1",
            &(key, review_evidence::pack_keys(loc)?),
        );
    }
    Some(key)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod review_tests;
