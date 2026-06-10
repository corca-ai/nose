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

use nose_il::{stable_symbol_hash, Builtin, Il, Interner, LoopKind, NodeId, NodeKind, Op, Payload};
use nose_semantics::{
    admitted_builtin_semantics_at_call, builtin_demand_profile,
    direct_function_call_target_at_call, exact_java_this_field, exact_java_this_var,
    exact_self_field_write_assignment, hof_contract, BuiltinDemandProfile, DemandOperation,
    EagerBuiltinContract, HofDemandProfile,
};
use rustc_hash::{FxHashMap, FxHashSet};

/// A runtime value. `List` is nested so `zip`/`enumerate` can yield pairs.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Value {
    Int(i64),
    Bool(bool),
    /// A string/builder value modeled as the FREE MONOID over its appended pieces: an
    /// ordered sequence of opaque token hashes. A literal is one token; `+`/concat
    /// appends (associative, identity = empty), and is ORDER-SENSITIVE — so `s + x` and
    /// `x + s` differ, exactly as string concatenation does (this is what makes the
    /// builder/join family interpretable and exposes any unsound commutative treatment of
    /// `+` on strings). No real character content is needed — the ordered pieces capture
    /// append behavior. (Char-level ops like length/index stay `Err`: unknown from pieces.)
    Str(Vec<u64>),
    List(Vec<Value>),
    Null,
    /// A runtime error (type mismatch, out-of-range, divide-by-zero). This is itself
    /// observable behavior — two equivalent programs err on the same inputs.
    Err,
    /// A SYMBOLIC value: the result of an operation the interpreter cannot execute —
    /// an opaque (unproven/unadmitted) call, an unproven field read, or any
    /// composition over such a value — identified by a stable structural hash of the
    /// operation and its operand values. This is a *differential convention*, not a
    /// semantics claim: two runs produce the same `Sym` iff they performed the same
    /// opaque operation on equal operands, in the same observable order (opaque calls
    /// are also recorded in the effect trace). Control flow is never guessed:
    /// branching on a `Sym` (truthiness, loop bounds, iteration) still bails the
    /// unit. Because symbolic identity is keyed on pre-canonicalization syntax, a
    /// behavior containing a `Sym` must never feed the hard SOUND gate — the verify
    /// report routes Sym-bearing disagreements to a separate advisory lane.
    Sym(u64),
}

/// Stable hash of a runtime value (deterministic: `FxHasher` carries no random
/// state), used to compose symbolic identities.
fn vhash(v: &Value) -> u64 {
    hashed(v)
}

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
    use std::hash::{Hash, Hasher};
    let mut h = rustc_hash::FxHasher::default();
    tag.hash(&mut h);
    for p in parts {
        p.hash(&mut h);
    }
    h.finish()
}

/// Stable hash of any hashable tag (operator, builtin, contract).
fn hashed<T: std::hash::Hash>(t: &T) -> u64 {
    use std::hash::Hasher;
    let mut h = rustc_hash::FxHasher::default();
    t.hash(&mut h);
    h.finish()
}

/// Does this behavior carry any symbolic value? Sym-bearing behaviors are
/// comparable under the differential convention, but a disagreement involving one
/// must never feed the hard SOUND gate: symbolic identity is keyed on pre-canon
/// syntax, so a proof-backed canonicalization (e.g. AC ordering) can legitimately
/// make two equivalent units' symbolic traces differ.
pub fn behavior_has_sym(b: &Behavior) -> bool {
    contains_sym(&b.ret)
        || b.effects.iter().any(contains_sym)
        || b.fields.iter().any(|(_, v)| contains_sym(v))
}

/// Does the value contain a `Sym` anywhere (including inside lists)? Concrete
/// operations must never run over a hidden symbolic operand — `sum([f(x)])`
/// collapsing to a concrete `Err` would launder unknownness into the hard
/// soundness lane. Every composition guard uses this DEEP check.
fn contains_sym(v: &Value) -> bool {
    match v {
        Value::Sym(_) => true,
        Value::List(xs) => xs.iter().any(contains_sym),
        _ => false,
    }
}

/// A receiver identity proven by the IL shape during interpretation.
#[derive(Clone, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub enum FieldPlace {
    SelfReceiver,
}

/// A concrete final field-state slot: receiver identity plus field name.
#[derive(Clone, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub struct FieldKey {
    pub receiver: FieldPlace,
    pub field: u64,
}

/// The observable behavior of one run: the returned value, an ordered I/O effect trace
/// (appended/printed values, in order — order IS observable), and the final per-place
/// object state (`this.x = ...`) as a receiver+name→value map in canonical place order.
/// Field state is order-INSENSITIVE across distinct places but reflects
/// last-write-wins per receiver+field. Two units are behaviorally equal on an input iff
/// all three components match.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Behavior {
    pub ret: Value,
    pub effects: Vec<Value>,
    pub fields: Vec<(FieldKey, Value)>,
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
}

