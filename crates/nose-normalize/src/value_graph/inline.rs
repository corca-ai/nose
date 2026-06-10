//! Evidence-gated interprocedural pure inlining.
//!
//! A value-only inline is sound only after two independent checks:
//! - the target body is effect-free and value-only, so evaluating it cannot drop an observable
//!   statement effect;
//! - the call occurrence has explicit `CallTarget::DirectFunction` evidence for that target.
//!
//! The raw callee spelling is intentionally ignored here. Language/library packs own call-target
//! facts; value-graph consumers only consume those facts through the semantic evidence facade.

use super::*;
use nose_il::UnitKind;
use nose_semantics::direct_function_call_target_span_at_call;

/// A file-level inline candidate, computed ONCE per file by
/// [`ValueFingerprintContext`] instead of per unit (the per-unit registry build
/// re-walked every unit body for every unit — quadratic in file size). The two
/// conditions the per-unit build folded in are deferred to the call site:
/// the consuming unit's exclusion (no function inlines into itself through one
/// of its sub-units) and the global-binding requirement (`required_globals`
/// must all be seeded in the consuming builder's `global_env` — per-unit
/// module-binding seeding varies with what the unit references).
#[derive(Clone)]
pub(super) struct InlineCandidate {
    pub(super) root: NodeId,
    pub(super) function: InlineFunction,
    /// Free (module-symbol) names the body's safety verdict depends on, sorted.
    pub(super) required_globals: Vec<Symbol>,
}

impl<'a> Builder<'a> {
    /// Make `candidates` the unit's inline registry and snapshot the
    /// currently-seeded global bindings. The snapshot pins the registry to the
    /// post-seed, pre-process state the per-unit build used to see: module
    /// container units may add `global_env` entries while their statements are
    /// processed, and a mid-processing inline admission would otherwise depend
    /// on statement order.
    pub(super) fn adopt_inline_candidates(
        &mut self,
        root: NodeId,
        candidates: Cow<'a, [InlineCandidate]>,
    ) {
        self.inline_candidates = Some(candidates);
        self.inline_exclude_root = Some(root);
        self.inline_env_keys = self.global_env.keys().copied().collect();
    }

    /// File-level inline candidates for [`ValueFingerprintContext`]: every pure
    /// function/method body, with its safety's global-name requirements
    /// recorded instead of resolved (resolution happens per consuming unit).
    pub(super) fn collect_inline_candidates(&self) -> Vec<InlineCandidate> {
        let mut out = Vec::new();
        for unit in &self.il.units {
            if !matches!(unit.kind, UnitKind::Function | UnitKind::Method) {
                continue;
            }
            let mut required_globals = Vec::new();
            if !self.function_binding_safe_collect(unit.root, unit.root, &mut required_globals) {
                continue;
            }
            let Some(body) = self.inline_pure_body(unit.root) else {
                continue;
            };
            let kids = self.il.children(unit.root);
            let params: Vec<u32> = kids
                .iter()
                .filter_map(|&p| match self.il.node(p).payload {
                    Payload::Cid(c) if self.il.kind(p) == NodeKind::Param => Some(c),
                    _ => None,
                })
                .collect();
            required_globals.sort_unstable();
            required_globals.dedup();
            out.push(InlineCandidate {
                root: unit.root,
                function: InlineFunction { params, body },
                required_globals,
            });
        }
        out
    }

    /// [`Builder::function_binding_safe`] with the `global_env` membership test
    /// replaced by collection: free names are recorded in `required` and assumed
    /// available; the caller re-checks them against the consuming unit's
    /// `global_env`, which yields exactly the per-unit verdict the eager check
    /// produced (the check is monotone in `global_env`).
    fn function_binding_safe_collect(
        &self,
        root: NodeId,
        node: NodeId,
        required: &mut Vec<Symbol>,
    ) -> bool {
        match self.il.kind(node) {
            NodeKind::Raw
            | NodeKind::HoF
            | NodeKind::Lambda
            | NodeKind::Loop
            | NodeKind::Try
            | NodeKind::Throw => false,
            NodeKind::Func if node != root => false,
            NodeKind::Call => match self.il.node(node).payload {
                Payload::Builtin(builtin) => self.admitted_builtin_call(node, builtin),
                _ => false,
            },
            NodeKind::Var => match self.il.node(node).payload {
                Payload::Cid(_) => true,
                Payload::Name(s) => {
                    required.push(s);
                    true
                }
                _ => false,
            },
            NodeKind::Lit => matches!(
                self.il.node(node).payload,
                Payload::LitInt(_)
                    | Payload::LitBool(_)
                    | Payload::LitStr(_)
                    | Payload::LitFloat(_)
                    | Payload::Lit(nose_il::LitClass::Null)
            ),
            _ => self
                .il
                .children(node)
                .iter()
                .all(|&c| self.function_binding_safe_collect(root, c, required)),
        }
    }

