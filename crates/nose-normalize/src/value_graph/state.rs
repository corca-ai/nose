//! Builder initialization, value-domain gates, and evidence-backed state helpers.
//!
//! proof-obligation: normalize.value_graph.free_monoid

use super::*;

impl<'a> Builder<'a> {
    pub(super) fn new(il: &'a Il, interner: &'a Interner) -> Self {
        Builder {
            il,
            interner,
            nodes: Vec::new(),
            vhash: Vec::new(),
            node_span: Vec::new(),
            cur_span: None,
            intern: FxHashMap::default(),
            sinks: Vec::new(),
            opaque_ctr: 0,
            field_env: FxHashMap::default(),
            subtree_hash: None,
            shared_subtree_hashes: None,
            valued_subtree_hash: None,
            vty: Vec::new(),
            param_ty: Vec::new(),
            param_domain: FxHashMap::default(),
            path: Vec::new(),
            bound_order_facts: Vec::new(),
            effect_slot: 0,
            building: FxHashMap::default(),
            building_kind: FxHashMap::default(),
            global_env: FxHashMap::default(),
            inline_fns: FxHashMap::default(),
            local_scope_nodes: Cow::Owned(local_scope_nodes(il)),
            loop_recurrence: None,
            next_loop_key_base: 0,
            contracts: Vec::new(),
            value_laws: Vec::new(),
            clamp_candidate_count: 0,
            clamp_proof_backed_candidate_count: 0,
        }
    }

    pub(super) fn vty(&self, v: ValueId) -> ValueDomain {
        self.vty
            .get(v as usize)
            .copied()
            .unwrap_or(ValueDomain::Unknown)
    }

    pub(super) fn value_law_satisfied(&self, law: ValueLaw, values: &[ValueId]) -> bool {
        semantics(self.il.meta.lang)
            .operators()
            .value_law(law)
            .is_some_and(|contract| {
                contract
                    .requirement
                    .accepts(values.iter().map(|&v| self.vty(v)))
            })
    }

    pub(super) fn add_values_not_concat(&self, law: ValueLaw, values: &[ValueId]) -> bool {
        self.value_law_satisfied(law, values)
    }

    pub(super) fn record_value_law(&mut self, law: ValueLaw) {
        if nose_semantics::pack_facing_value_law(law).is_some() {
            self.value_laws.push(law);
        }
    }

    /// Bottom-up kernel value domain of a fresh node from its op and operands.
    pub(super) fn value_domain_of(&self, op: &ValOp, args: &[ValueId]) -> ValueDomain {
        let at = |i: usize| {
            args.get(i)
                .map(|&a| self.vty(a))
                .unwrap_or(ValueDomain::Unknown)
        };
        let operators = semantics(self.il.meta.lang).operators();
        match op {
            ValOp::Const(k) => const_value_domain(*k),
            ValOp::Input(k) => self
                .param_ty
                .get(*k as usize)
                .copied()
                .unwrap_or(ValueDomain::Unknown),
            ValOp::Bin(o) => {
                if *o == MIN_CODE || *o == MAX_CODE {
                    ValueDomain::Number
                } else if let Some(op) = op_from_code(*o) {
                    operators.binary_result_domain(op, at(0), at(1))
                } else {
                    ValueDomain::Unknown
                }
            }
            ValOp::Un(o) => {
                if *o == ABS_CODE {
                    ValueDomain::Number
                } else if let Some(op) = op_from_code(*o) {
                    operators.unary_result_domain(op)
                } else {
                    ValueDomain::Unknown
                }
            }
            ValOp::Seq(_) | ValOp::CollectionParam | ValOp::ArrayParam => ValueDomain::Sequence,
            ValOp::Clamp => ValueDomain::Number,
            ValOp::StringParam => ValueDomain::String,
            ValOp::Call(tag)
                if matches!(
                    *tag,
                    x if x == builtin_tag(Builtin::IsEmpty)
                        || x == builtin_tag(Builtin::StartsWith)
                        || x == builtin_tag(Builtin::EndsWith)
                        || x == builtin_tag(Builtin::Contains)
                        || x == JS_PROTOTYPE_IN_CODE
                ) =>
            {
                operators.builtin_result_domain(Builtin::Contains)
            }
            _ => ValueDomain::Unknown,
        }
    }

