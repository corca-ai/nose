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
use nose_semantics::direct_function_call_target_at_call;

impl<'a> Builder<'a> {
    /// Build the interprocedural inline registry: pure, file-local functions/methods that can be
    /// inlined to their body's value. Excludes the unit currently being built, and any enclosing
    /// function/method for sub-unit roots, so a function is never inlined into itself through one
    /// of its blocks or exact fragments.
    pub(super) fn build_inline_registry(&mut self, root: NodeId) {
        if !self.inline_fns.is_empty() {
            return;
        }
        for unit in self.il.units.clone() {
            if !matches!(unit.kind, UnitKind::Function | UnitKind::Method)
                || self.subtree_contains(unit.root, root)
            {
                continue;
            }
            if !self.function_binding_safe(unit.root, unit.root) {
                continue;
            }
            // SOUNDNESS: only inline an EFFECT-FREE body: a `return <expr>` or a straight-line
            // block of LOCAL bindings ending in a `return`. `function_binding_safe` alone is too
            // weak because it admits field/index WRITES, an effect the value-only inline would
            // silently drop. The interpreter oracle also checks cross-function calls end-to-end,
            // but the syntactic gate stays conservative by construction.
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
            self.inline_fns
                .insert(unit.root, InlineFunction { params, body });
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
        let mut found = None;
        for (&root, function) in &self.inline_fns {
            if !direct_function_call_target_at_call(self.il, call, root) {
                continue;
            }
            if found.is_some() {
                return None;
            }
            found = Some(function.clone());
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

    pub(super) fn function_binding_safe(&self, root: NodeId, node: NodeId) -> bool {
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
                Payload::Name(s) => self.global_env.contains_key(&s),
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
                .all(|&c| self.function_binding_safe(root, c)),
        }
    }
}
