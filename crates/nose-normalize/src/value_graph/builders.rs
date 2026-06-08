//! Active collection/map builder recognition for value-graph loops.

use super::*;

impl<'a> Builder<'a> {
    /// The canonical value of a dict key→value entry — `Call(DictEntry, [k, v])` — shared by
    /// a dict `pair`, a dict-comprehension body, and a `d[k]=v` building loop, so all three
    /// converge, while staying DISTINCT from a tuple `Seq([k, v])` (a list of pairs is a
    /// different value than a dict). Covered by the functor/map obligation; the build is a map.
    pub(super) fn dict_entry(&mut self, kv: Vec<ValueId>) -> ValueId {
        self.mk(ValOp::Call(builtin_tag(Builtin::DictEntry)), kv)
    }

    /// If `assign` writes `Index(Var c, k)` for an ACTIVE dict-builder `c` (seeded by a proven
    /// empty map), record
    /// the per-element `DictEntry(k, rhs)` under the current path guard and return true — a
    /// `d[k] = v` write IS the build, so `d={}; for x: d[k]=v` converges with `{k: v for x}`.
    /// A second write spoils it (→ ordinary effect). The write must be backed by kernel effect
    /// evidence; raw index shape alone is not proof.
    pub(super) fn try_record_index_assign(
        &mut self,
        assign: NodeId,
        rhs: ValueId,
        env: &FxHashMap<u32, ValueId>,
    ) -> bool {
        let Some((base, key, _value)) = self.builder_index_write_parts(assign) else {
            return false;
        };
        let (NodeKind::Var, Payload::Cid(c)) = (self.il.kind(base), self.il.node(base).payload)
        else {
            return false;
        };
        if !self.building.contains_key(&c) {
            return false;
        }
        if self.building_kind.get(&c) != Some(&BuilderKind::Map) {
            self.building.insert(c, None);
            return true;
        }
        let Some(keyn) = key else {
            self.building.insert(c, None);
            return true;
        };
        let k = self.eval(keyn, env);
        let entry = self.dict_entry(vec![k, rhs]);
        let guard = self.path_cond();
        self.building.insert(c, Some((entry, guard)));
        true
    }

    pub(super) fn builder_index_write_parts(
        &self,
        assign: NodeId,
    ) -> Option<(NodeId, Option<NodeId>, NodeId)> {
        exact_non_overloadable_index_assignment_parts(self.il, assign)
            .or_else(|| self.contextual_map_builder_index_write_parts(assign))
    }

    pub(super) fn contextual_map_builder_index_write_parts(
        &self,
        assign: NodeId,
    ) -> Option<(NodeId, Option<NodeId>, NodeId)> {
        let contract = map_builder_index_write_contract(self.il.meta.lang)?;
        if contract.required_effect != EffectEvidenceKind::BindingWrite
            || contract.receiver != IndexWriteReceiverContract::ActiveMapBuilder
        {
            return None;
        }
        let target = binding_write_target(self.il, assign)?;
        if self.il.kind(target) != NodeKind::Index {
            return None;
        }
        let assign_kids = self.il.children(assign);
        let value = *assign_kids.get(1)?;
        let target_kids = self.il.children(target);
        Some((*target_kids.first()?, target_kids.get(1).copied(), value))
    }

    /// If `e` is a single-item `append(r, item)` to an ACTIVE builder var `r`, record the
    /// per-element contribution under the current path guard and return true (the append IS
    /// the build, not an effect). A multi-item form spoils the builder.
    /// Recognize a single-item builder append to a var `r`, returning `(r_cid, item_args)`.
    /// Exact append-effect evidence or an admitted append method occurrence is required;
    /// an active-builder receiver alone does not prove that a raw method selector has
    /// builder-append semantics.
    pub(super) fn list_append_parts(&self, e: NodeId) -> Option<(u32, Vec<NodeId>)> {
        let (receiver, item) = builder_append_call_args(self.il, self.interner, e)
            .or_else(|| admitted_builder_append_method_call_args(self.il, self.interner, e))?;
        let (NodeKind::Var, Payload::Cid(c)) =
            (self.il.kind(receiver), self.il.node(receiver).payload)
        else {
            return None;
        };
        Some((c, vec![item]))
    }

    pub(super) fn single_arg_field_call_parts(
        &self,
        node: NodeId,
    ) -> Option<(NodeId, Symbol, NodeId)> {
        if self.il.kind(node) != NodeKind::Call {
            return None;
        }
        let [callee, arg] = self.il.children(node) else {
            return None;
        };
        if self.il.kind(*callee) != NodeKind::Field {
            return None;
        }
        let Payload::Name(method) = self.il.node(*callee).payload else {
            return None;
        };
        let receiver = *self.il.children(*callee).first()?;
        Some((receiver, method, *arg))
    }

    pub(super) fn try_record_append(
        &mut self,
        e: NodeId,
        env: &mut FxHashMap<u32, ValueId>,
    ) -> bool {
        let Some((c, items)) = self.list_append_parts(e) else {
            return false;
        };
        if !self.building.contains_key(&c) {
            return false;
        }
        if self.building_kind.get(&c) != Some(&BuilderKind::List) {
            self.building.insert(c, None);
            return true;
        }
        if items.len() != 1 {
            self.building.insert(c, None); // multi-item append — not a clean map
            return true;
        }
        let contrib = self.eval(items[0], env);
        let guard = self.path_cond();
        self.building.insert(c, Some((contrib, guard)));
        true
    }

