use super::OperatorSemantics;
use crate::{
    domain_evidence_for_receiver, domain_evidence_for_var_reference, js_like_lang,
    source_operator_at_node, DomainEvidence,
};
use nose_il::{Il, Interner, Lang, LitClass, NodeId, NodeKind, Op, Payload, SourceOperatorKind};

/// Primitive value domains narrow enough to prove that a callback operator cannot invoke
/// user dispatch, coercion hooks, or a language-level trap.
#[derive(Clone, Copy, PartialEq, Eq)]
enum PrimitiveEffectDomain {
    Boolean,
    Integer,
    Float,
    Number,
    String,
}

impl PrimitiveEffectDomain {
    fn from_evidence(lang: Lang, domain: DomainEvidence) -> Option<Self> {
        match domain {
            DomainEvidence::Boolean => Some(Self::Boolean),
            DomainEvidence::Integer if !js_like_lang(lang) => Some(Self::Integer),
            DomainEvidence::Float => Some(Self::Float),
            DomainEvidence::Number => Some(Self::Number),
            DomainEvidence::String => Some(Self::String),
            _ => None,
        }
    }

    fn is_numeric(self) -> bool {
        matches!(self, Self::Integer | Self::Float | Self::Number)
    }
}

impl OperatorSemantics {
    /// Return whether a value-transform operator expression is proven effect-closed.
    ///
    /// This is deliberately stricter than ordinary value-domain inference: transform
    /// admission must rule out overloaded dispatch, observable coercion, and traps. Unknown
    /// domains therefore close the proof instead of being inferred from the operator itself.
    pub(crate) fn pure_transform_operator_effect_closed(
        self,
        il: &Il,
        interner: Option<&Interner>,
        node: NodeId,
    ) -> bool {
        if self.lang == Lang::Swift || !self.callback_source_operator_effect_closed(il, node) {
            return false;
        }
        self.pure_transform_expression_domain(il, interner, node)
            .is_some()
    }

    fn callback_source_operator_effect_closed(self, il: &Il, node: NodeId) -> bool {
        if !js_like_lang(self.lang) {
            return true;
        }
        let Payload::Op(op) = il.node(node).payload else {
            return false;
        };
        match op {
            // JavaScript lowers equality and `instanceof` onto the same abstract
            // Eq/Ne shapes. Require a unique admitted equality source fact so a
            // missing, ambiguous, dependency-broken, or TypeMembership fact can
            // never be reinterpreted as primitive equality.
            Op::Eq => matches!(
                source_operator_at_node(il, node),
                Some(SourceOperatorKind::StrictEquality | SourceOperatorKind::LooseEquality)
            ),
            Op::Ne => matches!(
                source_operator_at_node(il, node),
                Some(SourceOperatorKind::StrictInequality | SourceOperatorKind::LooseInequality)
            ),
            _ => source_operator_at_node(il, node) != Some(SourceOperatorKind::TypeMembership),
        }
    }

    fn pure_transform_expression_domain(
        self,
        il: &Il,
        interner: Option<&Interner>,
        node: NodeId,
    ) -> Option<PrimitiveEffectDomain> {
        match il.kind(node) {
            NodeKind::Lit => self.callback_literal_domain(il.node(node).payload),
            NodeKind::Var => {
                let domain = match interner {
                    Some(interner) => domain_evidence_for_receiver(il, interner, node),
                    None => domain_evidence_for_var_reference(il, node),
                }?;
                PrimitiveEffectDomain::from_evidence(self.lang, domain)
            }
            NodeKind::BinOp => {
                let [left, right] = il.children(node) else {
                    return None;
                };
                let Payload::Op(op) = il.node(node).payload else {
                    return None;
                };
                let left = self.pure_transform_expression_domain(il, interner, *left)?;
                let right = self.pure_transform_expression_domain(il, interner, *right)?;
                self.callback_binary_operator_result(op, left, right)
            }
            NodeKind::UnOp => {
                let [operand] = il.children(node) else {
                    return None;
                };
                let Payload::Op(op) = il.node(node).payload else {
                    return None;
                };
                let operand = self.pure_transform_expression_domain(il, interner, *operand)?;
                self.callback_unary_operator_result(op, operand)
            }
            _ => None,
        }
    }

    fn callback_literal_domain(self, payload: Payload) -> Option<PrimitiveEffectDomain> {
        match payload {
            Payload::LitInt(_) => Some(PrimitiveEffectDomain::Integer),
            // JS/TS lowers BigInt and other non-i64 numeric spellings to the same abstract
            // integer class. Until source evidence distinguishes Number from BigInt, treating
            // that class as Number could admit mixed-BigInt arithmetic that throws.
            Payload::Lit(LitClass::Int) if !js_like_lang(self.lang) => {
                Some(PrimitiveEffectDomain::Integer)
            }
            Payload::LitFloat(_) | Payload::Lit(LitClass::Float) => {
                Some(PrimitiveEffectDomain::Float)
            }
            Payload::LitStr(_) | Payload::Lit(LitClass::Str) => Some(PrimitiveEffectDomain::String),
            Payload::LitBool(_) | Payload::Lit(LitClass::Bool) => {
                Some(PrimitiveEffectDomain::Boolean)
            }
            _ => None,
        }
    }

    fn callback_binary_operator_result(
        self,
        op: Op,
        left: PrimitiveEffectDomain,
        right: PrimitiveEffectDomain,
    ) -> Option<PrimitiveEffectDomain> {
        if js_like_lang(self.lang) {
            return js_like_callback_binary_operator_result(op, left, right);
        }
        None
    }

    fn callback_unary_operator_result(
        self,
        op: Op,
        operand: PrimitiveEffectDomain,
    ) -> Option<PrimitiveEffectDomain> {
        if js_like_lang(self.lang) {
            if op == Op::Not {
                return Some(PrimitiveEffectDomain::Boolean);
            }
            if matches!(op, Op::Neg | Op::Pos | Op::BitNot) && operand.is_numeric() {
                return Some(PrimitiveEffectDomain::Number);
            }
        }
        None
    }
}

fn js_like_callback_binary_operator_result(
    op: Op,
    left: PrimitiveEffectDomain,
    right: PrimitiveEffectDomain,
) -> Option<PrimitiveEffectDomain> {
    if matches!(op, Op::Eq | Op::Ne | Op::Lt | Op::Le | Op::Gt | Op::Ge) {
        return Some(PrimitiveEffectDomain::Boolean);
    }
    if op == Op::Add
        && (left == PrimitiveEffectDomain::String || right == PrimitiveEffectDomain::String)
    {
        return Some(PrimitiveEffectDomain::String);
    }
    if matches!(op, Op::And | Op::Or) && left == right {
        return Some(left);
    }
    if !left.is_numeric() || !right.is_numeric() {
        return None;
    }
    matches!(
        op,
        Op::Add
            | Op::Sub
            | Op::Mul
            | Op::TrueDiv
            | Op::Mod
            | Op::Pow
            | Op::BitAnd
            | Op::BitOr
            | Op::BitXor
            | Op::Shl
            | Op::Shr
    )
    .then_some(PrimitiveEffectDomain::Number)
}
