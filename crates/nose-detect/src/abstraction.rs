//! Typed weak-claim witnesses for the experimental `abstraction` scan surface.
//!
//! This module owns the anti-unification policy. Unit extraction only records the
//! pre-order token stream; deciding whether a token difference is a meaningful
//! refactoring-template hole lives here so future type/domain/operator holes do
//! not get mixed back into ordinary unit feature extraction.

use nose_il::{Il, Interner, Lang, LitClass, NodeId, NodeKind, Payload, UnitKind};
use nose_normalize::node_tag_valued;

use crate::{AbstractionHole, AbstractionWitness};

const CLAIM: &str = "weak-refactoring-template";
const BASIS_FAMILY: &str = "family";
const TEMPLATE_FORMAT: &str = "normalized-il-preorder";

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
pub(crate) struct WitnessToken {
    kind: NodeKind,
    arity: u16,
    shape_tag: u64,
    exact_tag: u64,
    literal: Option<LiteralClass>,
    line: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
enum LiteralClass {
    Int,
    Float,
    Str,
}

impl LiteralClass {
    fn label(self) -> &'static str {
        match self {
            LiteralClass::Int => "int-literal",
            LiteralClass::Float => "float-literal",
            LiteralClass::Str => "string-literal",
        }
    }
}

#[derive(Clone, Copy)]
enum HoleKind {
    Literal,
}

impl HoleKind {
    fn label(self) -> &'static str {
        match self {
            HoleKind::Literal => "literal",
        }
    }

    fn role(self) -> &'static str {
        match self {
            HoleKind::Literal => "leaf",
        }
    }
}

#[derive(Clone, Copy)]
enum ReasonCode {
    TypeParametric,
    LiteralAbstracted,
}

impl ReasonCode {
    fn label(self) -> &'static str {
        match self {
            ReasonCode::TypeParametric => "type-parametric",
            ReasonCode::LiteralAbstracted => "literal-abstracted",
        }
    }
}

#[derive(Clone, Copy)]
enum Caveat {
    NumericDomainSensitive,
}

impl Caveat {
    fn label(self) -> &'static str {
        match self {
            Caveat::NumericDomainSensitive => "numeric-domain-sensitive",
        }
    }
}

struct HoleDecision {
    kind: HoleKind,
    reason_code: ReasonCode,
    caveats: Vec<Caveat>,
}

struct HoleMatch {
    template_index: usize,
    left: WitnessToken,
    right: WitnessToken,
    decision: HoleDecision,
    observed: Vec<LiteralClass>,
}

pub(crate) fn token_for(il: &Il, interner: &Interner, nid: NodeId, shape_tag: u64) -> WitnessToken {
    let n = il.node(nid);
    WitnessToken {
        kind: n.kind,
        arity: il.children(nid).len().min(u16::MAX as usize) as u16,
        shape_tag,
        exact_tag: node_tag_valued(n.kind, n.payload, interner),
        literal: literal_class(n.payload),
        line: n.span.start_line,
    }
}

pub(crate) fn family_witness(
    units: &[(Lang, UnitKind, &[WitnessToken])],
) -> Option<AbstractionWitness> {
    if units.len() < 2 {
        return None;
    }

    let &(base_lang, base_kind, base_tokens) = units.first()?;
    if base_tokens.is_empty() {
        return None;
    }

    let mut hole_index = None;
    for &(lang, kind, tokens) in units {
        if lang != base_lang || kind != base_kind || tokens.len() != base_tokens.len() {
            return None;
        }
        for (idx, (base, token)) in base_tokens.iter().zip(tokens).enumerate() {
            if same_token(base, token) {
                continue;
            }
            match hole_index {
                Some(existing) if existing != idx => return None,
                Some(_) => {}
                None => hole_index = Some(idx),
            }
        }
    }

    let hole_index = hole_index?;
    let left = base_tokens[hole_index];
    let right = units
        .iter()
        .map(|(_, _, tokens)| tokens[hole_index])
        .find(|token| !same_token(&left, token))?;
    let hole_tokens = units
        .iter()
        .map(|(_, _, tokens)| tokens[hole_index])
        .collect::<Vec<_>>();
    let decision = literal_family_hole(&hole_tokens)?;
    let observed = observed_literal_classes(&hole_tokens)?;

    Some(render_witness(
        BASIS_FAMILY,
        units.len(),
        base_tokens,
        HoleMatch {
            template_index: hole_index,
            left,
            right,
            decision,
            observed,
        },
    ))
}

fn render_witness(
    basis: &'static str,
    members_checked: usize,
    tokens: &[WitnessToken],
    hole: HoleMatch,
) -> AbstractionWitness {
    let template = render_template(tokens, hole.template_index, hole.decision.kind);
    AbstractionWitness {
        claim: CLAIM,
        basis,
        members_checked: members_checked.min(u32::MAX as usize) as u32,
        reason_code: hole.decision.reason_code.label(),
        template_format: TEMPLATE_FORMAT,
        template,
        holes: vec![AbstractionHole {
            index: 1,
            template_index: hole.template_index as u32,
            kind: hole.decision.kind.label(),
            role: hole.decision.kind.role(),
            left: hole.left.literal.expect("literal witness left").label(),
            right: hole.right.literal.expect("literal witness right").label(),
            observed: hole
                .observed
                .iter()
                .map(|literal| literal.label())
                .collect(),
            left_line: hole.left.line,
            right_line: hole.right.line,
        }],
        caveats: hole
            .decision
            .caveats
            .iter()
            .map(|caveat| caveat.label())
            .collect(),
    }
}

