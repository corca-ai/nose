//! Evidence-gated interprocedural pure inlining.
//!
//! A value-only inline is sound only after two independent checks:
//! - evaluating the target body emits no observable statement effect — enforced at
//!   evaluation time by a sink fence (any `Effect`/`Throw`/`Break`/`Cond` sink, exact
//!   field write, or in-loop return poisons the attempt and the call falls back to the
//!   opaque path);
//! - the call occurrence has explicit `CallTarget::DirectFunction` evidence for that target.
//!
//! Admission is deliberately wider than the old straight-line whitelist: loops, branches,
//! and nested proven calls are all admitted, because the statement processor models the
//! pure forms of those (builder loops, reductions, guarded returns) without emitting
//! sinks — and anything it cannot model purely *does* emit a sink, which the fence turns
//! into a fail-closed abort. The syntactic walk below only rejects shapes that poison
//! every evaluation (`try`/`throw`/`break`/`Raw`/nested function declarations), bounds
//! the body size, and collects the free module names the evaluation will resolve.
//!
//! The raw callee spelling is intentionally ignored here. Language/library packs own
//! call-target facts; value-graph consumers only consume those facts through the semantic
//! evidence facade.

use super::*;
use nose_il::UnitKind;
use nose_semantics::direct_function_call_target_span_at_call;

