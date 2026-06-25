//! Operator and value-domain semantic contracts.

use super::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct OperatorSemantics {
    pub(super) lang: Lang,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ComparisonLaw {
    DirectionCanon,
    Negation,
    EqualityCommutativity,
    LatticeLeNeToLt,
    LatticeLtEqToLe,
    LatticeStrictAbsorbsNonstrict,
    AbsSignTernary,
    MinMaxTernary,
    SelectionReductionGuard,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OperatorEvidence {
    ModeledIlOperator,
    PrimitiveTotalOrder,
    StaticCardinalityThreshold,
    JsLikeStaticIndexMembershipThreshold,
    CIntegerBytePack,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct OperatorLawContract {
    pub law: ComparisonLaw,
    pub channel: ChannelEligibility,
    pub evidence: OperatorEvidence,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ComparisonTransformContract {
    pub law: ComparisonLaw,
    pub input: Op,
    pub output: Op,
    pub swap_operands: bool,
    pub channel: ChannelEligibility,
    pub evidence: OperatorEvidence,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CardinalityThreshold {
    Zero,
    One,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CardinalityPredicate {
    Empty,
    NonEmpty,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CardinalityThresholdContract {
    pub threshold: CardinalityThreshold,
    pub predicate: CardinalityPredicate,
    pub channel: ChannelEligibility,
    pub evidence: OperatorEvidence,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StaticIndexMembershipThresholdContract {
    pub threshold: IndexMembershipThreshold,
    pub channel: ChannelEligibility,
    pub evidence: OperatorEvidence,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MembershipOperatorReceiverContract {
    ExactCollectionOrMap,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MembershipOperatorContract {
    pub operator: Op,
    pub receiver: MembershipOperatorReceiverContract,
    pub channel: ChannelEligibility,
    pub evidence: OperatorEvidence,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CBytePackWidth {
    U16,
    U32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CIntegerBytePackContract {
    pub width: CBytePackWidth,
    pub base_domain: DomainRequirement,
    pub required_high_lane_cast: Option<SourceFactKind>,
    pub channel: ChannelEligibility,
    pub evidence: OperatorEvidence,
}

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

    /// Whether `+` can coerce a mixed string/non-string operand pair (so grouping and order
    /// are observable and the value-graph must not freely associate/commute it). True for the
    /// JS family and Java (`"a" + 2 + 3` is `"a23"`, not `"a" + 5`). The single source for what
    /// was duplicated as `plus_has_mixed_string_coercion` in `algebra` and the value graph.
    pub fn plus_coerces_strings(self) -> bool {
        js_like_lang(self.lang) || self.lang == Lang::Java
    }

    /// Whether relational operators (`<`/`>`/…) coerce strings (so their direction/ordering is
    /// type-observable). The JS family; Java relationals are numeric-only, unlike its `+`.
    pub fn relational_coerces_strings(self) -> bool {
        js_like_lang(self.lang)
    }

    /// Whether `*` is (asymmetric) sequence repetition rather than purely numeric — Ruby only
    /// (`"ab" * 3` is `"ababab"`, but `3 * "ab"` raises). Gates both the `algebra` constant-fold
    /// reorder and the value-graph commutation of a `*` chain.
    pub fn mul_is_sequence_repetition(self) -> bool {
        self.lang == Lang::Ruby
    }

    pub fn strict_operand_domain(self, op: Op) -> Option<ValueDomain> {
        if self.strict_numeric_operand_operator(op) {
            Some(ValueDomain::Number)
        } else {
            None
        }
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
        let mut params: Vec<u32> = Vec::new();
        for &child in il.children(root) {
            if il.kind(child) == NodeKind::Param {
                if let Payload::Cid(cid) = il.node(child).payload {
                    params.push(cid);
                }
            }
        }
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
        let add = |cid: u32, domain: ValueDomain, ev: &mut FxHashMap<u32, ValueDomain>| {
            ev.entry(cid)
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
                        for i in 0..2 {
                            if let Some(cid) = cid_of(kids[i], il) {
                                if matches!(
                                    domains[1 - i],
                                    ValueDomain::Number | ValueDomain::String
                                ) {
                                    add(cid, domains[1 - i], next);
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

    pub fn comparison_law(self, law: ComparisonLaw) -> Option<OperatorLawContract> {
        let evidence = match law {
            ComparisonLaw::LatticeStrictAbsorbsNonstrict => {
                if !matches!(self.lang, Lang::C | Lang::Go | Lang::Java) {
                    return None;
                }
                OperatorEvidence::PrimitiveTotalOrder
            }
            ComparisonLaw::DirectionCanon
            | ComparisonLaw::Negation
            | ComparisonLaw::EqualityCommutativity
            | ComparisonLaw::LatticeLeNeToLt
            | ComparisonLaw::LatticeLtEqToLe
            | ComparisonLaw::AbsSignTernary
            | ComparisonLaw::MinMaxTernary
            | ComparisonLaw::SelectionReductionGuard => OperatorEvidence::ModeledIlOperator,
        };
        Some(OperatorLawContract {
            law,
            channel: ChannelEligibility::ExactProven,
            evidence,
        })
    }

    pub fn comparison_direction(self, op: Op) -> Option<ComparisonTransformContract> {
        let output = match op {
            Op::Gt => Op::Lt,
            Op::Ge => Op::Le,
            _ => return None,
        };
        let law = self.comparison_law(ComparisonLaw::DirectionCanon)?;
        Some(ComparisonTransformContract {
            law: law.law,
            input: op,
            output,
            swap_operands: true,
            channel: law.channel,
            evidence: law.evidence,
        })
    }

    pub fn comparison_reverse(self, op: Op) -> Option<ComparisonTransformContract> {
        let output = match op {
            Op::Lt => Op::Gt,
            Op::Le => Op::Ge,
            Op::Gt => Op::Lt,
            Op::Ge => Op::Le,
            Op::Eq => Op::Eq,
            Op::Ne => Op::Ne,
            _ => return None,
        };
        let law = self.comparison_law(ComparisonLaw::DirectionCanon)?;
        Some(ComparisonTransformContract {
            law: law.law,
            input: op,
            output,
            swap_operands: true,
            channel: law.channel,
            evidence: law.evidence,
        })
    }

    pub fn comparison_complement(self, op: Op) -> Option<ComparisonTransformContract> {
        let output = match op {
            Op::Lt => Op::Ge,
            Op::Le => Op::Gt,
            Op::Gt => Op::Le,
            Op::Ge => Op::Lt,
            Op::Eq => Op::Ne,
            Op::Ne => Op::Eq,
            _ => return None,
        };
        let law = self.comparison_law(ComparisonLaw::Negation)?;
        Some(ComparisonTransformContract {
            law: law.law,
            input: op,
            output,
            swap_operands: false,
            channel: law.channel,
            evidence: law.evidence,
        })
    }

    pub fn canonical_negated_comparison(self, op: Op) -> Option<ComparisonTransformContract> {
        let (output, swap_operands) = match op {
            Op::Eq => (Op::Ne, false),
            Op::Ne => (Op::Eq, false),
            Op::Lt => (Op::Le, true),
            Op::Le => (Op::Lt, true),
            Op::Gt => (Op::Le, false),
            Op::Ge => (Op::Lt, false),
            _ => return None,
        };
        let law = self.comparison_law(ComparisonLaw::Negation)?;
        Some(ComparisonTransformContract {
            law: law.law,
            input: op,
            output,
            swap_operands,
            channel: law.channel,
            evidence: law.evidence,
        })
    }

    /// Source comparison operators are primitive total-order comparisons rather
    /// than receiver-overloadable/user-dispatched comparisons. This gates lattice
    /// comparison absorption rules.
    pub fn primitive_order_comparisons(self) -> bool {
        self.comparison_law(ComparisonLaw::LatticeStrictAbsorbsNonstrict)
            .is_some()
    }

    pub fn zero_cardinality_equality(self, op: Op) -> Option<CardinalityThresholdContract> {
        let predicate = match op {
            Op::Eq => CardinalityPredicate::Empty,
            Op::Ne => CardinalityPredicate::NonEmpty,
            _ => return None,
        };
        Some(CardinalityThresholdContract {
            threshold: CardinalityThreshold::Zero,
            predicate,
            channel: ChannelEligibility::ExactProven,
            evidence: OperatorEvidence::StaticCardinalityThreshold,
        })
    }

    pub fn cardinality_threshold(
        self,
        op: Op,
        count_on_right: bool,
        threshold: CardinalityThreshold,
        predicate: CardinalityPredicate,
    ) -> Option<CardinalityThresholdContract> {
        let matches = match (predicate, threshold) {
            (CardinalityPredicate::NonEmpty, CardinalityThreshold::Zero) => {
                threshold_excludes_floor(op, count_on_right)
            }
            (CardinalityPredicate::NonEmpty, CardinalityThreshold::One) => {
                threshold_reaches_floor(op, count_on_right)
            }
            (CardinalityPredicate::Empty, CardinalityThreshold::Zero) => {
                threshold_at_or_below_floor(op, count_on_right)
            }
            (CardinalityPredicate::Empty, CardinalityThreshold::One) => {
                threshold_below_floor(op, count_on_right)
            }
        };
        matches.then_some(CardinalityThresholdContract {
            threshold,
            predicate,
            channel: ChannelEligibility::ExactProven,
            evidence: OperatorEvidence::StaticCardinalityThreshold,
        })
    }

    pub fn static_index_membership_threshold(
        self,
        op: Op,
        index_call_on_right: bool,
        threshold: IndexMembershipThreshold,
    ) -> Option<StaticIndexMembershipThresholdContract> {
        if !js_like_lang(self.lang) {
            return None;
        }
        index_membership_threshold_matches(op, index_call_on_right, threshold).then_some(
            StaticIndexMembershipThresholdContract {
                threshold,
                channel: ChannelEligibility::ExactProven,
                evidence: OperatorEvidence::JsLikeStaticIndexMembershipThreshold,
            },
        )
    }

    pub fn membership_operator(self, op: Op) -> Option<MembershipOperatorContract> {
        (self.lang == Lang::Python && op == Op::In).then_some(MembershipOperatorContract {
            operator: op,
            receiver: MembershipOperatorReceiverContract::ExactCollectionOrMap,
            channel: ChannelEligibility::ExactProven,
            evidence: OperatorEvidence::ModeledIlOperator,
        })
    }

    /// C unsigned byte/word packing contracts are currently first-party only for
    /// the C lowering, where explicit byte-buffer and unsigned-cast facts are
    /// recovered by the frontend.
    pub fn c_integer_byte_pack_contract(
        self,
        width: CBytePackWidth,
    ) -> Option<CIntegerBytePackContract> {
        (self.lang == Lang::C).then_some(CIntegerBytePackContract {
            width,
            base_domain: DomainRequirement::BYTE_ARRAY,
            required_high_lane_cast: match width {
                CBytePackWidth::U16 => None,
                CBytePackWidth::U32 => Some(SourceFactKind::Cast(SourceCastKind::CUnsigned32)),
            },
            channel: ChannelEligibility::ExactProven,
            evidence: OperatorEvidence::CIntegerBytePack,
        })
    }
}
