use crate::intern::{Interner, Symbol};
use crate::node::{
    EvidenceAnchor, EvidenceEmitter, EvidenceId, EvidenceKind, EvidenceProvenance, EvidenceRecord,
    EvidenceStatus, Node, NodeId, NodeKind, Payload,
};
use crate::sexpr;
use crate::span::{FileId, FileMeta, Span};
use crate::unit::Unit;
use serde::{Deserialize, Serialize};

mod evidence_index;
use evidence_index::EvidenceIndex;

/// One lowered source file. `nodes` is the arena; child links live out-of-line in
/// `edges` so each [`Node`] stays small. `file == ` this file's index in the
/// owning [`crate::Corpus`].
#[derive(Debug, Serialize, Deserialize)]
pub struct Il {
    pub nodes: Vec<Node>,
    pub edges: Vec<NodeId>,
    pub root: NodeId,
    pub file: FileId,
    pub meta: FileMeta,
    pub units: Vec<Unit>,
    /// `cid_names[cid]` is the original identifier name a canonical id came from,
    /// for human-readable reports. Empty before alpha-renaming.
    pub cid_names: Vec<Symbol>,
    /// Byte ranges (start, end) of units dropped by inline `// nose-ignore`. Set at
    /// lowering; consulted by the contiguous channel so suppressed code is excluded
    /// there too (the structural channel excludes it by dropping the unit). Not
    /// preserved across normalization (which rebuilds the arena) — only the raw IL
    /// carries it, which is what the contiguous channel reads.
    #[serde(default)]
    pub suppressed: Vec<(u32, u32)>,
    /// Pack-facing semantic evidence records keyed by stable source anchors.
    /// Source-origin and parameter-domain proof is stored here directly; exact
    /// consumers must not use side-table mirrors as alternate proof channels.
    #[serde(default)]
    pub evidence: Vec<EvidenceRecord>,
    /// Lazy whole-arena nearest-enclosing-scope index (see [`Il::nearest_scope`]).
    /// Never serialized; recomputed on first use. Sound to cache because nodes are
    /// immutable once an `Il` is built — passes rebuild the arena instead.
    #[serde(skip)]
    scope_index: std::sync::OnceLock<Vec<Option<NodeId>>>,
    /// Lazy byte-span → node-ids index (see [`Il::nodes_spanning`]). Sound under
    /// the same immutability discipline as `scope_index`: node spans and kinds
    /// are never rewritten in place (only payloads/edges are), and passes that
    /// restructure the tree rebuild the arena.
    #[serde(skip)]
    span_index: std::sync::OnceLock<std::collections::HashMap<(u32, u32), Vec<u32>>>,
    /// Lazy nearest-scope → assign-node-ids index (see [`Il::assigns_in_scope`]).
    /// Keyed by the scope's node id (+1, with `0` for module level), each bucket
    /// in arena order. Same immutability discipline as `scope_index`.
    #[serde(skip)]
    assign_scope_index: std::sync::OnceLock<std::collections::HashMap<u32, Vec<NodeId>>>,
    /// Lazy evidence lookup index (see [`Il::evidence_anchored_at`]). Appends to
    /// `evidence` are picked up incrementally (the index tracks how many records
    /// it has seen); code that mutates existing records IN PLACE must call
    /// [`Il::invalidate_evidence_index`]. Never serialized.
    #[serde(skip)]
    evidence_index: std::sync::RwLock<Option<EvidenceIndex>>,
}

impl Clone for Il {
    fn clone(&self) -> Self {
        Il {
            nodes: self.nodes.clone(),
            edges: self.edges.clone(),
            root: self.root,
            file: self.file,
            meta: self.meta.clone(),
            units: self.units.clone(),
            cid_names: self.cid_names.clone(),
            suppressed: self.suppressed.clone(),
            evidence: self.evidence.clone(),
            // Caches are cheap to recompute and a clone is usually about to be
            // mutated — start fresh.
            scope_index: std::sync::OnceLock::new(),
            span_index: std::sync::OnceLock::new(),
            assign_scope_index: std::sync::OnceLock::new(),
            evidence_index: std::sync::RwLock::new(None),
        }
    }
}

