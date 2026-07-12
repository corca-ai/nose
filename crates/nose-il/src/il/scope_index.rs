use super::Il;
use crate::intern::Symbol;
use crate::node::{LoopKind, NodeId, NodeKind, Payload};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Default)]
pub(super) struct ScopeBindingIndex {
    by_scope: HashMap<u32, ScopeBindings>,
}

#[derive(Debug, Default)]
struct ScopeBindings {
    names: HashSet<Symbol>,
    name_param_counts: HashMap<Symbol, usize>,
    cid_param_counts: HashMap<u32, usize>,
    written_names: HashSet<Symbol>,
    written_cids: HashSet<u32>,
}

impl Il {
    /// The nearest enclosing `Func`/`Lambda` scope of `node` by source span: the
    /// smallest-width scope whose span contains the node's span, ties broken by
    /// the lowest scope id. Computed for the whole arena on first use and cached —
    /// the per-query linear pass this replaces was O(nodes) *per call, which went
    /// quadratic on minified-bundle-sized inputs.
    pub fn nearest_scope(&self, node: NodeId) -> Option<NodeId> {
        let index = self.scope_index.get_or_init(|| self.build_scope_index());
        index.get(node.0 as usize).copied().flatten()
    }

    /// The next enclosing `Func`/`Lambda` scope outside `scope`.
    ///
    /// Source scopes are lexical (nested or disjoint), so a per-file interval
    /// stack builds this index in O(scopes log scopes). Equal-span scopes retain
    /// the same `(width, id)` order as [`Il::nearest_scope`]. Malformed crossing
    /// intervals are not treated as ancestors, which keeps proof consumers
    /// fail-closed instead of borrowing parameters from a sibling scope.
    pub fn parent_scope(&self, scope: NodeId) -> Option<NodeId> {
        let index = self
            .scope_parent_index
            .get_or_init(|| self.build_scope_parent_index());
        index.get(scope.0 as usize).copied().flatten()
    }

