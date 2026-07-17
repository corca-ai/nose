//! A small interpreter over the normalized IL — the *behavioral oracle* for the
//! value-graph soundness check (§AJ).
//!
//! The value graph claims that two units with the same fingerprint compute the same
//! thing. Nothing verified that until now. This interpreter runs a unit on concrete
//! inputs and returns its observable behavior (the value it returns, plus an effect
//! trace), so a checker can assert: **fingerprint-equal ⟹ behavior-equal on every
//! sampled input** (soundness — no false merges, the cardinal sin of a clone
//! detector). It is intentionally partial: any construct it cannot model (opaque
//! calls, unwritten field access, exception handlers, …) makes the whole unit
//! *uninterpretable*, and the checker excludes it rather than guess. Determinism + a
//! step budget guarantee termination; the exact arithmetic need not match any real
//! language, only be self-consistent — a genuinely-equivalent pair agrees under *any*
//! consistent semantics, so a fingerprint merge the interpreter contradicts is a real
//! bug. A bare `throw`/`raise` is modeled as observable `Err` behavior; exception
//! handlers remain unsupported.
//!
//! proof-obligation: normalize.value_graph.field_writes
//! proof-obligation: normalize.value_graph.free_monoid

use nose_il::{
    stable_symbol_hash, Builtin, HoFKind, Il, Interner, LoopKind, NodeId, NodeKind, Op, Payload,
    Symbol,
};
use nose_semantics::{
    admitted_builtin_semantics_at_call_with_interner, builtin_demand_profile,
    direct_function_call_target_at_call, exact_java_this_field, exact_java_this_var,
    exact_self_field_write_assignment, hof_contract, semantics, BuiltinDemandProfile,
    DemandOperation, EagerBuiltinContract, HofDemandProfile,
};
use rustc_hash::{FxHashMap, FxHashSet};
use std::hash::{Hash, Hasher};

mod calls;
mod control;
mod eval;
mod exec;
mod field_state;
mod hof;
mod ops;
mod value;
use ops::*;
pub use value::{behavior_equiv, behavior_has_sym, Behavior, Value, F64};
use value::{
    coerce_to_declared_domain, compact_javascript_positive_zero, contains_sym, hashed, vhash,
    FieldKey, FieldPlace,
};

/// Stable structural signature of an IL subtree: pre-order over (kind, payload,
/// child count), with `Name` symbols resolved through the interner so the signature
/// does not depend on interner-local symbol ids. Used as the identity of an opaque
/// callee/operation. Cids are alpha-renamed in declaration order, so fingerprint-equal
/// units assign matching cids and their opaque signatures stay comparable.
fn subtree_sig(il: &Il, interner: &Interner, root: NodeId) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = rustc_hash::FxHasher::default();
    let mut stack = vec![root];
    while let Some(x) = stack.pop() {
        let n = il.node(x);
        n.kind.hash(&mut h);
        match n.payload {
            Payload::Name(s) => {
                0xF00Du64.hash(&mut h);
                interner.resolve(s).hash(&mut h);
            }
            p => p.hash(&mut h),
        }
        let kids = il.children(x);
        kids.len().hash(&mut h);
        stack.extend(kids.iter().rev().copied());
    }
    h.finish()
}

/// Fold a tagged sequence of operand hashes into one symbolic identity.
fn sym_id(tag: u64, parts: &[u64]) -> u64 {
    let mut h = rustc_hash::FxHasher::default();
    tag.hash(&mut h);
    for p in parts {
        p.hash(&mut h);
    }
    h.finish()
}

/// Stable diagnostics for the first capability that kept a unit out of the
/// behavioral oracle. These records are reporting-only: the public interpreter
/// entry points still fail closed with `None` exactly as before.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct InterpreterBlocker {
    pub category: &'static str,
    pub capability_id: &'static str,
    pub blocker_stack: Vec<InterpreterBlockerFrame>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct InterpreterBlockerFrame {
    pub role: &'static str,
    pub construct: String,
}

/// Marker: the unit hit a construct the interpreter does not model. The whole unit is
/// then excluded from the soundness check (we never guess at behavior).
struct Unsupported {
    category: &'static str,
    capability_id: &'static str,
    blocker_stack: Vec<InterpreterBlockerFrame>,
}

impl Unsupported {
    fn new(category: &'static str, capability_id: &'static str) -> Self {
        Self {
            category,
            capability_id,
            blocker_stack: Vec::new(),
        }
    }

    fn il(capability_id: &'static str) -> Self {
        Self::new("il", capability_id)
    }

    fn protocol(capability_id: &'static str) -> Self {
        Self::new("protocol", capability_id)
    }

    fn value(capability_id: &'static str) -> Self {
        Self::new("value", capability_id)
    }

    fn budget(capability_id: &'static str) -> Self {
        Self::new("budget", capability_id)
    }

