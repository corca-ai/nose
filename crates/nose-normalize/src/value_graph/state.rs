//! Builder initialization, value-domain gates, and evidence-backed state helpers.
//!
//! proof-obligation: normalize.value_graph.free_monoid

use super::*;

impl<'a> Builder<'a> {
    pub(super) fn new(il: &'a Il, interner: &'a Interner) -> Self {
        Self::new_with_local_scope_nodes(il, interner, Cow::Owned(local_scope_nodes(il)))
    }

    pub(super) fn new_with_local_scope_nodes(
        il: &'a Il,
        interner: &'a Interner,
        local_scope_nodes: Cow<'a, [bool]>,
    ) -> Self {
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
            index_env: FxHashMap::default(),
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
            inline_candidates: None,
            inline_exclude_root: None,
            inline_env_keys: FxHashSet::default(),
            inline_stack: Vec::new(),
            inline_capture: Vec::new(),
            loop_depth: 0,
            local_scope_nodes,
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

    /// Whether a `+` chain may be treated as numeric/non-concat rather than ordered
    /// string/list concat. `value_law_satisfied` alone accepts an UNKNOWN operand
    /// optimistically, which false-merges `a+b` with `b+a` for two untyped params that are
    /// actually strings (`"x"+"y" != "y"+"x"`, #283-C).
    ///
    /// Non-coercive languages can commute when at least one operand is proven non-concat:
    /// `num + str` errors regardless of order. JS/TS/Java are stricter because their `+`
    /// coerces mixed string/non-string operands instead of erroring, so `x + 4` is ordered
    /// unless every leaf is proven non-concat.
    pub(super) fn add_values_not_concat(&self, law: ValueLaw, values: &[ValueId]) -> bool {
        self.value_law_satisfied(law, values)
            && if self.plus_has_mixed_string_coercion() {
                values.iter().all(|&v| self.proven_non_concat(v))
            } else {
                values.iter().any(|&v| self.proven_non_concat(v))
            }
    }

    pub(super) fn plus_has_mixed_string_coercion(&self) -> bool {
        semantics(self.il.meta.lang)
            .operators()
            .plus_coerces_strings()
    }

    pub(super) fn relational_has_string_ordering(&self) -> bool {
        semantics(self.il.meta.lang)
            .operators()
            .relational_coerces_strings()
    }

    /// Whether a `+` chain may be re-associated. In JS/TS/Java, grouping itself is
    /// observable under mixed string coercion (`"a"+2+3` vs `"a"+(2+3)`), so association
    /// needs the same all-leaves non-concat proof as commutation.
    pub(super) fn add_association_safe(&self, values: &[ValueId]) -> bool {
        !self.plus_has_mixed_string_coercion()
            || self.add_values_not_concat(ValueLaw::AddAssociativity, values)
    }

    /// Whether an associative-commutative chain's operands may be COMMUTED (reordered).
    /// Associativity — regrouping a flat chain — is sound for every type and handled by the
    /// caller; this gates only the SORT. Commutativity is op- and language-specific:
    /// - `+` is ordered string/list concat unless proven non-concat (#283-C, via `add_law`);
    ///   in JS/TS/Java mixed string coercion requires every operand to be proven non-concat.
    /// - `*` is string/list REPETITION in Ruby, and there it is asymmetric: `"ab" * 3` →
    ///   "ababab" but `3 * "ab"` raises (`Integer#*` rejects a String). Python repetition
    ///   commutes (`3 * "ab"` == `"ab" * 3`), and JS/TS/Java/Go/C `*` is always numeric — so
    ///   only Ruby `*` needs gating, and only when an operand could be a string/sequence
    ///   (series 9). Reordering a numeric `*` is always sound.
    /// - `& | ^` Err on a non-numeric operand in every order, so they always commute.
    pub(super) fn ac_chain_commutes(
        &self,
        op: u32,
        operands: &[ValueId],
        add_law: ValueLaw,
    ) -> bool {
        if op == Op::Add as u32 {
            self.add_values_not_concat(add_law, operands)
        } else if op == Op::Mul as u32 {
            !semantics(self.il.meta.lang)
                .operators()
                .mul_is_sequence_repetition()
                || operands.iter().all(|&v| self.proven_non_concat(v))
        } else {
            true
        }
    }

    /// Whether `v` provably evaluates to a Number on every input it does not Err on, using
    /// ONLY genuine domain evidence — numeric literals and annotated / pack-typed params —
    /// never the OPTIMISTIC "this param is Num because a numeric op was applied to it"
    /// inference that `vty` (via `infer_param_value_domains`) folds into a param's domain.
    ///
    /// A type-gated rewrite that ERASES the very operation it inferred the domain from —
    /// `-(-a) → a`, `a & a → a` — cannot trust `vty`: the only thing proving `a: Num` was
    /// the `-`/`&` being deleted, so the canonical `a` would carry no numeric constraint
    /// and merge with a bare `def ident(a): a` (differ on a list: `-(-a)` Errs, `a` does
    /// not). That is #283-B, a confirmed false merge. Such rewrites gate on THIS instead of
    /// `value_law_satisfied` alone. Fails closed: an untyped param leaf is NOT proven, so
    /// the rewrite simply does not fire — typed/annotated operands still converge.
    pub(super) fn proven_numeric(&self, v: ValueId) -> bool {
        match self.nodes[v as usize].op {
            ValOp::Const { kind, .. } => const_value_domain(kind) == ValueDomain::Number,
            ValOp::Input(cid) => self
                .param_domain
                .get(&cid)
                .is_some_and(|d| d.is_integer_or_number()),
            ValOp::Clamp => true,
            // A unary op whose result is numeric ONLY when its operand is (`Neg`, `~`,
            // `abs`): recurse so the proof bottoms out at a genuine leaf, never at an
            // optimistic param domain.
            // `ToInt32` ALWAYS yields a number (it coerces, never Errs), regardless of its
            // operand; `abs`/`Neg`/`~` are numeric only when their operand is.
            ValOp::Un(o) if o == TO_INT32_CODE => true,
            ValOp::Un(o) => {
                (o == ABS_CODE || matches!(op_from_code(o), Some(Op::Neg | Op::BitNot)))
                    && self.nodes[v as usize]
                        .args
                        .first()
                        .is_some_and(|&a| self.proven_numeric(a))
            }
            _ => false,
        }
    }

    /// Whether `v` provably evaluates to a FLOAT (so `+`/`*`/`-` over it is non-associative
    /// and its source grouping must be preserved, #283 C-float). Genuine evidence only — a
    /// float literal, a true-division result (`a / b` is float even of integers, the
    /// value-blind i64 model notwithstanding), a float-typed parameter, or a sign-flip of one.
    /// Fails closed: an untyped param is NOT proven float. This gates `eval_sub_chain` (a float
    /// `-` stays a literal `Sub`, not `a + (-b)`) AND `eval_assoc_comm_chain` (a float `+`/`*`
    /// chain rebuilds its SOURCE grouping rather than flatten/sort) — in BOTH the general path
    /// and the string-coercion `+` path (JS/TS/Java). The fingerprint is structure-sensitive,
    /// so the held grouping survives. This is GENUINE proof only; the fully-untyped chain is
    /// handled (possibly-float) by `possibly_float`, and the oracle witnesses it via a float
    /// battery (#342, oracle-value-model §3.3).
    pub(super) fn proven_float(&self, v: ValueId) -> bool {
        match self.nodes[v as usize].op {
            ValOp::Const { kind, .. } => kind == ConstKind::Float,
            ValOp::Input(cid) => self.param_domain.get(&cid).is_some_and(|d| d.is_float()),
            ValOp::Bin(o) => op_from_code(o) == Some(Op::TrueDiv),
            ValOp::Un(o) => {
                matches!(op_from_code(o), Some(Op::Neg | Op::Pos))
                    && self.nodes[v as usize]
                        .args
                        .first()
                        .is_some_and(|&a| self.proven_float(a))
            }
            _ => false,
        }
    }

    /// Whether `v` COULD be a float — `proven_float`, OR a bare parameter in a dynamically-typed
    /// language (Python/JS/TS/Ruby), where an untyped param carries no static type and so may be
    /// a float at runtime (#342). Used ONLY by the grouping HOLDS (associativity / `eval_sub_chain`
    /// / `ac_chain_canon`), never by a value-creating rewrite: holding a chain is split-only, so a
    /// false positive here costs recall, never soundness (corpus family delta measured 0). The
    /// statically-typed languages decide float-ness by proven domain (`proven_float`) instead.
    pub(super) fn possibly_float(&self, v: ValueId) -> bool {
        if self.proven_float(v) {
            return true;
        }
        if !semantics(self.il.meta.lang).is_dynamically_typed() {
            return false;
        }
        match self.nodes[v as usize].op {
            // A TRULY-UNTYPED param (no domain evidence) could be a float at runtime → hold; the
            // float battery feeds it directly, so the oracle witnesses the non-associativity. Any
            // param WITH a domain (`a: int`, inferred `Number`, `str`, …) is fed a coerced
            // concrete value by `coerce_to_declared_domain` (a `Number` becomes an `Int`), so the
            // oracle cannot witness float there — holding it would only cost recall (it would
            // split int/`number` clones, including across languages) with no soundness gain.
            ValOp::Input(cid) => !self.param_domain.contains_key(&cid),
            _ => false,
        }
    }

    /// Whether `v` is already an int32-valued node — a JS-family bitwise result (its result
    /// is int32) or an existing `ToInt32`. Such a value needs no further narrowing when it
    /// feeds another bitwise op, which keeps the wrap to the LEAF operands only (#283-D).
    pub(super) fn is_int32_valued(&self, v: ValueId) -> bool {
        match self.nodes[v as usize].op {
            ValOp::Un(o) => o == TO_INT32_CODE || matches!(op_from_code(o), Some(Op::BitNot)),
            ValOp::Bin(o) => matches!(
                op_from_code(o),
                Some(Op::BitAnd | Op::BitOr | Op::BitXor | Op::Shl | Op::Shr)
            ),
            _ => false,
        }
    }

    /// Wrap a JS-family bitwise operand in `ToInt32` (the coercion every JS bitwise operator
    /// applies), unless it is already int32-valued. A no-op for non-JS languages, whose
    /// bitwise ops are arbitrary-precision (`Python`/`Ruby`) or type-width. #283-D.
    pub(super) fn js_int32_narrow(&mut self, v: ValueId) -> ValueId {
        if self.is_js_like_lang() && !self.is_int32_valued(v) {
            self.mk(ValOp::Un(TO_INT32_CODE), vec![v])
        } else {
            v
        }
    }

    /// Whether `v` provably never evaluates to a String or Sequence — the condition `+`
    /// COMMUTATIVITY actually needs (concat is the only non-commutative `+`, #283-C). This
    /// is WEAKER than `proven_numeric`: it admits values that are numeric-OR-error but never
    /// a string/list, so a sound reorder is not lost just because an operand's numeric-ness
    /// is unprovable. The key fact: among the modeled ops only `Add` (string concat) and
    /// `Mul` (string repetition, `"ab"*3`) can yield a string/list; every other op
    /// (`Sub`/`Div`/`Mod`/`Pow`/bitwise/shift/comparison and all unary) ERRORS on a
    /// string/list operand, so its result is never one. Fails closed: an untyped param,
    /// index, field, call, or `Phi` could be a string, so it is NOT proven non-concat.
    pub(super) fn proven_non_concat(&self, v: ValueId) -> bool {
        match self.nodes[v as usize].op {
            // Non-string literals (Number / Boolean / Null). A String const is concat.
            ValOp::Const { kind, .. } => !matches!(const_value_domain(kind), ValueDomain::String),
            // Only a GENUINELY non-string-typed param (numeric/boolean evidence) qualifies;
            // an untyped param could be a string.
            ValOp::Input(cid) => self
                .param_domain
                .get(&cid)
                .is_some_and(|d| d.is_integer_or_number() || d.is_boolean()),
            ValOp::Clamp => true,
            // Every unary op (`Neg`/`~`/`!`/`abs`/int32-narrow) errors or returns a
            // bool/number on a string operand — never a string.
            ValOp::Un(_) => true,
            ValOp::Bin(o) => {
                let args = &self.nodes[v as usize].args;
                if let Some(op) = op_from_code(o) {
                    match op {
                        // Concat / repetition: result is a string/list only when an operand
                        // is — so non-concat iff every operand is. `Mul(x, x)` is special:
                        // `str * str` errors (you cannot repeat by a string), so a squared
                        // value is never a string regardless of `x`.
                        Op::Add => args.iter().all(|&a| self.proven_non_concat(a)),
                        Op::Mul => {
                            (args.len() == 2 && args[0] == args[1])
                                || args.iter().all(|&a| self.proven_non_concat(a))
                        }
                        // Short-circuit `&&`/`||` yield an operand's VALUE, so non-concat iff
                        // both operands are.
                        Op::And | Op::Or => args.iter().all(|&a| self.proven_non_concat(a)),
                        // Everything else (`Sub`/`Div`/`Mod`/`Pow`/`FloorDiv`/`FloorMod`/
                        // bitwise/shift/comparisons) errors on a string/list → never one.
                        _ => true,
                    }
                } else {
                    // MIN/MAX of the modeled ternary are numeric; other synthesized codes
                    // (byte-pack, etc.) are numeric too.
                    o == MIN_CODE || o == MAX_CODE
                }
            }
            _ => false,
        }
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
            ValOp::Const { kind, .. } => const_value_domain(*kind),
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
                if *o == ABS_CODE || *o == TO_INT32_CODE {
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
        self.subtree_hash
            .get_or_insert_with(|| crate::subtree_hashes(self.il, self.interner))
            .get(expr.0 as usize)
            .copied()
            .unwrap_or(0)
    }

    pub(super) fn valued_subtree_hash(&mut self, expr: NodeId) -> u64 {
        let (il, interner) = (self.il, self.interner);
        self.valued_subtree_hash
            .get_or_insert_with(|| {
                let mut hashes = vec![0u64; il.nodes.len()];
                for i in 0..il.nodes.len() {
                    let id = NodeId(i as u32);
                    let node = il.node(id);
                    let mut h = crate::node_tag_valued(node.kind, node.payload, interner);
                    for &child in il.children(id) {
                        h = combine(h, hashes[child.0 as usize]);
                    }
                    hashes[i] = h;
                }
                hashes
            })
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