    /// `Assign` node ids whose [`Il::nearest_scope`] is `scope` (`None` =
    /// module level), in arena order. Backed by a lazy index: binding-LHS
    /// resolution filters assignments by scope per reference, which was a
    /// whole-arena pass per query before.
    pub fn assigns_in_scope(&self, scope: Option<NodeId>) -> &[NodeId] {
        let index = self.assign_scope_index.get_or_init(|| {
            let mut by_scope: HashMap<u32, Vec<NodeId>> = HashMap::new();
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

    /// `Param` nodes carrying `Payload::Cid(cid)`, in arena order.
    ///
    /// Most references resolve through their cached lexical scope chain. This
    /// index supports zero-width synthetic IL whose spans intentionally encode
    /// no containment, without restoring the old per-reference arena scan.
    pub fn params_with_cid(&self, cid: u32) -> impl Iterator<Item = NodeId> + '_ {
        let index = self.param_cid_index.get_or_init(|| {
            let mut by_cid: HashMap<u32, Vec<NodeId>> = HashMap::new();
            for (idx, node) in self.nodes.iter().enumerate() {
                if node.kind == NodeKind::Param {
                    if let Payload::Cid(cid) = node.payload {
                        by_cid.entry(cid).or_default().push(NodeId(idx as u32));
                    }
                }
            }
            by_cid
        });
        index
            .get(&cid)
            .map(|params| params.as_slice())
            .unwrap_or_default()
            .iter()
            .copied()
    }

    /// Whether `scope` binds `name` as a parameter, assignment target, or
    /// foreach-pattern target at its own lexical level.
    pub fn scope_binds_name(&self, scope: NodeId, name: Symbol) -> bool {
        self.scope_bindings(scope)
            .is_some_and(|bindings| bindings.names.contains(&name))
    }

    /// Number of pre-alpha `Param` binders named `name` attributed to `scope`.
    /// More than one is ambiguous for parameter-domain/purity lookup.
    pub fn scope_name_param_count(&self, scope: NodeId, name: Symbol) -> usize {
        self.scope_bindings(scope)
            .and_then(|bindings| bindings.name_param_counts.get(&name).copied())
            .unwrap_or(0)
    }

    /// Number of post-alpha `Param` binders carrying `cid` in `scope`, including
    /// non-direct catch-style binders. More than one is ambiguous.
    pub fn scope_cid_param_count(&self, scope: NodeId, cid: u32) -> usize {
        self.scope_bindings(scope)
            .and_then(|bindings| bindings.cid_param_counts.get(&cid).copied())
            .unwrap_or(0)
    }

    /// Whether `scope` writes `name` through an assignment/destructuring target
    /// or foreach pattern. Parameters alone are not writes.
    pub fn scope_writes_name(&self, scope: NodeId, name: Symbol) -> bool {
        self.scope_bindings(scope)
            .is_some_and(|bindings| bindings.written_names.contains(&name))
    }

    /// Canonical-id counterpart of [`Il::scope_writes_name`].
    pub fn scope_writes_cid(&self, scope: NodeId, cid: u32) -> bool {
        self.scope_bindings(scope)
            .is_some_and(|bindings| bindings.written_cids.contains(&cid))
    }

    /// Drop payload-dependent local-binder facts after an in-place binding
    /// rewrite such as alpha-renaming. Span/kind indexes remain valid.
    pub fn invalidate_scope_binding_index(&mut self) {
        let _ = self.scope_binding_index.take();
    }

    fn scope_bindings(&self, scope: NodeId) -> Option<&ScopeBindings> {
        self.scope_binding_index
            .get_or_init(|| self.build_scope_binding_index())
            .by_scope
            .get(&scope.0)
    }

    fn build_scope_binding_index(&self) -> ScopeBindingIndex {
        let mut index = ScopeBindingIndex::default();
        let mut direct_params = HashSet::new();
        let mut reachable = vec![false; self.nodes.len()];
        let mut stack = vec![self.root];
        stack.extend(self.units.iter().map(|unit| unit.root));
        while let Some(node) = stack.pop() {
            let Some(seen) = reachable.get_mut(node.0 as usize) else {
                continue;
            };
            if std::mem::replace(seen, true) {
                continue;
            }
            stack.extend(self.children(node).iter().copied());
        }
        // Function/lambda parameters have an exact structural owner. Prefer
        // that edge over span containment so zero-width/equal-span synthetic IL
        // cannot attribute an outer Func parameter to a lower-id sibling Lambda.
        for (idx, node) in self.nodes.iter().enumerate() {
            if !reachable[idx] || !matches!(node.kind, NodeKind::Func | NodeKind::Lambda) {
                continue;
            }
            let scope = NodeId(idx as u32);
            for &param in self
                .children(scope)
                .iter()
                .filter(|&&child| self.kind(child) == NodeKind::Param)
            {
                direct_params.insert(param);
                let bindings = index.by_scope.entry(scope.0).or_default();
                match self.node(param).payload {
                    Payload::Name(name) => record_name_param(bindings, name),
                    Payload::Cid(cid) => record_cid_param(bindings, cid),
                    _ => {}
                }
            }
        }
        for (idx, node) in self.nodes.iter().enumerate() {
            if !reachable[idx] {
                continue;
            }
            let id = NodeId(idx as u32);
            match node.kind {
                NodeKind::Param if !direct_params.contains(&id) => {
                    let Some(scope) = self.nearest_scope(id) else {
                        continue;
                    };
                    let bindings = index.by_scope.entry(scope.0).or_default();
                    match node.payload {
                        Payload::Name(name) => record_name_param(bindings, name),
                        Payload::Cid(cid) => record_cid_param(bindings, cid),
                        _ => {}
                    }
                }
                NodeKind::Assign => {
                    let Some((&target, scope)) =
                        self.children(id).first().zip(self.nearest_scope(id))
                    else {
                        continue;
                    };
                    collect_binding_target(
                        self,
                        target,
                        index.by_scope.entry(scope.0).or_default(),
                    );
                }
                NodeKind::Loop if matches!(node.payload, Payload::Loop(LoopKind::ForEach)) => {
                    let Some((&target, scope)) =
                        self.children(id).first().zip(self.nearest_scope(id))
                    else {
                        continue;
                    };
                    collect_binding_target(
                        self,
                        target,
                        index.by_scope.entry(scope.0).or_default(),
                    );
                }
                _ => {}
            }
        }
        index
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

    fn build_scope_parent_index(&self) -> Vec<Option<NodeId>> {
        let mut parent_by_scope = vec![None; self.nodes.len()];
        let mut scopes: Vec<NodeId> = self
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| matches!(node.kind, NodeKind::Func | NodeKind::Lambda))
            .map(|(idx, _)| NodeId(idx as u32))
            .collect();
        scopes.sort_unstable_by_key(|&scope| {
            let span = self.node(scope).span;
            (
                span.file.0,
                span.start_byte,
                std::cmp::Reverse(span.end_byte),
                scope.0,
            )
        });

        let mut stack: Vec<NodeId> = Vec::new();
        let mut cursor = 0;
        while cursor < scopes.len() {
            let first = scopes[cursor];
            let span = self.node(first).span;
            if stack
                .last()
                .is_some_and(|&scope| self.node(scope).span.file != span.file)
            {
                stack.clear();
            }
            while stack
                .last()
                .is_some_and(|&scope| !self.node(scope).span.contains(span))
            {
                stack.pop();
            }

            let mut end = cursor + 1;
            while end < scopes.len() && self.node(scopes[end]).span == span {
                end += 1;
            }
            for pair in scopes[cursor..end].windows(2) {
                parent_by_scope[pair[0].0 as usize] = Some(pair[1]);
            }
            parent_by_scope[scopes[end - 1].0 as usize] = stack.last().copied();

            // The lowest-id equal-span scope is the nearest one under the
            // established `(width, id)` preference. Its parent chain links the
            // remainder of the equal-span group before the lexical parent.
            stack.push(first);
            cursor = end;
        }
        parent_by_scope
    }
}

fn collect_binding_target(il: &Il, target: NodeId, bindings: &mut ScopeBindings) {
    match (il.kind(target), il.node(target).payload) {
        (NodeKind::Var, Payload::Name(name)) => {
            bindings.names.insert(name);
            bindings.written_names.insert(name);
        }
        (NodeKind::Var, Payload::Cid(cid)) => {
            bindings.written_cids.insert(cid);
        }
        (NodeKind::Seq, _) => {
            for &child in il.children(target) {
                collect_binding_target(il, child, bindings);
            }
        }
        _ => {}
    }
}

fn record_name_param(bindings: &mut ScopeBindings, name: Symbol) {
    bindings.names.insert(name);
    *bindings.name_param_counts.entry(name).or_default() += 1;
}

fn record_cid_param(bindings: &mut ScopeBindings, cid: u32) {
    *bindings.cid_param_counts.entry(cid).or_default() += 1;
}