    fn with_frame(mut self, il: &Il, node: NodeId, role: &'static str) -> Self {
        self.blocker_stack.push(InterpreterBlockerFrame {
            role,
            construct: blocker_construct(il, node),
        });
        self
    }

    fn into_report(self) -> InterpreterBlocker {
        InterpreterBlocker {
            category: self.category,
            capability_id: self.capability_id,
            blocker_stack: self.blocker_stack,
        }
    }
}

fn blocker_construct(il: &Il, node: NodeId) -> String {
    match il.node(node).payload {
        Payload::Builtin(builtin) => format!("builtin:{builtin:?}"),
        Payload::HoF(kind) => format!("hof:{kind:?}"),
        Payload::Loop(kind) => format!("loop:{kind:?}"),
        Payload::Op(op) => format!("op:{op:?}"),
        Payload::Lit(class) => format!("literal:{class:?}"),
        _ => format!("kind:{:?}", il.kind(node)),
    }
}

type R<T> = Result<T, Unsupported>;

enum Flow {
    Normal,
    Ret(Value),
    Break,
    Continue,
    /// A type error in a CONDITION (an `Err` value used as an if/loop/ternary test). It
    /// propagates as `Err` behavior rather than being silently treated as false — so a
    /// lenient manual form (a `x>0?x:-x` abs, an accumulator loop) ERRS on a
    /// type-mismatched input exactly as the strict builtin it canonicalizes to (`abs`,
    /// `sum`) does. Without this the two diverged on non-numeric battery inputs (the
    /// manual form returned a value / its init while the builtin returned `Err`),
    /// surfacing as a false merge the value graph correctly unified.
    Err,
}

/// Terminal control channel of one interpreted unit execution.
///
/// Ordinary whole-function verification intentionally compares only [`Behavior`], preserving
/// the established convention that an implicit null/void fallthrough and an explicit matching
/// return are equivalent. Exact sub-function fragments additionally observe this channel: an
/// early return from the enclosing function is not the same as falling through the fragment.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum UnitExit {
    Fallthrough,
    Return,
    Error,
}

const STEP_BUDGET: u64 = 200_000;

/// Symbolic-condition path exploration cap (#244): at most this many symbolic
/// If/ternary decision SITES per execution, so a row explores ≤ 2^cap paths.
/// Beyond it the unit fails closed (path-bail), never guessed.
pub const MAX_SYM_BRANCH_SITES: usize = 3;

/// Effect-trace marker for an assumed symbolic condition: `Sym(assume ⊕ cond ⊕ arm)`.
/// Because the marker is symbolic, every path-explored behavior carries `Sym`, which
/// routes any cross-unit disagreement to verify's ADVISORY lane by construction —
/// path exploration can never create a hard SOUND violation.
const SYM_ASSUME: u64 = 0xA55E_0011;

/// State for bounded symbolic-condition path exploration. `prescribed` replays
/// decisions for sites already enumerated (depth-first, true-arm first); past its
/// end a new site assumes `true` and appends to `taken`.
#[derive(Default)]
struct Explore {
    prescribed: Vec<bool>,
    taken: Vec<bool>,
    cap_hit: bool,
}

struct Interp<'a> {
    il: &'a Il,
    interner: &'a Interner,
    steps: u64,
    effects: Vec<Value>,
    fields: FxHashMap<FieldKey, Value>,
    /// Direct immutable module strings, proven by the shared module scope/mutation boundary.
    globals: FxHashMap<Symbol, Value>,
    /// Parameter cids — appending to one is a caller-visible mutation (an effect); appending
    /// to a LOCAL list var builds that list's value (faithful, converges with a comprehension).
    params: FxHashSet<u32>,
    /// In-file function/method roots that the oracle may execute, but only when a `CallTarget`
    /// evidence record admits the exact call occurrence. This lets the oracle interpret proven
    /// recursive and interprocedural calls without treating raw callee spelling as proof.
    callable_roots: Vec<NodeId>,
    /// `None` = strict (a symbolic condition bails the unit — `run_unit`'s contract,
    /// kept for canon validation and the fragment oracle). `Some` = #244 bounded
    /// two-arm exploration with each assumption recorded in the effect trace.
    explore: Option<Explore>,
}

fn callable_roots(il: &Il) -> Vec<NodeId> {
    il.units
        .iter()
        .filter(|u| {
            matches!(
                u.kind,
                nose_il::UnitKind::Function | nose_il::UnitKind::Method
            )
        })
        .map(|u| u.root)
        .collect()
}

/// Run the `Func` unit at `root` with `args` bound to its parameters (in order).
/// Returns its [`Behavior`], or `None` if the unit is uninterpretable. A symbolic
/// branch condition bails (strict contract; see [`run_unit_paths`] for the
/// exploring variant).
pub fn run_unit(il: &Il, interner: &Interner, root: NodeId, args: &[Value]) -> Option<Behavior> {
    PreparedInterpreter::new(il, interner, true).run(root, args)
}

