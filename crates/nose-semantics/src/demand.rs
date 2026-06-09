//! Demand and evaluation contracts for admitted builtins and higher-order forms.
//!
//! These contracts describe how an already-admitted semantic operation evaluates
//! children and callbacks. They do not admit APIs by spelling; language/library
//! admission remains the job of the occurrence contract tables and evidence checks.

use nose_il::{Builtin, HoFKind, Lang, SourceComprehensionKind, SourceProtocolKind};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DemandOperation {
    Eager,
    FoldReduction,
    ShortCircuitQuantifier,
    AppendMutation,
    NullishDefault,
    PerElementHof,
    PullLazyHof,
    CallByNeedThunk,
    AsyncContinuation,
    GeneratorSuspension,
    ChannelOperation,
    ProtocolBoundary,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EvaluationOrder {
    SourceOrder,
    ShortCircuit,
    PerElementSourceOrder,
    DeferredUntilObserved,
    RuntimeScheduled,
    ProtocolDefined,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChildDemand {
    Always,
    Never,
    Conditional,
    ShortCircuitUntilKnown,
    PerElementPull,
    MaybeRepeated,
    CallByNeedMemoized,
    SuspendedUntilObserved,
    AsyncContinuation,
    ChannelBoundary,
    ProtocolBoundary,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EffectVisibility {
    Immediate,
    OnlyIfDemanded,
    DelayedUntilPull,
    MemoizedFirstDemand,
    AsyncBoundary,
    YieldBoundary,
    ChannelBoundary,
    ProtocolBoundary,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DemandEffectProfile {
    pub operation: DemandOperation,
    pub order: EvaluationOrder,
    pub child_demand: ChildDemand,
    pub callback: Option<CallbackDemandProfile>,
    pub effect_visibility: EffectVisibility,
}

impl DemandEffectProfile {
    pub const fn eager() -> Self {
        Self {
            operation: DemandOperation::Eager,
            order: EvaluationOrder::SourceOrder,
            child_demand: ChildDemand::Always,
            callback: None,
            effect_visibility: EffectVisibility::Immediate,
        }
    }

    pub fn callback_effects_delayed_until_pull(self) -> bool {
        matches!(self.effect_visibility, EffectVisibility::DelayedUntilPull)
    }

    pub fn is_async_boundary(self) -> bool {
        matches!(self.effect_visibility, EffectVisibility::AsyncBoundary)
    }

    pub fn proves_eager_per_element_callback_demand(self) -> bool {
        matches!(self.operation, DemandOperation::PerElementHof)
            && matches!(self.order, EvaluationOrder::PerElementSourceOrder)
            && self.callback.is_some()
            && !self.callback_effects_delayed_until_pull()
    }
}

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

    pub fn demand_effect_profile(self) -> DemandEffectProfile {
        match self {
            BuiltinDemandProfile::Eager { .. } => DemandEffectProfile::eager(),
            BuiltinDemandProfile::FoldReduction => DemandEffectProfile {
                operation: DemandOperation::FoldReduction,
                order: EvaluationOrder::PerElementSourceOrder,
                child_demand: ChildDemand::PerElementPull,
                callback: Some(CallbackDemandProfile::left_fold()),
                effect_visibility: EffectVisibility::OnlyIfDemanded,
            },
            BuiltinDemandProfile::ShortCircuitQuantifier { .. } => DemandEffectProfile {
                operation: DemandOperation::ShortCircuitQuantifier,
                order: EvaluationOrder::ShortCircuit,
                child_demand: ChildDemand::ShortCircuitUntilKnown,
                callback: None,
                effect_visibility: EffectVisibility::OnlyIfDemanded,
            },
            BuiltinDemandProfile::AppendMutation => DemandEffectProfile {
                operation: DemandOperation::AppendMutation,
                order: EvaluationOrder::SourceOrder,
                child_demand: ChildDemand::Always,
                callback: None,
                effect_visibility: EffectVisibility::Immediate,
            },
            BuiltinDemandProfile::NullishDefault => DemandEffectProfile {
                operation: DemandOperation::NullishDefault,
                order: EvaluationOrder::ShortCircuit,
                child_demand: ChildDemand::Conditional,
                callback: None,
                effect_visibility: EffectVisibility::OnlyIfDemanded,
            },
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

pub fn builtin_demand_effect_profile(builtin: Builtin) -> DemandEffectProfile {
    builtin_demand_profile(builtin).demand_effect_profile()
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
    AsyncContinuation,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CallbackArgumentDemand {
    Element,
    AccumulatorAndElement,
    SettledValue,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CallbackResultDemand {
    Value,
    IterableToFlatten,
    OptionalValue,
    Predicate,
    Accumulator,
    ContinuationValue,
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

    pub const fn async_continuation() -> Self {
        Self {
            invocation: CallbackInvocationDemand::AsyncContinuation,
            arguments: CallbackArgumentDemand::SettledValue,
            result: CallbackResultDemand::ContinuationValue,
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
pub enum HofDemandTiming {
    EagerPerElement,
    PullLazy,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HofDemandSource {
    SourceComprehension(SourceComprehensionKind),
    LibraryApi(HofDemandTiming),
}

impl HofDemandSource {
    fn timing(self) -> Option<HofDemandTiming> {
        match self {
            HofDemandSource::SourceComprehension(
                SourceComprehensionKind::PythonDictComprehension
                | SourceComprehensionKind::PythonListComprehension,
            ) => Some(HofDemandTiming::EagerPerElement),
            HofDemandSource::SourceComprehension(
                SourceComprehensionKind::PythonGeneratorExpression,
            ) => Some(HofDemandTiming::PullLazy),
            HofDemandSource::SourceComprehension(
                SourceComprehensionKind::PythonSetComprehension,
            ) => None,
            HofDemandSource::LibraryApi(timing) => Some(timing),
        }
    }
}

impl HofDemandProfile {
    pub const fn callback(self) -> CallbackDemandProfile {
        match self {
            HofDemandProfile::Map { callback }
            | HofDemandProfile::FlatMap { callback }
            | HofDemandProfile::FilterMap { callback }
            | HofDemandProfile::Filter { callback }
            | HofDemandProfile::Reduce { callback } => callback,
        }
    }

    pub fn demand_effect_profile(self, timing: HofDemandTiming) -> DemandEffectProfile {
        let lazy_generator = matches!(timing, HofDemandTiming::PullLazy);
        DemandEffectProfile {
            operation: if lazy_generator {
                DemandOperation::PullLazyHof
            } else {
                DemandOperation::PerElementHof
            },
            order: if lazy_generator {
                EvaluationOrder::DeferredUntilObserved
            } else {
                EvaluationOrder::PerElementSourceOrder
            },
            child_demand: ChildDemand::PerElementPull,
            callback: Some(self.callback()),
            effect_visibility: if lazy_generator {
                EffectVisibility::DelayedUntilPull
            } else {
                EffectVisibility::OnlyIfDemanded
            },
        }
    }
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

pub fn hof_demand_effect_profile(
    kind: HoFKind,
    source: HofDemandSource,
) -> Option<DemandEffectProfile> {
    source
        .timing()
        .map(|timing| hof_contract(kind).demand.demand_effect_profile(timing))
}

pub fn library_hof_demand_timing(lang: Lang, kind: HoFKind) -> Option<HofDemandTiming> {
    Some(match (lang, kind) {
        (
            Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html,
            HoFKind::Map | HoFKind::FlatMap | HoFKind::Filter,
        )
        | (Lang::Ruby, HoFKind::Map | HoFKind::Filter) => HofDemandTiming::EagerPerElement,
        (Lang::Rust, HoFKind::Map | HoFKind::FlatMap | HoFKind::FilterMap | HoFKind::Filter)
        | (Lang::Java, HoFKind::Map | HoFKind::FlatMap | HoFKind::Filter) => {
            HofDemandTiming::PullLazy
        }
        _ => return None,
    })
}

pub fn library_hof_demand_effect_profile(lang: Lang, kind: HoFKind) -> Option<DemandEffectProfile> {
    let timing = library_hof_demand_timing(lang, kind)?;
    hof_demand_effect_profile(kind, HofDemandSource::LibraryApi(timing))
}

pub fn source_comprehension_hof_demand_effect_profile(
    kind: HoFKind,
    source: SourceComprehensionKind,
) -> Option<DemandEffectProfile> {
    hof_demand_effect_profile(kind, HofDemandSource::SourceComprehension(source))
}

pub fn promise_then_demand_effect_profile() -> DemandEffectProfile {
    DemandEffectProfile {
        operation: DemandOperation::AsyncContinuation,
        order: EvaluationOrder::RuntimeScheduled,
        child_demand: ChildDemand::AsyncContinuation,
        callback: Some(CallbackDemandProfile::async_continuation()),
        effect_visibility: EffectVisibility::AsyncBoundary,
    }
}

pub fn source_protocol_demand_effect_profile(protocol: SourceProtocolKind) -> DemandEffectProfile {
    match protocol {
        SourceProtocolKind::Await => DemandEffectProfile {
            operation: DemandOperation::AsyncContinuation,
            order: EvaluationOrder::RuntimeScheduled,
            child_demand: ChildDemand::AsyncContinuation,
            callback: None,
            effect_visibility: EffectVisibility::AsyncBoundary,
        },
        SourceProtocolKind::AsyncBlock => DemandEffectProfile {
            operation: DemandOperation::CallByNeedThunk,
            order: EvaluationOrder::DeferredUntilObserved,
            child_demand: ChildDemand::SuspendedUntilObserved,
            callback: None,
            effect_visibility: EffectVisibility::AsyncBoundary,
        },
        SourceProtocolKind::Yield => DemandEffectProfile {
            operation: DemandOperation::GeneratorSuspension,
            order: EvaluationOrder::DeferredUntilObserved,
            child_demand: ChildDemand::SuspendedUntilObserved,
            callback: None,
            effect_visibility: EffectVisibility::YieldBoundary,
        },
        SourceProtocolKind::ChannelReceive
        | SourceProtocolKind::ChannelSelect
        | SourceProtocolKind::ChannelSelectCase
        | SourceProtocolKind::ChannelSelectDefault
        | SourceProtocolKind::ChannelSend => DemandEffectProfile {
            operation: DemandOperation::ChannelOperation,
            order: EvaluationOrder::ProtocolDefined,
            child_demand: ChildDemand::ChannelBoundary,
            callback: None,
            effect_visibility: EffectVisibility::ChannelBoundary,
        },
        SourceProtocolKind::Defer | SourceProtocolKind::GoRoutine => DemandEffectProfile {
            operation: DemandOperation::ProtocolBoundary,
            order: EvaluationOrder::ProtocolDefined,
            child_demand: ChildDemand::ProtocolBoundary,
            callback: None,
            effect_visibility: EffectVisibility::ProtocolBoundary,
        },
        SourceProtocolKind::TryPropagation => DemandEffectProfile {
            operation: DemandOperation::ShortCircuitQuantifier,
            order: EvaluationOrder::ShortCircuit,
            child_demand: ChildDemand::Conditional,
            callback: None,
            effect_visibility: EffectVisibility::OnlyIfDemanded,
        },
    }
}
