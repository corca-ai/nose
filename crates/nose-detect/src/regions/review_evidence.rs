//! Content-only projections of detector-owned review evidence.
use super::digest;
use crate::{AbstractionWitness, Loc, RefactorFamily};
use nose_il::ContentDigest;
use nose_semantics::{
    SemanticPackExternalExactProvenance, SemanticPackNearDependency, SemanticPackNearProvenance,
};
use std::collections::BTreeSet;

pub(super) fn has_complete_pack_members(family: &RefactorFamily) -> bool {
    let near: BTreeSet<_> = family
        .locations
        .iter()
        .flat_map(|l| &l.semantic_pack_near)
        .collect();
    let exact: BTreeSet<_> = family
        .locations
        .iter()
        .flat_map(|l| &l.semantic_pack_external_exact)
        .collect();
    near == family.semantic_pack_near.iter().collect()
        && exact == family.semantic_pack_external_exact.iter().collect()
}

fn dependency_key(dependency: &SemanticPackNearDependency) -> ContentDigest {
    let mut sources: Vec<_> = dependency
        .sources
        .iter()
        .map(|s| &s.content_digest)
        .collect();
    sources.sort();
    digest(
        b"nose.review-pack-dependency/v1",
        &(
            &dependency.coordinate,
            &dependency.declared_version,
            &dependency.matched_version,
            sources,
        ),
    )
}

fn call_selector(loc: &Loc, file: &str, start: u32, end: u32) -> Option<(u32, u32)> {
    (file == loc.file && start >= loc.start_line && start <= end && end <= loc.end_line)
        .then(|| (start - loc.start_line, end - loc.start_line))
}

fn near_key(loc: &Loc, evidence: &SemanticPackNearProvenance) -> Option<ContentDigest> {
    let selector = call_selector(
        loc,
        &evidence.occurrence_file,
        evidence.call_start_line,
        evidence.call_end_line,
    )?;
    let mut caveats = evidence.caveats.clone();
    caveats.sort();
    Some(digest(
        b"nose.review-pack-near/v1",
        &(
            &evidence.pack_id,
            &evidence.row_id,
            &evidence.semantic_digest,
            &evidence.row_digest,
            evidence.lane,
            &evidence.trust,
            evidence.operation,
            dependency_key(&evidence.dependency),
            selector,
            caveats,
        ),
    ))
}

fn exact_key(loc: &Loc, evidence: &SemanticPackExternalExactProvenance) -> Option<ContentDigest> {
    let selector = call_selector(
        loc,
        &evidence.occurrence_file,
        evidence.call_start_line,
        evidence.call_end_line,
    )?;
    let mut caveats = evidence.caveats.clone();
    caveats.sort();
    Some(digest(
        b"nose.review-pack-exact/v1",
        &(
            &evidence.pack_id,
            &evidence.row_id,
            &evidence.semantic_digest,
            &evidence.row_digest,
            evidence.lane,
            &evidence.assurance,
            &evidence.trust,
            dependency_key(&evidence.dependency),
            &evidence.receipt_digest,
            selector,
            caveats,
        ),
    ))
}

pub(super) fn pack_keys(loc: &Loc) -> Option<Vec<ContentDigest>> {
    let mut keys = loc
        .semantic_pack_near
        .iter()
        .map(|e| near_key(loc, e))
        .chain(
            loc.semantic_pack_external_exact
                .iter()
                .map(|e| exact_key(loc, e)),
        )
        .collect::<Option<Vec<_>>>()?;
    keys.sort();
    Some(keys)
}

pub(super) fn abstraction_key(witness: &AbstractionWitness) -> ContentDigest {
    let mut holes: Vec<_> = witness
        .holes
        .iter()
        .map(|hole| {
            let mut observed = hole.observed.clone();
            observed.sort();
            // Left/right classes and lines describe representative presentation;
            // template positions and all observed classes describe the family claim.
            digest(
                b"nose.review-abstraction-hole/v1",
                &(
                    hole.index,
                    hole.template_index,
                    hole.kind,
                    hole.role,
                    observed,
                ),
            )
        })
        .collect();
    holes.sort();
    let mut caveats = witness.caveats.clone();
    caveats.sort();
    digest(
        b"nose.review-abstraction-evidence/v1",
        &(
            witness.claim,
            witness.basis,
            witness.members_checked,
            witness.reason_code,
            witness.template_format,
            &witness.template,
            holes,
            caveats,
        ),
    )
}
