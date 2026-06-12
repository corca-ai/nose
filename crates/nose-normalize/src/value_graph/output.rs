//! Value-graph fingerprint and anchor extraction.

use super::*;

impl<'a> Builder<'a> {
    /// Multiset of value-node hashes reachable from the unit's sinks, plus the
    /// sink-tagged hashes themselves, and (separately) just the literal `Const`
    /// hashes. Sorted for a canonical, order-independent fingerprint.
    /// Heavy sub-DAG ANCHORS: the structural hashes of reachable value-nodes whose
    /// sub-computation is at least `min_weight` nodes. Two units that share an anchor hash
    /// compute the *exact same* sub-value (hash-consed) — so a shared, large, rare anchor is a
    /// partial clone: an extractable common sub-computation, even when the units differ
    /// elsewhere (the case whole-unit Jaccard misses). `Const`/`Input`/`Elem` leaves are never
    /// anchors (no computation to extract). Weight is the memoized subtree size (args precede
    /// their parent in id order); capped so a deeply-shared DAG can't blow it up. Returned as
    /// `(hash, weight)` so the detector can RANK a shared sub-DAG by how big the shared
    /// computation is (a larger shared chunk is a stronger partial-clone signal).
    pub(super) fn anchors(&self, min_weight: u32) -> Anchors {
        const WEIGHT_CAP: u32 = 1 << 20;
        let n = self.nodes.len();
        let mut reachable = vec![false; n];
        let mut stack: Vec<ValueId> = self.sinks.iter().map(|s| s.value).collect();
        while let Some(v) = stack.pop() {
            let vi = v as usize;
            if reachable[vi] {
                continue;
            }
            reachable[vi] = true;
            for &a in &self.nodes[vi].args {
                if !reachable[a as usize] {
                    stack.push(a);
                }
            }
        }
        let mut weight = vec![0u32; n];
        for i in 0..n {
            let mut w: u32 = 1;
            for &a in &self.nodes[i].args {
                w = w.saturating_add(weight[a as usize]);
            }
            weight[i] = w.min(WEIGHT_CAP);
        }
        let mut out: Anchors = Vec::new();
        for i in 0..n {
            if reachable[i]
                && weight[i] >= min_weight
                && !matches!(
                    self.nodes[i].op,
                    ValOp::Const(_)
                        | ValOp::Input(_)
                        | ValOp::Elem(_)
                        | ValOp::ImportNamespace { .. }
                        | ValOp::ImportBinding { .. }
                )
            {
                let (line_start, line_end) = self
                    .node_span
                    .get(i)
                    .copied()
                    .flatten()
                    .map_or((0, 0), |s| (s.start_line, s.end_line));
                out.push(Anchor {
                    hash: self.vhash[i],
                    weight: weight[i],
                    line_start,
                    line_end,
                });
            }
        }
        // Dedup by hash (a given sub-DAG hash has a deterministic weight); sort hash-asc,
        // weight-desc so the kept entry is the largest if a hash ever recurs.
        out.sort_unstable_by(|a, b| a.hash.cmp(&b.hash).then(b.weight.cmp(&a.weight)));
        out.dedup_by_key(|a| a.hash);
        out
    }

    /// The unit's sink profile for the containment channel: whether the build emitted
    /// exactly one `Return` and nothing irreversible (no ordered effects, throws, or
    /// breaks — `flush_fields` has already run, so flushed field state would show up as
    /// `Effect` sinks here), plus the sorted guard-value hashes of its `Cond` sinks
    /// (loop iteration guards — value-relevant bookkeeping a containment match must
    /// also find in the container).
    pub(super) fn sink_profile(&self) -> (bool, Vec<u64>) {
        let mut returns = 0usize;
        let mut conds: Vec<u64> = Vec::new();
        let mut other = false;
        for s in &self.sinks {
            match s.kind {
                SinkKind::Return => returns += 1,
                SinkKind::Cond => conds.push(self.vhash[s.value as usize]),
                _ => other = true,
            }
        }
        conds.sort_unstable();
        conds.dedup();
        (!other && returns == 1, conds)
    }

    pub(super) fn fingerprint_lits(&self) -> (Vec<u64>, Vec<u64>, Vec<u64>) {
        let h = &self.vhash; // structural hashes, maintained during construction
                             // reachable from sinks
        let mut reachable = vec![false; self.nodes.len()];
        let mut stack: Vec<ValueId> = self.sinks.iter().map(|sink| sink.value).collect();
        while let Some(v) = stack.pop() {
            let vi = v as usize;
            if reachable[vi] {
                continue;
            }
            reachable[vi] = true;
            for &a in &self.nodes[vi].args {
                if !reachable[a as usize] {
                    stack.push(a);
                }
            }
        }
        let mut out: Vec<u64> = Vec::new();
        let mut lits: Vec<u64> = Vec::new();
        for i in 0..self.nodes.len() {
            if reachable[i] {
                out.push(h[i]);
                if matches!(self.nodes[i].op, ValOp::Const(_)) {
                    lits.push(h[i]);
                }
            }
        }
        let mut returns: Vec<u64> = Vec::new();
        for sink in &self.sinks {
            let mut tag = 0x5117 + sink.kind as u64;
            if matches!(sink.kind, SinkKind::Effect) {
                if let Some(ord) = sink.effect_ord {
                    tag = combine(tag, EFFECT_ORDINAL_SINK_TAG ^ u64::from(ord));
                }
            }
            out.push(combine(tag, h[sink.value as usize]));
            if matches!(sink.kind, SinkKind::Return) {
                returns.push(h[sink.value as usize]);
            }
        }
        out.sort_unstable();
        lits.sort_unstable();
        returns.sort_unstable();
        (out, lits, returns)
    }

    pub(super) fn seq_tag(&self, node: NodeId) -> u64 {
        if let Payload::Name(tag) = self.il.node(node).payload {
            if self.interner.resolve(tag) == "record_guard" {
                return if record_shape_guard_for_node(self.il, self.interner, node) {
                    SEQ_VALUE_RECORD_GUARD
                } else {
                    SEQ_VALUE_UNTAGGED
                };
            }
            if self.interner.resolve(tag) == "own_property_guard" {
                return if own_property_guard_for_node(self.il, self.interner, node) {
                    SEQ_VALUE_OWN_PROPERTY_GUARD
                } else {
                    SEQ_VALUE_UNTAGGED
                };
            }
        }
        match (self.seq_surface(node), self.il.node(node).payload) {
            (Some(contract), _) => contract.value_tag,
            _ => SEQ_VALUE_UNTAGGED,
        }
    }

    pub(super) fn seq_surface(&self, node: NodeId) -> Option<SeqSurfaceContract> {
        seq_surface_contract_for_node(self.il, self.interner, node)
    }
}
