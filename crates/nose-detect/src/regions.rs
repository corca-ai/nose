//! Source identity and conservative cross-snapshot correspondence. Equal
//! content is an equivalence class; correspondence never proves edit history.

mod candidate_index;
mod reconcile;
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
    digest(
        b"nose.region-analysis/v1",
        &(
            &unit.value,
            &unit.returns,
            &unit.cond_sinks,
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
    if family.abstraction_witness.is_some()
        || !family.semantic_pack_near.is_empty()
        || !family.semantic_pack_external_exact.is_empty()
    {
        return None;
    }
    let mut members = family
        .locations
        .iter()
        .map(member_review_key)
        .collect::<Option<Vec<_>>>()?;
    let mut edges = Vec::with_capacity(family.direct_edges.len());
    for edge in &family.direct_edges {
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
    Some(digest(
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
    ))
}

fn member_review_key(loc: &Loc) -> Option<ContentDigest> {
    // External evidence has occurrence coordinates. Until its complete canonical
    // dependency contract is available, do not issue a partial review signature.
    if !loc.semantic_pack_near.is_empty() || !loc.semantic_pack_external_exact.is_empty() {
        return None;
    }
    let shared = match loc.shared_subdag {
        Some((start, end)) => Some((
            start.checked_sub(loc.start_line)?,
            end.checked_sub(loc.start_line)?,
        )),
        None => None,
    };
    Some(digest(
        b"nose.review-member/v1",
        &(region_key(loc)?, loc.analysis_digest, shared),
    ))
}

#[cfg(test)]
mod tests;
