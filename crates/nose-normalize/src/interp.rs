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

use nose_il::{Builtin, HoFKind, Il, LoopKind, NodeId, NodeKind, Op, Payload, Symbol};
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
}

/// The observable behavior of one run: the returned value, an ordered I/O effect trace
/// (appended/printed values, in order — order IS observable), and the final per-field
/// object state (`self.x = …`) as a name→value map in canonical name order. Field state
/// is order-INSENSITIVE across distinct fields (writing two distinct fields commutes —
/// the resulting object is identical) but reflects last-write-wins per field. Two units
/// are behaviorally equal on an input iff all three components match.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Behavior {
    pub ret: Value,
    pub effects: Vec<Value>,
    pub fields: Vec<(i64, Value)>,
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
    steps: u64,
    effects: Vec<Value>,
    fields: FxHashMap<i64, Value>,
    /// Parameter cids — appending to one is a caller-visible mutation (an effect); appending
    /// to a LOCAL list var builds that list's value (faithful, converges with a comprehension).
    params: FxHashSet<u32>,
    /// The `Func` node being run and its name, so a same-named call inside the body is
    /// executed as **direct self-recursion** (a fresh frame, shared effect trace / step
    /// budget) rather than left opaque. This lets the oracle interpret the *pre-canon*
    /// recursive form and so validate the recursion→iteration canonicalization; unbounded
    /// recursion safely hits the step budget and the unit becomes uninterpretable. Any other
    /// (non-self) opaque call stays unsupported.
    func_root: NodeId,
    func_name: Option<Symbol>,
}