impl Il {
    pub(crate) fn from_parts(
        nodes: Vec<Node>,
        edges: Vec<NodeId>,
        root: NodeId,
        file: FileId,
        meta: FileMeta,
        units: Vec<Unit>,
        cid_names: Vec<Symbol>,
    ) -> Self {
        Self {
            nodes,
            edges,
            root,
            file,
            meta,
            units,
            cid_names,
            suppressed: Vec::new(),
            evidence: Vec::new(),
            scope_index: std::sync::OnceLock::new(),
            span_index: std::sync::OnceLock::new(),
            assign_scope_index: std::sync::OnceLock::new(),
            evidence_index: std::sync::RwLock::new(None),
        }
    }

    #[inline]
    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id.0 as usize]
    }

    #[inline]
    pub fn kind(&self, id: NodeId) -> NodeKind {
        self.nodes[id.0 as usize].kind
    }

    /// The child node ids of `id`, in order.
    #[inline]
    pub fn children(&self, id: NodeId) -> &[NodeId] {
        let n = &self.nodes[id.0 as usize];
        let s = n.child_start as usize;
        &self.edges[s..s + n.child_len as usize]
    }

    #[inline]
    pub fn var_name(&self, id: NodeId) -> Option<Symbol> {
        match (self.kind(id), self.node(id).payload) {
            (NodeKind::Var, Payload::Name(name)) => Some(name),
            _ => None,
        }
    }

    #[inline]
    pub fn var_cid(&self, id: NodeId) -> Option<u32> {
        match (self.kind(id), self.node(id).payload) {
            (NodeKind::Var, Payload::Cid(cid)) => Some(cid),
            _ => None,
        }
    }

    /// The bound name symbol of a `Var` node, resolving a `Payload::Cid` through
    /// `cid_names`. Unlike [`Self::var_name`] (which matches `Payload::Name` only), this
    /// is the full Var-name lookup several passes need; `None` for a non-`Var` node.
    #[inline]
    pub fn var_binding_name(&self, id: NodeId) -> Option<Symbol> {
        match (self.kind(id), self.node(id).payload) {
            (NodeKind::Var, Payload::Name(name)) => Some(name),
            (NodeKind::Var, Payload::Cid(cid)) => self.cid_names.get(cid as usize).copied(),
            _ => None,
        }
    }

    #[inline]
    pub fn assignment_parts(&self, id: NodeId) -> Option<(NodeId, NodeId)> {
        if self.kind(id) != NodeKind::Assign {
            return None;
        }
        let [lhs, rhs] = self.children(id) else {
            return None;
        };
        Some((*lhs, *rhs))
    }

    #[inline]
    pub fn assignment_var_parts(&self, id: NodeId) -> Option<(NodeId, NodeId)> {
        let (lhs, rhs) = self.assignment_parts(id)?;
        (self.kind(lhs) == NodeKind::Var).then_some((lhs, rhs))
    }

    /// The nearest enclosing `Func`/`Lambda` scope of `node` by source span: the
    /// smallest-width scope whose span contains the node's span, ties broken by
    /// the lowest scope id. Computed for the whole arena on first use and cached —
    /// the per-query linear pass this replaces was O(nodes) *per call*, which went
    /// quadratic (4-minute single files) on minified-bundle-sized inputs.
    pub fn nearest_scope(&self, node: NodeId) -> Option<NodeId> {
        let index = self.scope_index.get_or_init(|| self.build_scope_index());
        index.get(node.0 as usize).copied().flatten()
    }

    pub fn span_inside_local_scope(&self, span: Span) -> bool {
        self.nodes.iter().any(|node| {
            matches!(
                node.kind,
                NodeKind::Block | NodeKind::Func | NodeKind::Lambda
            ) && node.span.file == span.file
                && node.span.start_byte <= span.start_byte
                && span.end_byte <= node.span.end_byte
                && (node.span.start_byte < span.start_byte || span.end_byte < node.span.end_byte)
        })
    }

    pub fn nearest_module_scope_containing_span(&self, span: Span) -> Option<NodeId> {
        self.nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| {
                node.kind == NodeKind::Module
                    && node.span.file == span.file
                    && node.span.start_byte <= span.start_byte
                    && span.end_byte <= node.span.end_byte
            })
            .min_by_key(|(_, node)| node.span.end_byte.saturating_sub(node.span.start_byte))
            .map(|(idx, _)| NodeId(idx as u32))
    }

    /// One-pass exact computation of [`Il::nearest_scope`] for every node.
    ///
    /// Scopes are visited in (width asc, id asc) order — the same preference order
    /// a per-node argmin would use — and each scope claims every still-unclaimed
    /// node whose span it contains, so the first claim is the best one. A
    /// path-compressed "next unclaimed position" skip list over the start-sorted
    /// node order makes each node's claim O(α); per scope, only nodes that start
    /// inside but end outside its span (its ancestors — O(depth) of them) are
    /// re-examined later.
    fn build_scope_index(&self) -> Vec<Option<NodeId>> {
        let n = self.nodes.len();
        let mut scopes: Vec<(u32, u32)> = self
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| matches!(node.kind, NodeKind::Func | NodeKind::Lambda))
            .map(|(idx, node)| {
                let width = node.span.end_byte.saturating_sub(node.span.start_byte);
                (width, idx as u32)
            })
            .collect();
        scopes.sort_unstable();

        let mut order: Vec<u32> = (0..n as u32).collect();
        order.sort_unstable_by_key(|&idx| (self.nodes[idx as usize].span.start_byte, idx));
        let starts: Vec<u32> = order
            .iter()
            .map(|&idx| self.nodes[idx as usize].span.start_byte)
            .collect();

        let mut by_node: Vec<Option<NodeId>> = vec![None; n];
        // next[pos] = the next possibly-unclaimed position at or after pos.
        let mut next: Vec<u32> = (0..=n as u32).collect();
        fn next_unclaimed(next: &mut [u32], from: u32) -> u32 {
            let mut root = from;
            while next[root as usize] != root {
                root = next[root as usize];
            }
            let mut cur = from;
            while next[cur as usize] != root {
                let hop = next[cur as usize];
                next[cur as usize] = root;
                cur = hop;
            }
            root
        }

        for (_, scope_idx) in scopes {
            let scope_span = self.nodes[scope_idx as usize].span;
            let lo = starts.partition_point(|&start| start < scope_span.start_byte) as u32;
            let mut pos = next_unclaimed(&mut next, lo);
            while (pos as usize) < n {
                let target = order[pos as usize];
                let target_span = self.nodes[target as usize].span;
                if target_span.start_byte > scope_span.end_byte {
                    break;
                }
                if target_span.file == scope_span.file
                    && target_span.end_byte <= scope_span.end_byte
                {
                    by_node[target as usize] = Some(NodeId(scope_idx));
                    next[pos as usize] = pos + 1;
                }
                pos = next_unclaimed(&mut next, pos + 1);
            }
        }
        by_node
    }

    pub fn find_or_push_builtin_evidence(
        &mut self,
        anchor: EvidenceAnchor,
        kind: EvidenceKind,
        pack_id: &str,
        rule: &str,
        dependencies: Vec<EvidenceId>,
    ) -> EvidenceId {
        let provenance = EvidenceProvenance::builtin(pack_id, rule);
        self.find_or_push_builtin_evidence_with_provenance(anchor, kind, provenance, dependencies)
    }

    pub fn find_or_push_builtin_evidence_with_provenance(
        &mut self,
        anchor: EvidenceAnchor,
        kind: EvidenceKind,
        provenance: EvidenceProvenance,
        dependencies: Vec<EvidenceId>,
    ) -> EvidenceId {
        let provenance = EvidenceProvenance {
            emitter: EvidenceEmitter::Builtin,
            pack_hash: provenance.pack_hash,
            rule_hash: provenance.rule_hash,
        };
        // Index-backed dedup: only records anchored at this exact span can match,
        // so the previous whole-`evidence` pass (quadratic over an emit-heavy
        // pass) narrows to one bucket.
        if let Some(id) = self.evidence_anchored_at(anchor.span()).find_map(|record| {
            (record.anchor == anchor
                && record.kind == kind
                && record.status == EvidenceStatus::Asserted
                && record.provenance == provenance
                && record.dependencies == dependencies)
                .then_some(record.id)
        }) {
            return id;
        }
        let id = EvidenceId(self.evidence.len() as u32);
        self.evidence.push(EvidenceRecord::new(
            id,
            anchor,
            kind,
            provenance,
            dependencies,
            EvidenceStatus::Asserted,
        ));
        id
    }

    pub fn find_or_push_first_party_evidence(
        &mut self,
        anchor: EvidenceAnchor,
        kind: EvidenceKind,
        pack_id: &str,
        rule: &str,
        dependencies: Vec<EvidenceId>,
    ) -> EvidenceId {
        self.find_or_push_builtin_evidence(anchor, kind, pack_id, rule, dependencies)
    }

    pub fn find_or_push_first_party_evidence_with_provenance(
        &mut self,
        anchor: EvidenceAnchor,
        kind: EvidenceKind,
        provenance: EvidenceProvenance,
        dependencies: Vec<EvidenceId>,
    ) -> EvidenceId {
        self.find_or_push_builtin_evidence_with_provenance(anchor, kind, provenance, dependencies)
    }

    #[inline]
    pub fn evidence_record_by_id(&self, id: EvidenceId) -> Option<&EvidenceRecord> {
        self.evidence
            .get(id.0 as usize)
            .filter(|record| record.id == id)
            .or_else(|| self.evidence.iter().find(|record| record.id == id))
    }

    /// Whether every dependency of `record` (transitively) resolves and is
    /// `Asserted`. Id resolution goes through the lazy evidence index — the
    /// previous per-call resolution was a top hot spot on evidence-dense
    /// minified inputs.
    pub fn evidence_dependencies_asserted(&self, record: &EvidenceRecord) -> bool {
        if record.dependencies.is_empty() {
            return true;
        }
        self.with_evidence_index(|index| index.deps_walk(&self.evidence, &record.dependencies))
    }

    /// Evidence records whose anchor sits exactly at `span` (all anchor kinds
    /// match by exact span equality). Backed by the lazy evidence index, so a
    /// caller no longer pays a full `evidence` pass per query.
    pub fn evidence_anchored_at(&self, span: Span) -> impl Iterator<Item = &EvidenceRecord> {
        let indices = self.with_evidence_index(|index| {
            index
                .by_anchor_span
                .get(&(span.file.0, span.start_byte, span.end_byte))
                .cloned()
                .unwrap_or_default()
        });
        indices
            .into_iter()
            .map(move |idx| &self.evidence[idx as usize])
    }

    /// Node ids whose span covers exactly these bytes (callers still compare
    /// full [`Span`]/kind/payload as needed), in arena order. Replaces
    /// whole-arena passes for span-keyed lookups — those were quadratic when a
    /// consumer queried per node. Backed by a lazy index under the arena
    /// immutability discipline (see the `span_index` field).
    pub fn nodes_spanning(&self, span: Span) -> impl Iterator<Item = NodeId> + '_ {
        let index = self.span_index.get_or_init(|| {
            let mut by_bytes: std::collections::HashMap<(u32, u32), Vec<u32>> =
                std::collections::HashMap::new();
            for (idx, node) in self.nodes.iter().enumerate() {
                by_bytes
                    .entry((node.span.start_byte, node.span.end_byte))
                    .or_default()
                    .push(idx as u32);
            }
            by_bytes
        });
        index
            .get(&(span.start_byte, span.end_byte))
            .map(|ids| ids.as_slice())
            .unwrap_or_default()
            .iter()
            .map(|&idx| NodeId(idx))
    }

    /// `Assign` node ids whose [`Il::nearest_scope`] is `scope` (`None` =
    /// module level), in arena order. Backed by a lazy index: binding-LHS
    /// resolution filters assignments by scope per *reference*, which was a
    /// whole-arena pass per query before.
    pub fn assigns_in_scope(&self, scope: Option<NodeId>) -> &[NodeId] {
        let index = self.assign_scope_index.get_or_init(|| {
            let mut by_scope: std::collections::HashMap<u32, Vec<NodeId>> =
                std::collections::HashMap::new();
            for (idx, node) in self.nodes.iter().enumerate() {
                if node.kind != NodeKind::Assign {
                    continue;
                }
                let id = NodeId(idx as u32);
                let key = self.nearest_scope(id).map_or(0, |scope| scope.0 + 1);
                by_scope.entry(key).or_default().push(id);
            }
            by_scope
        });
        let key = scope.map_or(0, |scope| scope.0 + 1);
        index
            .get(&key)
            .map(|ids| ids.as_slice())
            .unwrap_or_default()
    }

    /// Indices into [`Il::evidence`] for records whose anchor sits exactly at
    /// `span`, in evidence order — the mutating sibling of
    /// [`Il::evidence_anchored_at`] (returning indices lets a caller re-borrow
    /// `evidence` mutably while walking the bucket).
    pub fn evidence_indices_anchored_at(&self, span: Span) -> Vec<u32> {
        self.with_evidence_index(|index| {
            index
                .by_anchor_span
                .get(&(span.file.0, span.start_byte, span.end_byte))
                .cloned()
                .unwrap_or_default()
        })
    }

    /// Evidence records with an [`EvidenceAnchor::Binding`] anchor carrying this
    /// `local_hash`, in evidence order. Binding anchors are matched by hash (not
    /// span) by their consumers, so they get a dedicated index bucket.
    pub fn evidence_binding_anchored(
        &self,
        local_hash: u64,
    ) -> impl Iterator<Item = &EvidenceRecord> {
        let indices = self.with_evidence_index(|index| {
            index
                .by_binding_hash
                .get(&local_hash)
                .cloned()
                .unwrap_or_default()
        });
        indices
            .into_iter()
            .map(move |idx| &self.evidence[idx as usize])
    }

    /// Drop the lazy evidence index. Appends are picked up automatically,
    /// removals trigger a rebuild, and `status`/`dependencies` are read live —
    /// call this only after rewriting a record's `anchor` or `id` in place.
    pub fn invalidate_evidence_index(&mut self) {
        *self
            .evidence_index
            .write()
            .expect("evidence index lock poisoned") = None;
    }

    fn with_evidence_index<T>(&self, read: impl FnOnce(&EvidenceIndex) -> T) -> T {
        {
            let guard = self
                .evidence_index
                .read()
                .expect("evidence index lock poisoned");
            if let Some(index) = guard.as_ref() {
                if index.indexed_len == self.evidence.len() && index.prefix_intact(&self.evidence) {
                    return read(index);
                }
            }
        }
        let mut guard = self
            .evidence_index
            .write()
            .expect("evidence index lock poisoned");
        let index = guard.get_or_insert_with(EvidenceIndex::default);
        if index.indexed_len > self.evidence.len() || !index.prefix_intact(&self.evidence) {
            // Records were removed or the prefix was rewritten (e.g. a `retain`
            // or `clear` + re-push); indices are stale — rebuild.
            *index = EvidenceIndex::default();
        }
        if index.indexed_len != self.evidence.len() {
            index.extend_from(&self.evidence);
        }
        read(index)
    }

    /// Render a subtree as an s-expression (used by `nose il --format sexpr`).
    pub fn to_sexpr(&self, root: NodeId, interner: &Interner) -> String {
        sexpr::to_sexpr(self, root, interner)
    }

    /// Structural verifier for the arena — the IL analogue of a compiler's IR
    /// validator (LLVM's `verify`, rustc's MIR validator). Every frontend and
    /// normalization pass must produce IL satisfying these invariants; a violation
    /// means an arena-construction bug, not bad input. Cheap and O(n), so it runs
    /// under `debug_assert!` after normalization and in the test harness.
    ///
    /// Returns `Err` with the first violation found.
    pub fn validate(&self) -> Result<(), String> {
        let n = self.nodes.len();
        let edges = self.edges.len();
        if (self.root.0 as usize) >= n {
            return Err(format!("root {} out of bounds (nodes: {n})", self.root.0));
        }
        for (i, node) in self.nodes.iter().enumerate() {
            let start = node.child_start as usize;
            let end = start + node.child_len as usize;
            if end > edges {
                return Err(format!(
                    "node {i} child range {start}..{end} exceeds edge arena (len {edges})"
                ));
            }
            let s = node.span;
            if s.start_byte > s.end_byte || s.start_line > s.end_line {
                return Err(format!("node {i} has a reversed span ({s:?})"));
            }
        }
        for (e, child) in self.edges.iter().enumerate() {
            if (child.0 as usize) >= n {
                return Err(format!("edge {e} references node {} >= {n}", child.0));
            }
        }
        for (u, unit) in self.units.iter().enumerate() {
            if (unit.root.0 as usize) >= n {
                return Err(format!("unit {u} root {} out of bounds", unit.root.0));
            }
        }
        Ok(())
    }
}