/// Strict single-path execution with its terminal control channel retained.
///
/// This is the fragment-oracle counterpart to [`run_unit`]. It does not widen interpreter
/// support or guess at control: unsupported and symbolic conditions still return `None`.
pub fn run_unit_observing_exit(
    il: &Il,
    interner: &Interner,
    root: NodeId,
    args: &[Value],
) -> Option<(Behavior, UnitExit)> {
    PreparedInterpreter::new(il, interner, true).run_observing_exit(root, args)
}

/// Immutable interpreter facts prepared once for repeated executions of one IL.
///
/// Behavioral verification runs every unit against a battery of rows. Callable roots and
/// module-string bindings depend only on the IL, so rebuilding them for every row adds cost
/// without changing semantics.
pub struct PreparedInterpreter<'a> {
    il: &'a Il,
    interner: &'a Interner,
    callable_roots: Vec<NodeId>,
    globals: FxHashMap<Symbol, Value>,
}

impl<'a> PreparedInterpreter<'a> {
    pub fn new(il: &'a Il, interner: &'a Interner, include_immutable_module_strings: bool) -> Self {
        Self {
            il,
            interner,
            callable_roots: callable_roots(il),
            globals: immutable_module_string_globals(
                il,
                interner,
                include_immutable_module_strings,
            ),
        }
    }

    pub fn run(&self, root: NodeId, args: &[Value]) -> Option<Behavior> {
        self.run_observing_exit(root, args)
            .map(|(behavior, _)| behavior)
    }

    pub fn run_observing_exit(&self, root: NodeId, args: &[Value]) -> Option<(Behavior, UnitExit)> {
        run_unit_once(
            self.il,
            self.interner,
            root,
            args,
            self.callable_roots.clone(),
            self.globals.clone(),
            None,
        )
        .0
        .ok()
    }

    pub fn run_paths_diagnostic(
        &self,
        root: NodeId,
        args: &[Value],
    ) -> Result<Vec<Behavior>, InterpreterBlocker> {
        self.run_paths_observing_exit_diagnostic(root, args)
            .map(|paths| paths.into_iter().map(|(behavior, _)| behavior).collect())
    }

    pub fn run_paths_observing_exit_diagnostic(
        &self,
        root: NodeId,
        args: &[Value],
    ) -> Result<Vec<(Behavior, UnitExit)>, InterpreterBlocker> {
        let mut out = Vec::new();
        let mut prescribed: Vec<bool> = Vec::new();
        loop {
            let explore = Explore {
                prescribed: prescribed.clone(),
                ..Explore::default()
            };
            let (result, ex) = run_unit_once(
                self.il,
                self.interner,
                root,
                args,
                self.callable_roots.clone(),
                self.globals.clone(),
                Some(explore),
            );
            let ex = ex.expect("explore state survives the run");
            out.push(result.map_err(Unsupported::into_report)?);
            let mut next = ex.taken;
            while next.last() == Some(&false) {
                next.pop();
            }
            match next.last_mut() {
                Some(last) => *last = false,
                None => break,
            }
            prescribed = next;
        }
        Ok(out)
    }
}

/// Every behavior of the unit on `args`, one per explored symbolic-condition path
/// (deterministic depth-first order, true-arm first; a unit with no symbolic
/// conditions yields exactly one). Each path's effect trace records its assumed
/// conditions as `Sym` markers, so two units compare equal only when their
/// assumptions AND outcomes align. Returns `None` when any path is
/// uninterpretable or the per-execution symbolic-site cap is exceeded
/// (fail-closed); `path_cap` reports the cap case for the exclusion census.
pub fn run_unit_paths(
    il: &Il,
    interner: &Interner,
    root: NodeId,
    args: &[Value],
    path_cap: &mut bool,
) -> Option<Vec<Behavior>> {
    match run_unit_paths_diagnostic(il, interner, root, args) {
        Ok(behaviors) => Some(behaviors),
        Err(blocker) => {
            if blocker.capability_id == "budget.symbolic-branch-sites" {
                *path_cap = true;
            }
            None
        }
    }
}

/// Diagnostic twin of [`run_unit_paths`]. It follows the same execution order
/// and fail-closed rules, but returns the first unsupported capability and its
/// leaf-first execution stack for offline coverage planning.
pub fn run_unit_paths_diagnostic(
    il: &Il,
    interner: &Interner,
    root: NodeId,
    args: &[Value],
) -> Result<Vec<Behavior>, InterpreterBlocker> {
    run_unit_paths_diagnostic_with_module_strings(il, interner, root, args, true)
}