/// Run the `Func` unit at `root` with `args` bound to its parameters (in order).
/// Returns its [`Behavior`], or `None` if the unit is uninterpretable.
pub fn run_unit(il: &Il, root: NodeId, args: &[Value]) -> Option<Behavior> {
    if il.kind(root) != NodeKind::Func {
        return None;
    }
    let func_name = il
        .units
        .iter()
        .find(|u| u.root == root)
        .and_then(|u| u.name);
    let mut it = Interp {
        il,
        steps: 0,
        effects: Vec::new(),
        fields: FxHashMap::default(),
        params: FxHashSet::default(),
        func_root: root,
        func_name,
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
    let mut fields: Vec<(i64, Value)> = it.fields.into_iter().collect();
    fields.sort_by_key(|&(k, _)| k);
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
                self.bind(kids[0], rhs, env)?;
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
                if truthy(&cond) {
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
                    if !truthy(&c) {
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
                    _ => return Err(Unsupported),
                };
                for item in seq {
                    self.tick()?;
                    self.bind(kids[0], item, env)?;
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
                    if !truthy(&c) {
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
    fn bind(&mut self, target: NodeId, val: Value, env: &mut FxHashMap<u32, Value>) -> R<()> {
        match self.il.kind(target) {
            NodeKind::Var => {
                if let Payload::Cid(c) = self.il.node(target).payload {
                    env.insert(c, val);
                    Ok(())
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
                    self.bind(t, v, env)?;
                }
                Ok(())
            }
            // A store into a field/index is an effect; record *what* is written to as
            // well as the value, so `self.a = x` and `self.b = x` (or `xs[i]=` vs
            // `xs[j]=`) are distinguished — otherwise field-blindness conflates
            // near-twin setters and pollutes the completeness signal with false leads.
            // A field store updates per-field object state (last-write-wins), keyed by
            // field name — NOT an ordered effect. Writing distinct fields commutes (same
            // resulting object), so `self.a=x; self.b=y` and the swap are behaviorally
            // equal; same-field overwrites keep the last value. (Previously pushed to the
            // ordered effect trace, which wrongly made commuting field-write order
            // observable — splitting equivalent constructors the value graph merges.)
            NodeKind::Field => {
                if let Payload::Name(sym) = self.il.node(target).payload {
                    self.fields.insert(nose_il::symbol_index(sym) as i64, val);
                    Ok(())
                } else {
                    Err(Unsupported)
                }
            }
            NodeKind::Index => {
                if let Some(&ix) = self.il.children(target).to_vec().get(1) {
                    let iv = self.eval(ix, env)?;
                    self.effects.push(iv);
                }
                self.effects.push(val);
                Ok(())
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
            NodeKind::BinOp => {
                let kids = self.il.children(node).to_vec();
                if kids.len() != 2 {
                    return Err(Unsupported);
                }
                let op = op_of(n.payload);
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
                    return Ok(if matches!(a, Value::Err) || truthy(&a) {
                        a
                    } else {
                        self.eval(kids[1], env)?
                    });
                }
                if matches!(op, Op::And) {
                    return Ok(if matches!(a, Value::Err) || !truthy(&a) {
                        a
                    } else {
                        self.eval(kids[1], env)?
                    });
                }
                let b = self.eval(kids[1], env)?;
                Ok(bin(op, &a, &b))
            }
            NodeKind::UnOp => {
                let kids = self.il.children(node).to_vec();
                let a = self.eval(*kids.first().ok_or(Unsupported)?, env)?;
                Ok(un(op_of(n.payload), &a))
            }
            NodeKind::Index => {
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
                match (base, idx) {
                    (Value::List(xs), Value::Int(i)) => {
                        let i = if i < 0 { i + xs.len() as i64 } else { i };
                        Ok(xs.get(i as usize).cloned().unwrap_or(Value::Err))
                    }
                    _ => Ok(Value::Err),
                }
            }
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
            NodeKind::Field => match n.payload {
                Payload::Name(sym) => self
                    .fields
                    .get(&(nose_il::symbol_index(sym) as i64))
                    .cloned()
                    .ok_or(Unsupported),
                _ => Err(Unsupported),
            },
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
                if truthy(&c) {
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

    fn eval_call(&mut self, node: NodeId, env: &mut FxHashMap<u32, Value>) -> R<Value> {
        let b = match self.il.node(node).payload {
            Payload::Builtin(b) => b,
            _ => return self.eval_user_call(node, env), // self-recursion, else opaque
        };
        let kids = self.il.children(node).to_vec();
        let mut args = Vec::new();
        // `reduce(f, xs, init)` carries a lambda; evaluate it specially.
        if matches!(b, Builtin::Reduce) {
            return self.eval_reduce_call(&kids, env);
        }
        // `any`/`all` short-circuit over a (possibly predicate-mapped) collection. The
        // method form `xs.some(λ)` carries a lambda; the generator form `any(p for x in xs)`
        // is a pre-mapped list of predicate values.
        if matches!(b, Builtin::Any | Builtin::All) {
            return self.eval_any_all_call(matches!(b, Builtin::All), &kids, env);
        }
        if matches!(b, Builtin::Append) {
            return self.eval_append(&kids, env);
        }
        if matches!(b, Builtin::ValueOrDefault) {
            return self.eval_value_or_default_call(&kids, env);
        }
        for &k in &kids {
            args.push(self.eval(k, env)?);
        }
        if args.iter().any(|arg| matches!(arg, Value::Err)) {
            return Ok(Value::Err);
        }
        match b {
            Builtin::Len => match args.first() {
                Some(Value::List(xs)) => Ok(Value::Int(xs.len() as i64)),
                // A string is the free monoid over opaque piece hashes; its character
                // length is unknown (piece count ≠ char count), so `len` stays `Err` —
                // matching the type doc and the `IsEmpty` sibling. Returning a constant
                // `Int(1)` falsely equated `len(any_string)` with the literal `1`.
                _ => Ok(Value::Err),
            },
            Builtin::IsEmpty => match args.first() {
                Some(Value::List(xs)) => Ok(Value::Bool(xs.is_empty())),
                _ => Ok(Value::Err),
            },
            Builtin::IsNull => match args.first() {
                Some(value) => Ok(Value::Bool(matches!(value, Value::Null))),
                _ => Ok(Value::Err),
            },
            Builtin::IsNotNull => match args.first() {
                Some(value) => Ok(Value::Bool(!matches!(value, Value::Null))),
                _ => Ok(Value::Err),
            },
            Builtin::StartsWith => Ok(string_affix(args.first(), args.get(1), true)),
            Builtin::EndsWith => Ok(string_affix(args.first(), args.get(1), false)),
            Builtin::Contains => match (args.first(), args.get(1)) {
                (Some(element), Some(Value::List(items))) => Ok(Value::Bool(
                    items.iter().any(|candidate| candidate == element),
                )),
                _ => Ok(Value::Err),
            },
            Builtin::Join => Ok(join_strings(args.first(), args.get(1))),
            Builtin::Abs => match args.first() {
                Some(Value::Int(v)) => Ok(Value::Int(v.abs())),
                _ => Ok(Value::Err),
            },
            Builtin::Sum => Ok(fold_ints(args.first(), 0, |a, x| a + x)),
            Builtin::Min => Ok(fold_opt(args.first(), |a, x| a.min(x))),
            Builtin::Max => Ok(fold_opt(args.first(), |a, x| a.max(x))),
            Builtin::Range => range_values(&args),
            Builtin::Zip => match (args.first(), args.get(1)) {
                (Some(Value::List(a)), Some(Value::List(b))) => Ok(Value::List(
                    a.iter()
                        .zip(b.iter())
                        .map(|(x, y)| Value::List(vec![x.clone(), y.clone()]))
                        .collect(),
                )),
                _ => Ok(Value::Err),
            },
            Builtin::Enumerate => match args.first() {
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
            Builtin::Keys => match args.first() {
                Some(Value::List(xs)) => {
                    Ok(Value::List((0..xs.len() as i64).map(Value::Int).collect()))
                }
                _ => Ok(Value::Err),
            },
            Builtin::Append => unreachable!("handled by eval_append before arg eval"),
            Builtin::Print => {
                for a in &args {
                    self.effects.push(a.clone());
                }
                Ok(Value::Null)
            }
            // Dicts are not modeled — a `DictEntry` makes its unit non-interpretable (Err),
            // so dict-building units are excluded from the oracle rather than risk a false
            // merge. Their convergence rests on the DistinctEntry-vs-tuple representation.
            Builtin::DictEntry => Ok(Value::Err),
            Builtin::GetOrDefault => Ok(Value::Err),
            Builtin::ValueOrDefault => unreachable!("handled before eager arg eval"),
            Builtin::Reduce | Builtin::Any | Builtin::All => unreachable!(),
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
        if matches!(value, Value::Null) {
            return match kids.get(1) {
                Some(&default) => self.eval(default, env),
                None => Ok(Value::Null),
            };
        }
        Ok(value)
    }

    /// A non-builtin `callee(args…)`. Modeled ONLY when `callee` names the function being
    /// run — i.e. **direct self-recursion** — by binding the evaluated arguments to the
    /// function's parameters in a fresh frame and executing its body; the effect trace,
    /// field state, and step budget are shared with the caller, so effects stay ordered and
    /// runaway recursion terminates as `Unsupported`. Every other opaque call is unsupported
    /// (the unit is excluded from the soundness check rather than guessed at).
    fn eval_user_call(&mut self, node: NodeId, env: &mut FxHashMap<u32, Value>) -> R<Value> {
        let kids = self.il.children(node).to_vec();
        let callee = *kids.first().ok_or(Unsupported)?;
        let is_self = match (self.il.node(callee).payload, self.func_name) {
            (Payload::Name(s), Some(name)) => s == name,
            _ => false,
        };
        if !is_self {
            return Err(Unsupported);
        }
        // Evaluate the arguments in the CURRENT frame (call-by-value), left to right.
        let mut argv = Vec::with_capacity(kids.len().saturating_sub(1));
        for &a in &kids[1..] {
            let value = self.eval(a, env)?;
            if matches!(value, Value::Err) {
                return Ok(Value::Err);
            }
            argv.push(value);
        }
        // Bind them positionally to the callee's parameters in a fresh environment; locals
        // start empty, exactly like a real call.
        let params = self.il.children(self.func_root).to_vec();
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
        match self.exec(body, &mut fenv)? {
            Flow::Ret(v) => Ok(v),
            Flow::Err => Ok(Value::Err),
            _ => Ok(Value::Null),
        }
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
                if self.il.kind(target) != NodeKind::Field {
                    let target_value = self.eval(target, env)?;
                    if matches!(target_value, Value::Err) {
                        return Ok(Some(Flow::Err));
                    }
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
            let t = truthy(&v);
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
            _ => return Ok(Value::Err),
        };
        let f = kids[1];
        match kind {
            HoFKind::Map => {
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
            HoFKind::Filter => {
                let mut out = Vec::new();
                for x in coll {
                    let keep = self.apply(f, std::slice::from_ref(&x), env)?;
                    if matches!(keep, Value::Err) {
                        return Ok(Value::Err);
                    }
                    if truthy(&keep) {
                        out.push(x);
                    }
                }
                Ok(Value::List(out))
            }
            HoFKind::Reduce => {
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
        match self.exec(body, &mut local)? {
            Flow::Ret(v) => Ok(v),
            Flow::Err => Ok(Value::Err),
            Flow::Normal => Ok(Value::Null),
            Flow::Break | Flow::Continue => Err(Unsupported),
        }
    }
}

fn op_of(p: Payload) -> Op {
    match p {
        Payload::Op(o) => o,
        _ => Op::Add,
    }
}

fn truthy(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Int(i) => *i != 0,
        Value::List(xs) => !xs.is_empty(),
        Value::Str(v) => !v.is_empty(),
        Value::Null | Value::Err => false,
    }
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
        (Op::Not, _) => Value::Bool(!truthy(a)),
        _ => Value::Err,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nose_il::{FileId, FileMeta, IlBuilder, Interner, Lang, LitClass, Span, Unit, UnitKind};

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
        run_unit(&il, func, &[]).expect("run_unit").ret
    }

    #[test]
    fn len_of_string_is_err_not_one() {
        // Strings are the free monoid over opaque piece hashes — character length is
        // unknown, so `len(str)` must be `Err` (matching the documented contract and the
        // sibling `IsEmpty`), not a hardcoded `Int(1)`.
        assert_eq!(run_len_of_string(), Value::Err);
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
        run_unit(&il, func, &[]).expect("run_unit").ret
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
        run_unit(&il, func, &[]).expect("run_unit").ret
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
        run_unit(&il, func, &[]).expect("run_unit").ret
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
        run_unit(&il, func, &[]).map(|behavior| behavior.ret)
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
        run_unit(&il, func, &[]).expect("run_unit").ret
    }

    #[test]
    fn throw_is_err_behavior_and_stops_execution() {
        assert_eq!(run_throw_then_return(), Value::Err);
    }

    fn run_field_write_read() -> (Behavior, i64) {
        let sp = Span::synthetic(FileId(0));
        let mut b = IlBuilder::new(FileId(0));
        let interner = Interner::new();
        let base = b.add(NodeKind::Lit, Payload::Lit(LitClass::Null), sp, &[]);
        let field_name = interner.intern("x");
        let field_key = nose_il::symbol_index(field_name) as i64;
        let write_target = b.add(NodeKind::Field, Payload::Name(field_name), sp, &[base]);
        let seven = b.add(NodeKind::Lit, Payload::LitInt(7), sp, &[]);
        let assign = b.add(NodeKind::Assign, Payload::None, sp, &[write_target, seven]);
        let read_target = b.add(NodeKind::Field, Payload::Name(field_name), sp, &[base]);
        let ret = b.add(NodeKind::Return, Payload::None, sp, &[read_target]);
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
        (run_unit(&il, func, &[]).expect("run_unit"), field_key)
    }

    #[test]
    fn field_write_can_be_read_back() {
        let (behavior, field_key) = run_field_write_read();
        assert_eq!(behavior.ret, Value::Int(7));
        assert_eq!(behavior.fields, vec![(field_key, Value::Int(7))]);
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
        run_unit(&il, func, &[]).expect("run_unit").ret
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
        run_unit(&il, func, &[]).expect("run_unit").ret
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
        run_unit(&il, func, &[]).expect("run_unit").ret
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
        run_unit(&il, func, &[Value::Err]).expect("run_unit")
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
        run_unit(&il, func, &[]).expect("run_unit")
    }

    #[test]
    fn statement_append_checks_error_expr_target_before_items() {
        let behavior = statement_append_on_error_expr_target_with_effect_arg();
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
        run_unit(&il, func, &[]).expect("run_unit")
    }

    #[test]
    fn index_checks_error_base_before_index_expr() {
        let behavior = index_on_error_base_with_effect_index();
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
            run_unit(&il, func, &[]).expect("run_unit").ret,
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
        assert_eq!(run_unit(&il, func, &[]).expect("run_unit").ret, Value::Err);
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
        run_unit(&il, func, &[]).expect("run_unit").ret
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
        run_unit(&il, func, &[]).expect("run_unit").ret
    }

    #[test]
    fn hof_map_propagates_lambda_errors() {
        assert_eq!(hof_with_error_lambda(HoFKind::Map), Value::Err);
    }

    #[test]
    fn hof_filter_propagates_lambda_errors() {
        assert_eq!(hof_with_error_lambda(HoFKind::Filter), Value::Err);
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
        run_unit(&il, func, &[]).expect("run_unit").ret
    }

    #[test]
    fn eager_builtin_argument_err_stops_execution() {
        assert_eq!(print_with_error_arg_then_return(), Value::Err);
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
        run_unit(&il, func, &[Value::Bool(false), Value::Int(0)])
            .expect("run_unit")
            .ret
    }

    #[test]
    fn self_call_argument_err_stops_execution() {
        assert_eq!(self_call_with_error_arg_ignored_by_callee(), Value::Err);
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
        run_unit(&il, func, &[]).expect("run_unit").ret
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
        run_unit(&il, func, &[]).expect("run_unit").ret
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
        run_unit(&il, func, &[]).expect("run_unit").ret
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
        run_unit(&il, func, &[]).expect("run_unit").ret
    }

    #[test]
    fn neg_of_i64_min_wraps_instead_of_panicking() {
        // Plain `-i` panics on `i64::MIN` (overflow); every other arithmetic op here uses
        // wrapping semantics, so negation must too — `wrapping_neg(i64::MIN) == i64::MIN`.
        assert_eq!(run_neg(5), Value::Int(-5));
        assert_eq!(run_neg(i64::MIN), Value::Int(i64::MIN));
    }
}
