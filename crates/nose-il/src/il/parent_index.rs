use super::{Il, NodeId};

impl Il {
    /// The unique parent in the whole arena, or `None` for zero or multiple parents.
    /// Repeated edges from the same parent still denote one parent. This indexes
    /// edges, not lexical scopes, and includes nodes unreachable from the root.
    /// Arena edits invalidate the index; evidence-only edits preserve it.
    pub fn unique_parent(&self, child: NodeId) -> Option<NodeId> {
        self.unique_parent_index.get_or_init(|| {
            let mut parents = vec![None; self.nodes.len()];
            let mut ambiguous = vec![false; self.nodes.len()];
            for i in 0..self.nodes.len() {
                let parent = NodeId(i as u32);
                for &child in self.children(parent) {
                    let index = child.0 as usize;
                    if ambiguous[index] {
                        continue;
                    }
                    if parents[index].is_some_and(|previous| previous != parent) {
                        parents[index] = None;
                        ambiguous[index] = true;
                    } else {
                        parents[index] = Some(parent);
                    }
                }
            }
            parents
        })[child.0 as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FileId, FileMeta, IlBuilder, Lang, NodeKind, Payload, Span};

    #[test]
    fn unique_parent_preserves_ambiguity_and_refreshes_after_edge_edits() {
        let file = FileId(0);
        let span = Span::new(file, 0, 1, 1, 1);
        let mut builder = IlBuilder::new(file);
        let child = builder.add(NodeKind::Lit, Payload::LitInt(1), span, &[]);
        let left = builder.add(NodeKind::Block, Payload::None, span, &[child, child]);
        let right = builder.add(NodeKind::Block, Payload::None, span, &[]);
        let root = builder.add(NodeKind::Block, Payload::None, span, &[left, right]);
        let mut il = builder.finish(
            root,
            FileMeta {
                path: "p.js".into(),
                lang: Lang::JavaScript,
            },
            Vec::new(),
            Vec::new(),
        );
        assert_eq!(il.unique_parent(child), Some(left));
        assert_eq!(il.unique_parent(root), None);
        let bytes = serde_json::to_vec(&il).unwrap();
        let restored: Il = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(restored.unique_parent(child), Some(left));
        // Make the root reference the child too: a distinct second parent closes the lookup.
        let root_edge = il.node(root).child_start as usize;
        il.edges[root_edge] = child;
        assert_eq!(il.unique_parent(child), None);
        il.edges[root_edge] = left;
        assert_eq!(il.unique_parent(child), Some(left));
    }
}
