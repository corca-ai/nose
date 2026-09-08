use super::{EvidenceAnchor, EvidenceId, EvidenceIndex, EvidenceRecord};

/// Exclusive edit of one evidence record. Changes to indexed identity invalidate
/// lookups; changes to live metadata preserve the existing index.
pub struct EvidenceEdit<'a> {
    record: &'a mut EvidenceRecord,
    cache: &'a mut Option<EvidenceIndex>,
    previous: Option<EvidenceIndex>,
    identity: (EvidenceId, EvidenceAnchor),
}

impl<'a> EvidenceEdit<'a> {
    pub(super) fn new(
        record: &'a mut EvidenceRecord,
        cache: &'a mut Option<EvidenceIndex>,
    ) -> Self {
        Self {
            identity: (record.id, record.anchor),
            record,
            // Clear before granting access: forgetting the guard is safe too.
            previous: cache.take(),
            cache,
        }
    }
}

impl std::ops::Deref for EvidenceEdit<'_> {
    type Target = EvidenceRecord;
    fn deref(&self) -> &Self::Target {
        self.record
    }
}

impl std::ops::DerefMut for EvidenceEdit<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.record
    }
}

impl Drop for EvidenceEdit<'_> {
    fn drop(&mut self) {
        if self.identity == (self.record.id, self.record.anchor) {
            *self.cache = self.previous.take();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DomainEvidence, EvidenceKind, EvidenceProvenance, EvidenceStatus, FileId, NodeKind, Span,
    };

    fn record() -> EvidenceRecord {
        EvidenceRecord::new(
            EvidenceId(0),
            EvidenceAnchor::node(Span::new(FileId(0), 0, 4, 1, 1), NodeKind::Module),
            EvidenceKind::Domain(DomainEvidence::Collection),
            EvidenceProvenance::builtin("test", "test"),
            Vec::new(),
            EvidenceStatus::Asserted,
        )
    }

    #[test]
    fn metadata_updates_retain_indexes_but_identity_updates_invalidate_them() {
        let mut record = record();
        let mut cache = Some(EvidenceIndex::default());
        cache
            .as_mut()
            .unwrap()
            .extend_from(std::slice::from_ref(&record));
        for _ in 0..100 {
            EvidenceEdit::new(&mut record, &mut cache).status = EvidenceStatus::Ambiguous;
            assert_eq!(cache.as_ref().unwrap().indexed_len, 1);
        }
        EvidenceEdit::new(&mut record, &mut cache).id = EvidenceId(5);
        assert!(cache.is_none());
        cache = Some(EvidenceIndex::default());
        EvidenceEdit::new(&mut record, &mut cache).anchor =
            EvidenceAnchor::binding(Span::new(FileId(0), 10, 14, 2, 2), 7);
        assert!(cache.is_none());
    }

    #[test]
    fn forgotten_edit_cannot_leave_a_stale_index() {
        let mut record = record();
        let mut cache = Some(EvidenceIndex::default());
        let mut edit = EvidenceEdit::new(&mut record, &mut cache);
        edit.id = EvidenceId(5);
        std::mem::forget(edit);
        assert!(cache.is_none());
    }
}