/// Body-size ceiling (IL nodes) for an inline candidate — keeps per-call-site re-evaluation
/// bounded; helpers worth inlining are small.
const INLINE_MAX_BODY_NODES: u32 = 320;
/// Nested-inline depth ceiling (cycles are excluded separately by the inline stack).
const INLINE_MAX_DEPTH: usize = 3;

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
    /// Free (module-symbol) names the body's evaluation depends on, sorted.
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

    /// File-level inline candidates for [`ValueFingerprintContext`]: every
    /// function/method body passing the generalized purity *shape* walk, with its
    /// free-name requirements recorded instead of resolved (resolution happens per
    /// consuming unit). The evaluation-time sink fence is the authoritative purity gate.
    pub(super) fn collect_inline_candidates(&self) -> Vec<InlineCandidate> {
        let mut out = Vec::new();
        for unit in &self.il.units {
            if !matches!(unit.kind, UnitKind::Function | UnitKind::Method) {
                continue;
            }
            let mut required_globals = Vec::new();
            let Some(body) = self.pure_callable_body(unit.root, &mut required_globals) else {
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

    /// The body of a function that qualifies for generalized value-only inlining: any
    /// statement tree free of the constructs the inline evaluator can never represent
    /// faithfully — `try`/`throw` (exceptional control), `break` (loop-prefix
    /// truncation), unlowered `Raw` statements, and nested function declarations — that
    /// provably ends in a `return` on every path (a fall-off-the-end body returns a
    /// language-specific void this model does not claim). Everything else — loops,
    /// branches, builder appends, nested proven calls — is admitted here and judged at
    /// evaluation time by the sink fence. Free module names are collected for the
    /// per-unit `global_env` availability check (deterministic against the adopt-time
    /// snapshot, never mid-unit state).
    pub(super) fn pure_callable_body(
        &self,
        root: NodeId,
        required: &mut Vec<Symbol>,
    ) -> Option<NodeId> {
        let &body = self.il.children(root).last()?;
        let mut budget = INLINE_MAX_BODY_NODES;
        if !self.pure_callable_walk(root, body, required, &mut budget) {
            return None;
        }
        // `branch_returns` is the existing "every path ends in an explicit return"
        // verdict (a return, a block ending in one, or an exhaustive if/else of them) —
        // exactly the no-fall-off-the-end requirement value-only inlining needs.
        self.branch_returns(body).then_some(body)
    }

    /// Whether a function root passes the inline shape walk at all — the admission used
    /// for content-keyed identity seeding, where free-name availability is irrelevant
    /// (the seed is a literal-sensitive body hash, not an evaluation).
    pub(super) fn pure_callable_shape(&self, root: NodeId) -> bool {
        self.pure_callable_body(root, &mut Vec::new()).is_some()
    }

    fn pure_callable_walk(
        &self,
        root: NodeId,
        node: NodeId,
        required: &mut Vec<Symbol>,
        budget: &mut u32,
    ) -> bool {
        if *budget == 0 {
            return false;
        }
        *budget -= 1;
        match self.il.kind(node) {
            NodeKind::Raw | NodeKind::Try | NodeKind::Throw | NodeKind::Break => false,
            NodeKind::Func if node != root => false,
            NodeKind::Var => {
                if let Payload::Name(s) = self.il.node(node).payload {
                    required.push(s);
                }
                true
            }
            _ => self
                .il
                .children(node)
                .iter()
                .all(|&c| self.pure_callable_walk(root, c, required, budget)),
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
    /// argument values, run the ordinary statement processor over its body with returns captured,
    /// and fold the captured returns to a single value. Returns `None` — leaving the opaque-call
    /// fallback to run — for missing or ambiguous call-target evidence, unknown targets, arity
    /// mismatch, recursion, or any body whose evaluation the sink fence rejects as impure.
    // proof-obligation: normalize.value_graph.pure_inline
    pub(super) fn eval_inlined_call(
        &mut self,
        call: NodeId,
        kids: &[NodeId],
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        let (root, target) = self.inline_target_for_call(call)?;
        if target.params.len() != kids.len().saturating_sub(1) {
            return None;
        }
        if self.inline_stack.len() >= INLINE_MAX_DEPTH || self.inline_stack.contains(&root) {
            return None;
        }
        // Bind arguments to parameters by the shared plan: positional left-to-right,
        // keyword (`name=value`) by name. A keyword naming no param, a double-bind, or
        // any param left unbound fails closed to the opaque path — so `helper(b=p, a=q)`
        // substitutes the RIGHT operands, never the positional ones (#301).
        let plan = crate::call_args::keyword_arg_binding_plan(self.il, &target.params, &kids[1..])?;
        let mut fenv: FxHashMap<u32, ValueId> = FxHashMap::default();
        for (pc, arg) in plan {
            let v = self.eval(arg, env);
            fenv.insert(pc, v);
        }
        self.inline_stack.push(root);
        let value = self.inline_eval_pure_body(target.body, &mut fenv);
        self.inline_stack.pop();
        value
    }

    /// Evaluate an admitted callee body in the caller's builder behind a sink fence.
    ///
    /// The body runs through the SAME statement processor a unit build uses — so builder
    /// loops, reductions, guarded returns, and every canonicalization behave exactly as
    /// they would for the hand-inlined form — with three differences: returns are
    /// captured (with their callee-relative path guard) instead of emitted as sinks; the
    /// loop machinery state is swapped out so the callee cannot interact with a caller
    /// loop in progress; and afterwards the fence checks that the evaluation emitted no
    /// sink at all. Any sink (an ordered effect, a throw, a break, a while-loop guard) or
    /// poison (in-loop return, exact field write) means the body's behavior is more than
    /// its value — the attempt rolls back (truncating only state the fence owns; created
    /// value nodes are unreachable and never enter the reachability-filtered fingerprint
    /// or anchors) and the call stays opaque.
    fn inline_eval_pure_body(
        &mut self,
        body: NodeId,
        fenv: &mut FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        let sink_base = self.sinks.len();
        let effect_slot_base = self.effect_slot;
        let path_base = self.path.len();
        let facts_base = self.bound_order_facts.len();
        let contracts_base = self.contracts.len();
        let building_saved = std::mem::take(&mut self.building);
        let building_kind_saved = std::mem::take(&mut self.building_kind);
        let loop_recurrence_saved = self.loop_recurrence.take();
        self.inline_capture.push(InlineCaptureFrame {
            path_base,
            loop_depth_base: self.loop_depth,
            poisoned: false,
            returns: Vec::new(),
        });

        self.inline_process_body(body, fenv);

        let frame = self
            .inline_capture
            .pop()
            .expect("inline capture frame is balanced");
        self.building = building_saved;
        self.building_kind = building_kind_saved;
        self.loop_recurrence = loop_recurrence_saved;
        self.path.truncate(path_base);
        self.bound_order_facts.truncate(facts_base);

        // `Cond` sinks are loop iteration mechanics the hand-inlined form of this body
        // would emit identically — they are KEPT (that is fingerprint parity, not a
        // dropped effect). Anything else (an ordered effect, a throw, a break) is
        // behavior beyond the call's value, so the attempt fails closed.
        let foreign_sink = self.sinks[sink_base..]
            .iter()
            .any(|s| !matches!(s.kind, SinkKind::Cond));
        let clean = !frame.poisoned && !foreign_sink;
        let value = if clean {
            self.fold_captured_returns(&frame.returns)
        } else {
            None
        };
        if value.is_none() {
            // Roll back what the fence owns; interned nodes stay but are unreachable.
            self.sinks.truncate(sink_base);
            self.effect_slot = effect_slot_base;
            self.contracts.truncate(contracts_base);
        }
        value
    }

    /// Process the callee body statements, stopping at the first *unconditional*
    /// captured return (later statements are dead — and evaluating them could trip the
    /// fence on effects that can never execute).
    fn inline_process_body(&mut self, body: NodeId, env: &mut FxHashMap<u32, ValueId>) {
        if self.il.kind(body) != NodeKind::Block {
            self.process_stmt(body, env);
            return;
        }
        let kids = self.il.children(body).to_vec();
        for &stmt in &kids {
            self.process_stmt(stmt, env);
            if self.inline_returned_unconditionally() {
                return;
            }
        }
    }

    fn inline_returned_unconditionally(&self) -> bool {
        self.inline_capture
            .last()
            .is_some_and(|f| f.returns.last().is_some_and(|&(g, _)| g.is_none()))
    }

    /// Fold captured `(guard, value)` returns — first match wins — into one value:
    /// `[(g1,v1), …, (None,vn)]` becomes `Phi(g1, v1, Phi(…, vn))`. A guarded FINAL
    /// return is accepted only as the exact complement of the one before it (the
    /// exhaustive `if c return a; else return b` shape); anything else fails closed.
    fn fold_captured_returns(&mut self, returns: &[(Option<ValueId>, ValueId)]) -> Option<ValueId> {
        let (&(last_guard, last_value), prefix) = returns.split_last()?;
        let (mut acc, prefix) = match last_guard {
            None => (last_value, prefix),
            Some(lg) => {
                let (&(prev_guard, prev_value), rest) = prefix.split_last()?;
                let pg = prev_guard?;
                if !self.complementary_conds(pg, lg) {
                    return None;
                }
                (self.mk(ValOp::Phi, vec![pg, prev_value, last_value]), rest)
            }
        };
        for &(guard, value) in prefix.iter().rev() {
            let g = guard?;
            acc = self.mk(ValOp::Phi, vec![g, value, acc]);
        }
        Some(acc)
    }

    fn complementary_conds(&self, a: ValueId, b: ValueId) -> bool {
        let not_of = |x: ValueId, y: ValueId| {
            matches!(self.nodes[x as usize].op, ValOp::Un(o) if o == Op::Not as u32)
                && self.nodes[x as usize].args == [y]
        };
        not_of(a, b) || not_of(b, a)
    }

    fn inline_target_for_call(&self, call: NodeId) -> Option<(NodeId, InlineFunction)> {
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
            found = Some((candidate.root, candidate.function.clone()));
        }
        found
    }

    /// The straight-line body-safety walk retained for content-keyed identity seeding
    /// parity checks: free names are recorded in `required` and assumed available; the
    /// caller re-checks them against the consuming unit's `global_env`.
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
                    | Payload::Lit(LitClass::Null)
            ),
            _ => self
                .il
                .children(node)
                .iter()
                .all(|&c| self.function_binding_safe_collect(root, c, required)),
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
