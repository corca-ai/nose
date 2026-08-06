use super::*;

impl OperatorSemantics {
    pub fn value_law(self, law: ValueLaw) -> Option<ValueLawContract> {
        let requirement = match law {
            ValueLaw::AddCommutativity | ValueLaw::AddAssociativity => {
                ValueDomainRequirement::NoConcatOperands
            }
            ValueLaw::NumericNegationInvolution
            | ValueLaw::NumericBitwiseIdempotence
            | ValueLaw::NumericFactorDistribution
            | ValueLaw::StructuralNumericFold => ValueDomainRequirement::NumericOperands,
            ValueLaw::BooleanIdempotence
            | ValueLaw::BooleanCommutativity
            | ValueLaw::BooleanAssociativity => ValueDomainRequirement::BooleanOperands,
            ValueLaw::IntegerClampOrderedMinMax => return None,
        };
        Some(ValueLawContract {
            law,
            requirement,
            channel: ChannelEligibility::ExactProven,
            evidence: ValueDomainEvidence::ModeledOperatorResult,
        })
    }

    /// Whether `+` can coerce a mixed string/non-string operand pair.
    pub fn plus_coerces_strings(self) -> bool {
        js_like_lang(self.lang) || self.lang == Lang::Java
    }

    /// Whether relational operators can coerce strings.
    pub fn relational_coerces_strings(self) -> bool {
        js_like_lang(self.lang)
    }

    /// Whether `*` is asymmetric sequence repetition rather than purely numeric.
    pub fn mul_is_sequence_repetition(self) -> bool {
        self.lang == Lang::Ruby
    }

    pub fn strict_operand_domain(self, op: Op) -> Option<ValueDomain> {
        self.strict_numeric_operand_operator(op)
            .then_some(ValueDomain::Number)
    }

    fn strict_numeric_operand_operator(self, op: Op) -> bool {
        if op == Op::Mul && matches!(self.lang, Lang::Python | Lang::Ruby) {
            return false;
        }
        strict_numeric_operand_operator(op)
    }

    pub fn unary_operand_domain(self, op: Op) -> Option<ValueDomain> {
        match op {
            Op::Neg | Op::Pos | Op::BitNot => Some(ValueDomain::Number),
            _ => None,
        }
    }

    pub fn unary_result_domain(self, op: Op) -> ValueDomain {
        match op {
            Op::Neg | Op::Pos | Op::BitNot => ValueDomain::Number,
            Op::Not => ValueDomain::Boolean,
            _ => ValueDomain::Unknown,
        }
    }

    pub fn binary_result_domain(
        self,
        op: Op,
        left: ValueDomain,
        right: ValueDomain,
    ) -> ValueDomain {
        if op == Op::Mul && (left == ValueDomain::String || right == ValueDomain::String) {
            ValueDomain::String
        } else if self.strict_numeric_operand_operator(op) {
            if left.is_known() || right.is_known() {
                if left == ValueDomain::Number && right == ValueDomain::Number {
                    ValueDomain::Number
                } else {
                    ValueDomain::Unknown
                }
            } else {
                ValueDomain::Number
            }
        } else if matches!(
            op,
            Op::Lt | Op::Le | Op::Gt | Op::Ge | Op::Eq | Op::Ne | Op::In
        ) {
            ValueDomain::Boolean
        } else if op == Op::Add {
            if left == ValueDomain::Number && right == ValueDomain::Number {
                ValueDomain::Number
            } else if left == ValueDomain::String || right == ValueDomain::String {
                ValueDomain::String
            } else if left == ValueDomain::Sequence || right == ValueDomain::Sequence {
                ValueDomain::Sequence
            } else {
                ValueDomain::Unknown
            }
        } else if matches!(op, Op::And | Op::Or)
            && left == ValueDomain::Boolean
            && right == ValueDomain::Boolean
        {
            ValueDomain::Boolean
        } else {
            ValueDomain::Unknown
        }
    }

    pub fn builtin_result_domain(self, builtin: Builtin) -> ValueDomain {
        match builtin {
            Builtin::Len | Builtin::UnsignedCast32 => ValueDomain::Number,
            Builtin::IsEmpty
            | Builtin::IsNull
            | Builtin::IsNotNull
            | Builtin::StartsWith
            | Builtin::EndsWith
            | Builtin::Contains
            | Builtin::StringContains => ValueDomain::Boolean,
            Builtin::Join => ValueDomain::String,
            _ => ValueDomain::Unknown,
        }
    }

    pub fn literal_value_domain(self, payload: Payload) -> Option<ValueDomain> {
        match payload {
            Payload::LitInt(_) | Payload::LitFloat(_) => Some(ValueDomain::Number),
            Payload::LitStr(_) => Some(ValueDomain::String),
            Payload::LitBool(_) => Some(ValueDomain::Boolean),
            Payload::Lit(LitClass::Int) | Payload::Lit(LitClass::Float) => {
                Some(ValueDomain::Number)
            }
            Payload::Lit(LitClass::Str) => Some(ValueDomain::String),
            Payload::Lit(LitClass::Bool) => Some(ValueDomain::Boolean),
            _ => None,
        }
    }

