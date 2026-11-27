use super::*;
use rustc_hash::FxHashMap;

#[cfg(test)]
mod tests;

/// See [`Il::evidence_anchored_at`]: exact-anchor-span buckets and id→index
/// resolution, replacing the per-query linear `evidence` scans that were
/// quadratic on minified-bundle-sized files. Only the IMMUTABLE parts of a
/// record (anchor, id) are indexed; `status`/`dependencies` are always read
/// live, so in-place mutation of those fields needs no invalidation.
#[derive(Debug, Default)]
pub(super) struct EvidenceIndex {
    pub(super) indexed_len: usize,
    /// `(id, anchor)` of the last indexed record, used to check the append
    /// boundary. Arbitrary edits must invalidate through `Il`; this sentinel
    /// alone cannot detect changes elsewhere in an existing prefix.
    pub(super) sentinel: Option<(u32, EvidenceAnchor)>,
    pub(super) by_anchor_span: FxHashMap<(u32, u32, u32), Vec<u32>>,
    /// `Binding` anchors are queried by `local_hash` (not span) — see
    /// [`Il::evidence_binding_anchored`] — so they get their own bucket.
    pub(super) by_binding_hash: FxHashMap<u64, Vec<u32>>,
    /// Positional ids resolve directly in the evidence arena. Only non-positional
    /// ids need a fallback entry, retaining the first occurrence for duplicates.
    sparse_ids: FxHashMap<u32, u32>,
}

impl EvidenceIndex {
    /// `false` when the already-indexed prefix no longer ends with the record
    /// the index last saw — evidence was rewritten, not appended to.
    pub(super) fn prefix_intact(&self, evidence: &[EvidenceRecord]) -> bool {
        match self.sentinel {
            None => true,
            Some((id, anchor)) => self
                .indexed_len
                .checked_sub(1)
                .and_then(|last| evidence.get(last))
                .is_some_and(|record| record.id.0 == id && record.anchor == anchor),
        }
    }

    pub(super) fn extend_from(&mut self, evidence: &[EvidenceRecord]) {
        for (idx, record) in evidence.iter().enumerate().skip(self.indexed_len) {
            let span = record.anchor.span();
            self.by_anchor_span
                .entry((span.file.0, span.start_byte, span.end_byte))
                .or_default()
                .push(idx as u32);
            if let EvidenceAnchor::Binding { local_hash, .. } = record.anchor {
                self.by_binding_hash
                    .entry(local_hash)
                    .or_default()
                    .push(idx as u32);
            }
            let id = record.id.0;
            if id as usize != idx {
                self.sparse_ids.entry(id).or_insert(idx as u32);
            }
        }
        self.indexed_len = evidence.len();
        self.sentinel = evidence.last().map(|record| (record.id.0, record.anchor));
    }

    /// The same walk as the pre-index `evidence_dependencies_asserted`: every
    /// transitively-reachable dependency must resolve and be `Asserted`; a cycle
    /// is benign (revisits are skipped). Statuses and dependency lists are read
    /// live from `evidence` — only id resolution uses the index.
    pub(super) fn deps_walk(
        &self,
        evidence: &[EvidenceRecord],
        dependencies: &[EvidenceId],
    ) -> bool {
        let mut stack = dependencies.to_vec();
        let mut seen = Vec::new();
        while let Some(id) = stack.pop() {
            if seen.contains(&id) {
                continue;
            }
            seen.push(id);
            // Mirror `evidence_record_by_id`: a positional id wins even when
            // an earlier non-positional record has the same id.
            let Some(dep) = evidence
                .get(id.0 as usize)
                .filter(|record| record.id == id)
                .or_else(|| {
                    self.sparse_ids
                        .get(&id.0)
                        .map(|&idx| &evidence[idx as usize])
                })
            else {
                return false;
            };
            if dep.status != EvidenceStatus::Asserted {
                return false;
            }
            stack.extend_from_slice(&dep.dependencies);
        }
        true
    }
}