fn same_token(left: &WitnessToken, right: &WitnessToken) -> bool {
    left.kind == right.kind && left.arity == right.arity && left.exact_tag == right.exact_tag
}

fn literal_family_hole(tokens: &[WitnessToken]) -> Option<HoleDecision> {
    let first = tokens.first()?;
    for token in tokens {
        if token.kind != NodeKind::Lit || token.arity != 0 || token.literal.is_none() {
            return None;
        }
    }

    let observed = observed_literal_classes(tokens)?;
    let numeric_only = observed
        .iter()
        .all(|literal| matches!(literal, LiteralClass::Int | LiteralClass::Float));
    let same_shape = tokens
        .iter()
        .all(|token| token.shape_tag == first.shape_tag);
    if !same_shape && !numeric_only {
        return None;
    }

    match observed.as_slice() {
        [LiteralClass::Int, LiteralClass::Float] => Some(HoleDecision {
            kind: HoleKind::Literal,
            reason_code: ReasonCode::TypeParametric,
            caveats: vec![Caveat::NumericDomainSensitive],
        }),
        [LiteralClass::Int] | [LiteralClass::Float] | [LiteralClass::Str] => Some(HoleDecision {
            kind: HoleKind::Literal,
            reason_code: ReasonCode::LiteralAbstracted,
            caveats: Vec::new(),
        }),
        _ => None,
    }
}

fn observed_literal_classes(tokens: &[WitnessToken]) -> Option<Vec<LiteralClass>> {
    let mut observed = tokens
        .iter()
        .map(|token| token.literal)
        .collect::<Option<Vec<_>>>()?;
    observed.sort_unstable();
    observed.dedup();
    Some(observed)
}

fn literal_class(payload: Payload) -> Option<LiteralClass> {
    match payload {
        Payload::LitInt(_) | Payload::Lit(LitClass::Int) => Some(LiteralClass::Int),
        Payload::LitFloat(_) | Payload::Lit(LitClass::Float) => Some(LiteralClass::Float),
        Payload::LitStr(_) | Payload::Lit(LitClass::Str) => Some(LiteralClass::Str),
        _ => None,
    }
}

fn render_template(
    tokens: &[WitnessToken],
    hole_template_index: usize,
    hole_kind: HoleKind,
) -> Vec<String> {
    tokens
        .iter()
        .enumerate()
        .map(|(idx, token)| {
            if idx == hole_template_index {
                format!("<hole 1: {}>", hole_kind.label())
            } else {
                token_label(token).to_string()
            }
        })
        .collect()
}

fn token_label(token: &WitnessToken) -> &'static str {
    match token.literal {
        Some(LiteralClass::Int) => "Lit<Int>",
        Some(LiteralClass::Float) => "Lit<Float>",
        Some(LiteralClass::Str) => "Lit<String>",
        None => match token.kind {
            NodeKind::Module => "Module",
            NodeKind::Func => "Func",
            NodeKind::Param => "Param",
            NodeKind::Block => "Block",
            NodeKind::Assign => "Assign",
            NodeKind::ExprStmt => "ExprStmt",
            NodeKind::Return => "Return",
            NodeKind::If => "If",
            NodeKind::Loop => "Loop",
            NodeKind::Break => "Break",
            NodeKind::Continue => "Continue",
            NodeKind::Throw => "Throw",
            NodeKind::Try => "Try",
            NodeKind::Var => "Var",
            NodeKind::Lit => "Lit",
            NodeKind::BinOp => "BinOp",
            NodeKind::UnOp => "UnOp",
            NodeKind::Call => "Call",
            NodeKind::Index => "Index",
            NodeKind::Field => "Field",
            NodeKind::Lambda => "Lambda",
            NodeKind::Seq => "Seq",
            NodeKind::HoF => "HoF",
            NodeKind::KwArg => "KwArg",
            NodeKind::Raw => "Raw",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(kind: NodeKind, exact_tag: u64) -> WitnessToken {
        WitnessToken {
            kind,
            arity: 0,
            shape_tag: exact_tag,
            exact_tag,
            literal: None,
            line: 1,
        }
    }

    fn int_lit(exact_tag: u64, line: u32) -> WitnessToken {
        WitnessToken {
            kind: NodeKind::Lit,
            arity: 0,
            shape_tag: 100,
            exact_tag,
            literal: Some(LiteralClass::Int),
            line,
        }
    }

    #[test]
    fn family_witness_rejects_mixed_hole_positions() {
        let base = [node(NodeKind::Return, 1), int_lit(10, 2), int_lit(20, 3)];
        let first_literal_changed = [node(NodeKind::Return, 1), int_lit(11, 2), int_lit(20, 3)];
        let second_literal_changed = [node(NodeKind::Return, 1), int_lit(10, 2), int_lit(21, 3)];
        let units = [
            (Lang::Python, UnitKind::Function, base.as_slice()),
            (
                Lang::Python,
                UnitKind::Function,
                first_literal_changed.as_slice(),
            ),
            (
                Lang::Python,
                UnitKind::Function,
                second_literal_changed.as_slice(),
            ),
        ];

        assert!(
            family_witness(&units).is_none(),
            "one weak witness must not cover more than one template hole"
        );
    }
}