/// Soundness Lab replay variant with explicit immutable-module-string control.
/// Product callers keep the shared module facts enabled; the flag only supports
/// provenance-bound tranche ablations.
pub fn run_unit_paths_diagnostic_with_module_strings(
    il: &Il,
    interner: &Interner,
    root: NodeId,
    args: &[Value],
    include_immutable_module_strings: bool,
) -> Result<Vec<Behavior>, InterpreterBlocker> {
    PreparedInterpreter::new(il, interner, include_immutable_module_strings)
        .run_paths_diagnostic(root, args)
}

/// Diagnostic path-exploring execution with terminal control retained for every path.
///
/// Exact fragments use this to distinguish an enclosing-function return from normal fragment
/// completion while preserving the same deterministic path order and fail-closed budget as
/// [`run_unit_paths_diagnostic`].
pub fn run_unit_paths_observing_exit_diagnostic(
    il: &Il,
    interner: &Interner,
    root: NodeId,
    args: &[Value],
) -> Result<Vec<(Behavior, UnitExit)>, InterpreterBlocker> {
    run_unit_paths_observing_exit_diagnostic_with_module_strings(il, interner, root, args, true)
}

/// Exit-observing Soundness Lab replay variant with explicit immutable-module-string control.
pub fn run_unit_paths_observing_exit_diagnostic_with_module_strings(
    il: &Il,
    interner: &Interner,
    root: NodeId,
    args: &[Value],
    include_immutable_module_strings: bool,
) -> Result<Vec<(Behavior, UnitExit)>, InterpreterBlocker> {
    PreparedInterpreter::new(il, interner, include_immutable_module_strings)
        .run_paths_observing_exit_diagnostic(root, args)
}

fn immutable_module_string_globals(
    il: &Il,
    interner: &Interner,
    include: bool,
) -> FxHashMap<Symbol, Value> {
    if !include {
        return FxHashMap::default();
    }
    crate::module_facts::immutable_module_string_bindings(il, interner)
        .into_iter()
        .map(|binding| (binding.name, Value::Str(vec![binding.literal_hash])))
        .collect()
}

fn run_unit_once(
    il: &Il,
    interner: &Interner,
    root: NodeId,
    args: &[Value],
    callable_roots: Vec<NodeId>,
    globals: FxHashMap<Symbol, Value>,
    explore: Option<Explore>,
) -> (R<(Behavior, UnitExit)>, Option<Explore>) {
    if il.kind(root) != NodeKind::Func {
        return (
            Err(Unsupported::il("il.root-not-function").with_frame(il, root, "root")),
            explore,
        );
    }
    let mut it = Interp {
        il,
        interner,
        steps: 0,
        effects: Vec::new(),
        fields: FxHashMap::default(),
        globals,
        params: FxHashSet::default(),
        callable_roots,
        explore,
    };
    let mut env: FxHashMap<u32, Value> = FxHashMap::default();
    let kids = il.children(root).to_vec();
    let mut pi = 0;
    for &k in &kids {
        if il.kind(k) == NodeKind::Param {
            if let Payload::Cid(c) = il.node(k).payload {
                // Bind under the param's DECLARED domain (the §BE convention:
                // interpret under the same contracts the value graph used to
                // merge). A typed `int` parameter never receives a List at
                // runtime; feeding one explores a type-state the language rules
                // out and flags order-insensitive typed field writes as false
                // merges (#210). Coercion is deterministic in the input value,
                // so equally-declared twins see identical effective rows.
                let raw = args.get(pi).cloned().unwrap_or(Value::Null);
                let v = match nose_semantics::domain_evidence_for_param(il, k) {
                    Some(d) => coerce_to_declared_domain(raw, d),
                    None => raw,
                };
                let v = if it.bitwise_result_is_int32() {
                    compact_javascript_positive_zero(v)
                } else {
                    v
                };
                env.insert(c, v);
                it.params.insert(c);
                pi += 1;
            }
        }
    }
    let Some(&body) = kids.last() else {
        return (
            Err(Unsupported::il("il.function-body-missing").with_frame(il, root, "root")),
            it.explore,
        );
    };
    let (ret, exit) = match it.exec(body, &mut env) {
        Ok(Flow::Ret(v)) => (v, UnitExit::Return),
        Ok(Flow::Err) => (Value::Err, UnitExit::Error),
        Ok(_) => (Value::Null, UnitExit::Fallthrough),
        Err(blocker) => return (Err(blocker), it.explore),
    };
    let mut fields: Vec<(FieldKey, Value)> = it.fields.into_iter().collect();
    fields.sort_by(|(left, _), (right, _)| left.cmp(right));
    let behavior = Behavior {
        ret,
        effects: it.effects,
        fields,
    };
    (Ok((behavior, exit)), it.explore)
}

#[cfg(test)]
mod tests;