    fn subtree_contains(&self, root: NodeId, needle: NodeId) -> bool {
        let mut stack = vec![root];
        let mut seen = FxHashSet::default();
        while let Some(node) = stack.pop() {
            if node == needle {
                return true;
            }
            if seen.insert(node) {
                stack.extend(self.il.children(node).iter().copied());
            }
        }
        false
    }

    /// Inline a call to a PURE registered function: bind its parameters to the caller-evaluated
    /// argument values and evaluate its body to a single value. Returns `None` for missing or
    /// ambiguous call-target evidence, unknown targets, or arity mismatch, leaving the opaque-call
    /// fallback to run.
    // proof-obligation: normalize.value_graph.pure_inline
    pub(super) fn eval_inlined_call(
        &mut self,
        call: NodeId,
        kids: &[NodeId],
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        let target = self.inline_target_for_call(call)?;
        if target.params.len() != kids.len().saturating_sub(1) {
            return None;
        }
        let mut fenv: FxHashMap<u32, ValueId> = FxHashMap::default();
        for (pi, &pc) in target.params.iter().enumerate() {
            let av = self.eval(kids[pi + 1], env);
            fenv.insert(pc, av);
        }
        // Evaluate the body to its return value, binding any local `let`s along the way: the same
        // sink-free evaluator used for lambda bodies, so locals thread through but no effect sink
        // is emitted.
        self.eval_block_return(target.body, &mut fenv)
    }

    fn inline_target_for_call(&self, call: NodeId) -> Option<InlineFunction> {
        let candidates = self.inline_candidates.as_deref()?;
        // Resolve the call's DirectFunction evidence once, then apply the
        // per-unit conditions deferred from candidate collection (see
        // `InlineCandidate`): the consuming-unit exclusion and the seeded
        // global-binding requirement (against the adopt-time snapshot).
        let proven_span = direct_function_call_target_span_at_call(self.il, call)?;
        let exclude_root = self.inline_exclude_root;
        let mut found = None;
        for candidate in candidates {
            if self.il.kind(candidate.root) != NodeKind::Func
                || self.il.node(candidate.root).span != proven_span
            {
                continue;
            }
            if exclude_root.is_some_and(|root| self.subtree_contains(candidate.root, root)) {
                continue;
            }
            if !candidate
                .required_globals
                .iter()
                .all(|name| self.inline_env_keys.contains(name))
            {
                continue;
            }
            if found.is_some() {
                return None;
            }
            found = Some(candidate.function.clone());
        }
        found
    }

    /// The body of a function that qualifies for value-only inlining: a bare `return <expr>`, or a
    /// straight-line block of LOCAL bindings (`let x = ...`, an `Assign` to a `Var`) ending in a
    /// `return`. Returns `None` for any statement effect: a field/index write, a bare effect
    /// expression, or control flow.
    fn inline_pure_body(&self, root: NodeId) -> Option<NodeId> {
        let &body = self.il.children(root).last()?;
        match self.il.kind(body) {
            NodeKind::Return => Some(body),
            NodeKind::Block => {
                let (last, prefix) = self.il.children(body).split_last()?;
                if self.il.kind(*last) != NodeKind::Return {
                    return None;
                }
                let local_binding = |&s: &NodeId| {
                    self.il.kind(s) == NodeKind::Assign
                        && self
                            .il
                            .children(s)
                            .first()
                            .is_some_and(|&t| self.il.kind(t) == NodeKind::Var)
                };
                prefix.iter().all(local_binding).then_some(body)
            }
            _ => None,
        }
    }

    /// The eager form of [`Builder::function_binding_safe_collect`]: same body
    /// verdict, with the free-name requirements resolved against the current
    /// `global_env` immediately.
    pub(super) fn function_binding_safe(&self, root: NodeId, node: NodeId) -> bool {
        let mut required = Vec::new();
        self.function_binding_safe_collect(root, node, &mut required)
            && required
                .iter()
                .all(|name| self.global_env.contains_key(name))
    }
}
