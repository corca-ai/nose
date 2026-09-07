//! Exact endpoint sets that do not require expanding the accepted graph.
use super::AcceptedEdges;
use std::ops::Range;

impl AcceptedEdges {
    /// Returns the exact set of incident site indices when its representation
    /// already proves a contiguous prefix. `None` requires examining the edges;
    /// unreferenced sites must not become additional coverage obligations.
    /// This query never materializes a deferred graph.
    pub fn known_incident_sites(&self) -> Option<Range<usize>> {
        let count = if let Some(deferred) = &self.deferred {
            deferred.incident_sites?
        } else if let Some(packed) = &self.packed {
            packed.complete.as_ref()?.incident_sites()
        } else {
            return self.appended.is_empty().then_some(0..0);
        };
        self.appended
            .iter()
            .all(|edge| (edge.left as usize) < count && (edge.right as usize) < count)
            .then_some(0..count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AcceptedEdge, SiteEdges};
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    #[test]
    fn incident_certificate_is_lazy_shared_and_invalidated_by_outside_appends() {
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = calls.clone();
        let edges = AcceptedEdges::deferred(true, Some(4), move || {
            counter.fetch_add(1, Ordering::SeqCst);
            SiteEdges::complete(4, 1.0)
        });
        let mut changed = edges.clone();
        let edge = |right| AcceptedEdge {
            left: 0,
            right,
            score: 1.0,
            witness_kind: "exact-value-graph",
        };
        changed.push(edge(3));
        assert_eq!(changed.known_incident_sites(), Some(0..4));
        assert_eq!(edges.known_incident_sites(), Some(0..4));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        changed.push(edge(u32::MAX));
        assert_eq!(changed.known_incident_sites(), None);
        std::thread::scope(|scope| {
            for _ in 0..4 {
                let edges = &edges;
                scope.spawn(move || {
                    assert_eq!(edges.known_incident_sites(), Some(0..4));
                    assert_eq!(edges.len(), 6);
                });
            }
        });
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(edges.known_incident_sites(), Some(0..4));
        assert_eq!(changed.known_incident_sites(), None);
        assert!(changed.iter().any(|e| e.right == u32::MAX));
        assert_eq!(
            AcceptedEdges::from(vec![edge(3)]).known_incident_sites(),
            None
        );
    }

    #[test]
    fn complete_incidence_matches_expanded_endpoint_union_including_empty_graphs() {
        for count in [0, 1, 2, 7, 65] {
            let edges = AcceptedEdges::from_packed(SiteEdges::complete(count, 1.0));
            let expected = edges
                .iter()
                .flat_map(|e| [e.left as usize, e.right as usize])
                .collect::<std::collections::BTreeSet<_>>();
            assert_eq!(
                edges
                    .known_incident_sites()
                    .unwrap()
                    .collect::<std::collections::BTreeSet<_>>(),
                expected
            );
        }
    }
}
