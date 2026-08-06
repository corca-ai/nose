//! Shared post-lower evidence facade.
//!
//! Evidence lookup, API-record construction, and bound-order proof recovery
//! have different consumers and change independently. Their implementations
//! stay behind this facade so existing lowering call sites retain one import.

use super::*;

mod api_records;
mod bound_order;
mod symbols;

pub(super) use api_records::*;
pub(super) use bound_order::record_post_lower_bound_order_guard_evidence;
pub(super) use symbols::*;

pub(super) fn post_lower_find_or_push_evidence(
    il: &mut Il,
    anchor: EvidenceAnchor,
    kind: EvidenceKind,
    rule: &str,
    dependencies: Vec<EvidenceId>,
) -> Option<EvidenceId> {
    let _ = rule;
    let (pack_id, producer_id) = language_core_evidence_provenance(il.meta.lang);
    Some(il.find_or_push_builtin_evidence(anchor, kind, pack_id, producer_id, dependencies))
}
