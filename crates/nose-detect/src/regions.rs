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
use serde::Serialize;

pub(crate) fn digest(domain: &[u8], value: &impl Serialize) -> ContentDigest {
    let bytes = rmp_serde::to_vec_named(value).expect("identity records serialize");
    ContentDigest::derive(domain, &[&bytes])
}

pub(crate) fn unit_analysis_key(unit: &UnitFeat) -> ContentDigest {
    let (values, returns, cond_sinks) = unit
        .review_value
        .as_ref()
        .map_or((&unit.value, &unit.returns, &unit.cond_sinks), |review| {
            (&review.values, &review.returns, &review.cond_sinks)
        });
    digest(
        b"nose.region-analysis/v1",
        &(
            values,
            returns,
            cond_sinks,
            unit.exact_safe,
            &unit.proof_facts,
            &unit.semantic_laws,
        ),
    )
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
