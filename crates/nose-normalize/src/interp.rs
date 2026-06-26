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
mod eval;
mod exec;
mod field_state;
mod hof;
mod ops;
mod value;
use ops::*;
pub use value::{behavior_equiv, behavior_has_sym, Behavior, Value, F64};
use value::{coerce_to_declared_domain, contains_sym, hashed, vhash, FieldKey, FieldPlace};

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

/// Marker: the unit hit a construct the interpreter does not model. The whole unit is
/// then excluded from the soundness check (we never guess at behavior).
struct Unsupported;

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
    run_unit_once(il, interner, root, args, callable_roots(il), None).0
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
    let roots = callable_roots(il);
    let mut out = Vec::new();
    let mut prescribed: Vec<bool> = Vec::new();
    loop {
        let explore = Explore {
            prescribed: prescribed.clone(),
            ..Explore::default()
        };
        let (beh, ex) = run_unit_once(il, interner, root, args, roots.clone(), Some(explore));
        let ex = ex.expect("explore state survives the run");
        if ex.cap_hit {
            *path_cap = true;
            return None;
        }
        out.push(beh?);
        // Advance depth-first: flip the deepest `true` decision to `false`,
        // dropping exhausted (`false`) tails — a binary counter over sites.
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
    Some(out)
}

fn run_unit_once(
    il: &Il,
    interner: &Interner,
    root: NodeId,
    args: &[Value],
    callable_roots: Vec<NodeId>,
    explore: Option<Explore>,
) -> (Option<Behavior>, Option<Explore>) {
    if il.kind(root) != NodeKind::Func {
        return (None, explore);
    }
    let mut it = Interp {
        il,
        interner,
        steps: 0,
        effects: Vec::new(),
        fields: FxHashMap::default(),
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
                env.insert(c, v);
                it.params.insert(c);
                pi += 1;
            }
        }
    }
    let Some(&body) = kids.last() else {
        return (None, it.explore);
    };
    let ret = match it.exec(body, &mut env) {
        Ok(Flow::Ret(v)) => v,
        Ok(Flow::Err) => Value::Err,
        Ok(_) => Value::Null,
        Err(_) => return (None, it.explore),
    };
    let mut fields: Vec<(FieldKey, Value)> = it.fields.into_iter().collect();
    fields.sort_by(|(left, _), (right, _)| left.cmp(right));
    let behavior = Behavior {
        ret,
        effects: it.effects,
        fields,
    };
    (Some(behavior), it.explore)
}

impl<'a> Interp<'a> {
    fn tick(&mut self) -> R<()> {
        self.steps += 1;
        if self.steps > STEP_BUDGET {
            Err(Unsupported)
        } else {
            Ok(())
        }
    }

    /// Truthiness of an If/ternary condition. A concrete value decides as usual.
    /// A symbolic condition bails in strict mode; under #244 exploration it takes
    /// the prescribed arm (or assumes `true` at a new site, depth-first) and
    /// RECORDS the assumption as a `Sym` effect marker — so the decision is
    /// conditioned, never guessed, and any cross-unit disagreement involving an
    /// explored path stays in the advisory lane (the marker keeps the behavior
    /// symbolic). Loop conditions deliberately stay strict: an assumption per
    /// iteration is an unbounded chain, not a bounded fork.
    fn cond_truthy(&mut self, v: &Value) -> R<bool> {
        if let Some(t) = truthy(v) {
            return Ok(t);
        }
        let Value::Sym(h) = v else {
            return Err(Unsupported);
        };
        let h = *h;
        let Some(ex) = self.explore.as_mut() else {
            return Err(Unsupported);
        };
        if ex.taken.len() >= MAX_SYM_BRANCH_SITES {
            ex.cap_hit = true;
            return Err(Unsupported);
        }
        let taken = ex.prescribed.get(ex.taken.len()).copied().unwrap_or(true);
        ex.taken.push(taken);
        self.effects
            .push(Value::Sym(sym_id(SYM_ASSUME, &[h, u64::from(taken)])));
        Ok(taken)
    }
}

#[cfg(test)]
mod tests;