    /// Go's functional append `r = append(r, item)` (an `Assign` whose RHS is `Append(r, …)`
    /// over the same var) to an ACTIVE builder var `r` is the same single-item build as the
    /// effect-form `r.append(item)`: record the per-element contribution under the path guard.
    /// A multi-item `append(r, a, b)` spoils the builder, like the effect form.
    pub(super) fn try_record_reassign_append(
        &mut self,
        target: NodeId,
        rhs: NodeId,
        env: &mut FxHashMap<u32, ValueId>,
    ) -> bool {
        let (NodeKind::Var, Payload::Cid(c)) = (self.il.kind(target), self.il.node(target).payload)
        else {
            return false;
        };
        if !self.building.contains_key(&c) {
            return false;
        }
        let Some((receiver, value)) = builder_append_call_args(self.il, self.interner, rhs) else {
            return false;
        };
        // The append's receiver must be the same var being reassigned (`r = append(r, …)`).
        let same_receiver = self.il.kind(receiver) == NodeKind::Var
            && matches!(self.il.node(receiver).payload, Payload::Cid(fc) if fc == c);
        if !same_receiver {
            return false;
        }
        if self.building_kind.get(&c) != Some(&BuilderKind::List) {
            self.building.insert(c, None);
            return true;
        }
        let contrib = self.eval(value, env);
        let guard = self.path_cond();
        self.building.insert(c, Some((contrib, guard)));
        true
    }

    /// Local list-builder candidates of a loop body: a var `r` (1) bound to an empty list
    /// before the loop, (2) the target of exactly one single-item `append`, and (3) not
    /// otherwise mentioned in the body. Such a loop builds `Map(elem, contrib)` — the same
    /// node the comprehension `[contrib for x in xs]` / `.map`/`.collect` produces.
    pub(super) fn builder_candidates(
        &self,
        body: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> Vec<BuilderCandidate> {
        let mut appends: FxHashMap<(u32, BuilderKind), u32> = FxHashMap::default();
        let mut mentions: FxHashMap<u32, u32> = FxHashMap::default();
        let mut spoiled: FxHashSet<u32> = FxHashSet::default();
        let mut stack = vec![body];
        while let Some(n) = stack.pop() {
            // Go functional append `r = append(r, item)`: count it as r's single build append
            // and a single build mention (mirroring the effect form's one receiver mention),
            // then scan ONLY `item` — the two r occurrences (assign target + append receiver)
            // are the build, not other uses that would disqualify r.
            if self.il.kind(n) == NodeKind::Assign {
                if let [tgt, rhs] = self.il.children(n) {
                    if let (NodeKind::Var, Payload::Cid(c)) =
                        (self.il.kind(*tgt), self.il.node(*tgt).payload)
                    {
                        if let Some((receiver, value)) =
                            builder_append_call_args(self.il, self.interner, *rhs)
                        {
                            let same = self.il.kind(receiver) == NodeKind::Var
                                && matches!(self.il.node(receiver).payload, Payload::Cid(fc) if fc == c);
                            if same {
                                *mentions.entry(c).or_insert(0) += 1;
                                *appends.entry((c, BuilderKind::List)).or_insert(0) += 1;
                                stack.push(value);
                                continue;
                            }
                        }
                    }
                }
            }
            if let (NodeKind::Var, Payload::Cid(c)) = (self.il.kind(n), self.il.node(n).payload) {
                *mentions.entry(c).or_insert(0) += 1;
            }
            // An effect-form append — `r.append/push(item)` (canonical) or Java `r.add(item)` —
            // counts as r's build append (the receiver mention is counted by the generic Var
            // scan above / below as the node's children are walked).
            if let Some((c, items)) = self.list_append_parts(n) {
                if items.len() == 1 {
                    *appends.entry((c, BuilderKind::List)).or_insert(0) += 1;
                } else {
                    spoiled.insert(c);
                }
            }
            // A `d[k] = v` assignment is a DICT build for `d` — counted like an append, so
            // `d={}; for x: d[k]=v` is recognized as a builder (finalized to a `Map` of
            // `DictEntry`s, converging with `{k: v for x}`).
            if self.il.kind(n) == NodeKind::Assign {
                if let Some((base, _, _)) = self.builder_index_write_parts(n) {
                    if let (NodeKind::Var, Payload::Cid(c)) =
                        (self.il.kind(base), self.il.node(base).payload)
                    {
                        *appends.entry((c, BuilderKind::Map)).or_insert(0) += 1;
                    }
                }
            }
            stack.extend(self.il.children(n).iter().copied());
        }
        appends
            .iter()
            .filter(|&(&(c, kind), &n)| {
                n == 1
                    && !spoiled.contains(&c)
                    && mentions.get(&c).copied() == Some(1)
                    && env
                        .get(&c)
                        .and_then(|&v| self.builder_seed_kind(v))
                        .is_some_and(|seed_kind| seed_kind == kind)
            })
            .map(|(&(cid, kind), _)| BuilderCandidate { cid, kind })
            .collect()
    }

    pub(super) fn builder_seed_kind(&self, value: ValueId) -> Option<BuilderKind> {
        let node = &self.nodes[value as usize];
        if !node.args.is_empty() {
            return None;
        }
        match node.op {
            ValOp::Seq(SEQ_VALUE_COLLECTION) => Some(BuilderKind::List),
            ValOp::Seq(SEQ_VALUE_MAP) => Some(BuilderKind::Map),
            _ => None,
        }
    }
}