/// Run the `Func` unit at `root` with `args` bound to its parameters (in order).
/// Returns its [`Behavior`], or `None` if the unit is uninterpretable.
pub fn run_unit(il: &Il, interner: &Interner, root: NodeId, args: &[Value]) -> Option<Behavior> {
    if il.kind(root) != NodeKind::Func {
        return None;
    }
    let callable_roots = il
        .units
        .iter()
        .filter(|u| {
            matches!(
                u.kind,
                nose_il::UnitKind::Function | nose_il::UnitKind::Method
            )
        })
        .map(|u| u.root)
        .collect();
    let mut it = Interp {
        il,
        interner,
        steps: 0,
        effects: Vec::new(),
        fields: FxHashMap::default(),
        params: FxHashSet::default(),
        callable_roots,
    };
    let mut env: FxHashMap<u32, Value> = FxHashMap::default();
    let kids = il.children(root).to_vec();
    let mut pi = 0;
    for &k in &kids {
        if il.kind(k) == NodeKind::Param {
            if let Payload::Cid(c) = il.node(k).payload {
                env.insert(c, args.get(pi).cloned().unwrap_or(Value::Null));
                it.params.insert(c);
                pi += 1;
            }
        }
    }
    let body = *kids.last()?;
    let ret = match it.exec(body, &mut env) {
        Ok(Flow::Ret(v)) => v,
        Ok(Flow::Err) => Value::Err,
        Ok(_) => Value::Null,
        Err(_) => return None,
    };
    let mut fields: Vec<(FieldKey, Value)> = it.fields.into_iter().collect();
    fields.sort_by(|(left, _), (right, _)| left.cmp(right));
    Some(Behavior {
        ret,
        effects: it.effects,
        fields,
    })
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

    fn exact_field_place(&self, node: NodeId) -> Option<FieldPlace> {
        if self.il.kind(node) == NodeKind::Var && exact_java_this_var(self.il, self.interner, node)
        {
            Some(FieldPlace::SelfReceiver)
        } else {
            None
        }
    }

    fn exact_field_write_key(&self, assign: NodeId, target: NodeId) -> Option<FieldKey> {
        if !exact_self_field_write_assignment(self.il, self.interner, assign) {
            return None;
        }
        self.exact_field_key(target)
    }

    fn exact_field_key(&self, target: NodeId) -> Option<FieldKey> {
        if self.il.kind(target) != NodeKind::Field {
            return None;
        }
        if !exact_java_this_field(self.il, self.interner, target) {
            return None;
        }
        let Payload::Name(field) = self.il.node(target).payload else {
            return None;
        };
        let receiver = self.il.children(target).first().copied()?;
        let receiver = self.exact_field_place(receiver)?;
        Some(FieldKey {
            receiver,
            field: stable_symbol_hash(self.interner.resolve(field)),
        })
    }

    fn field_receiver_errored(
        &mut self,
        receiver: NodeId,
        env: &mut FxHashMap<u32, Value>,
    ) -> R<bool> {
        if exact_java_this_var(self.il, self.interner, receiver) {
            return Ok(false);
        }
        Ok(matches!(self.eval(receiver, env)?, Value::Err))
    }

    /// Execute a statement (or block), threading control flow.
    fn exec(&mut self, node: NodeId, env: &mut FxHashMap<u32, Value>) -> R<Flow> {
        self.tick()?;
        match self.il.kind(node) {
            NodeKind::Block => {
                for s in self.il.children(node).to_vec() {
                    match self.exec(s, env)? {
                        Flow::Normal => {}
                        other => return Ok(other),
                    }
                }
                Ok(Flow::Normal)
            }
            NodeKind::Assign => {
                let kids = self.il.children(node).to_vec();
                if kids.len() != 2 {
                    return Err(Unsupported);
                }
                let rhs = self.eval(kids[1], env)?;
                if matches!(rhs, Value::Err) {
                    return Ok(Flow::Err);
                }
                if self.bind(kids[0], rhs, env, Some(node))? {
                    return Ok(Flow::Err);
                }
                Ok(Flow::Normal)
            }
            NodeKind::ExprStmt => {
                if let Some(&e) = self.il.children(node).first() {
                    if let Some(flow) = self.exec_stmt_append(e, env)? {
                        return Ok(flow);
                    }
                    if matches!(self.eval(e, env)?, Value::Err) {
                        return Ok(Flow::Err);
                    }
                }
                Ok(Flow::Normal)
            }
            NodeKind::Return => {
                let v = match self.il.children(node).first() {
                    Some(&e) => self.eval(e, env)?,
                    None => Value::Null,
                };
                if matches!(v, Value::Err) {
                    return Ok(Flow::Err);
                }
                Ok(Flow::Ret(v))
            }
            NodeKind::Throw => {
                if let Some(&e) = self.il.children(node).first() {
                    self.eval(e, env)?;
                }
                Ok(Flow::Err)
            }
            NodeKind::If => {
                let kids = self.il.children(node).to_vec();
                if kids.is_empty() {
                    return Ok(Flow::Normal);
                }
                let cond = self.eval(kids[0], env)?;
                if matches!(cond, Value::Err) {
                    return Ok(Flow::Err);
                }
                if truthy(&cond).ok_or(Unsupported)? {
                    if let Some(&t) = kids.get(1) {
                        return self.exec(t, env);
                    }
                } else if let Some(&e) = kids.get(2) {
                    return self.exec(e, env);
                }
                Ok(Flow::Normal)
            }
            NodeKind::Loop => self.exec_loop(node, env),
            NodeKind::Try => self.exec_try(node, env),
            NodeKind::Break => Ok(Flow::Break),
            NodeKind::Continue => Ok(Flow::Continue),
            // Empty block / no-op pass lowers to an empty Block (handled above) or a
            // Seq with no children; anything else as a statement we don't model.
            NodeKind::Seq if self.il.children(node).is_empty() => Ok(Flow::Normal),
            _ => Err(Unsupported),
        }
    }

    fn exec_loop(&mut self, node: NodeId, env: &mut FxHashMap<u32, Value>) -> R<Flow> {
        let kind = match self.il.node(node).payload {
            Payload::Loop(k) => k,
            _ => LoopKind::While,
        };
        let kids = self.il.children(node).to_vec();
        match kind {
            LoopKind::While if kids.len() == 2 => {
                loop {
                    self.tick()?;
                    let c = self.eval(kids[0], env)?;
                    if matches!(c, Value::Err) {
                        return Ok(Flow::Err); // type error in the loop test → Err behavior
                    }
                    if !truthy(&c).ok_or(Unsupported)? {
                        break;
                    }
                    match self.exec(kids[1], env)? {
                        Flow::Normal | Flow::Continue => {}
                        Flow::Break => break,
                        other => return Ok(other), // Ret / Err propagate
                    }
                }
                Ok(Flow::Normal)
            }
            LoopKind::ForEach if kids.len() == 3 => {
                let seq = match self.eval(kids[1], env)? {
                    Value::List(xs) => xs,
                    Value::Err => return Ok(Flow::Err),
                    // Iterating a non-iterable (int/bool/null/string) is a runtime TYPE ERROR in
                    // every modeled language (Python/JS `TypeError`, …), not an unmodelable
                    // construct — so it is `Err` behavior, NOT `Unsupported`. This keeps a
                    // foreach-accumulator (the headline cross-language Type-4 pattern) interpretable
                    // even on the battery's scalar rows: it `Err`s there and computes on the list
                    // rows, so the oracle CHECKS it instead of excluding the whole unit.
                    _ => return Ok(Flow::Err),
                };
                for item in seq {
                    self.tick()?;
                    if self.bind(kids[0], item, env, None)? {
                        return Ok(Flow::Err);
                    }
                    match self.exec(kids[2], env)? {
                        Flow::Normal | Flow::Continue => {}
                        Flow::Break => break,
                        other => return Ok(other), // Ret / Err propagate
                    }
                }
                Ok(Flow::Normal)
            }
            LoopKind::CStyle if kids.len() == 4 => {
                // [init, cond, update, body] — desugar normally rewrites this away.
                match self.exec(kids[0], env)? {
                    Flow::Normal => {}
                    other => return Ok(other),
                }
                loop {
                    self.tick()?;
                    let c = self.eval(kids[1], env)?;
                    if matches!(c, Value::Err) {
                        return Ok(Flow::Err);
                    }
                    if !truthy(&c).ok_or(Unsupported)? {
                        break;
                    }
                    match self.exec(kids[3], env)? {
                        Flow::Normal | Flow::Continue => {}
                        Flow::Break => break,
                        other => return Ok(other), // Ret / Err propagate
                    }
                    match self.exec(kids[2], env)? {
                        Flow::Normal => {}
                        other => return Ok(other), // Ret / Break / Continue / Err propagate
                    }
                }
                Ok(Flow::Normal)
            }
            _ => Err(Unsupported),
        }
    }

    fn exec_try(&mut self, node: NodeId, env: &mut FxHashMap<u32, Value>) -> R<Flow> {
        let kids = self.il.children(node).to_vec();
        if kids.len() != 2 || self.il.children(kids[1]).is_empty() {
            return Err(Unsupported);
        }
        match self.exec(kids[0], env)? {
            Flow::Err => self.exec(kids[1], env),
            other => Ok(other),
        }
    }

    /// Bind a target (Var / tuple `Seq` / `Index` store) to a value.
    /// Returns true when evaluating the target itself raised a runtime `Err`.
    fn bind(
        &mut self,
        target: NodeId,
        val: Value,
        env: &mut FxHashMap<u32, Value>,
        assignment: Option<NodeId>,
    ) -> R<bool> {
        match self.il.kind(target) {
            NodeKind::Var => {
                if let Payload::Cid(c) = self.il.node(target).payload {
                    env.insert(c, val);
                    Ok(false)
                } else {
                    Err(Unsupported)
                }
            }
            NodeKind::Seq => {
                // tuple unpack: `a, b = pair`
                let names = self.il.children(target).to_vec();
                let vals = match val {
                    Value::List(vs) if vs.len() == names.len() => vs,
                    _ => return Err(Unsupported),
                };
                for (t, v) in names.into_iter().zip(vals) {
                    if self.bind(t, v, env, None)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            // A field store updates per-place object state (last-write-wins), keyed by
            // receiver identity plus field name. Writing distinct receiver+field places
            // commutes; same-place overwrites keep the last value.
            NodeKind::Field => {
                let Some(&receiver) = self.il.children(target).first() else {
                    return Err(Unsupported);
                };
                if self.field_receiver_errored(receiver, env)? {
                    return Ok(true);
                }
                if let Some(assign) = assignment {
                    let Some(key) = self.exact_field_write_key(assign, target) else {
                        return Err(Unsupported);
                    };
                    self.fields.insert(key, val);
                    Ok(false)
                } else {
                    Err(Unsupported)
                }
            }
            NodeKind::Index => {
                let kids = self.il.children(target).to_vec();
                if let Some(&base) = kids.first() {
                    let base_value = self.eval(base, env)?;
                    if matches!(base_value, Value::Err) {
                        return Ok(true);
                    }
                }
                if let Some(&ix) = kids.get(1) {
                    let iv = self.eval(ix, env)?;
                    if matches!(iv, Value::Err) {
                        return Ok(true);
                    }
                    self.effects.push(iv);
                }
                self.effects.push(val);
                Ok(false)
            }
            _ => Err(Unsupported),
        }
    }

    fn eval(&mut self, node: NodeId, env: &mut FxHashMap<u32, Value>) -> R<Value> {
        self.tick()?;
        let n = *self.il.node(node);
        match n.kind {
            NodeKind::Var => match n.payload {
                Payload::Cid(c) => env.get(&c).cloned().ok_or(Unsupported),
                _ => Err(Unsupported),
            },
            NodeKind::Lit => match n.payload {
                Payload::LitInt(v) => Ok(Value::Int(v)),
                Payload::LitBool(b) => Ok(Value::Bool(b)),
                Payload::LitStr(h) => Ok(Value::Str(vec![h])),
                Payload::Lit(c) => match c {
                    // A bool literal whose value wasn't retained — unknown, can't model.
                    // (Retained bools take the `LitBool` arm above.)
                    nose_il::LitClass::Bool => Err(Unsupported),
                    nose_il::LitClass::Null => Ok(Value::Null),
                    // Non-retained numeric/string literal: value unknown → can't model.
                    _ => Err(Unsupported),
                },
                _ => Err(Unsupported),
            },
            NodeKind::BinOp => self.eval_bin_op(node, n.payload, env),
            NodeKind::UnOp => {
                let kids = self.il.children(node).to_vec();
                let a = self.eval(*kids.first().ok_or(Unsupported)?, env)?;
                Ok(un(op_of(n.payload), &a))
            }
            NodeKind::Index => self.eval_index(node, env),
            NodeKind::Seq => {
                let mut out = Vec::new();
                for c in self.il.children(node).to_vec() {
                    let value = self.eval(c, env)?;
                    if matches!(value, Value::Err) {
                        return Ok(Value::Err);
                    }
                    out.push(value);
                }
                Ok(Value::List(out))
            }
            NodeKind::Field => {
                let Some(&receiver) = self.il.children(node).first() else {
                    return Err(Unsupported);
                };
                // Proven self-field reads keep their concrete store semantics; an
                // UNWRITTEN self-field reads its (symbolic) initial state.
                if let Some(key) = self.exact_field_key(node) {
                    if self.field_receiver_errored(receiver, env)? {
                        return Ok(Value::Err);
                    }
                    return match self.fields.get(&key) {
                        Some(v) => Ok(v.clone()),
                        None => Ok(Value::Sym(sym_id(0x00F1_E1D0, &[key.field]))),
                    };
                }
                // Any other field read is a symbolic projection keyed by the field
                // name and the receiver VALUE (pure-read convention, applied to both
                // sides of a merge alike).
                let Payload::Name(field) = n.payload else {
                    return Err(Unsupported);
                };
                let rv = self.eval(receiver, env)?;
                if matches!(rv, Value::Err) {
                    return Ok(Value::Err);
                }
                Ok(Value::Sym(sym_id(
                    0x00F1_E1D1,
                    &[hashed(&self.interner.resolve(field)), vhash(&rv)],
                )))
            }
            NodeKind::If => {
                // ternary expression
                let kids = self.il.children(node).to_vec();
                if kids.len() < 3 {
                    return Err(Unsupported);
                }
                let c = self.eval(kids[0], env)?;
                // A type error in the test is itself the result (matches the strict
                // builtin a lenient `x>0?x:-x` canonicalizes to — both Err on non-numbers).
                if matches!(c, Value::Err) {
                    return Ok(Value::Err);
                }
                if truthy(&c).ok_or(Unsupported)? {
                    self.eval(kids[1], env)
                } else {
                    self.eval(kids[2], env)
                }
            }
            NodeKind::Call => self.eval_call(node, env),
            NodeKind::HoF => self.eval_hof(node, env),
            _ => Err(Unsupported),
        }
    }

    fn eval_bin_op(
        &mut self,
        node: NodeId,
        payload: Payload,
        env: &mut FxHashMap<u32, Value>,
    ) -> R<Value> {
        let kids = self.il.children(node).to_vec();
        if kids.len() != 2 {
            return Err(Unsupported);
        }
        let op = op_of(payload);
        // SHORT-CIRCUIT `and`/`or` — real Python/JS/Go/C semantics: the right
        // operand is evaluated ONLY when the left doesn't already decide the result,
        // and the operator yields the deciding OPERAND's value (value-and/or), not a
        // coerced bool. So `a or b` ≡ `a if a else b` and `a and b` ≡ `b if a else a`
        // exactly — including laziness (`x or f()` does not run `f()` when `x` is
        // truthy) and Err-propagation only on the evaluated side. (Previously both
        // operands were evaluated eagerly through `bin`, so `5 or (1/0)` wrongly
        // Err'd and a value-or never converged with its ternary — an oracle bug.)
        let a = self.eval(kids[0], env)?;
        if matches!(op, Op::Or) {
            return Ok(
                if matches!(a, Value::Err) || truthy(&a).ok_or(Unsupported)? {
                    a
                } else {
                    self.eval(kids[1], env)?
                },
            );
        }
        if matches!(op, Op::And) {
            return Ok(
                if matches!(a, Value::Err) || !truthy(&a).ok_or(Unsupported)? {
                    a
                } else {
                    self.eval(kids[1], env)?
                },
            );
        }
        if matches!(a, Value::Err) {
            return Ok(Value::Err);
        }
        let b = self.eval(kids[1], env)?;
        Ok(bin(op, &a, &b))
    }

    fn eval_index(&mut self, node: NodeId, env: &mut FxHashMap<u32, Value>) -> R<Value> {
        let kids = self.il.children(node).to_vec();
        if kids.len() != 2 {
            return Err(Unsupported);
        }
        let base = self.eval(kids[0], env)?;
        if matches!(base, Value::Err) {
            return Ok(Value::Err);
        }
        let idx = self.eval(kids[1], env)?;
        if matches!(idx, Value::Err) {
            return Ok(Value::Err);
        }
        if contains_sym(&base) || contains_sym(&idx) {
            return Ok(Value::Sym(sym_id(
                0x1DEF_00D0,
                &[vhash(&base), vhash(&idx)],
            )));
        }
        match (base, idx) {
            (Value::List(xs), Value::Int(i)) => {
                let i = if i < 0 { i + xs.len() as i64 } else { i };
                Ok(xs.get(i as usize).cloned().unwrap_or(Value::Err))
            }
            _ => Ok(Value::Err),
        }
    }

    fn eval_call(&mut self, node: NodeId, env: &mut FxHashMap<u32, Value>) -> R<Value> {
        let b = match self.il.node(node).payload {
            Payload::Builtin(b) => b,
            _ => return self.eval_user_call(node, env), // self-recursion, else opaque
        };
        let kids = self.il.children(node).to_vec();
        if !admitted_builtin_semantics_at_call(self.il, node, b) {
            // Unproven builtin spelling: an opaque, identified operation — not a bail.
            return self.opaque_call(sym_id(0xCA11_B170, &[hashed(&b)]), &kids, env);
        }
        let mut args = Vec::new();
        let profile = builtin_demand_profile(b);
        let demand = profile.demand_effect_profile();
        let eager_contract = match (demand.operation, profile) {
            (DemandOperation::FoldReduction, _) => return self.eval_reduce_call(&kids, env),
            (
                DemandOperation::ShortCircuitQuantifier,
                BuiltinDemandProfile::ShortCircuitQuantifier { all },
            ) => {
                return self.eval_any_all_call(all, &kids, env);
            }
            (DemandOperation::AppendMutation, _) => return self.eval_append(&kids, env),
            (DemandOperation::NullishDefault, _) => {
                return self.eval_value_or_default_call(&kids, env);
            }
            (_, BuiltinDemandProfile::Eager { contract }) => contract,
            // Admitted but unmodeled demand profile: opaque, identified by the builtin.
            _ => return self.opaque_call(sym_id(0xCA11_B170, &[hashed(&b)]), &kids, env),
        };
        for &k in &kids {
            let arg = self.eval(k, env)?;
            if matches!(arg, Value::Err) {
                return Ok(Value::Err);
            }
            args.push(arg);
        }
        self.eval_eager_builtin(eager_contract, args)
    }

    fn eval_eager_builtin(
        &mut self,
        eager_contract: EagerBuiltinContract,
        args: Vec<Value>,
    ) -> R<Value> {
        // A symbolic operand anywhere (including inside a list) makes the result
        // symbolic — the concrete arms below must never see one, or unknownness
        // would launder into a concrete `Err`.
        if args.iter().any(contains_sym) {
            let mut parts = vec![hashed(&format!("{eager_contract:?}"))];
            parts.extend(args.iter().map(vhash));
            return Ok(Value::Sym(sym_id(0x00EA_9E12, &parts)));
        }
        match eager_contract {
            EagerBuiltinContract::Len => match args.first() {
                Some(Value::List(xs)) => Ok(Value::Int(xs.len() as i64)),
                // A string is the free monoid over opaque piece hashes; its character
                // length is unknown (piece count ≠ char count), so `len` stays `Err` —
                // matching the type doc and the `IsEmpty` sibling. Returning a constant
                // `Int(1)` falsely equated `len(any_string)` with the literal `1`.
                _ => Ok(Value::Err),
            },
            EagerBuiltinContract::IsEmpty => match args.first() {
                Some(Value::List(xs)) => Ok(Value::Bool(xs.is_empty())),
                _ => Ok(Value::Err),
            },
            EagerBuiltinContract::IsNull => match args.first() {
                Some(value) => Ok(Value::Bool(matches!(value, Value::Null))),
                _ => Ok(Value::Err),
            },
            EagerBuiltinContract::IsNotNull => match args.first() {
                Some(value) => Ok(Value::Bool(!matches!(value, Value::Null))),
                _ => Ok(Value::Err),
            },
            EagerBuiltinContract::StartsWith => Ok(string_affix(args.first(), args.get(1), true)),
            EagerBuiltinContract::EndsWith => Ok(string_affix(args.first(), args.get(1), false)),
            EagerBuiltinContract::Contains => match (args.first(), args.get(1)) {
                (Some(element), Some(Value::List(items))) => Ok(Value::Bool(
                    items.iter().any(|candidate| candidate == element),
                )),
                _ => Ok(Value::Err),
            },
            EagerBuiltinContract::Join => Ok(join_strings(args.first(), args.get(1))),
            EagerBuiltinContract::Abs => match args.first() {
                Some(Value::Int(v)) => Ok(Value::Int(v.abs())),
                _ => Ok(Value::Err),
            },
            EagerBuiltinContract::UnsignedCast32 => match args.first() {
                Some(Value::Int(v)) => Ok(Value::Int(v.rem_euclid(1_i64 << 32))),
                _ => Ok(Value::Err),
            },
            EagerBuiltinContract::Sum => Ok(fold_ints(args.first(), 0, |a, x| a + x)),
            EagerBuiltinContract::Min => Ok(min_max_value(&args, |a, x| a.min(x))),
            EagerBuiltinContract::Max => Ok(min_max_value(&args, |a, x| a.max(x))),
            EagerBuiltinContract::Range => range_values(&args),
            EagerBuiltinContract::Zip => match (args.first(), args.get(1)) {
                (Some(Value::List(a)), Some(Value::List(b))) => Ok(Value::List(
                    a.iter()
                        .zip(b.iter())
                        .map(|(x, y)| Value::List(vec![x.clone(), y.clone()]))
                        .collect(),
                )),
                _ => Ok(Value::Err),
            },
            EagerBuiltinContract::Enumerate => match args.first() {
                Some(Value::List(xs)) => Ok(Value::List(
                    xs.iter()
                        .enumerate()
                        .map(|(i, x)| Value::List(vec![Value::Int(i as i64), x.clone()]))
                        .collect(),
                )),
                _ => Ok(Value::Err),
            },
            // `for (x in list)` iterates the indices 0..n-1 (keys). Objects aren't
            // modeled → Err (so such loops are non-interpretable, not falsely merged).
            EagerBuiltinContract::Keys => match args.first() {
                Some(Value::List(xs)) => {
                    Ok(Value::List((0..xs.len() as i64).map(Value::Int).collect()))
                }
                _ => Ok(Value::Err),
            },
            EagerBuiltinContract::Print => {
                for a in &args {
                    self.effects.push(a.clone());
                }
                Ok(Value::Null)
            }
            // Dicts are not modeled — a `DictEntry` makes its unit non-interpretable (Err),
            // so dict-building units are excluded from the oracle rather than risk a false
            // merge. Their convergence rests on the DistinctEntry-vs-tuple representation.
            EagerBuiltinContract::DictEntry => Ok(Value::Err),
            EagerBuiltinContract::GetOrDefault => Ok(Value::Err),
        }
    }

    fn eval_value_or_default_call(
        &mut self,
        kids: &[NodeId],
        env: &mut FxHashMap<u32, Value>,
    ) -> R<Value> {
        let value = self.eval(*kids.first().ok_or(Unsupported)?, env)?;
        if matches!(value, Value::Err) {
            return Ok(Value::Err);
        }
        // Null-ness of a top-level Sym is unknown: compose symbolically (the default
        // is evaluated eagerly under the same convention on both sides of a merge).
        if matches!(value, Value::Sym(_)) {
            let d = match kids.get(1) {
                Some(&default) => self.eval(default, env)?,
                None => Value::Null,
            };
            if matches!(d, Value::Err) {
                return Ok(Value::Err);
            }
            return Ok(Value::Sym(sym_id(0x9015_4E11, &[vhash(&value), vhash(&d)])));
        }
        if matches!(value, Value::Null) {
            return match kids.get(1) {
                Some(&default) => self.eval(default, env),
                None => Ok(Value::Null),
            };
        }
        Ok(value)
    }

    /// A non-builtin `callee(args…)`. Modeled only when call-target evidence resolves the
    /// occurrence to an in-file function root. The arguments are evaluated call-by-value in the
    /// caller, then bound to a fresh callee frame; effects, field state, and step budget are
    /// shared so recursion stays ordered and bounded. Every unproven or ambiguous call remains
    /// unsupported rather than guessed.
    /// An opaque call: the callee's semantics are unknown, but its IDENTITY and its
    /// argument values are not. Evaluate the arguments left-to-right (an `Err` argument
    /// propagates, as for every call), produce the symbolic application value, and
    /// RECORD it in the effect trace — an unknown callee may observably act, and call
    /// order is behavior. Both sides of a fingerprint merge see the same convention,
    /// so symbolic traces stay comparable; Sym-bearing disagreements are routed to the
    /// advisory lane by the verify report, never the hard SOUND gate.
    fn opaque_call(
        &mut self,
        ident: u64,
        args: &[NodeId],
        env: &mut FxHashMap<u32, Value>,
    ) -> R<Value> {
        let mut parts = vec![ident];
        for &a in args {
            let v = self.eval(a, env)?;
            if matches!(v, Value::Err) {
                return Ok(Value::Err);
            }
            parts.push(vhash(&v));
        }
        let sym = Value::Sym(sym_id(0x0CA1_1E55, &parts));
        self.effects.push(sym.clone());
        Ok(sym)
    }

    fn eval_user_call(&mut self, node: NodeId, env: &mut FxHashMap<u32, Value>) -> R<Value> {
        let kids = self.il.children(node).to_vec();
        let &callee = kids.first().ok_or(Unsupported)?;
        let Some(target) = self.proven_call_target(node) else {
            // Unproven/ambiguous target: an opaque call identified by the callee's
            // structural signature (pre-canon syntax — fingerprint-equal units have
            // matching alpha-renamed cids, so signatures stay comparable).
            let ident = subtree_sig(self.il, self.interner, callee);
            return self.opaque_call(ident, &kids[1..], env);
        };
        // Evaluate the arguments in the CURRENT frame (call-by-value), left to right.
        let mut argv = Vec::with_capacity(kids.len().saturating_sub(1));
        for &a in &kids[1..] {
            let value = self.eval(a, env)?;
            if matches!(value, Value::Err) {
                return Ok(Value::Err);
            }
            argv.push(value);
        }
        // Bind them positionally to the CALLEE's parameters in a fresh environment; locals
        // start empty, exactly like a real call. The effect trace, field state, and step budget
        // are shared with the caller (so effects stay ordered and runaway recursion terminates).
        let params = self.il.children(target).to_vec();
        let mut fenv: FxHashMap<u32, Value> = FxHashMap::default();
        let mut pi = 0;
        for &p in &params {
            if self.il.kind(p) == NodeKind::Param {
                if let Payload::Cid(c) = self.il.node(p).payload {
                    fenv.insert(c, argv.get(pi).cloned().unwrap_or(Value::Null));
                    pi += 1;
                }
            }
        }
        let body = *params.last().ok_or(Unsupported)?;
        let result = self.exec(body, &mut fenv);
        match result? {
            Flow::Ret(v) => Ok(v),
            Flow::Err => Ok(Value::Err),
            _ => Ok(Value::Null),
        }
    }

    fn proven_call_target(&self, call: NodeId) -> Option<NodeId> {
        let mut found = None;
        for &root in &self.callable_roots {
            if !direct_function_call_target_at_call(self.il, call, root) {
                continue;
            }
            if found.replace(root).is_some() {
                return None;
            }
        }
        found
    }

    /// `any`/`all` over a collection: short-circuit existential/universal truth. The method
    /// form `[coll, λ]` applies the predicate per element; the generator form `[mapped-list]`
    /// reads each element's truthiness directly. `all` of empty = true, `any` of empty =
    /// false (the AND/OR identities).
    /// `append(coll, items…)` as a VALUE (e.g. Go's `s = append(s, x...)`, which returns the
    /// extended slice and does NOT mutate in place): functional — return `coll ++ items`.
    /// The Python/JS *statement* form `r.append(x)` is handled in `exec` (in-place build for
    /// a local list, effect for a parameter), not here.
    fn eval_append(&mut self, kids: &[NodeId], env: &mut FxHashMap<u32, Value>) -> R<Value> {
        let mut xs = match kids.first() {
            Some(&t) => match self.eval(t, env)? {
                Value::List(xs) => xs,
                Value::Err => return Ok(Value::Err),
                v if contains_sym(&v) => {
                    // append over an unknown collection: compose, do not launder to Err
                    let mut parts = vec![vhash(&v)];
                    for &k in kids.iter().skip(1) {
                        let item = self.eval(k, env)?;
                        if matches!(item, Value::Err) {
                            return Ok(Value::Err);
                        }
                        parts.push(vhash(&item));
                    }
                    return Ok(Value::Sym(sym_id(0x00A9_9E4D, &parts)));
                }
                _ => return Ok(Value::Err),
            },
            None => return Ok(Value::Err),
        };
        let mut items = Vec::with_capacity(kids.len().saturating_sub(1));
        for &k in kids.iter().skip(1) {
            let item = self.eval(k, env)?;
            if matches!(item, Value::Err) {
                return Ok(Value::Err);
            }
            items.push(item);
        }
        xs.extend(items);
        Ok(Value::List(xs))
    }

    /// A statement-level `r.append(x)` / `r.push(x)`: build `r` in place when it is a LOCAL
    /// list var (so `return r` yields the constructed list, converging with `[x for …]`);
    /// when `r` is a parameter (or non-list / non-var target) the append is a caller-visible
    /// mutation, recorded as an effect. Returns `Some` if `e` was an append handled here.
    fn exec_stmt_append(&mut self, e: NodeId, env: &mut FxHashMap<u32, Value>) -> R<Option<Flow>> {
        if self.il.kind(e) != NodeKind::Call
            || !matches!(self.il.node(e).payload, Payload::Builtin(Builtin::Append))
            || !admitted_builtin_semantics_at_call(self.il, e, Builtin::Append)
        {
            return Ok(None);
        }
        let kids = self.il.children(e).to_vec();
        let target_cid = kids.first().and_then(|&t| {
            if let (NodeKind::Var, Payload::Cid(c)) = (self.il.kind(t), self.il.node(t).payload) {
                Some(c)
            } else {
                None
            }
        });
        if target_cid.is_some_and(|c| matches!(env.get(&c), Some(Value::Err))) {
            return Ok(Some(Flow::Err));
        }
        if target_cid.is_none() {
            if let Some(&target) = kids.first() {
                let target_value = if self.il.kind(target) == NodeKind::Field {
                    match self.il.children(target).first() {
                        Some(&receiver) => self.eval(receiver, env)?,
                        None => Value::Null,
                    }
                } else {
                    self.eval(target, env)?
                };
                if matches!(target_value, Value::Err) {
                    return Ok(Some(Flow::Err));
                }
            }
        }
        let mut items = Vec::with_capacity(kids.len().saturating_sub(1));
        for &k in kids.iter().skip(1) {
            let item = self.eval(k, env)?;
            if matches!(item, Value::Err) {
                return Ok(Some(Flow::Err));
            }
            items.push(item);
        }
        if let Some(c) = target_cid {
            if !self.params.contains(&c) {
                if let Some(Value::List(xs)) = env.get_mut(&c) {
                    xs.extend(items);
                    return Ok(Some(Flow::Normal));
                }
            }
        }
        for a in items {
            self.effects.push(a);
        }
        Ok(Some(Flow::Normal))
    }

    fn eval_any_all_call(
        &mut self,
        all: bool,
        kids: &[NodeId],
        env: &mut FxHashMap<u32, Value>,
    ) -> R<Value> {
        let coll = match self.eval(*kids.first().ok_or(Unsupported)?, env)? {
            Value::List(xs) => xs,
            v if contains_sym(&v) => {
                let mut parts = vec![u64::from(all), vhash(&v)];
                parts.extend(
                    kids.iter()
                        .skip(1)
                        .map(|&k| subtree_sig(self.il, self.interner, k)),
                );
                return Ok(Value::Sym(sym_id(0x0A11_A4E0, &parts)));
            }
            _ => return Ok(Value::Err),
        };
        let pred = kids
            .get(1)
            .filter(|&&k| self.il.kind(k) == NodeKind::Lambda);
        for x in coll {
            let v = match pred {
                Some(&l) => self.apply(l, &[x], env)?,
                None => x,
            };
            if matches!(v, Value::Err) {
                return Ok(Value::Err);
            }
            let t = truthy(&v).ok_or(Unsupported)?;
            // short-circuit: `any` stops at the first truthy, `all` at the first falsy.
            if all != t {
                return Ok(Value::Bool(t));
            }
        }
        Ok(Value::Bool(all))
    }

    /// `reduce(f, xs[, init])`: fold `f` over `xs`.
    fn eval_reduce_call(&mut self, kids: &[NodeId], env: &mut FxHashMap<u32, Value>) -> R<Value> {
        if kids.len() < 2 {
            return Err(Unsupported);
        }
        let lambda = kids[0];
        let seq = match self.eval(kids[1], env)? {
            Value::List(xs) => xs,
            v if contains_sym(&v) => {
                let mut parts = vec![subtree_sig(self.il, self.interner, lambda), vhash(&v)];
                if let Some(&i) = kids.get(2) {
                    let init = self.eval(i, env)?;
                    if matches!(init, Value::Err) {
                        return Ok(Value::Err);
                    }
                    parts.push(vhash(&init));
                }
                return Ok(Value::Sym(sym_id(0x04ED_0CE0, &parts)));
            }
            _ => return Ok(Value::Err),
        };
        let mut it = seq.into_iter();
        let mut acc = match kids.get(2) {
            Some(&i) => self.eval(i, env)?,
            None => match it.next() {
                Some(v) => v,
                None => return Ok(Value::Err),
            },
        };
        if matches!(acc, Value::Err) {
            return Ok(Value::Err);
        }
        for x in it {
            acc = self.apply(lambda, &[acc, x], env)?;
            if matches!(acc, Value::Err) {
                return Ok(Value::Err);
            }
        }
        Ok(acc)
    }

    fn eval_hof(&mut self, node: NodeId, env: &mut FxHashMap<u32, Value>) -> R<Value> {
        let kind = match self.il.node(node).payload {
            Payload::HoF(h) => h,
            _ => return Err(Unsupported),
        };
        let kids = self.il.children(node).to_vec();
        if kids.len() < 2 {
            return Err(Unsupported);
        }
        let coll = match self.eval(kids[0], env)? {
            Value::List(xs) => xs,
            v if contains_sym(&v) => {
                let mut parts = vec![hashed(&kind), vhash(&v)];
                parts.extend(
                    kids.iter()
                        .skip(1)
                        .map(|&k| subtree_sig(self.il, self.interner, k)),
                );
                return Ok(Value::Sym(sym_id(0x040F_5E90, &parts)));
            }
            _ => return Ok(Value::Err),
        };
        let f = kids[1];
        match hof_contract(kind).demand {
            HofDemandProfile::Map { .. } => {
                let mut out = Vec::new();
                for x in coll {
                    let value = self.apply(f, &[x], env)?;
                    if matches!(value, Value::Err) {
                        return Ok(Value::Err);
                    }
                    out.push(value);
                }
                Ok(Value::List(out))
            }
            HofDemandProfile::FlatMap { .. } => {
                let mut out = Vec::new();
                for x in coll {
                    match self.apply(f, &[x], env)? {
                        Value::Err => return Ok(Value::Err),
                        Value::List(items) => out.extend(items),
                        _ => return Ok(Value::Err),
                    }
                }
                Ok(Value::List(out))
            }
            HofDemandProfile::FilterMap { .. } => {
                let mut out = Vec::new();
                for x in coll {
                    match self.apply(f, &[x], env)? {
                        Value::Err => return Ok(Value::Err),
                        Value::Null => {}
                        value => out.push(value),
                    }
                }
                Ok(Value::List(out))
            }
            HofDemandProfile::Filter { .. } => {
                let mut out = Vec::new();
                for x in coll {
                    let keep = self.apply(f, std::slice::from_ref(&x), env)?;
                    if matches!(keep, Value::Err) {
                        return Ok(Value::Err);
                    }
                    if truthy(&keep).ok_or(Unsupported)? {
                        out.push(x);
                    }
                }
                Ok(Value::List(out))
            }
            HofDemandProfile::Reduce { .. } => {
                let mut it = coll.into_iter();
                let mut acc = match it.next() {
                    Some(v) => v,
                    None => return Ok(Value::Err),
                };
                for x in it {
                    acc = self.apply(f, &[acc, x], env)?;
                    if matches!(acc, Value::Err) {
                        return Ok(Value::Err);
                    }
                }
                Ok(acc)
            }
        }
    }

    /// Apply a `Lambda` node to positional `args`, returning its body's value. The
    /// lambda's single tuple parameter destructures a pair element (zip/enumerate).
    fn apply(
        &mut self,
        lambda: NodeId,
        args: &[Value],
        env: &mut FxHashMap<u32, Value>,
    ) -> R<Value> {
        if self.il.kind(lambda) != NodeKind::Lambda {
            return Err(Unsupported);
        }
        let kids = self.il.children(lambda).to_vec();
        let mut local = env.clone();
        let params: Vec<NodeId> = kids
            .iter()
            .copied()
            .filter(|&k| self.il.kind(k) == NodeKind::Param)
            .collect();
        // Bind params positionally; a single param receiving a pair stays a list.
        if params.len() == args.len() {
            for (p, a) in params.iter().zip(args) {
                if let Payload::Cid(c) = self.il.node(*p).payload {
                    local.insert(c, a.clone());
                }
            }
        } else if params.len() > 1 && args.len() == 1 {
            // tuple-destructured params over a pair element: `λ(x,y). …` applied to a
            // `[x, y]` element (a comprehension over zip/enumerate).
            if let Value::List(vs) = &args[0] {
                if vs.len() == params.len() {
                    for (p, v) in params.iter().zip(vs) {
                        if let Payload::Cid(c) = self.il.node(*p).payload {
                            local.insert(c, v.clone());
                        }
                    }
                } else {
                    return Err(Unsupported);
                }
            } else {
                return Err(Unsupported);
            }
        } else {
            return Err(Unsupported);
        }
        let body = *kids.last().ok_or(Unsupported)?;
        match self.il.kind(body) {
            NodeKind::Block
            | NodeKind::Assign
            | NodeKind::ExprStmt
            | NodeKind::Return
            | NodeKind::Throw
            | NodeKind::Loop
            | NodeKind::Try
            | NodeKind::Break
            | NodeKind::Continue => match self.exec(body, &mut local)? {
                Flow::Ret(v) => Ok(v),
                Flow::Err => Ok(Value::Err),
                Flow::Normal => Ok(Value::Null),
                Flow::Break | Flow::Continue => Err(Unsupported),
            },
            _ => self.eval(body, &mut local),
        }
    }
}

fn op_of(p: Payload) -> Op {
    match p {
        Payload::Op(o) => o,
        _ => Op::Add,
    }
}

/// Concrete truthiness; `None` for a symbolic value. Control flow is never guessed
/// from a `Sym` — every branching caller must bail (`Unsupported`) on `None`.
fn truthy(v: &Value) -> Option<bool> {
    Some(match v {
        Value::Bool(b) => *b,
        Value::Int(i) => *i != 0,
        Value::List(xs) => !xs.is_empty(),
        Value::Str(v) => !v.is_empty(),
        Value::Null | Value::Err => false,
        Value::Sym(_) => return None,
    })
}

fn fold_ints(v: Option<&Value>, init: i64, f: impl Fn(i64, i64) -> i64) -> Value {
    match v {
        Some(Value::List(xs)) => {
            let mut acc = init;
            for x in xs {
                match x {
                    Value::Int(i) => acc = f(acc, *i),
                    _ => return Value::Err,
                }
            }
            Value::Int(acc)
        }
        _ => Value::Err,
    }
}

/// `min`/`max` over either canonical form: a single collection (`min(xs)`) or two
/// scalars (`min(a, b)` — the 2-way selection that `[a, b].min()` and the
/// `a if a < b else b` ternary also canonicalize to). Without the scalar form the
/// oracle was blind to exactly the convergences the value graph claims for it.
fn min_max_value(args: &[Value], f: impl Fn(i64, i64) -> i64) -> Value {
    match args {
        [Value::Int(a), Value::Int(b)] => Value::Int(f(*a, *b)),
        // The 2-arg form is the SCALAR selection; on non-Int operands it is a
        // type error. Falling through to the collection fold here ignored the
        // second argument (`max([1,2,3,4], 7)` returned 4), which made the
        // builtin chain disagree with the if-chain it soundly merges with (#210).
        [_, _] => Value::Err,
        [coll] => fold_opt(Some(coll), f),
        _ => Value::Err,
    }
}

fn fold_opt(v: Option<&Value>, f: impl Fn(i64, i64) -> i64) -> Value {
    match v {
        Some(Value::List(xs)) => {
            let mut acc: Option<i64> = None;
            for x in xs {
                match x {
                    Value::Int(i) => acc = Some(acc.map_or(*i, |a| f(a, *i))),
                    _ => return Value::Err,
                }
            }
            acc.map(Value::Int).unwrap_or(Value::Err)
        }
        _ => Value::Err,
    }
}

fn string_affix(value: Option<&Value>, affix: Option<&Value>, prefix: bool) -> Value {
    match (value, affix) {
        (Some(Value::Str(value)), Some(Value::Str(affix))) => {
            if affix.len() > value.len() {
                return Value::Bool(false);
            }
            let matches = if prefix {
                value.starts_with(affix)
            } else {
                value.ends_with(affix)
            };
            Value::Bool(matches)
        }
        _ => Value::Err,
    }
}

fn join_strings(separator: Option<&Value>, collection: Option<&Value>) -> Value {
    let (Some(Value::Str(separator)), Some(Value::List(items))) = (separator, collection) else {
        return Value::Err;
    };
    let mut out = Vec::new();
    for (idx, item) in items.iter().enumerate() {
        let Value::Str(piece) = item else {
            return Value::Err;
        };
        if idx > 0 {
            out.extend(separator.iter().copied());
        }
        out.extend(piece.iter().copied());
    }
    Value::Str(out)
}

fn range_values(args: &[Value]) -> R<Value> {
    let (start, stop, step) = match args {
        [Value::Int(stop)] => (0, *stop, 1),
        [Value::Int(start), Value::Int(stop)] => (*start, *stop, 1),
        [Value::Int(start), Value::Int(stop), Value::Int(step)] => (*start, *stop, *step),
        _ => return Ok(Value::Err),
    };
    if step == 0 {
        return Ok(Value::Err);
    }

    let mut out = Vec::new();
    let mut cur = start;
    while if step > 0 { cur < stop } else { cur > stop } {
        if out.len() as u64 > STEP_BUDGET {
            return Err(Unsupported);
        }
        out.push(Value::Int(cur));
        let Some(next) = cur.checked_add(step) else {
            return Err(Unsupported);
        };
        cur = next;
    }
    Ok(Value::List(out))
}

fn bin(op: Op, a: &Value, b: &Value) -> Value {
    use Value::{Bool, Int};
    // Symbolic composition: an operation over an unknown value is itself unknown,
    // keyed by (operator, operand identities). Never collapses to Bool/Err —
    // comparisons over `Sym` stay symbolic (`f(x) == f(x)` is NOT provably true for
    // an impure callee), and branching on the result bails at the truthiness gate.
    if contains_sym(a) || contains_sym(b) {
        return Value::Sym(sym_id(0x00B1_0911, &[hashed(&op), vhash(a), vhash(b)]));
    }
    match (a, b) {
        (Int(x), Int(y)) => match op {
            Op::Add => Int(x.wrapping_add(*y)),
            Op::Sub => Int(x.wrapping_sub(*y)),
            Op::Mul => Int(x.wrapping_mul(*y)),
            Op::Div => {
                if *y == 0 {
                    Value::Err
                } else {
                    Int(x.wrapping_div(*y))
                }
            }
            Op::Mod => {
                if *y == 0 {
                    Value::Err
                } else {
                    Int(x.wrapping_rem(*y))
                }
            }
            // Floor division rounds the quotient toward −∞ (Python `//`): the
            // truncating quotient is decremented when the remainder is nonzero
            // and the operands disagree in sign (`-5 // 2 == -3`, `5 // -2 == -3`).
            Op::FloorDiv => {
                if *y == 0 {
                    Value::Err
                } else {
                    let q = x.wrapping_div(*y);
                    let r = x.wrapping_rem(*y);
                    Int(if r != 0 && (r < 0) != (*y < 0) {
                        q - 1
                    } else {
                        q
                    })
                }
            }
            // An exponent that isn't a non-negative `u32` has no usable value here: a
            // negative one is fractional, and one past `u32::MAX` truncated under `as u32`
            // (so `b ** 2^32` collapsed onto `b ** 0 == 1`). Both err, like Div/Mod by zero,
            // rather than silently colliding distinct exponents.
            Op::Pow if !(0..=u32::MAX as i64).contains(y) => Value::Err,
            Op::Pow => Int(x.wrapping_pow(*y as u32)),
            Op::Eq => Bool(x == y),
            Op::Ne => Bool(x != y),
            Op::Lt => Bool(x < y),
            Op::Le => Bool(x <= y),
            Op::Gt => Bool(x > y),
            Op::Ge => Bool(x >= y),
            Op::BitAnd => Int(x & y),
            Op::BitOr => Int(x | y),
            Op::BitXor => Int(x ^ y),
            Op::Shl => Int(x.wrapping_shl(*y as u32)),
            Op::Shr => Int(x.wrapping_shr(*y as u32)),
            Op::And => Int(if *x != 0 { *y } else { *x }),
            Op::Or => Int(if *x != 0 { *x } else { *y }),
            _ => Value::Err,
        },
        (Bool(x), Bool(y)) => match op {
            Op::And => Bool(*x && *y),
            Op::Or => Bool(*x || *y),
            Op::Eq => Bool(x == y),
            Op::Ne => Bool(x != y),
            _ => Value::Err,
        },
        // String/builder concatenation — the free-monoid op: ordered append of pieces.
        // Order-sensitive (`s + x` ≠ `x + s`), the defining non-commutative behavior.
        (Value::Str(x), Value::Str(y)) if op == Op::Add => {
            let mut v = x.clone();
            v.extend_from_slice(y);
            Value::Str(v)
        }
        // Membership `a in b`: a is an element of list b (directional). Modeled for
        // lists so the value graph's `Op::In` is oracle-verifiable; other collections
        // (strings/dicts) aren't modeled → Err.
        (_, Value::List(items)) if op == Op::In => Bool(items.iter().any(|e| e == a)),
        // Equality across the same shape (lists, strings, null).
        _ => match op {
            Op::Eq => Bool(a == b),
            Op::Ne => Bool(a != b),
            _ => Value::Err,
        },
    }
}

fn un(op: Op, a: &Value) -> Value {
    if contains_sym(a) {
        return Value::Sym(sym_id(0x0004_0911, &[hashed(&op), vhash(a)]));
    }
    match (op, a) {
        // `wrapping_neg` (not `-i`) so negating `i64::MIN` wraps to `i64::MIN` instead of
        // panicking on overflow — consistent with the wrapping binary arithmetic above.
        (Op::Neg, Value::Int(i)) => Value::Int(i.wrapping_neg()),
        (Op::Pos, Value::Int(i)) => Value::Int(*i),
        (Op::BitNot, Value::Int(i)) => Value::Int(!i),
        // Negating an ERROR propagates the error — `not (1/0)` raises in Python, it does
        // NOT yield `True`. Without this, `not (a<=b)` on non-numeric operands wrongly gave
        // `Bool(true)` while the direct `a>b` gave `Err`, making the SOUND comparison-
        // negation canon (`!(a<=b) ≡ a>b`, a total order) look like a false merge.
        (Op::Not, Value::Err) => Value::Err,
        // Operand is concrete here — the symbolic arm above intercepts `Sym`.
        (Op::Not, _) => truthy(a).map_or(Value::Err, |b| Value::Bool(!b)),
        _ => Value::Err,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nose_il::{
        stable_symbol_hash, CallTargetEvidenceKind, EffectEvidenceKind, EvidenceAnchor,
        EvidenceEmitter, EvidenceId, EvidenceKind, EvidenceProvenance, EvidenceRecord,
        EvidenceStatus, FileId, FileMeta, HoFKind, IlBuilder, Interner, Lang,
        LibraryApiEvidenceKind, LitClass, PlaceEvidenceKind, SourceCastKind, SourceFactKind, Span,
        Unit, UnitKind,
    };
    use nose_semantics::{
        library_api_callee_contract_hash, library_api_contract_id_hash, LibraryApiCalleeContract,
        LibraryApiContractId, LibraryApiShadowPolicy, MethodSemanticContract, FIRST_PARTY_PACK_ID,
    };

    fn run_admitted_unit(mut il: Il, root: NodeId, args: &[Value]) -> Option<Behavior> {
        admit_test_builtin_calls(&mut il);
        let interner = Interner::new();
        run_unit(&il, &interner, root, args)
    }

    fn test_span(offset: u32) -> Span {
        Span::new(FileId(0), offset, offset + 1, offset + 1, offset + 1)
    }

    fn admit_test_builtin_calls(il: &mut Il) {
        let mut seen_library_records = Vec::new();
        let mut next_id = 1000;
        for idx in 0..il.nodes.len() {
            let node = NodeId(idx as u32);
            let (NodeKind::Call, Payload::Builtin(builtin)) =
                (il.kind(node), il.node(node).payload)
            else {
                continue;
            };
            let span = il.node(node).span;
            if matches!(builtin, Builtin::Append) {
                il.evidence.push(test_effect_record(
                    next_id,
                    span,
                    EffectEvidenceKind::BuilderAppendCall,
                ));
                next_id += 1;
            } else if let Some(contract_id) = test_library_contract_id_for_builtin(builtin) {
                if seen_library_records
                    .iter()
                    .any(|&(seen_span, seen_builtin)| seen_span == span && seen_builtin == builtin)
                {
                    continue;
                }
                seen_library_records.push((span, builtin));
                il.evidence.push(test_library_api_record(
                    next_id,
                    span,
                    contract_id,
                    test_callee_contract(),
                ));
                next_id += 1;
            } else if matches!(builtin, Builtin::UnsignedCast32) {
                il.evidence.push(test_source_record(
                    next_id,
                    span,
                    SourceFactKind::Cast(SourceCastKind::CUnsigned32),
                ));
                next_id += 1;
            }
        }
    }

    fn test_library_contract_id_for_builtin(builtin: Builtin) -> Option<LibraryApiContractId> {
        match builtin {
            Builtin::Len
            | Builtin::Print
            | Builtin::Range
            | Builtin::Sum
            | Builtin::Min
            | Builtin::Max
            | Builtin::Abs
            | Builtin::Zip
            | Builtin::Enumerate
            | Builtin::Any
            | Builtin::All => Some(LibraryApiContractId::FreeFunctionBuiltin(builtin)),
            Builtin::IsEmpty
            | Builtin::StartsWith
            | Builtin::EndsWith
            | Builtin::Contains
            | Builtin::GetOrDefault
            | Builtin::ValueOrDefault
            | Builtin::IsNull
            | Builtin::IsNotNull
            | Builtin::Join
            | Builtin::Reduce => Some(LibraryApiContractId::MethodCall(
                MethodSemanticContract::Builtin(builtin),
            )),
            Builtin::Append | Builtin::Keys | Builtin::DictEntry | Builtin::UnsignedCast32 => None,
        }
    }

    fn test_callee_contract() -> LibraryApiCalleeContract {
        LibraryApiCalleeContract::FreeName {
            name: "__test_builtin__",
            shadow: LibraryApiShadowPolicy::None,
        }
    }

    fn test_library_api_record(
        id: u32,
        span: Span,
        contract_id: LibraryApiContractId,
        callee: LibraryApiCalleeContract,
    ) -> EvidenceRecord {
        EvidenceRecord {
            id: EvidenceId(id),
            anchor: EvidenceAnchor::node(span, NodeKind::Call),
            kind: EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
                contract_hash: library_api_contract_id_hash(contract_id),
                callee_hash: library_api_callee_contract_hash(callee),
                arity: 0,
            }),
            provenance: EvidenceProvenance {
                emitter: EvidenceEmitter::FirstParty,
                pack_hash: Some(stable_symbol_hash(FIRST_PARTY_PACK_ID)),
                rule_hash: Some(stable_symbol_hash("interp-test")),
            },
            dependencies: Vec::new(),
            status: EvidenceStatus::Asserted,
        }
    }

    fn test_effect_record(id: u32, span: Span, effect: EffectEvidenceKind) -> EvidenceRecord {
        EvidenceRecord {
            id: EvidenceId(id),
            anchor: EvidenceAnchor::node(span, NodeKind::Call),
            kind: EvidenceKind::Effect(effect),
            provenance: EvidenceProvenance {
                emitter: EvidenceEmitter::FirstParty,
                pack_hash: Some(stable_symbol_hash(FIRST_PARTY_PACK_ID)),
                rule_hash: Some(stable_symbol_hash("interp-test")),
            },
            dependencies: Vec::new(),
            status: EvidenceStatus::Asserted,
        }
    }

    fn test_node_place_record(
        id: u32,
        il: &Il,
        node: NodeId,
        place: PlaceEvidenceKind,
        dependencies: Vec<EvidenceId>,
    ) -> EvidenceRecord {
        EvidenceRecord {
            id: EvidenceId(id),
            anchor: EvidenceAnchor::node(il.node(node).span, il.kind(node)),
            kind: EvidenceKind::Place(place),
            provenance: EvidenceProvenance {
                emitter: EvidenceEmitter::FirstParty,
                pack_hash: Some(stable_symbol_hash(FIRST_PARTY_PACK_ID)),
                rule_hash: Some(stable_symbol_hash("interp-test")),
            },
            dependencies,
            status: EvidenceStatus::Asserted,
        }
    }

    fn test_node_effect_record(
        id: u32,
        il: &Il,
        node: NodeId,
        effect: EffectEvidenceKind,
        dependencies: Vec<EvidenceId>,
    ) -> EvidenceRecord {
        EvidenceRecord {
            id: EvidenceId(id),
            anchor: EvidenceAnchor::node(il.node(node).span, il.kind(node)),
            kind: EvidenceKind::Effect(effect),
            provenance: EvidenceProvenance {
                emitter: EvidenceEmitter::FirstParty,
                pack_hash: Some(stable_symbol_hash(FIRST_PARTY_PACK_ID)),
                rule_hash: Some(stable_symbol_hash("interp-test")),
            },
            dependencies,
            status: EvidenceStatus::Asserted,
        }
    }

    fn admit_test_self_field(
        il: &mut Il,
        interner: &Interner,
        receiver: NodeId,
        field: NodeId,
        field_name: nose_il::Symbol,
        first_id: u32,
    ) -> EvidenceId {
        let receiver_id = EvidenceId(first_id);
        let field_id = EvidenceId(first_id + 1);
        let receiver_record = test_node_place_record(
            first_id,
            il,
            receiver,
            PlaceEvidenceKind::SelfReceiver,
            Vec::new(),
        );
        let field_record = test_node_place_record(
            first_id + 1,
            il,
            field,
            PlaceEvidenceKind::SelfField {
                field_hash: stable_symbol_hash(interner.resolve(field_name)),
            },
            vec![receiver_id],
        );
        il.evidence.push(receiver_record);
        il.evidence.push(field_record);
        field_id
    }

    fn admit_test_self_field_write(
        il: &mut Il,
        interner: &Interner,
        receiver: NodeId,
        field: NodeId,
        assign: NodeId,
        field_name: nose_il::Symbol,
        first_id: u32,
    ) {
        let field_id = admit_test_self_field(il, interner, receiver, field, field_name, first_id);
        let effect_record = test_node_effect_record(
            first_id + 2,
            il,
            assign,
            EffectEvidenceKind::SelfFieldWrite {
                field_hash: stable_symbol_hash(interner.resolve(field_name)),
            },
            vec![field_id],
        );
        il.evidence.push(effect_record);
    }

    fn test_source_record(id: u32, span: Span, fact: SourceFactKind) -> EvidenceRecord {
        EvidenceRecord {
            id: EvidenceId(id),
            anchor: EvidenceAnchor::source_span(span),
            kind: EvidenceKind::Source(fact),
            provenance: EvidenceProvenance {
                emitter: EvidenceEmitter::FirstParty,
                pack_hash: Some(stable_symbol_hash(FIRST_PARTY_PACK_ID)),
                rule_hash: Some(stable_symbol_hash("interp-test")),
            },
            dependencies: Vec::new(),
            status: EvidenceStatus::Asserted,
        }
    }

    fn test_call_target_record(
        id: u32,
        call_span: Span,
        target_span: Span,
        name_hash: u64,
    ) -> EvidenceRecord {
        EvidenceRecord {
            id: EvidenceId(id),
            anchor: EvidenceAnchor::node(call_span, NodeKind::Call),
            kind: EvidenceKind::CallTarget(CallTargetEvidenceKind::DirectFunction {
                target_span,
                name_hash,
            }),
            provenance: EvidenceProvenance {
                emitter: EvidenceEmitter::FirstParty,
                pack_hash: Some(stable_symbol_hash(FIRST_PARTY_PACK_ID)),
                rule_hash: Some(stable_symbol_hash("interp-test")),
            },
            dependencies: Vec::new(),
            status: EvidenceStatus::Asserted,
        }
    }

    /// Build `fn() { return len(<str literal>) }` and run it.
    fn run_len_of_string() -> Value {
        let sp = Span::synthetic(FileId(0));
        let mut b = IlBuilder::new(FileId(0));
        let s = b.add(NodeKind::Lit, Payload::LitStr(0xABCD), sp, &[]);
        let call = b.add(NodeKind::Call, Payload::Builtin(Builtin::Len), sp, &[s]);
        let ret = b.add(NodeKind::Return, Payload::None, sp, &[call]);
        let func = b.add(NodeKind::Func, Payload::None, sp, &[ret]);
        let il = b.finish(
            func,
            FileMeta {
                path: "t".into(),
                lang: Lang::Python,
            },
            Vec::new(),
            Vec::new(),
        );
        run_admitted_unit(il, func, &[]).expect("run_unit").ret
    }

    #[test]
    fn len_of_string_is_err_not_one() {
        // Strings are the free monoid over opaque piece hashes — character length is
        // unknown, so `len(str)` must be `Err` (matching the documented contract and the
        // sibling `IsEmpty`), not a hardcoded `Int(1)`.
        assert_eq!(run_len_of_string(), Value::Err);
    }

    #[test]
    fn unadmitted_builtin_calls_become_identified_symbolic_effects() {
        let sp = Span::synthetic(FileId(0));
        let mut b = IlBuilder::new(FileId(0));
        let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
        let xs = b.add(NodeKind::Seq, Payload::None, sp, &[one]);
        let call = b.add(NodeKind::Call, Payload::Builtin(Builtin::Len), sp, &[xs]);
        let ret = b.add(NodeKind::Return, Payload::None, sp, &[call]);
        let func = b.add(NodeKind::Func, Payload::None, sp, &[ret]);
        let il = b.finish(
            func,
            FileMeta {
                path: "t".into(),
                lang: Lang::Python,
            },
            Vec::new(),
            Vec::new(),
        );

        // Without admission the call no longer bails the unit: it interprets to a
        // SYMBOLIC value, recorded in the effect trace (an unknown callee may act).
        let interner = Interner::new();
        let beh = run_unit(&il, &interner, func, &[]).expect("symbolic run");
        assert!(matches!(beh.ret, Value::Sym(_)));
        assert_eq!(beh.effects, vec![beh.ret.clone()]);
        // The admitted run keeps its CONCRETE semantics — and differs from symbolic.
        assert_eq!(
            run_admitted_unit(il, func, &[]).expect("admitted run").ret,
            Value::Int(1)
        );
    }

    fn run_value_or_default(value: NodeId, default: NodeId, mut b: IlBuilder, sp: Span) -> Value {
        let call = b.add(
            NodeKind::Call,
            Payload::Builtin(Builtin::ValueOrDefault),
            sp,
            &[value, default],
        );
        let ret = b.add(NodeKind::Return, Payload::None, sp, &[call]);
        let func = b.add(NodeKind::Func, Payload::None, sp, &[ret]);
        let il = b.finish(
            func,
            FileMeta {
                path: "t".into(),
                lang: Lang::JavaScript,
            },
            Vec::new(),
            Vec::new(),
        );
        run_admitted_unit(il, func, &[]).expect("run_unit").ret
    }

    #[test]
    fn value_or_default_uses_default_for_null() {
        let sp = Span::synthetic(FileId(0));
        let mut b = IlBuilder::new(FileId(0));
        let value = b.add(NodeKind::Lit, Payload::Lit(LitClass::Null), sp, &[]);
        let default = b.add(NodeKind::Lit, Payload::LitInt(7), sp, &[]);
        assert_eq!(run_value_or_default(value, default, b, sp), Value::Int(7));
    }

    #[test]
    fn value_or_default_short_circuits_present_value() {
        let sp = Span::synthetic(FileId(0));
        let mut b = IlBuilder::new(FileId(0));
        let value = b.add(NodeKind::Lit, Payload::LitInt(7), sp, &[]);
        let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
        let zero = b.add(NodeKind::Lit, Payload::LitInt(0), sp, &[]);
        let default_err = b.add(NodeKind::BinOp, Payload::Op(Op::Div), sp, &[one, zero]);
        assert_eq!(
            run_value_or_default(value, default_err, b, sp),
            Value::Int(7)
        );
    }

    #[test]
    fn value_or_default_keeps_error_value() {
        let sp = Span::synthetic(FileId(0));
        let mut b = IlBuilder::new(FileId(0));
        let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
        let zero = b.add(NodeKind::Lit, Payload::LitInt(0), sp, &[]);
        let value_err = b.add(NodeKind::BinOp, Payload::Op(Op::Div), sp, &[one, zero]);
        let default = b.add(NodeKind::Lit, Payload::LitInt(7), sp, &[]);
        assert_eq!(run_value_or_default(value_err, default, b, sp), Value::Err);
    }

    fn run_range(args: &[i64]) -> Value {
        let sp = Span::synthetic(FileId(0));
        let mut b = IlBuilder::new(FileId(0));
        let args: Vec<NodeId> = args
            .iter()
            .map(|arg| b.add(NodeKind::Lit, Payload::LitInt(*arg), sp, &[]))
            .collect();
        let call = b.add(NodeKind::Call, Payload::Builtin(Builtin::Range), sp, &args);
        let ret = b.add(NodeKind::Return, Payload::None, sp, &[call]);
        let func = b.add(NodeKind::Func, Payload::None, sp, &[ret]);
        let il = b.finish(
            func,
            FileMeta {
                path: "t".into(),
                lang: Lang::Python,
            },
            Vec::new(),
            Vec::new(),
        );
        run_admitted_unit(il, func, &[]).expect("run_unit").ret
    }

    #[test]
    fn range_interprets_start_stop_and_step() {
        assert_eq!(
            run_range(&[1, 5, 2]),
            Value::List(vec![Value::Int(1), Value::Int(3)])
        );
    }

    #[test]
    fn range_zero_step_is_err_behavior() {
        assert_eq!(run_range(&[1, 5, 0]), Value::Err);
    }

    fn run_cstyle_loop_with_update_err() -> Value {
        let sp = Span::synthetic(FileId(0));
        let mut b = IlBuilder::new(FileId(0));
        let i = b.add(NodeKind::Var, Payload::Cid(0), sp, &[]);
        let zero = b.add(NodeKind::Lit, Payload::LitInt(0), sp, &[]);
        let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
        let init = b.add(NodeKind::Assign, Payload::None, sp, &[i, zero]);
        let cond = b.add(NodeKind::BinOp, Payload::Op(Op::Lt), sp, &[i, one]);
        let div = b.add(NodeKind::BinOp, Payload::Op(Op::Div), sp, &[one, zero]);
        let j = b.add(NodeKind::Var, Payload::Cid(1), sp, &[]);
        let update = b.add(NodeKind::Assign, Payload::None, sp, &[j, div]);
        let set_done = b.add(NodeKind::Assign, Payload::None, sp, &[i, one]);
        let body = b.add(NodeKind::Block, Payload::None, sp, &[set_done]);
        let loop_node = b.add(
            NodeKind::Loop,
            Payload::Loop(LoopKind::CStyle),
            sp,
            &[init, cond, update, body],
        );
        let seven = b.add(NodeKind::Lit, Payload::LitInt(7), sp, &[]);
        let ret = b.add(NodeKind::Return, Payload::None, sp, &[seven]);
        let block = b.add(NodeKind::Block, Payload::None, sp, &[loop_node, ret]);
        let func = b.add(NodeKind::Func, Payload::None, sp, &[block]);
        let il = b.finish(
            func,
            FileMeta {
                path: "t".into(),
                lang: Lang::C,
            },
            Vec::new(),
            Vec::new(),
        );
        run_admitted_unit(il, func, &[]).expect("run_unit").ret
    }

    #[test]
    fn cstyle_loop_update_err_stops_execution() {
        assert_eq!(run_cstyle_loop_with_update_err(), Value::Err);
    }

    fn run_foreach_with_iterable_err() -> Option<Value> {
        let sp = Span::synthetic(FileId(0));
        let mut b = IlBuilder::new(FileId(0));
        let target = b.add(NodeKind::Var, Payload::Cid(0), sp, &[]);
        let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
        let zero = b.add(NodeKind::Lit, Payload::LitInt(0), sp, &[]);
        let iter_err = b.add(NodeKind::BinOp, Payload::Op(Op::Div), sp, &[one, zero]);
        let body = b.add(NodeKind::Block, Payload::None, sp, &[]);
        let loop_node = b.add(
            NodeKind::Loop,
            Payload::Loop(LoopKind::ForEach),
            sp,
            &[target, iter_err, body],
        );
        let seven = b.add(NodeKind::Lit, Payload::LitInt(7), sp, &[]);
        let ret = b.add(NodeKind::Return, Payload::None, sp, &[seven]);
        let block = b.add(NodeKind::Block, Payload::None, sp, &[loop_node, ret]);
        let func = b.add(NodeKind::Func, Payload::None, sp, &[block]);
        let il = b.finish(
            func,
            FileMeta {
                path: "t".into(),
                lang: Lang::Python,
            },
            Vec::new(),
            Vec::new(),
        );
        run_admitted_unit(il, func, &[]).map(|behavior| behavior.ret)
    }

    #[test]
    fn foreach_iterable_err_stops_execution() {
        assert_eq!(run_foreach_with_iterable_err(), Some(Value::Err));
    }

    fn run_throw_then_return() -> Value {
        let sp = Span::synthetic(FileId(0));
        let mut b = IlBuilder::new(FileId(0));
        let thrown = b.add(NodeKind::Lit, Payload::LitStr(0xBAD), sp, &[]);
        let throw = b.add(NodeKind::Throw, Payload::None, sp, &[thrown]);
        let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
        let ret = b.add(NodeKind::Return, Payload::None, sp, &[one]);
        let block = b.add(NodeKind::Block, Payload::None, sp, &[throw, ret]);
        let func = b.add(NodeKind::Func, Payload::None, sp, &[block]);
        let il = b.finish(
            func,
            FileMeta {
                path: "t".into(),
                lang: Lang::Python,
            },
            Vec::new(),
            Vec::new(),
        );
        run_admitted_unit(il, func, &[]).expect("run_unit").ret
    }

    #[test]
    fn throw_is_err_behavior_and_stops_execution() {
        assert_eq!(run_throw_then_return(), Value::Err);
    }

    fn run_field_write_read() -> (Behavior, FieldKey) {
        let interner = Interner::new();
        let this_name = interner.intern("this");
        let field_name = interner.intern("x");
        let field_key = FieldKey {
            receiver: FieldPlace::SelfReceiver,
            field: stable_symbol_hash(interner.resolve(field_name)),
        };
        let mut b = IlBuilder::new(FileId(0));
        let write_receiver = b.add(NodeKind::Var, Payload::Name(this_name), test_span(1), &[]);
        let write_target = b.add(
            NodeKind::Field,
            Payload::Name(field_name),
            test_span(2),
            &[write_receiver],
        );
        let seven = b.add(NodeKind::Lit, Payload::LitInt(7), test_span(3), &[]);
        let assign = b.add(
            NodeKind::Assign,
            Payload::None,
            test_span(4),
            &[write_target, seven],
        );
        let read_receiver = b.add(NodeKind::Var, Payload::Name(this_name), test_span(5), &[]);
        let read_target = b.add(
            NodeKind::Field,
            Payload::Name(field_name),
            test_span(6),
            &[read_receiver],
        );
        let ret = b.add(
            NodeKind::Return,
            Payload::None,
            test_span(7),
            &[read_target],
        );
        let block = b.add(NodeKind::Block, Payload::None, test_span(8), &[assign, ret]);
        let func = b.add(NodeKind::Func, Payload::None, test_span(9), &[block]);
        let mut il = b.finish(
            func,
            FileMeta {
                path: "T.java".into(),
                lang: Lang::Java,
            },
            Vec::new(),
            Vec::new(),
        );
        admit_test_self_field_write(
            &mut il,
            &interner,
            write_receiver,
            write_target,
            assign,
            field_name,
            2000,
        );
        admit_test_self_field(
            &mut il,
            &interner,
            read_receiver,
            read_target,
            field_name,
            2010,
        );
        (
            run_unit(&il, &interner, func, &[]).expect("run_unit"),
            field_key,
        )
    }

    #[test]
    fn field_write_can_be_read_back() {
        let (behavior, field_key) = run_field_write_read();
        assert_eq!(behavior.ret, Value::Int(7));
        assert_eq!(behavior.fields, vec![(field_key, Value::Int(7))]);
    }

    #[test]
    fn raw_python_attribute_write_is_not_oracle_field_state_proof() {
        let interner = Interner::new();
        let field_name = interner.intern("x");
        let mut b = IlBuilder::new(FileId(0));
        let param = b.add(NodeKind::Param, Payload::Cid(0), test_span(1), &[]);
        let write_receiver = b.add(NodeKind::Var, Payload::Cid(0), test_span(2), &[]);
        let write_target = b.add(
            NodeKind::Field,
            Payload::Name(field_name),
            test_span(3),
            &[write_receiver],
        );
        let seven = b.add(NodeKind::Lit, Payload::LitInt(7), test_span(4), &[]);
        let assign = b.add(
            NodeKind::Assign,
            Payload::None,
            test_span(5),
            &[write_target, seven],
        );
        let read_receiver = b.add(NodeKind::Var, Payload::Cid(0), test_span(6), &[]);
        let read_target = b.add(
            NodeKind::Field,
            Payload::Name(field_name),
            test_span(7),
            &[read_receiver],
        );
        let ret = b.add(
            NodeKind::Return,
            Payload::None,
            test_span(8),
            &[read_target],
        );
        let block = b.add(NodeKind::Block, Payload::None, test_span(9), &[assign, ret]);
        let func = b.add(
            NodeKind::Func,
            Payload::None,
            test_span(10),
            &[param, block],
        );
        let il = b.finish(
            func,
            FileMeta {
                path: "t.py".into(),
                lang: Lang::Python,
            },
            Vec::new(),
            Vec::new(),
        );
        assert!(
            run_unit(&il, &interner, func, &[Value::Null]).is_none(),
            "raw attribute spelling must not prove exact field readback"
        );
    }

    fn run_field_read_with_error_receiver() -> Behavior {
        let interner = Interner::new();
        let this_name = interner.intern("this");
        let field_name = interner.intern("x");
        let mut b = IlBuilder::new(FileId(0));
        let write_receiver = b.add(NodeKind::Var, Payload::Name(this_name), test_span(1), &[]);
        let write_target = b.add(
            NodeKind::Field,
            Payload::Name(field_name),
            test_span(2),
            &[write_receiver],
        );
        let seven = b.add(NodeKind::Lit, Payload::LitInt(7), test_span(3), &[]);
        let assign = b.add(
            NodeKind::Assign,
            Payload::None,
            test_span(4),
            &[write_target, seven],
        );
        let one = b.add(NodeKind::Lit, Payload::LitInt(1), test_span(5), &[]);
        let zero = b.add(NodeKind::Lit, Payload::LitInt(0), test_span(6), &[]);
        let error_receiver = b.add(
            NodeKind::BinOp,
            Payload::Op(Op::Div),
            test_span(7),
            &[one, zero],
        );
        let read_target = b.add(
            NodeKind::Field,
            Payload::Name(field_name),
            test_span(8),
            &[error_receiver],
        );
        let ret = b.add(
            NodeKind::Return,
            Payload::None,
            test_span(9),
            &[read_target],
        );
        let block = b.add(
            NodeKind::Block,
            Payload::None,
            test_span(10),
            &[assign, ret],
        );
        let func = b.add(NodeKind::Func, Payload::None, test_span(11), &[block]);
        let mut il = b.finish(
            func,
            FileMeta {
                path: "T.java".into(),
                lang: Lang::Java,
            },
            Vec::new(),
            Vec::new(),
        );
        admit_test_self_field_write(
            &mut il,
            &interner,
            write_receiver,
            write_target,
            assign,
            field_name,
            2000,
        );
        run_unit(&il, &interner, func, &[]).expect("run_unit")
    }

    #[test]
    fn field_read_propagates_receiver_err_before_cached_value() {
        assert_eq!(run_field_read_with_error_receiver().ret, Value::Err);
    }

    fn run_field_write_with_error_receiver() -> Behavior {
        let sp = Span::synthetic(FileId(0));
        let mut b = IlBuilder::new(FileId(0));
        let interner = Interner::new();
        let field_name = interner.intern("x");
        let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
        let zero = b.add(NodeKind::Lit, Payload::LitInt(0), sp, &[]);
        let error_receiver = b.add(NodeKind::BinOp, Payload::Op(Op::Div), sp, &[one, zero]);
        let write_target = b.add(
            NodeKind::Field,
            Payload::Name(field_name),
            sp,
            &[error_receiver],
        );
        let seven = b.add(NodeKind::Lit, Payload::LitInt(7), sp, &[]);
        let assign = b.add(NodeKind::Assign, Payload::None, sp, &[write_target, seven]);
        let later = b.add(NodeKind::Lit, Payload::LitInt(9), sp, &[]);
        let ret = b.add(NodeKind::Return, Payload::None, sp, &[later]);
        let block = b.add(NodeKind::Block, Payload::None, sp, &[assign, ret]);
        let func = b.add(NodeKind::Func, Payload::None, sp, &[block]);
        let il = b.finish(
            func,
            FileMeta {
                path: "t".into(),
                lang: Lang::Python,
            },
            Vec::new(),
            Vec::new(),
        );
        run_admitted_unit(il, func, &[]).expect("run_unit")
    }

    #[test]
    fn field_write_propagates_receiver_err_before_cached_value() {
        let behavior = run_field_write_with_error_receiver();
        assert_eq!(behavior.ret, Value::Err);
        assert!(behavior.fields.is_empty());
    }

    fn run_self_field_writes(swapped: bool) -> Behavior {
        let interner = Interner::new();
        let this_name = interner.intern("this");
        let x_name = interner.intern("x");
        let y_name = interner.intern("y");
        let mut b = IlBuilder::new(FileId(0));
        let x_receiver = b.add(NodeKind::Var, Payload::Name(this_name), test_span(1), &[]);
        let x_target = b.add(
            NodeKind::Field,
            Payload::Name(x_name),
            test_span(2),
            &[x_receiver],
        );
        let one = b.add(NodeKind::Lit, Payload::LitInt(1), test_span(3), &[]);
        let x_assign = b.add(
            NodeKind::Assign,
            Payload::None,
            test_span(4),
            &[x_target, one],
        );
        let y_receiver = b.add(NodeKind::Var, Payload::Name(this_name), test_span(5), &[]);
        let y_target = b.add(
            NodeKind::Field,
            Payload::Name(y_name),
            test_span(6),
            &[y_receiver],
        );
        let two = b.add(NodeKind::Lit, Payload::LitInt(2), test_span(7), &[]);
        let y_assign = b.add(
            NodeKind::Assign,
            Payload::None,
            test_span(8),
            &[y_target, two],
        );
        let statements = if swapped {
            vec![y_assign, x_assign]
        } else {
            vec![x_assign, y_assign]
        };
        let block = b.add(NodeKind::Block, Payload::None, test_span(9), &statements);
        let func = b.add(NodeKind::Func, Payload::None, test_span(10), &[block]);
        let mut il = b.finish(
            func,
            FileMeta {
                path: "T.java".into(),
                lang: Lang::Java,
            },
            Vec::new(),
            Vec::new(),
        );
        admit_test_self_field_write(
            &mut il, &interner, x_receiver, x_target, x_assign, x_name, 2000,
        );
        admit_test_self_field_write(
            &mut il, &interner, y_receiver, y_target, y_assign, y_name, 2010,
        );
        run_unit(&il, &interner, func, &[]).expect("self-field writes should interpret")
    }

    #[test]
    fn self_field_final_state_is_order_insensitive() {
        assert_eq!(
            run_self_field_writes(false).fields,
            run_self_field_writes(true).fields
        );
    }

    fn run_try(body_stmt: NodeId, handler_stmt: NodeId, mut b: IlBuilder, sp: Span) -> Value {
        let body = b.add(NodeKind::Block, Payload::None, sp, &[body_stmt]);
        let handler = b.add(NodeKind::Block, Payload::None, sp, &[handler_stmt]);
        let try_node = b.add(NodeKind::Try, Payload::None, sp, &[body, handler]);
        let block = b.add(NodeKind::Block, Payload::None, sp, &[try_node]);
        let func = b.add(NodeKind::Func, Payload::None, sp, &[block]);
        let il = b.finish(
            func,
            FileMeta {
                path: "t".into(),
                lang: Lang::Python,
            },
            Vec::new(),
            Vec::new(),
        );
        run_admitted_unit(il, func, &[]).expect("run_unit").ret
    }

    #[test]
    fn try_handler_runs_on_throw_err() {
        let sp = Span::synthetic(FileId(0));
        let mut b = IlBuilder::new(FileId(0));
        let thrown = b.add(NodeKind::Lit, Payload::LitStr(0xBAD), sp, &[]);
        let throw = b.add(NodeKind::Throw, Payload::None, sp, &[thrown]);
        let seven = b.add(NodeKind::Lit, Payload::LitInt(7), sp, &[]);
        let handler_ret = b.add(NodeKind::Return, Payload::None, sp, &[seven]);
        assert_eq!(run_try(throw, handler_ret, b, sp), Value::Int(7));
    }

    #[test]
    fn try_handler_is_skipped_on_normal_return() {
        let sp = Span::synthetic(FileId(0));
        let mut b = IlBuilder::new(FileId(0));
        let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
        let body_ret = b.add(NodeKind::Return, Payload::None, sp, &[one]);
        let seven = b.add(NodeKind::Lit, Payload::LitInt(7), sp, &[]);
        let handler_ret = b.add(NodeKind::Return, Payload::None, sp, &[seven]);
        assert_eq!(run_try(body_ret, handler_ret, b, sp), Value::Int(1));
    }

    #[test]
    fn try_handler_catches_return_expression_err() {
        let sp = Span::synthetic(FileId(0));
        let mut b = IlBuilder::new(FileId(0));
        let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
        let zero = b.add(NodeKind::Lit, Payload::LitInt(0), sp, &[]);
        let div = b.add(NodeKind::BinOp, Payload::Op(Op::Div), sp, &[one, zero]);
        let body_ret = b.add(NodeKind::Return, Payload::None, sp, &[div]);
        let seven = b.add(NodeKind::Lit, Payload::LitInt(7), sp, &[]);
        let handler_ret = b.add(NodeKind::Return, Payload::None, sp, &[seven]);
        assert_eq!(run_try(body_ret, handler_ret, b, sp), Value::Int(7));
    }

    #[test]
    fn try_handler_catches_assignment_expression_err() {
        let sp = Span::synthetic(FileId(0));
        let mut b = IlBuilder::new(FileId(0));
        let var = b.add(NodeKind::Var, Payload::Cid(0), sp, &[]);
        let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
        let zero = b.add(NodeKind::Lit, Payload::LitInt(0), sp, &[]);
        let div = b.add(NodeKind::BinOp, Payload::Op(Op::Div), sp, &[one, zero]);
        let assign = b.add(NodeKind::Assign, Payload::None, sp, &[var, div]);
        let seven = b.add(NodeKind::Lit, Payload::LitInt(7), sp, &[]);
        let handler_ret = b.add(NodeKind::Return, Payload::None, sp, &[seven]);
        assert_eq!(run_try(assign, handler_ret, b, sp), Value::Int(7));
    }

    fn append_with_error_item_value() -> Value {
        let sp = Span::synthetic(FileId(0));
        let mut b = IlBuilder::new(FileId(0));
        let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
        let list = b.add(NodeKind::Seq, Payload::None, sp, &[one]);
        let zero = b.add(NodeKind::Lit, Payload::LitInt(0), sp, &[]);
        let div = b.add(NodeKind::BinOp, Payload::Op(Op::Div), sp, &[one, zero]);
        let append = b.add(
            NodeKind::Call,
            Payload::Builtin(Builtin::Append),
            sp,
            &[list, div],
        );
        let ret = b.add(NodeKind::Return, Payload::None, sp, &[append]);
        let func = b.add(NodeKind::Func, Payload::None, sp, &[ret]);
        let il = b.finish(
            func,
            FileMeta {
                path: "t".into(),
                lang: Lang::Go,
            },
            Vec::new(),
            Vec::new(),
        );
        run_admitted_unit(il, func, &[]).expect("run_unit").ret
    }

    #[test]
    fn value_append_propagates_error_items() {
        assert_eq!(append_with_error_item_value(), Value::Err);
    }

    fn statement_append_with_error_item_value() -> Value {
        let sp = Span::synthetic(FileId(0));
        let mut b = IlBuilder::new(FileId(0));
        let var = b.add(NodeKind::Var, Payload::Cid(0), sp, &[]);
        let empty = b.add(NodeKind::Seq, Payload::None, sp, &[]);
        let assign = b.add(NodeKind::Assign, Payload::None, sp, &[var, empty]);
        let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
        let zero = b.add(NodeKind::Lit, Payload::LitInt(0), sp, &[]);
        let div = b.add(NodeKind::BinOp, Payload::Op(Op::Div), sp, &[one, zero]);
        let append = b.add(
            NodeKind::Call,
            Payload::Builtin(Builtin::Append),
            sp,
            &[var, div],
        );
        let append_stmt = b.add(NodeKind::ExprStmt, Payload::None, sp, &[append]);
        let ret = b.add(NodeKind::Return, Payload::None, sp, &[var]);
        let block = b.add(
            NodeKind::Block,
            Payload::None,
            sp,
            &[assign, append_stmt, ret],
        );
        let func = b.add(NodeKind::Func, Payload::None, sp, &[block]);
        let il = b.finish(
            func,
            FileMeta {
                path: "t".into(),
                lang: Lang::Python,
            },
            Vec::new(),
            Vec::new(),
        );
        run_admitted_unit(il, func, &[]).expect("run_unit").ret
    }

    #[test]
    fn statement_append_propagates_error_items() {
        assert_eq!(statement_append_with_error_item_value(), Value::Err);
    }

    fn statement_append_on_error_target_with_effect_arg() -> Behavior {
        let sp = Span::synthetic(FileId(0));
        let mut b = IlBuilder::new(FileId(0));
        let param = b.add(NodeKind::Param, Payload::Cid(0), sp, &[]);
        let target = b.add(NodeKind::Var, Payload::Cid(0), sp, &[]);
        let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
        let print = b.add(NodeKind::Call, Payload::Builtin(Builtin::Print), sp, &[one]);
        let append = b.add(
            NodeKind::Call,
            Payload::Builtin(Builtin::Append),
            sp,
            &[target, print],
        );
        let append_stmt = b.add(NodeKind::ExprStmt, Payload::None, sp, &[append]);
        let seven = b.add(NodeKind::Lit, Payload::LitInt(7), sp, &[]);
        let ret = b.add(NodeKind::Return, Payload::None, sp, &[seven]);
        let block = b.add(NodeKind::Block, Payload::None, sp, &[append_stmt, ret]);
        let func = b.add(NodeKind::Func, Payload::None, sp, &[param, block]);
        let il = b.finish(
            func,
            FileMeta {
                path: "t".into(),
                lang: Lang::Python,
            },
            Vec::new(),
            Vec::new(),
        );
        run_admitted_unit(il, func, &[Value::Err]).expect("run_unit")
    }

    #[test]
    fn statement_append_checks_error_target_before_items() {
        let behavior = statement_append_on_error_target_with_effect_arg();
        assert_eq!(behavior.ret, Value::Err);
        assert!(behavior.effects.is_empty());
    }

    fn statement_append_on_error_expr_target_with_effect_arg() -> Behavior {
        let sp = Span::synthetic(FileId(0));
        let mut b = IlBuilder::new(FileId(0));
        let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
        let zero = b.add(NodeKind::Lit, Payload::LitInt(0), sp, &[]);
        let target_err = b.add(NodeKind::BinOp, Payload::Op(Op::Div), sp, &[one, zero]);
        let print = b.add(NodeKind::Call, Payload::Builtin(Builtin::Print), sp, &[one]);
        let append = b.add(
            NodeKind::Call,
            Payload::Builtin(Builtin::Append),
            sp,
            &[target_err, print],
        );
        let append_stmt = b.add(NodeKind::ExprStmt, Payload::None, sp, &[append]);
        let seven = b.add(NodeKind::Lit, Payload::LitInt(7), sp, &[]);
        let ret = b.add(NodeKind::Return, Payload::None, sp, &[seven]);
        let block = b.add(NodeKind::Block, Payload::None, sp, &[append_stmt, ret]);
        let func = b.add(NodeKind::Func, Payload::None, sp, &[block]);
        let il = b.finish(
            func,
            FileMeta {
                path: "t".into(),
                lang: Lang::Python,
            },
            Vec::new(),
            Vec::new(),
        );
        run_admitted_unit(il, func, &[]).expect("run_unit")
    }

    #[test]
    fn statement_append_checks_error_expr_target_before_items() {
        let behavior = statement_append_on_error_expr_target_with_effect_arg();
        assert_eq!(behavior.ret, Value::Err);
        assert!(behavior.effects.is_empty());
    }

    fn statement_append_on_error_field_receiver_with_effect_arg() -> Behavior {
        let sp = Span::synthetic(FileId(0));
        let mut b = IlBuilder::new(FileId(0));
        let interner = Interner::new();
        let field_name = interner.intern("x");
        let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
        let zero = b.add(NodeKind::Lit, Payload::LitInt(0), sp, &[]);
        let receiver_err = b.add(NodeKind::BinOp, Payload::Op(Op::Div), sp, &[one, zero]);
        let target = b.add(
            NodeKind::Field,
            Payload::Name(field_name),
            sp,
            &[receiver_err],
        );
        let print = b.add(NodeKind::Call, Payload::Builtin(Builtin::Print), sp, &[one]);
        let append = b.add(
            NodeKind::Call,
            Payload::Builtin(Builtin::Append),
            sp,
            &[target, print],
        );
        let append_stmt = b.add(NodeKind::ExprStmt, Payload::None, sp, &[append]);
        let seven = b.add(NodeKind::Lit, Payload::LitInt(7), sp, &[]);
        let ret = b.add(NodeKind::Return, Payload::None, sp, &[seven]);
        let block = b.add(NodeKind::Block, Payload::None, sp, &[append_stmt, ret]);
        let func = b.add(NodeKind::Func, Payload::None, sp, &[block]);
        let il = b.finish(
            func,
            FileMeta {
                path: "t".into(),
                lang: Lang::Python,
            },
            Vec::new(),
            Vec::new(),
        );
        run_admitted_unit(il, func, &[]).expect("run_unit")
    }

    #[test]
    fn statement_append_checks_error_field_receiver_before_items() {
        let behavior = statement_append_on_error_field_receiver_with_effect_arg();
        assert_eq!(behavior.ret, Value::Err);
        assert!(behavior.effects.is_empty());
    }

    fn index_on_error_base_with_effect_index() -> Behavior {
        let sp = Span::synthetic(FileId(0));
        let mut b = IlBuilder::new(FileId(0));
        let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
        let zero = b.add(NodeKind::Lit, Payload::LitInt(0), sp, &[]);
        let base_err = b.add(NodeKind::BinOp, Payload::Op(Op::Div), sp, &[one, zero]);
        let print = b.add(NodeKind::Call, Payload::Builtin(Builtin::Print), sp, &[one]);
        let index = b.add(NodeKind::Index, Payload::None, sp, &[base_err, print]);
        let index_stmt = b.add(NodeKind::ExprStmt, Payload::None, sp, &[index]);
        let seven = b.add(NodeKind::Lit, Payload::LitInt(7), sp, &[]);
        let ret = b.add(NodeKind::Return, Payload::None, sp, &[seven]);
        let block = b.add(NodeKind::Block, Payload::None, sp, &[index_stmt, ret]);
        let func = b.add(NodeKind::Func, Payload::None, sp, &[block]);
        let il = b.finish(
            func,
            FileMeta {
                path: "t".into(),
                lang: Lang::Python,
            },
            Vec::new(),
            Vec::new(),
        );
        run_admitted_unit(il, func, &[]).expect("run_unit")
    }

    #[test]
    fn index_checks_error_base_before_index_expr() {
        let behavior = index_on_error_base_with_effect_index();
        assert_eq!(behavior.ret, Value::Err);
        assert!(behavior.effects.is_empty());
    }

    fn binop_on_error_left_with_effect_right() -> Behavior {
        let sp = Span::synthetic(FileId(0));
        let mut b = IlBuilder::new(FileId(0));
        let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
        let zero = b.add(NodeKind::Lit, Payload::LitInt(0), sp, &[]);
        let left_err = b.add(NodeKind::BinOp, Payload::Op(Op::Div), sp, &[one, zero]);
        let print = b.add(NodeKind::Call, Payload::Builtin(Builtin::Print), sp, &[one]);
        let add = b.add(
            NodeKind::BinOp,
            Payload::Op(Op::Add),
            sp,
            &[left_err, print],
        );
        let add_stmt = b.add(NodeKind::ExprStmt, Payload::None, sp, &[add]);
        let seven = b.add(NodeKind::Lit, Payload::LitInt(7), sp, &[]);
        let ret = b.add(NodeKind::Return, Payload::None, sp, &[seven]);
        let block = b.add(NodeKind::Block, Payload::None, sp, &[add_stmt, ret]);
        let func = b.add(NodeKind::Func, Payload::None, sp, &[block]);
        let il = b.finish(
            func,
            FileMeta {
                path: "t".into(),
                lang: Lang::Python,
            },
            Vec::new(),
            Vec::new(),
        );
        run_admitted_unit(il, func, &[]).expect("run_unit")
    }

    #[test]
    fn binop_checks_error_left_before_right_expr() {
        let behavior = binop_on_error_left_with_effect_right();
        assert_eq!(behavior.ret, Value::Err);
        assert!(behavior.effects.is_empty());
    }

    #[test]
    fn try_handler_catches_statement_append_item_err() {
        let sp = Span::synthetic(FileId(0));
        let mut b = IlBuilder::new(FileId(0));
        let var = b.add(NodeKind::Var, Payload::Cid(0), sp, &[]);
        let empty = b.add(NodeKind::Seq, Payload::None, sp, &[]);
        let assign = b.add(NodeKind::Assign, Payload::None, sp, &[var, empty]);
        let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
        let zero = b.add(NodeKind::Lit, Payload::LitInt(0), sp, &[]);
        let div = b.add(NodeKind::BinOp, Payload::Op(Op::Div), sp, &[one, zero]);
        let append = b.add(
            NodeKind::Call,
            Payload::Builtin(Builtin::Append),
            sp,
            &[var, div],
        );
        let append_stmt = b.add(NodeKind::ExprStmt, Payload::None, sp, &[append]);
        let body = b.add(NodeKind::Block, Payload::None, sp, &[assign, append_stmt]);
        let seven = b.add(NodeKind::Lit, Payload::LitInt(7), sp, &[]);
        let handler_ret = b.add(NodeKind::Return, Payload::None, sp, &[seven]);
        let handler = b.add(NodeKind::Block, Payload::None, sp, &[handler_ret]);
        let try_node = b.add(NodeKind::Try, Payload::None, sp, &[body, handler]);
        let block = b.add(NodeKind::Block, Payload::None, sp, &[try_node]);
        let func = b.add(NodeKind::Func, Payload::None, sp, &[block]);
        let il = b.finish(
            func,
            FileMeta {
                path: "t".into(),
                lang: Lang::Python,
            },
            Vec::new(),
            Vec::new(),
        );
        assert_eq!(
            run_admitted_unit(il, func, &[]).expect("run_unit").ret,
            Value::Int(7)
        );
    }

    #[test]
    fn expression_statement_err_stops_later_execution() {
        let sp = Span::synthetic(FileId(0));
        let mut b = IlBuilder::new(FileId(0));
        let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
        let zero = b.add(NodeKind::Lit, Payload::LitInt(0), sp, &[]);
        let div = b.add(NodeKind::BinOp, Payload::Op(Op::Div), sp, &[one, zero]);
        let expr_stmt = b.add(NodeKind::ExprStmt, Payload::None, sp, &[div]);
        let later = b.add(NodeKind::Lit, Payload::LitInt(9), sp, &[]);
        let ret = b.add(NodeKind::Return, Payload::None, sp, &[later]);
        let block = b.add(NodeKind::Block, Payload::None, sp, &[expr_stmt, ret]);
        let func = b.add(NodeKind::Func, Payload::None, sp, &[block]);
        let il = b.finish(
            func,
            FileMeta {
                path: "t".into(),
                lang: Lang::Python,
            },
            Vec::new(),
            Vec::new(),
        );
        assert_eq!(
            run_admitted_unit(il, func, &[]).expect("run_unit").ret,
            Value::Err
        );
    }

    fn seq_with_error_item_value() -> Value {
        let sp = Span::synthetic(FileId(0));
        let mut b = IlBuilder::new(FileId(0));
        let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
        let zero = b.add(NodeKind::Lit, Payload::LitInt(0), sp, &[]);
        let div = b.add(NodeKind::BinOp, Payload::Op(Op::Div), sp, &[one, zero]);
        let seq = b.add(NodeKind::Seq, Payload::None, sp, &[div]);
        let ret = b.add(NodeKind::Return, Payload::None, sp, &[seq]);
        let func = b.add(NodeKind::Func, Payload::None, sp, &[ret]);
        let il = b.finish(
            func,
            FileMeta {
                path: "t".into(),
                lang: Lang::Python,
            },
            Vec::new(),
            Vec::new(),
        );
        run_admitted_unit(il, func, &[]).expect("run_unit").ret
    }

    #[test]
    fn seq_expression_propagates_error_items() {
        assert_eq!(seq_with_error_item_value(), Value::Err);
    }

    fn hof_with_error_lambda(kind: HoFKind) -> Value {
        let sp = Span::synthetic(FileId(0));
        let mut b = IlBuilder::new(FileId(0));
        let param = b.add(NodeKind::Param, Payload::Cid(0), sp, &[]);
        let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
        let zero = b.add(NodeKind::Lit, Payload::LitInt(0), sp, &[]);
        let div = b.add(NodeKind::BinOp, Payload::Op(Op::Div), sp, &[one, zero]);
        let lambda_ret = b.add(NodeKind::Return, Payload::None, sp, &[div]);
        let lambda_body = b.add(NodeKind::Block, Payload::None, sp, &[lambda_ret]);
        let lambda = b.add(NodeKind::Lambda, Payload::None, sp, &[param, lambda_body]);
        let coll = b.add(NodeKind::Seq, Payload::None, sp, &[one]);
        let hof = b.add(NodeKind::HoF, Payload::HoF(kind), sp, &[coll, lambda]);
        let ret = b.add(NodeKind::Return, Payload::None, sp, &[hof]);
        let func = b.add(NodeKind::Func, Payload::None, sp, &[ret]);
        let il = b.finish(
            func,
            FileMeta {
                path: "t".into(),
                lang: Lang::Python,
            },
            Vec::new(),
            Vec::new(),
        );
        run_admitted_unit(il, func, &[]).expect("run_unit").ret
    }

    #[test]
    fn hof_map_propagates_lambda_errors() {
        assert_eq!(hof_with_error_lambda(HoFKind::Map), Value::Err);
    }

    #[test]
    fn hof_filter_propagates_lambda_errors() {
        assert_eq!(hof_with_error_lambda(HoFKind::Filter), Value::Err);
    }

    #[test]
    fn hof_flat_map_propagates_lambda_errors() {
        assert_eq!(hof_with_error_lambda(HoFKind::FlatMap), Value::Err);
    }

    #[test]
    fn hof_filter_map_propagates_lambda_errors() {
        assert_eq!(hof_with_error_lambda(HoFKind::FilterMap), Value::Err);
    }

    fn hof_flat_map_value() -> Value {
        let sp = Span::synthetic(FileId(0));
        let mut b = IlBuilder::new(FileId(0));
        let param = b.add(NodeKind::Param, Payload::Cid(0), sp, &[]);
        let var = b.add(NodeKind::Var, Payload::Cid(0), sp, &[]);
        let pair = b.add(NodeKind::Seq, Payload::None, sp, &[var, var]);
        let lambda_ret = b.add(NodeKind::Return, Payload::None, sp, &[pair]);
        let lambda_body = b.add(NodeKind::Block, Payload::None, sp, &[lambda_ret]);
        let lambda = b.add(NodeKind::Lambda, Payload::None, sp, &[param, lambda_body]);
        let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
        let two = b.add(NodeKind::Lit, Payload::LitInt(2), sp, &[]);
        let coll = b.add(NodeKind::Seq, Payload::None, sp, &[one, two]);
        let hof = b.add(
            NodeKind::HoF,
            Payload::HoF(HoFKind::FlatMap),
            sp,
            &[coll, lambda],
        );
        let ret = b.add(NodeKind::Return, Payload::None, sp, &[hof]);
        let func = b.add(NodeKind::Func, Payload::None, sp, &[ret]);
        let il = b.finish(
            func,
            FileMeta {
                path: "t".into(),
                lang: Lang::Python,
            },
            Vec::new(),
            Vec::new(),
        );
        run_admitted_unit(il, func, &[]).expect("run_unit").ret
    }

    #[test]
    fn hof_flat_map_flattens_lambda_lists() {
        assert_eq!(
            hof_flat_map_value(),
            Value::List(vec![
                Value::Int(1),
                Value::Int(1),
                Value::Int(2),
                Value::Int(2)
            ])
        );
    }

    fn hof_scalar_flat_map_value() -> Value {
        let sp = Span::synthetic(FileId(0));
        let mut b = IlBuilder::new(FileId(0));
        let param = b.add(NodeKind::Param, Payload::Cid(0), sp, &[]);
        let var = b.add(NodeKind::Var, Payload::Cid(0), sp, &[]);
        let lambda_ret = b.add(NodeKind::Return, Payload::None, sp, &[var]);
        let lambda_body = b.add(NodeKind::Block, Payload::None, sp, &[lambda_ret]);
        let lambda = b.add(NodeKind::Lambda, Payload::None, sp, &[param, lambda_body]);
        let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
        let coll = b.add(NodeKind::Seq, Payload::None, sp, &[one]);
        let hof = b.add(
            NodeKind::HoF,
            Payload::HoF(HoFKind::FlatMap),
            sp,
            &[coll, lambda],
        );
        let ret = b.add(NodeKind::Return, Payload::None, sp, &[hof]);
        let func = b.add(NodeKind::Func, Payload::None, sp, &[ret]);
        let il = b.finish(
            func,
            FileMeta {
                path: "t".into(),
                lang: Lang::Python,
            },
            Vec::new(),
            Vec::new(),
        );
        run_admitted_unit(il, func, &[]).expect("run_unit").ret
    }

    #[test]
    fn hof_flat_map_scalar_lambda_result_is_err() {
        assert_eq!(hof_scalar_flat_map_value(), Value::Err);
    }

    fn hof_filter_map_value() -> Value {
        let sp = Span::synthetic(FileId(0));
        let mut b = IlBuilder::new(FileId(0));
        let param = b.add(NodeKind::Param, Payload::Cid(0), sp, &[]);
        let var = b.add(NodeKind::Var, Payload::Cid(0), sp, &[]);
        let zero = b.add(NodeKind::Lit, Payload::LitInt(0), sp, &[]);
        let is_zero = b.add(NodeKind::BinOp, Payload::Op(Op::Eq), sp, &[var, zero]);
        let var_again = b.add(NodeKind::Var, Payload::Cid(0), sp, &[]);
        let none = b.add(NodeKind::Lit, Payload::Lit(LitClass::Null), sp, &[]);
        let selected = b.add(NodeKind::If, Payload::None, sp, &[is_zero, var_again, none]);
        let lambda_ret = b.add(NodeKind::Return, Payload::None, sp, &[selected]);
        let lambda_body = b.add(NodeKind::Block, Payload::None, sp, &[lambda_ret]);
        let lambda = b.add(NodeKind::Lambda, Payload::None, sp, &[param, lambda_body]);
        let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
        let coll = b.add(NodeKind::Seq, Payload::None, sp, &[zero, one]);
        let hof = b.add(
            NodeKind::HoF,
            Payload::HoF(HoFKind::FilterMap),
            sp,
            &[coll, lambda],
        );
        let ret = b.add(NodeKind::Return, Payload::None, sp, &[hof]);
        let func = b.add(NodeKind::Func, Payload::None, sp, &[ret]);
        let il = b.finish(
            func,
            FileMeta {
                path: "t".into(),
                lang: Lang::Rust,
            },
            Vec::new(),
            Vec::new(),
        );
        run_admitted_unit(il, func, &[]).expect("run_unit").ret
    }

    #[test]
    fn hof_filter_map_drops_null_and_keeps_falsey_values() {
        assert_eq!(hof_filter_map_value(), Value::List(vec![Value::Int(0)]));
    }

    fn hof_with_empty_collection_and_error_lambda(kind: HoFKind) -> Value {
        let sp = Span::synthetic(FileId(0));
        let mut b = IlBuilder::new(FileId(0));
        let param = b.add(NodeKind::Param, Payload::Cid(0), sp, &[]);
        let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
        let zero = b.add(NodeKind::Lit, Payload::LitInt(0), sp, &[]);
        let div = b.add(NodeKind::BinOp, Payload::Op(Op::Div), sp, &[one, zero]);
        let lambda_ret = b.add(NodeKind::Return, Payload::None, sp, &[div]);
        let lambda_body = b.add(NodeKind::Block, Payload::None, sp, &[lambda_ret]);
        let lambda = b.add(NodeKind::Lambda, Payload::None, sp, &[param, lambda_body]);
        let coll = b.add(NodeKind::Seq, Payload::None, sp, &[]);
        let hof = b.add(NodeKind::HoF, Payload::HoF(kind), sp, &[coll, lambda]);
        let ret = b.add(NodeKind::Return, Payload::None, sp, &[hof]);
        let func = b.add(NodeKind::Func, Payload::None, sp, &[ret]);
        let il = b.finish(
            func,
            FileMeta {
                path: "t".into(),
                lang: Lang::Python,
            },
            Vec::new(),
            Vec::new(),
        );
        run_admitted_unit(il, func, &[]).expect("run_unit").ret
    }

    #[test]
    fn hof_empty_collections_skip_lambda_errors() {
        let empty = Value::List(Vec::new());
        assert_eq!(
            hof_with_empty_collection_and_error_lambda(HoFKind::Map),
            empty
        );
        assert_eq!(
            hof_with_empty_collection_and_error_lambda(HoFKind::Filter),
            empty
        );
        assert_eq!(
            hof_with_empty_collection_and_error_lambda(HoFKind::FlatMap),
            empty
        );
        assert_eq!(
            hof_with_empty_collection_and_error_lambda(HoFKind::FilterMap),
            empty
        );
    }

    fn hof_filter_map_with_scalar_collection() -> Value {
        let sp = Span::synthetic(FileId(0));
        let mut b = IlBuilder::new(FileId(0));
        let param = b.add(NodeKind::Param, Payload::Cid(0), sp, &[]);
        let var = b.add(NodeKind::Var, Payload::Cid(0), sp, &[]);
        let lambda_ret = b.add(NodeKind::Return, Payload::None, sp, &[var]);
        let lambda_body = b.add(NodeKind::Block, Payload::None, sp, &[lambda_ret]);
        let lambda = b.add(NodeKind::Lambda, Payload::None, sp, &[param, lambda_body]);
        let scalar = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
        let hof = b.add(
            NodeKind::HoF,
            Payload::HoF(HoFKind::FilterMap),
            sp,
            &[scalar, lambda],
        );
        let ret = b.add(NodeKind::Return, Payload::None, sp, &[hof]);
        let func = b.add(NodeKind::Func, Payload::None, sp, &[ret]);
        let il = b.finish(
            func,
            FileMeta {
                path: "t".into(),
                lang: Lang::Rust,
            },
            Vec::new(),
            Vec::new(),
        );
        run_admitted_unit(il, func, &[]).expect("run_unit").ret
    }

    #[test]
    fn hof_filter_map_scalar_collection_is_err() {
        assert_eq!(hof_filter_map_with_scalar_collection(), Value::Err);
    }

    fn hof_filter_map_with_captured_value() -> Value {
        let sp = Span::synthetic(FileId(0));
        let mut b = IlBuilder::new(FileId(0));
        let offset_var = b.add(NodeKind::Var, Payload::Cid(1), sp, &[]);
        let ten = b.add(NodeKind::Lit, Payload::LitInt(10), sp, &[]);
        let assign_offset = b.add(NodeKind::Assign, Payload::None, sp, &[offset_var, ten]);
        let param = b.add(NodeKind::Param, Payload::Cid(0), sp, &[]);
        let x = b.add(NodeKind::Var, Payload::Cid(0), sp, &[]);
        let offset = b.add(NodeKind::Var, Payload::Cid(1), sp, &[]);
        let sum = b.add(NodeKind::BinOp, Payload::Op(Op::Add), sp, &[x, offset]);
        let lambda_ret = b.add(NodeKind::Return, Payload::None, sp, &[sum]);
        let lambda_body = b.add(NodeKind::Block, Payload::None, sp, &[lambda_ret]);
        let lambda = b.add(NodeKind::Lambda, Payload::None, sp, &[param, lambda_body]);
        let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
        let two = b.add(NodeKind::Lit, Payload::LitInt(2), sp, &[]);
        let coll = b.add(NodeKind::Seq, Payload::None, sp, &[one, two]);
        let hof = b.add(
            NodeKind::HoF,
            Payload::HoF(HoFKind::FilterMap),
            sp,
            &[coll, lambda],
        );
        let ret = b.add(NodeKind::Return, Payload::None, sp, &[hof]);
        let block = b.add(NodeKind::Block, Payload::None, sp, &[assign_offset, ret]);
        let func = b.add(NodeKind::Func, Payload::None, sp, &[block]);
        let il = b.finish(
            func,
            FileMeta {
                path: "t".into(),
                lang: Lang::Rust,
            },
            Vec::new(),
            Vec::new(),
        );
        run_admitted_unit(il, func, &[]).expect("run_unit").ret
    }

    #[test]
    fn hof_filter_map_lambda_captures_outer_environment() {
        assert_eq!(
            hof_filter_map_with_captured_value(),
            Value::List(vec![Value::Int(11), Value::Int(12)])
        );
    }

    fn hof_filter_map_effectful_lambda() -> Behavior {
        let sp = Span::synthetic(FileId(0));
        let mut b = IlBuilder::new(FileId(0));
        let param = b.add(NodeKind::Param, Payload::Cid(0), sp, &[]);
        let printed = b.add(NodeKind::Var, Payload::Cid(0), sp, &[]);
        let print = b.add(
            NodeKind::Call,
            Payload::Builtin(Builtin::Print),
            sp,
            &[printed],
        );
        let print_stmt = b.add(NodeKind::ExprStmt, Payload::None, sp, &[print]);
        let returned = b.add(NodeKind::Var, Payload::Cid(0), sp, &[]);
        let lambda_ret = b.add(NodeKind::Return, Payload::None, sp, &[returned]);
        let lambda_body = b.add(
            NodeKind::Block,
            Payload::None,
            sp,
            &[print_stmt, lambda_ret],
        );
        let lambda = b.add(NodeKind::Lambda, Payload::None, sp, &[param, lambda_body]);
        let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
        let two = b.add(NodeKind::Lit, Payload::LitInt(2), sp, &[]);
        let coll = b.add(NodeKind::Seq, Payload::None, sp, &[one, two]);
        let hof = b.add(
            NodeKind::HoF,
            Payload::HoF(HoFKind::FilterMap),
            sp,
            &[coll, lambda],
        );
        let ret = b.add(NodeKind::Return, Payload::None, sp, &[hof]);
        let func = b.add(NodeKind::Func, Payload::None, sp, &[ret]);
        let il = b.finish(
            func,
            FileMeta {
                path: "t".into(),
                lang: Lang::Rust,
            },
            Vec::new(),
            Vec::new(),
        );
        run_admitted_unit(il, func, &[]).expect("run_unit")
    }

    #[test]
    fn hof_filter_map_effectful_lambda_records_effects() {
        let behavior = hof_filter_map_effectful_lambda();
        assert_eq!(
            behavior.ret,
            Value::List(vec![Value::Int(1), Value::Int(2)])
        );
        assert_eq!(behavior.effects, vec![Value::Int(1), Value::Int(2)]);
    }

    fn java_stream_nested_expression_lambda_behavior(outer_kind: HoFKind) -> Behavior {
        let sp = Span::synthetic(FileId(0));
        let mut b = IlBuilder::new(FileId(0));
        let xs_param = b.add(NodeKind::Param, Payload::Cid(0), sp, &[]);
        let ys_param = b.add(NodeKind::Param, Payload::Cid(1), sp, &[]);

        let x_param = b.add(NodeKind::Param, Payload::Cid(2), sp, &[]);
        let y_param = b.add(NodeKind::Param, Payload::Cid(3), sp, &[]);
        let x_in_inner = b.add(NodeKind::Var, Payload::Cid(2), sp, &[]);
        let y = b.add(NodeKind::Var, Payload::Cid(3), sp, &[]);
        let sum = b.add(NodeKind::BinOp, Payload::Op(Op::Add), sp, &[x_in_inner, y]);
        let inner_lambda = b.add(NodeKind::Lambda, Payload::None, sp, &[y_param, sum]);
        let ys = b.add(NodeKind::Var, Payload::Cid(1), sp, &[]);
        let inner_map = b.add(
            NodeKind::HoF,
            Payload::HoF(HoFKind::Map),
            sp,
            &[ys, inner_lambda],
        );
        let outer_lambda = b.add(NodeKind::Lambda, Payload::None, sp, &[x_param, inner_map]);
        let xs = b.add(NodeKind::Var, Payload::Cid(0), sp, &[]);
        let outer = b.add(
            NodeKind::HoF,
            Payload::HoF(outer_kind),
            sp,
            &[xs, outer_lambda],
        );
        let ret = b.add(NodeKind::Return, Payload::None, sp, &[outer]);
        let func = b.add(
            NodeKind::Func,
            Payload::None,
            sp,
            &[xs_param, ys_param, ret],
        );
        let il = b.finish(
            func,
            FileMeta {
                path: "T.java".into(),
                lang: Lang::Java,
            },
            Vec::new(),
            Vec::new(),
        );
        run_admitted_unit(
            il,
            func,
            &[
                Value::List(vec![Value::Int(1), Value::Int(2)]),
                Value::List(vec![Value::Int(10), Value::Int(20)]),
            ],
        )
        .expect("run_unit")
    }

    #[test]
    fn java_stream_flat_map_expression_lambdas_are_interpretable() {
        assert_eq!(
            java_stream_nested_expression_lambda_behavior(HoFKind::FlatMap).ret,
            Value::List(vec![
                Value::Int(11),
                Value::Int(21),
                Value::Int(12),
                Value::Int(22),
            ])
        );
    }

    #[test]
    fn java_stream_map_returning_stream_stays_nested() {
        assert_eq!(
            java_stream_nested_expression_lambda_behavior(HoFKind::Map).ret,
            Value::List(vec![
                Value::List(vec![Value::Int(11), Value::Int(21)]),
                Value::List(vec![Value::Int(12), Value::Int(22)]),
            ])
        );
    }

    fn java_stream_flat_map_effectful_lambda_behavior() -> Behavior {
        let sp = Span::synthetic(FileId(0));
        let mut b = IlBuilder::new(FileId(0));
        let xs_param = b.add(NodeKind::Param, Payload::Cid(0), sp, &[]);

        let x_param = b.add(NodeKind::Param, Payload::Cid(1), sp, &[]);
        let printed = b.add(NodeKind::Var, Payload::Cid(1), sp, &[]);
        let print = b.add(
            NodeKind::Call,
            Payload::Builtin(Builtin::Print),
            sp,
            &[printed],
        );
        let print_stmt = b.add(NodeKind::ExprStmt, Payload::None, sp, &[print]);
        let returned = b.add(NodeKind::Var, Payload::Cid(1), sp, &[]);
        let single = b.add(NodeKind::Seq, Payload::None, sp, &[returned]);
        let lambda_ret = b.add(NodeKind::Return, Payload::None, sp, &[single]);
        let lambda_body = b.add(
            NodeKind::Block,
            Payload::None,
            sp,
            &[print_stmt, lambda_ret],
        );
        let lambda = b.add(NodeKind::Lambda, Payload::None, sp, &[x_param, lambda_body]);
        let xs = b.add(NodeKind::Var, Payload::Cid(0), sp, &[]);
        let flat_map = b.add(
            NodeKind::HoF,
            Payload::HoF(HoFKind::FlatMap),
            sp,
            &[xs, lambda],
        );
        let ret = b.add(NodeKind::Return, Payload::None, sp, &[flat_map]);
        let func = b.add(NodeKind::Func, Payload::None, sp, &[xs_param, ret]);
        let il = b.finish(
            func,
            FileMeta {
                path: "T.java".into(),
                lang: Lang::Java,
            },
            Vec::new(),
            Vec::new(),
        );
        run_admitted_unit(il, func, &[Value::List(vec![Value::Int(1), Value::Int(2)])])
            .expect("run_unit")
    }

    #[test]
    fn java_stream_flat_map_effectful_lambda_records_effects() {
        let behavior = java_stream_flat_map_effectful_lambda_behavior();
        assert_eq!(
            behavior.ret,
            Value::List(vec![Value::Int(1), Value::Int(2)])
        );
        assert_eq!(behavior.effects, vec![Value::Int(1), Value::Int(2)]);
    }

    fn print_with_error_arg_then_return() -> Value {
        let sp = Span::synthetic(FileId(0));
        let mut b = IlBuilder::new(FileId(0));
        let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
        let zero = b.add(NodeKind::Lit, Payload::LitInt(0), sp, &[]);
        let div = b.add(NodeKind::BinOp, Payload::Op(Op::Div), sp, &[one, zero]);
        let print = b.add(NodeKind::Call, Payload::Builtin(Builtin::Print), sp, &[div]);
        let print_stmt = b.add(NodeKind::ExprStmt, Payload::None, sp, &[print]);
        let seven = b.add(NodeKind::Lit, Payload::LitInt(7), sp, &[]);
        let ret = b.add(NodeKind::Return, Payload::None, sp, &[seven]);
        let block = b.add(NodeKind::Block, Payload::None, sp, &[print_stmt, ret]);
        let func = b.add(NodeKind::Func, Payload::None, sp, &[block]);
        let il = b.finish(
            func,
            FileMeta {
                path: "t".into(),
                lang: Lang::Python,
            },
            Vec::new(),
            Vec::new(),
        );
        run_admitted_unit(il, func, &[]).expect("run_unit").ret
    }

    #[test]
    fn eager_builtin_argument_err_stops_execution() {
        assert_eq!(print_with_error_arg_then_return(), Value::Err);
    }

    fn print_with_error_arg_before_effect_arg() -> Behavior {
        let sp = Span::synthetic(FileId(0));
        let mut b = IlBuilder::new(FileId(0));
        let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
        let zero = b.add(NodeKind::Lit, Payload::LitInt(0), sp, &[]);
        let div = b.add(NodeKind::BinOp, Payload::Op(Op::Div), sp, &[one, zero]);
        let nested_print = b.add(NodeKind::Call, Payload::Builtin(Builtin::Print), sp, &[one]);
        let print = b.add(
            NodeKind::Call,
            Payload::Builtin(Builtin::Print),
            sp,
            &[div, nested_print],
        );
        let print_stmt = b.add(NodeKind::ExprStmt, Payload::None, sp, &[print]);
        let seven = b.add(NodeKind::Lit, Payload::LitInt(7), sp, &[]);
        let ret = b.add(NodeKind::Return, Payload::None, sp, &[seven]);
        let block = b.add(NodeKind::Block, Payload::None, sp, &[print_stmt, ret]);
        let func = b.add(NodeKind::Func, Payload::None, sp, &[block]);
        let il = b.finish(
            func,
            FileMeta {
                path: "t".into(),
                lang: Lang::Python,
            },
            Vec::new(),
            Vec::new(),
        );
        run_admitted_unit(il, func, &[]).expect("run_unit")
    }

    #[test]
    fn eager_builtin_argument_err_stops_later_arguments() {
        let behavior = print_with_error_arg_before_effect_arg();
        assert_eq!(behavior.ret, Value::Err);
        assert!(behavior.effects.is_empty());
    }

    fn index_assignment_with_error_index_after_rhs_effect() -> Behavior {
        let sp = Span::synthetic(FileId(0));
        let mut b = IlBuilder::new(FileId(0));
        let param = b.add(NodeKind::Param, Payload::Cid(0), sp, &[]);
        let target_base = b.add(NodeKind::Var, Payload::Cid(0), sp, &[]);
        let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
        let zero = b.add(NodeKind::Lit, Payload::LitInt(0), sp, &[]);
        let index_err = b.add(NodeKind::BinOp, Payload::Op(Op::Div), sp, &[one, zero]);
        let target = b.add(
            NodeKind::Index,
            Payload::None,
            sp,
            &[target_base, index_err],
        );
        let print = b.add(NodeKind::Call, Payload::Builtin(Builtin::Print), sp, &[one]);
        let assign = b.add(NodeKind::Assign, Payload::None, sp, &[target, print]);
        let seven = b.add(NodeKind::Lit, Payload::LitInt(7), sp, &[]);
        let ret = b.add(NodeKind::Return, Payload::None, sp, &[seven]);
        let block = b.add(NodeKind::Block, Payload::None, sp, &[assign, ret]);
        let func = b.add(NodeKind::Func, Payload::None, sp, &[param, block]);
        let il = b.finish(
            func,
            FileMeta {
                path: "t".into(),
                lang: Lang::Python,
            },
            Vec::new(),
            Vec::new(),
        );
        run_admitted_unit(il, func, &[Value::List(Vec::new())]).expect("run_unit")
    }

    #[test]
    fn index_assignment_error_index_stops_after_rhs_effect() {
        let behavior = index_assignment_with_error_index_after_rhs_effect();
        assert_eq!(behavior.ret, Value::Err);
        assert_eq!(behavior.effects, vec![Value::Int(1)]);
    }

    fn index_assignment_with_error_base_before_index_effect() -> Behavior {
        let sp = Span::synthetic(FileId(0));
        let mut b = IlBuilder::new(FileId(0));
        let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
        let zero = b.add(NodeKind::Lit, Payload::LitInt(0), sp, &[]);
        let base_err = b.add(NodeKind::BinOp, Payload::Op(Op::Div), sp, &[one, zero]);
        let print = b.add(NodeKind::Call, Payload::Builtin(Builtin::Print), sp, &[one]);
        let target = b.add(NodeKind::Index, Payload::None, sp, &[base_err, print]);
        let seven = b.add(NodeKind::Lit, Payload::LitInt(7), sp, &[]);
        let assign = b.add(NodeKind::Assign, Payload::None, sp, &[target, seven]);
        let later = b.add(NodeKind::Lit, Payload::LitInt(9), sp, &[]);
        let ret = b.add(NodeKind::Return, Payload::None, sp, &[later]);
        let block = b.add(NodeKind::Block, Payload::None, sp, &[assign, ret]);
        let func = b.add(NodeKind::Func, Payload::None, sp, &[block]);
        let il = b.finish(
            func,
            FileMeta {
                path: "t".into(),
                lang: Lang::Python,
            },
            Vec::new(),
            Vec::new(),
        );
        run_admitted_unit(il, func, &[]).expect("run_unit")
    }

    #[test]
    fn index_assignment_checks_error_base_before_index_expr() {
        let behavior = index_assignment_with_error_base_before_index_effect();
        assert_eq!(behavior.ret, Value::Err);
        assert!(behavior.effects.is_empty());
    }

    fn self_call_with_error_arg_ignored_by_callee() -> Value {
        let sp = Span::synthetic(FileId(0));
        let mut b = IlBuilder::new(FileId(0));
        let interner = Interner::new();
        let func_name = interner.intern("f");
        let done_param = b.add(NodeKind::Param, Payload::Cid(0), sp, &[]);
        let ignored_param = b.add(NodeKind::Param, Payload::Cid(1), sp, &[]);
        let done_var = b.add(NodeKind::Var, Payload::Cid(0), sp, &[]);
        let seven = b.add(NodeKind::Lit, Payload::LitInt(7), sp, &[]);
        let done_ret = b.add(NodeKind::Return, Payload::None, sp, &[seven]);
        let if_done = b.add(NodeKind::If, Payload::None, sp, &[done_var, done_ret]);
        let callee = b.add(NodeKind::Var, Payload::Name(func_name), sp, &[]);
        let true_value = b.add(NodeKind::Lit, Payload::LitBool(true), sp, &[]);
        let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
        let zero = b.add(NodeKind::Lit, Payload::LitInt(0), sp, &[]);
        let div = b.add(NodeKind::BinOp, Payload::Op(Op::Div), sp, &[one, zero]);
        let recursive_call = b.add(
            NodeKind::Call,
            Payload::None,
            sp,
            &[callee, true_value, div],
        );
        let recursive_ret = b.add(NodeKind::Return, Payload::None, sp, &[recursive_call]);
        let body = b.add(
            NodeKind::Block,
            Payload::None,
            sp,
            &[if_done, recursive_ret],
        );
        let func = b.add(
            NodeKind::Func,
            Payload::None,
            sp,
            &[done_param, ignored_param, body],
        );
        let mut il = b.finish(
            func,
            FileMeta {
                path: "t".into(),
                lang: Lang::Python,
            },
            vec![Unit {
                root: func,
                kind: UnitKind::Function,
                name: Some(func_name),
            }],
            Vec::new(),
        );
        il.evidence.push(test_call_target_record(
            2000,
            il.node(recursive_call).span,
            il.node(func).span,
            interner.symbol_hash(func_name),
        ));
        run_admitted_unit(il, func, &[Value::Bool(false), Value::Int(0)])
            .expect("run_unit")
            .ret
    }

    #[test]
    fn self_call_argument_err_stops_execution() {
        assert_eq!(self_call_with_error_arg_ignored_by_callee(), Value::Err);
    }

    #[test]
    fn unproven_call_becomes_symbolic_application_keyed_by_callee() {
        let sp = Span::synthetic(FileId(0));
        let mut b = IlBuilder::new(FileId(0));
        let interner = Interner::new();
        let func_name = interner.intern("f");
        let callee = b.add(NodeKind::Var, Payload::Name(func_name), sp, &[]);
        let call = b.add(NodeKind::Call, Payload::None, sp, &[callee]);
        let ret = b.add(NodeKind::Return, Payload::None, sp, &[call]);
        let body = b.add(NodeKind::Block, Payload::None, sp, &[ret]);
        let func = b.add(NodeKind::Func, Payload::None, sp, &[body]);
        let il = b.finish(
            func,
            FileMeta {
                path: "t".into(),
                lang: Lang::Python,
            },
            vec![Unit {
                root: func,
                kind: UnitKind::Function,
                name: Some(func_name),
            }],
            Vec::new(),
        );

        // No call-target evidence: the call is OPAQUE, not a bail — a symbolic
        // application keyed by the callee's structural signature, effect-recorded.
        let beh = run_unit(&il, &interner, func, &[]).expect("symbolic run");
        assert!(matches!(beh.ret, Value::Sym(_)));
        assert_eq!(beh.effects, vec![beh.ret.clone()]);
    }

    /// Build `fn() { return <name>(<litint arg>) }` with an unproven callee.
    fn opaque_call_behavior(name: &str, arg: i64, interner: &Interner) -> Behavior {
        let sp = Span::synthetic(FileId(0));
        let mut b = IlBuilder::new(FileId(0));
        let callee = b.add(NodeKind::Var, Payload::Name(interner.intern(name)), sp, &[]);
        let a = b.add(NodeKind::Lit, Payload::LitInt(arg), sp, &[]);
        let call = b.add(NodeKind::Call, Payload::None, sp, &[callee, a]);
        let ret = b.add(NodeKind::Return, Payload::None, sp, &[call]);
        let body = b.add(NodeKind::Block, Payload::None, sp, &[ret]);
        let func = b.add(NodeKind::Func, Payload::None, sp, &[body]);
        let il = b.finish(
            func,
            FileMeta {
                path: "t".into(),
                lang: Lang::Python,
            },
            Vec::new(),
            Vec::new(),
        );
        run_unit(&il, interner, func, &[]).expect("symbolic run")
    }

    #[test]
    fn symbolic_application_is_differential_same_call_equal_else_distinct() {
        let interner = Interner::new();
        // Same callee + same argument: behaviors agree (the convention is comparable).
        assert_eq!(
            opaque_call_behavior("f", 3, &interner),
            opaque_call_behavior("f", 3, &interner)
        );
        // Different callee name or different argument: behaviors differ.
        assert_ne!(
            opaque_call_behavior("f", 3, &interner),
            opaque_call_behavior("g", 3, &interner)
        );
        assert_ne!(
            opaque_call_behavior("f", 3, &interner),
            opaque_call_behavior("f", 4, &interner)
        );
    }

    /// Two opaque calls in sequence: swapping their order swaps the effect trace —
    /// call order stays observable behavior under the symbolic convention.
    #[test]
    fn opaque_call_order_is_observable() {
        let interner = Interner::new();
        let build = |first: &str, second: &str| {
            let sp = Span::synthetic(FileId(0));
            let mut b = IlBuilder::new(FileId(0));
            let stmt = |name: &str, b: &mut IlBuilder| {
                let callee = b.add(NodeKind::Var, Payload::Name(interner.intern(name)), sp, &[]);
                let call = b.add(NodeKind::Call, Payload::None, sp, &[callee]);
                b.add(NodeKind::ExprStmt, Payload::None, sp, &[call])
            };
            let s1 = stmt(first, &mut b);
            let s2 = stmt(second, &mut b);
            let body = b.add(NodeKind::Block, Payload::None, sp, &[s1, s2]);
            let func = b.add(NodeKind::Func, Payload::None, sp, &[body]);
            let il = b.finish(
                func,
                FileMeta {
                    path: "t".into(),
                    lang: Lang::Python,
                },
                Vec::new(),
                Vec::new(),
            );
            run_unit(&il, &interner, func, &[]).expect("symbolic run")
        };
        assert_eq!(build("f", "g"), build("f", "g"));
        assert_ne!(build("f", "g"), build("g", "f"));
    }

    /// Branching on a symbolic value still bails the unit — control flow is never guessed.
    #[test]
    fn branch_on_symbolic_value_bails() {
        let interner = Interner::new();
        let sp = Span::synthetic(FileId(0));
        let mut b = IlBuilder::new(FileId(0));
        let callee = b.add(NodeKind::Var, Payload::Name(interner.intern("f")), sp, &[]);
        let call = b.add(NodeKind::Call, Payload::None, sp, &[callee]);
        let t = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
        let e = b.add(NodeKind::Lit, Payload::LitInt(2), sp, &[]);
        let ternary = b.add(NodeKind::If, Payload::None, sp, &[call, t, e]);
        let ret = b.add(NodeKind::Return, Payload::None, sp, &[ternary]);
        let body = b.add(NodeKind::Block, Payload::None, sp, &[ret]);
        let func = b.add(NodeKind::Func, Payload::None, sp, &[body]);
        let il = b.finish(
            func,
            FileMeta {
                path: "t".into(),
                lang: Lang::Python,
            },
            Vec::new(),
            Vec::new(),
        );
        assert!(run_unit(&il, &interner, func, &[]).is_none());
    }

    /// A symbolic operand inside a concrete operation must stay symbolic — collapsing
    /// `len([f(x)])`-shaped compositions to a concrete `Err` would launder unknownness
    /// into the hard soundness lane.
    #[test]
    fn symbolic_operand_never_launders_to_concrete_err() {
        let s = Value::Sym(7);
        assert!(matches!(bin(Op::Add, &Value::Int(1), &s), Value::Sym(_)));
        assert!(matches!(bin(Op::Eq, &s, &s), Value::Sym(_)));
        assert!(matches!(un(Op::Not, &s), Value::Sym(_)));
        assert!(matches!(
            un(Op::Neg, &Value::List(vec![s.clone()])),
            Value::Sym(_)
        ));
        assert!(contains_sym(&Value::List(vec![Value::Int(1), s])));
    }

    /// `g(x) = x*x` and `f(x) = g(x) + 1` in one file — running `f(3)` must interpret the
    /// cross-function call to `g` (not bail out as opaque), giving `3*3 + 1 = 10`. This is what
    /// lets the oracle validate the interprocedural-inline canonicalization.
    fn cross_function_call_result() -> Value {
        let sp = |n| Span {
            file: FileId(0),
            start_byte: n,
            end_byte: n + 1,
            start_line: n,
            end_line: n,
        };
        let mut b = IlBuilder::new(FileId(0));
        let interner = Interner::new();
        let g_name = interner.intern("g");
        let f_name = interner.intern("f");
        // g(x) = x * x
        let g_param = b.add(NodeKind::Param, Payload::Cid(0), sp(1), &[]);
        let gx1 = b.add(NodeKind::Var, Payload::Cid(0), sp(2), &[]);
        let gx2 = b.add(NodeKind::Var, Payload::Cid(0), sp(3), &[]);
        let g_mul = b.add(NodeKind::BinOp, Payload::Op(Op::Mul), sp(4), &[gx1, gx2]);
        let g_ret = b.add(NodeKind::Return, Payload::None, sp(5), &[g_mul]);
        let g_body = b.add(NodeKind::Block, Payload::None, sp(6), &[g_ret]);
        let g_func = b.add(NodeKind::Func, Payload::None, sp(7), &[g_param, g_body]);
        // f(x) = g(x) + 1
        let f_param = b.add(NodeKind::Param, Payload::Cid(0), sp(8), &[]);
        let callee = b.add(NodeKind::Var, Payload::Name(g_name), sp(9), &[]);
        let fx = b.add(NodeKind::Var, Payload::Cid(0), sp(10), &[]);
        let call = b.add(NodeKind::Call, Payload::None, sp(11), &[callee, fx]);
        let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp(12), &[]);
        let f_add = b.add(NodeKind::BinOp, Payload::Op(Op::Add), sp(13), &[call, one]);
        let f_ret = b.add(NodeKind::Return, Payload::None, sp(14), &[f_add]);
        let f_body = b.add(NodeKind::Block, Payload::None, sp(15), &[f_ret]);
        let f_func = b.add(NodeKind::Func, Payload::None, sp(16), &[f_param, f_body]);
        let mut il = b.finish(
            f_func,
            FileMeta {
                path: "t".into(),
                lang: Lang::Python,
            },
            vec![
                Unit {
                    root: g_func,
                    kind: UnitKind::Function,
                    name: Some(g_name),
                },
                Unit {
                    root: f_func,
                    kind: UnitKind::Function,
                    name: Some(f_name),
                },
            ],
            Vec::new(),
        );
        il.evidence.push(test_call_target_record(
            2001,
            il.node(call).span,
            il.node(g_func).span,
            interner.symbol_hash(g_name),
        ));
        run_admitted_unit(il, f_func, &[Value::Int(3)])
            .expect("run_unit")
            .ret
    }

    #[test]
    fn cross_function_call_is_interpreted() {
        assert_eq!(cross_function_call_result(), Value::Int(10));
    }

    fn run_any_all_with_error_predicate(all: bool) -> Value {
        let sp = Span::synthetic(FileId(0));
        let mut b = IlBuilder::new(FileId(0));
        let param = b.add(NodeKind::Param, Payload::Cid(0), sp, &[]);
        let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
        let zero = b.add(NodeKind::Lit, Payload::LitInt(0), sp, &[]);
        let div = b.add(NodeKind::BinOp, Payload::Op(Op::Div), sp, &[one, zero]);
        let lambda_ret = b.add(NodeKind::Return, Payload::None, sp, &[div]);
        let lambda_body = b.add(NodeKind::Block, Payload::None, sp, &[lambda_ret]);
        let lambda = b.add(NodeKind::Lambda, Payload::None, sp, &[param, lambda_body]);
        let coll = b.add(NodeKind::Seq, Payload::None, sp, &[one]);
        let builtin = if all { Builtin::All } else { Builtin::Any };
        let call = b.add(
            NodeKind::Call,
            Payload::Builtin(builtin),
            sp,
            &[coll, lambda],
        );
        let ret = b.add(NodeKind::Return, Payload::None, sp, &[call]);
        let func = b.add(NodeKind::Func, Payload::None, sp, &[ret]);
        let il = b.finish(
            func,
            FileMeta {
                path: "t".into(),
                lang: Lang::Python,
            },
            Vec::new(),
            Vec::new(),
        );
        run_admitted_unit(il, func, &[]).expect("run_unit").ret
    }

    #[test]
    fn any_all_predicate_err_propagates() {
        assert_eq!(run_any_all_with_error_predicate(false), Value::Err);
        assert_eq!(run_any_all_with_error_predicate(true), Value::Err);
    }

    fn reduce_with_error_init_ignored_by_lambda() -> Value {
        let sp = Span::synthetic(FileId(0));
        let mut b = IlBuilder::new(FileId(0));
        let acc_param = b.add(NodeKind::Param, Payload::Cid(0), sp, &[]);
        let item_param = b.add(NodeKind::Param, Payload::Cid(1), sp, &[]);
        let seven = b.add(NodeKind::Lit, Payload::LitInt(7), sp, &[]);
        let lambda_ret = b.add(NodeKind::Return, Payload::None, sp, &[seven]);
        let lambda_body = b.add(NodeKind::Block, Payload::None, sp, &[lambda_ret]);
        let lambda = b.add(
            NodeKind::Lambda,
            Payload::None,
            sp,
            &[acc_param, item_param, lambda_body],
        );
        let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
        let coll = b.add(NodeKind::Seq, Payload::None, sp, &[one]);
        let zero = b.add(NodeKind::Lit, Payload::LitInt(0), sp, &[]);
        let init_err = b.add(NodeKind::BinOp, Payload::Op(Op::Div), sp, &[one, zero]);
        let reduce = b.add(
            NodeKind::Call,
            Payload::Builtin(Builtin::Reduce),
            sp,
            &[lambda, coll, init_err],
        );
        let ret = b.add(NodeKind::Return, Payload::None, sp, &[reduce]);
        let func = b.add(NodeKind::Func, Payload::None, sp, &[ret]);
        let il = b.finish(
            func,
            FileMeta {
                path: "t".into(),
                lang: Lang::Python,
            },
            Vec::new(),
            Vec::new(),
        );
        run_admitted_unit(il, func, &[]).expect("run_unit").ret
    }

    #[test]
    fn reduce_init_err_propagates() {
        assert_eq!(reduce_with_error_init_ignored_by_lambda(), Value::Err);
    }

    /// Build `fn() { return base ** exp }` over integer literals and run it.
    fn run_pow(base: i64, exp: i64) -> Value {
        let sp = Span::synthetic(FileId(0));
        let mut b = IlBuilder::new(FileId(0));
        let x = b.add(NodeKind::Lit, Payload::LitInt(base), sp, &[]);
        let y = b.add(NodeKind::Lit, Payload::LitInt(exp), sp, &[]);
        let pow = b.add(NodeKind::BinOp, Payload::Op(Op::Pow), sp, &[x, y]);
        let ret = b.add(NodeKind::Return, Payload::None, sp, &[pow]);
        let func = b.add(NodeKind::Func, Payload::None, sp, &[ret]);
        let il = b.finish(
            func,
            FileMeta {
                path: "t".into(),
                lang: Lang::Python,
            },
            Vec::new(),
            Vec::new(),
        );
        run_admitted_unit(il, func, &[]).expect("run_unit").ret
    }

    #[test]
    fn pow_negative_exponent_is_err_not_clamped_to_zero() {
        // The oracle models only i64; a negative exponent has no integer value, so it must
        // `Err` (like Div/Mod by zero) — NOT be silently clamped to `0`, which made
        // `2 ** -1` indistinguishable from `2 ** 0` and could license a false merge.
        assert_eq!(run_pow(2, 3), Value::Int(8));
        assert_eq!(run_pow(2, 0), Value::Int(1));
        assert_eq!(run_pow(2, -1), Value::Err);
    }

    #[test]
    fn pow_exponent_beyond_u32_is_err_not_truncated() {
        // The exponent was cast `as u32`, so `2 ** 2^32` truncated to `2 ** 0 == 1` —
        // colliding distinct exponents. An exponent that doesn't fit u32 has no usable
        // value here, so it errs rather than wrap to a smaller exponent.
        assert_eq!(run_pow(2, 1 << 32), Value::Err);
        assert_eq!(run_pow(2, (1 << 32) + 5), Value::Err);
    }

    /// Build `fn() { return -lit }` over an integer literal and run it.
    fn run_neg(v: i64) -> Value {
        let sp = Span::synthetic(FileId(0));
        let mut b = IlBuilder::new(FileId(0));
        let x = b.add(NodeKind::Lit, Payload::LitInt(v), sp, &[]);
        let neg = b.add(NodeKind::UnOp, Payload::Op(Op::Neg), sp, &[x]);
        let ret = b.add(NodeKind::Return, Payload::None, sp, &[neg]);
        let func = b.add(NodeKind::Func, Payload::None, sp, &[ret]);
        let il = b.finish(
            func,
            FileMeta {
                path: "t".into(),
                lang: Lang::Python,
            },
            Vec::new(),
            Vec::new(),
        );
        run_admitted_unit(il, func, &[]).expect("run_unit").ret
    }

    #[test]
    fn neg_of_i64_min_wraps_instead_of_panicking() {
        // Plain `-i` panics on `i64::MIN` (overflow); every other arithmetic op here uses
        // wrapping semantics, so negation must too — `wrapping_neg(i64::MIN) == i64::MIN`.
        assert_eq!(run_neg(5), Value::Int(-5));
        assert_eq!(run_neg(i64::MIN), Value::Int(i64::MIN));
    }
}