    /// Content hash of an IL subtree (surface kind + payload + children), cached for the
    /// whole graph. Used to key unlowered constructs by *what they are* rather than by
    /// position — so two behaviorally-different `Raw` nodes stay DISTINCT.
    pub(super) fn subtree_hash(&mut self, expr: NodeId) -> u64 {
        if let Some(shared) = self.shared_subtree_hashes {
            return shared
                .get_or_init(|| crate::subtree_hashes(self.il, self.interner))
                .get(expr.0 as usize)
                .copied()
                .unwrap_or(0);
        }
        if self.subtree_hash.is_none() {
            self.subtree_hash = Some(crate::subtree_hashes(self.il, self.interner));
        }
        self.subtree_hash
            .as_ref()
            .unwrap()
            .get(expr.0 as usize)
            .copied()
            .unwrap_or(0)
    }

    pub(super) fn valued_subtree_hash(&mut self, expr: NodeId) -> u64 {
        if self.valued_subtree_hash.is_none() {
            let mut hashes = vec![0u64; self.il.nodes.len()];
            for i in 0..self.il.nodes.len() {
                let id = NodeId(i as u32);
                let node = self.il.node(id);
                let mut h = crate::node_tag_valued(node.kind, node.payload, self.interner);
                for &child in self.il.children(id) {
                    h = combine(h, hashes[child.0 as usize]);
                }
                hashes[i] = h;
            }
            self.valued_subtree_hash = Some(hashes);
        }
        self.valued_subtree_hash
            .as_ref()
            .unwrap()
            .get(expr.0 as usize)
            .copied()
            .unwrap_or(0)
    }

    pub(super) fn source_salted_hash(&mut self, expr: NodeId, tag: u64) -> u64 {
        let span = self.il.node(expr).span;
        let mut h = combine(tag, self.valued_subtree_hash(expr));
        h = combine(h, span.file.0 as u64);
        h = combine(h, span.start_byte as u64);
        h = combine(h, span.end_byte as u64);
        h = combine(h, span.start_line as u64);
        combine(h, span.end_line as u64)
    }

    pub(super) fn is_unproven_membership_like_call(&self, expr: NodeId, kids: &[NodeId]) -> bool {
        if matches!(self.il.node(expr).payload, Payload::Builtin(_)) {
            return false;
        }
        let Some(&callee) = kids.first() else {
            return false;
        };
        if self.il.kind(callee) != NodeKind::Field {
            return false;
        }
        let Payload::Name(name) = self.il.node(callee).payload else {
            return false;
        };
        unproven_membership_like_method_contract(
            self.il.meta.lang,
            self.interner.resolve(name),
            kids.len().saturating_sub(1),
        )
        .is_some()
    }

    pub(super) fn admitted_builtin_call(&self, node: NodeId, builtin: Builtin) -> bool {
        admitted_builtin_semantics_at_call(self.il, node, builtin)
    }

    pub(super) fn domain_evidence_for_param(&self, param: NodeId) -> Option<DomainEvidence> {
        semantic_domain_evidence_for_param(self.il, param)
    }

    pub(super) fn seed_param_domains(&mut self, root: NodeId) {
        let scope = self.param_domain_scope(root).unwrap_or(root);
        for &k in self.il.children(scope) {
            if self.il.kind(k) != NodeKind::Param {
                continue;
            }
            if let (Payload::Cid(cid), Some(domain)) =
                (self.il.node(k).payload, self.domain_evidence_for_param(k))
            {
                self.param_domain.insert(cid, domain);
            }
        }
    }

    pub(super) fn seed_param_value_domains(&mut self, root: NodeId) {
        self.param_ty = semantics(self.il.meta.lang)
            .operators()
            .infer_param_value_domains(self.il, root);
        self.overlay_param_value_domains(root);
    }

    pub(super) fn overlay_param_value_domains(&mut self, root: NodeId) {
        let scope = self.param_domain_scope(root).unwrap_or(root);
        let mut pos = 0usize;
        for &k in self.il.children(scope) {
            if self.il.kind(k) != NodeKind::Param {
                continue;
            }
            if let Payload::Cid(cid) = self.il.node(k).payload {
                if let Some(value_domain) = self
                    .param_domain
                    .get(&cid)
                    .copied()
                    .and_then(ValueDomain::from_domain_evidence)
                {
                    if self.param_ty.len() <= pos {
                        self.param_ty.resize(pos + 1, ValueDomain::Unknown);
                    }
                    self.param_ty[pos] = value_domain;
                }
            }
            pos += 1;
        }
    }

