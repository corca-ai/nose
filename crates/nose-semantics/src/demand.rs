//! Demand and evaluation contracts for admitted builtins and higher-order forms.
//!
//! These contracts describe how an already-admitted semantic operation evaluates
//! children and callbacks. They do not admit APIs by spelling; language/library
//! admission remains the job of the occurrence contract tables and evidence checks.

use nose_il::{Builtin, HoFKind};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BuiltinDemand {
    Eager,
    Reduce,
    AnyAll { all: bool },
    Append,
    ValueOrDefault,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BuiltinDemandProfile {
    Eager { contract: EagerBuiltinContract },
    FoldReduction,
    ShortCircuitQuantifier { all: bool },
    AppendMutation,
    NullishDefault,
}

impl BuiltinDemandProfile {
    pub fn legacy_demand(self) -> BuiltinDemand {
        match self {
            BuiltinDemandProfile::Eager { .. } => BuiltinDemand::Eager,
            BuiltinDemandProfile::FoldReduction => BuiltinDemand::Reduce,
            BuiltinDemandProfile::ShortCircuitQuantifier { all } => BuiltinDemand::AnyAll { all },
            BuiltinDemandProfile::AppendMutation => BuiltinDemand::Append,
            BuiltinDemandProfile::NullishDefault => BuiltinDemand::ValueOrDefault,
        }
    }

    pub fn eager_contract(self) -> Option<EagerBuiltinContract> {
        match self {
            BuiltinDemandProfile::Eager { contract } => Some(contract),
            BuiltinDemandProfile::FoldReduction
            | BuiltinDemandProfile::ShortCircuitQuantifier { .. }
            | BuiltinDemandProfile::AppendMutation
            | BuiltinDemandProfile::NullishDefault => None,
        }
    }
}

pub fn builtin_demand_profile(builtin: Builtin) -> BuiltinDemandProfile {
    match builtin {
        Builtin::Reduce => BuiltinDemandProfile::FoldReduction,
        Builtin::Any => BuiltinDemandProfile::ShortCircuitQuantifier { all: false },
        Builtin::All => BuiltinDemandProfile::ShortCircuitQuantifier { all: true },
        Builtin::Append => BuiltinDemandProfile::AppendMutation,
        Builtin::ValueOrDefault => BuiltinDemandProfile::NullishDefault,
        _ => BuiltinDemandProfile::Eager {
            contract: eager_builtin_contract(builtin)
                .expect("every non-special builtin has an eager demand contract"),
        },
    }
}

pub fn builtin_demand(builtin: Builtin) -> BuiltinDemand {
    builtin_demand_profile(builtin).legacy_demand()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EagerBuiltinContract {
    Len,
    IsEmpty,
    IsNull,
    IsNotNull,
    StartsWith,
    EndsWith,
    Contains,
    Join,
    Abs,
    UnsignedCast32,
    Sum,
    Min,
    Max,
    Range,
    Zip,
    Enumerate,
    Keys,
    Print,
    DictEntry,
    GetOrDefault,
}

pub fn eager_builtin_contract(builtin: Builtin) -> Option<EagerBuiltinContract> {
    Some(match builtin {
        Builtin::Len => EagerBuiltinContract::Len,
        Builtin::IsEmpty => EagerBuiltinContract::IsEmpty,
        Builtin::IsNull => EagerBuiltinContract::IsNull,
        Builtin::IsNotNull => EagerBuiltinContract::IsNotNull,
        Builtin::StartsWith => EagerBuiltinContract::StartsWith,
        Builtin::EndsWith => EagerBuiltinContract::EndsWith,
        Builtin::Contains => EagerBuiltinContract::Contains,
        Builtin::Join => EagerBuiltinContract::Join,
        Builtin::Abs => EagerBuiltinContract::Abs,
        Builtin::UnsignedCast32 => EagerBuiltinContract::UnsignedCast32,
        Builtin::Sum => EagerBuiltinContract::Sum,
        Builtin::Min => EagerBuiltinContract::Min,
        Builtin::Max => EagerBuiltinContract::Max,
        Builtin::Range => EagerBuiltinContract::Range,
        Builtin::Zip => EagerBuiltinContract::Zip,
        Builtin::Enumerate => EagerBuiltinContract::Enumerate,
        Builtin::Keys => EagerBuiltinContract::Keys,
        Builtin::Print => EagerBuiltinContract::Print,
        Builtin::DictEntry => EagerBuiltinContract::DictEntry,
        Builtin::GetOrDefault => EagerBuiltinContract::GetOrDefault,
        Builtin::Reduce
        | Builtin::Any
        | Builtin::All
        | Builtin::Append
        | Builtin::ValueOrDefault => return None,
    })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ReductionBuiltinContract {
    Len,
    Sum,
    ExplicitFold,
    Selection { max: bool },
    Bool { all: bool },
    Join,
}

pub fn reduction_builtin_contract(builtin: Builtin) -> Option<ReductionBuiltinContract> {
    Some(match builtin {
        Builtin::Len => ReductionBuiltinContract::Len,
        Builtin::Sum => ReductionBuiltinContract::Sum,
        Builtin::Reduce => ReductionBuiltinContract::ExplicitFold,
        Builtin::Min => ReductionBuiltinContract::Selection { max: false },
        Builtin::Max => ReductionBuiltinContract::Selection { max: true },
        Builtin::Any => ReductionBuiltinContract::Bool { all: false },
        Builtin::All => ReductionBuiltinContract::Bool { all: true },
        Builtin::Join => ReductionBuiltinContract::Join,
        _ => return None,
    })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CallbackInvocationDemand {
    PerElementPull,
    LeftFoldStep,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CallbackArgumentDemand {
    Element,
    AccumulatorAndElement,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CallbackResultDemand {
    Value,
    IterableToFlatten,
    OptionalValue,
    Predicate,
    Accumulator,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CallbackDemandProfile {
    pub invocation: CallbackInvocationDemand,
    pub arguments: CallbackArgumentDemand,
    pub result: CallbackResultDemand,
}

impl CallbackDemandProfile {
    pub const fn unary_element(result: CallbackResultDemand) -> Self {
        Self {
            invocation: CallbackInvocationDemand::PerElementPull,
            arguments: CallbackArgumentDemand::Element,
            result,
        }
    }

    pub const fn left_fold() -> Self {
        Self {
            invocation: CallbackInvocationDemand::LeftFoldStep,
            arguments: CallbackArgumentDemand::AccumulatorAndElement,
            result: CallbackResultDemand::Accumulator,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HofDemandProfile {
    Map { callback: CallbackDemandProfile },
    FlatMap { callback: CallbackDemandProfile },
    FilterMap { callback: CallbackDemandProfile },
    Filter { callback: CallbackDemandProfile },
    Reduce { callback: CallbackDemandProfile },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct HofContract {
    pub kind: HoFKind,
    pub demand: HofDemandProfile,
}

pub fn hof_contract(kind: HoFKind) -> HofContract {
    let demand = match kind {
        HoFKind::Map => HofDemandProfile::Map {
            callback: CallbackDemandProfile::unary_element(CallbackResultDemand::Value),
        },
        HoFKind::FlatMap => HofDemandProfile::FlatMap {
            callback: CallbackDemandProfile::unary_element(CallbackResultDemand::IterableToFlatten),
        },
        HoFKind::FilterMap => HofDemandProfile::FilterMap {
            callback: CallbackDemandProfile::unary_element(CallbackResultDemand::OptionalValue),
        },
        HoFKind::Filter => HofDemandProfile::Filter {
            callback: CallbackDemandProfile::unary_element(CallbackResultDemand::Predicate),
        },
        HoFKind::Reduce => HofDemandProfile::Reduce {
            callback: CallbackDemandProfile::left_fold(),
        },
    };
    HofContract { kind, demand }
}
