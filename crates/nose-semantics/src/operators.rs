//! Operator and value-domain semantic contracts.

use super::*;

mod callback_effects;
mod collection_contracts;
mod comparisons;
mod value_domains;

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