    pub(super) fn param_domain_scope(&self, root: NodeId) -> Option<NodeId> {
        if self.il.kind(root) == NodeKind::Func {
            return Some(root);
        }
        let root_span = self.il.node(root).span;
        let mut best: Option<(u32, NodeId)> = None;
        for (idx, node) in self.il.nodes.iter().enumerate() {
            if node.kind != NodeKind::Func {
                continue;
            }
            let span = node.span;
            if span.start_byte > root_span.start_byte || span.end_byte < root_span.end_byte {
                continue;
            }
            let width = span.end_byte.saturating_sub(span.start_byte);
            if best.is_none_or(|(best_width, _)| width < best_width) {
                best = Some((width, NodeId(idx as u32)));
            }
        }
        best.map(|(_, node)| node)
    }

    pub(super) fn domain_evidence_of_expr(&self, expr: NodeId) -> Option<DomainEvidence> {
        nose_semantics::domain_evidence_for_receiver(self.il, self.interner, expr)
    }

    pub(super) fn is_collection_param_expr(&self, expr: NodeId) -> bool {
        nose_semantics::receiver_satisfies_domain(
            self.il,
            self.interner,
            expr,
            DomainRequirement::ArrayCollectionOrSet,
        )
    }

    pub(super) fn is_set_param_expr(&self, expr: NodeId) -> bool {
        nose_semantics::receiver_satisfies_domain(
            self.il,
            self.interner,
            expr,
            DomainRequirement::Set,
        )
    }

    pub(super) fn is_map_param_expr(&self, expr: NodeId) -> bool {
        nose_semantics::receiver_satisfies_domain(
            self.il,
            self.interner,
            expr,
            DomainRequirement::Map,
        )
    }

    pub(super) fn is_integer_param_expr(&self, expr: NodeId) -> bool {
        nose_semantics::receiver_satisfies_domain(
            self.il,
            self.interner,
            expr,
            DomainRequirement::Integer,
        )
    }

    /// Whether `value` is a parameter (an `Input`) carrying the given proof-gate domain.
    /// `is_array` adds the `ArrayParam` op on top.
    pub(super) fn is_param_value(&self, value: ValueId, domain: DomainEvidence) -> bool {
        matches!(self.nodes[value as usize].op, ValOp::Input(cid)
            if self.param_domain.get(&cid) == Some(&domain))
    }

    pub(super) fn is_array_param_value(&self, value: ValueId) -> bool {
        matches!(self.nodes[value as usize].op, ValOp::ArrayParam)
            || self.is_param_value(value, DomainEvidence::Array)
    }

    pub(super) fn param_domain_value(&mut self, value: ValueId) -> ValueId {
        let ValOp::Input(cid) = self.nodes[value as usize].op else {
            return value;
        };
        match self.param_domain.get(&cid).copied() {
            Some(domain) if domain.is_array() => self.mk(ValOp::ArrayParam, vec![value]),
            Some(domain) if domain.is_collection_or_set() => {
                self.mk(ValOp::CollectionParam, vec![value])
            }
            Some(domain) if domain.is_string() => self.mk(ValOp::StringParam, vec![value]),
            _ => value,
        }
    }

    pub(super) fn is_js_like_lang(&self) -> bool {
        semantics(self.il.meta.lang)
            .modules()
            .js_like_shadowed_module_bindings()
    }

    pub(super) fn free_name_input_key(&self, name: &str) -> u32 {
        let sym = self.interner.intern(name);
        self.free_name_key(sym)
    }

    pub(super) fn free_name_key(&self, sym: Symbol) -> u32 {
        0x8000_0000u32 | (self.interner.symbol_hash(sym) as u32)
    }

    pub(super) fn is_free_name_value(&self, value: ValueId, name: &str) -> bool {
        matches!(
            self.nodes[value as usize].op,
            ValOp::Input(key) if key == self.free_name_input_key(name)
        )
    }
}