    pub fn expression_value_domain<F>(self, il: &Il, node: NodeId, param_domain: &F) -> ValueDomain
    where
        F: Fn(u32) -> ValueDomain,
    {
        match il.node(node).kind {
            NodeKind::Lit => self
                .literal_value_domain(il.node(node).payload)
                .unwrap_or(ValueDomain::Unknown),
            NodeKind::Var => match il.node(node).payload {
                Payload::Cid(cid) => param_domain(cid),
                _ => ValueDomain::Unknown,
            },
            NodeKind::Seq => ValueDomain::Sequence,
            NodeKind::UnOp => match il.node(node).payload {
                Payload::Op(op) => self.unary_result_domain(op),
                _ => ValueDomain::Unknown,
            },
            NodeKind::BinOp => {
                let kids = il.children(node);
                let Payload::Op(op) = il.node(node).payload else {
                    return ValueDomain::Unknown;
                };
                if kids.len() == 2 {
                    let left = self.expression_value_domain(il, kids[0], param_domain);
                    let right = self.expression_value_domain(il, kids[1], param_domain);
                    self.binary_result_domain(op, left, right)
                } else {
                    self.binary_result_domain(op, ValueDomain::Unknown, ValueDomain::Unknown)
                }
            }
            NodeKind::Call => match il.node(node).payload {
                Payload::Builtin(builtin)
                    if admitted_builtin_semantics_at_call(il, node, builtin) =>
                {
                    self.builtin_result_domain(builtin)
                }
                _ => ValueDomain::Unknown,
            },
            _ => ValueDomain::Unknown,
        }
    }

    pub fn infer_param_value_domains(self, il: &Il, root: NodeId) -> Vec<ValueDomain> {
        if il.kind(root) != NodeKind::Func {
            return Vec::new();
        }
        let params = il
            .children(root)
            .iter()
            .filter_map(|&child| {
                (il.kind(child) == NodeKind::Param)
                    .then_some(il.node(child).payload)
                    .and_then(|payload| match payload {
                        Payload::Cid(cid) => Some(cid),
                        _ => None,
                    })
            })
            .collect::<Vec<_>>();
        let mut evidence: FxHashMap<u32, ValueDomain> = FxHashMap::default();
        for _ in 0..params.len() + 1 {
            let mut next = evidence.clone();
            let mut stack = vec![root];
            while let Some(node) = stack.pop() {
                let kids = il.children(node).to_vec();
                self.note_param_domain_evidence(il, node, &kids, &evidence, &mut next);
                stack.extend(kids);
            }
            if next == evidence {
                break;
            }
            evidence = next;
        }
        params
            .iter()
            .map(|cid| evidence.get(cid).copied().unwrap_or(ValueDomain::Unknown))
            .collect()
    }

    fn note_param_domain_evidence(
        self,
        il: &Il,
        node: NodeId,
        kids: &[NodeId],
        evidence: &FxHashMap<u32, ValueDomain>,
        next: &mut FxHashMap<u32, ValueDomain>,
    ) {
        let cid_of = |node: NodeId, il: &Il| -> Option<u32> {
            if il.kind(node) == NodeKind::Var {
                if let Payload::Cid(cid) = il.node(node).payload {
                    return Some(cid);
                }
            }
            None
        };
        let add = |cid: u32, domain: ValueDomain, values: &mut FxHashMap<u32, ValueDomain>| {
            values
                .entry(cid)
                .and_modify(|existing| *existing = existing.join(domain))
                .or_insert(domain);
        };
        match il.node(node).kind {
            NodeKind::BinOp => {
                if let Payload::Op(op) = il.node(node).payload {
                    if self.strict_operand_domain(op).is_some() && kids.len() == 2 {
                        for &kid in kids {
                            if let Some(cid) = cid_of(kid, il) {
                                add(cid, ValueDomain::Number, next);
                            }
                        }
                    } else if op == Op::Add && kids.len() == 2 {
                        let lookup =
                            |cid| evidence.get(&cid).copied().unwrap_or(ValueDomain::Unknown);
                        let domains = [
                            self.expression_value_domain(il, kids[0], &lookup),
                            self.expression_value_domain(il, kids[1], &lookup),
                        ];
                        for index in 0..2 {
                            if let Some(cid) = cid_of(kids[index], il) {
                                if matches!(
                                    domains[1 - index],
                                    ValueDomain::Number | ValueDomain::String
                                ) {
                                    add(cid, domains[1 - index], next);
                                }
                            }
                        }
                    }
                }
            }
            NodeKind::UnOp => {
                if let Payload::Op(op) = il.node(node).payload {
                    if self.unary_operand_domain(op).is_some() {
                        if let Some(cid) = kids.first().and_then(|&kid| cid_of(kid, il)) {
                            add(cid, ValueDomain::Number, next);
                        }
                    }
                }
            }
            NodeKind::Index => {
                if let Some(cid) = kids.get(1).and_then(|&kid| cid_of(kid, il)) {
                    add(cid, ValueDomain::Number, next);
                }
            }
            _ => {}
        }
    }
}
