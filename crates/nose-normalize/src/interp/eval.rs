use super::*;

impl<'a> Interp<'a> {
    pub(super) fn eval(&mut self, node: NodeId, env: &mut FxHashMap<u32, Value>) -> R<Value> {
        self.eval_inner(node, env)
            .map_err(|blocker| blocker.with_frame(self.il, node, "eval"))
    }

    fn eval_inner(&mut self, node: NodeId, env: &mut FxHashMap<u32, Value>) -> R<Value> {
        self.tick()?;
        let n = *self.il.node(node);
        match n.kind {
            NodeKind::Var => match n.payload {
                Payload::Cid(c) => env
                    .get(&c)
                    .cloned()
                    .ok_or_else(|| Unsupported::value("value.binding-missing")),
                _ => Err(Unsupported::il("il.variable-identity-missing")),
            },
            NodeKind::Lit => eval_literal(n.payload, self.bitwise_result_is_int32()),
            NodeKind::BinOp => self.eval_bin_op(node, n.payload, env),
            NodeKind::UnOp => self.eval_unary(node, n.payload, env),
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
                    return Err(Unsupported::il("il.field-receiver-missing"));
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
                    return Err(Unsupported::il("il.field-name-missing"));
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
                    return Err(Unsupported::il("il.conditional-shape"));
                }
                let c = self.eval(kids[0], env)?;
                // A type error in the test is itself the result (matches the strict
                // builtin a lenient `x>0?x:-x` canonicalizes to — both Err on non-numbers).
                if matches!(c, Value::Err) {
                    return Ok(Value::Err);
                }
                if self.cond_truthy(&c)? {
                    self.eval(kids[1], env)
                } else {
                    self.eval(kids[2], env)
                }
            }
            NodeKind::Call => self.eval_call(node, env),
            NodeKind::HoF => self.eval_hof(node, env),
            // A keyword argument reached outside a resolved call's own by-name binding
            // (e.g. inside an opaque/library call): evaluate its value. The keyword name
            // is not needed here — fingerprint-equal units share identical keyword
            // structure, so the name never distinguishes members of a checked group, and
            // `eval_user_call` does its own by-name binding for proven targets (#301).
            NodeKind::KwArg => self
                .il
                .children(node)
                .first()
                .ok_or_else(|| Unsupported::il("il.keyword-value-missing"))
                .and_then(|&v| self.eval(v, env)),
            // A spread argument reached in an opaque/library call: evaluate its inner
            // value. A spread to a PROVEN target already bailed (`keyword_arg_binding_plan`
            // rejects it), so this only feeds an opaque `Sym` — where the value graph also
            // keeps the spread fingerprint-distinct, so the two never share a checked group
            // (coevo series 7, S1).
            NodeKind::Splat => self
                .il
                .children(node)
                .first()
                .ok_or_else(|| Unsupported::il("il.spread-value-missing"))
                .and_then(|&v| self.eval(v, env)),
            _ => Err(Unsupported::il("il.expression-node-unsupported")),
        }
    }

    fn eval_unary(
        &mut self,
        node: NodeId,
        payload: Payload,
        env: &mut FxHashMap<u32, Value>,
    ) -> R<Value> {
        let operand = self
            .il
            .children(node)
            .first()
            .copied()
            .ok_or_else(|| Unsupported::il("il.unary-operand-missing"))?;
        let value = self.eval(operand, env)?;
        let op = op_of(payload);
        if self.bitwise_result_is_int32()
            && matches!(op, Op::Neg | Op::Pos)
            && !matches!(value, Value::Err | Value::Sym(_))
        {
            let number =
                js_to_number(&value).ok_or_else(|| Unsupported::value("value.js-to-number"))?;
            return Ok(Value::Float(F64(if matches!(op, Op::Neg) {
                -number
            } else {
                number
            })));
        }
        // JS `~x` is `~ToInt32(x)`. Preserve an operand error/symbol, but fail closed when
        // the concrete coercion is not represented by the oracle.
        let value = if matches!(op, Op::BitNot)
            && self.bitwise_result_is_int32()
            && !matches!(value, Value::Err | Value::Sym(_))
        {
            let coerced = to_int32(value);
            if !matches!(coerced, Value::Int(_)) {
                return Err(Unsupported::value("value.js-to-int32"));
            }
            coerced
        } else {
            value
        };
        if matches!(op, Op::Not) && !matches!(value, Value::Err | Value::Sym(_)) {
            return Ok(self
                .value_truthy(&value)
                .map_or(Value::Err, |truthy| Value::Bool(!truthy)));
        }
        Ok(un(op, &value))
    }

    pub(super) fn eval_bin_op(
        &mut self,
        node: NodeId,
        payload: Payload,
        env: &mut FxHashMap<u32, Value>,
    ) -> R<Value> {
        let kids = self.il.children(node).to_vec();
        if kids.len() != 2 {
            return Err(Unsupported::il("il.binary-shape"));
        }
        let op = op_of(payload);
        // This is an operator-level exclusion, not an operand-shape fallback. Check it before
        // evaluating either side so an unsupported nested coercion cannot turn Pow into a
        // concrete Err behavior.
        if matches!(op, Op::Pow) && self.bitwise_result_is_int32() {
            return Err(Unsupported::value("value.js-number-binary"));
        }
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
                if matches!(a, Value::Err)
                    || self
                        .value_truthy(&a)
                        .ok_or_else(|| Unsupported::value("value.condition-truthiness"))?
                {
                    a
                } else {
                    self.eval(kids[1], env)?
                },
            );
        }
        if matches!(op, Op::And) {
            return Ok(
                if matches!(a, Value::Err)
                    || !self
                        .value_truthy(&a)
                        .ok_or_else(|| Unsupported::value("value.condition-truthiness"))?
                {
                    a
                } else {
                    self.eval(kids[1], env)?
                },
            );
        }
        // Left operand short-circuits Err here (not in `bin`) so a raising left side never
        // evaluates the right — preserving laziness and effect order. The right operand's
        // Err is handled symmetrically inside `bin`.
        if matches!(a, Value::Err) {
            return Ok(Value::Err);
        }
        let b = self.eval(kids[1], env)?;
        if matches!(b, Value::Err) {
            return Ok(Value::Err);
        }
        // Every JS-family bitwise op narrows both operands to int32; shifts additionally mask
        // the rhs to five bits. Other languages retain the shared wide-integer model.
        if matches!(op, Op::BitAnd | Op::BitOr | Op::BitXor | Op::Shl | Op::Shr)
            && self.bitwise_result_is_int32()
        {
            return js_bitwise_bin(op, a, b).ok_or_else(|| Unsupported::value("value.js-to-int32"));
        }
        if self.bitwise_result_is_int32()
            && matches!(
                op,
                Op::Add | Op::Sub | Op::Mul | Op::Div | Op::TrueDiv | Op::Mod
            )
        {
            if contains_sym(&a) || contains_sym(&b) {
                return Ok(bin(op, &a, &b));
            }
            if matches!(op, Op::Add) && matches!((&a, &b), (Value::Str(_), Value::Str(_))) {
                return Ok(bin(op, &a, &b));
            }
            return js_number_bin(op, &a, &b)
                .ok_or_else(|| Unsupported::value("value.js-number-binary"));
        }
        if self.bitwise_result_is_int32()
            && matches!(a, Value::Int(_) | Value::Float(_))
            && matches!(b, Value::Int(_) | Value::Float(_))
        {
            return js_number_bin(op, &a, &b)
                .ok_or_else(|| Unsupported::value("value.js-number-binary"));
        }
        Ok(bin(op, &a, &b))
    }

    /// Whether this language's bitwise operators coerce their operands to int32 (the JS family).
    /// Mirrors the value graph's `is_js_like_lang` gate on the `ToInt32` narrowing, so the
    /// oracle's bitwise result matches the fingerprint's (#344, #283-D).
    pub(super) fn bitwise_result_is_int32(&self) -> bool {
        semantics(self.il.meta.lang)
            .modules()
            .js_like_shadowed_module_bindings()
    }

    pub(super) fn eval_index(&mut self, node: NodeId, env: &mut FxHashMap<u32, Value>) -> R<Value> {
        let kids = self.il.children(node).to_vec();
        if kids.len() != 2 {
            return Err(Unsupported::il("il.index-base-missing"));
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
}

fn eval_literal(payload: Payload, javascript_number: bool) -> R<Value> {
    match payload {
        Payload::LitInt(value) if javascript_number => Ok(Value::Float(F64(value as f64))),
        Payload::LitInt(value) => Ok(Value::Int(value)),
        Payload::LitBool(value) => Ok(Value::Bool(value)),
        Payload::LitStr(value) => Ok(Value::Str(vec![value])),
        // Retained values take the arms above. Unknown values stay fail-closed.
        Payload::Lit(nose_il::LitClass::Null) => Ok(Value::Null),
        Payload::Lit(_) => Err(Unsupported::value("value.literal-not-retained")),
        _ => Err(Unsupported::il("il.literal-payload-missing")),
    }
}
