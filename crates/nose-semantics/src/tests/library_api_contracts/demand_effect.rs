use super::*;

#[test]
fn receiver_mutation_contracts_are_language_scoped_rows() {
    let js_push = module_binding_mutating_method_contract(Lang::JavaScript, "push", 1)
        .expect("js push receiver mutation contract");
    assert_eq!(js_push.pack_id, JS_TS_LANGUAGE_PACK_ID);
    assert_eq!(js_push.lang, Lang::JavaScript);
    assert_eq!(js_push.effect, EffectEvidenceKind::ReceiverMutation);
    assert_eq!(
        js_push.receiver,
        MethodEffectReceiverContract::PotentiallyMutableReceiver
    );
    assert_eq!(
        module_binding_mutating_method_contract(Lang::TypeScript, "push", 1)
            .expect("typescript push receiver mutation contract")
            .pack_id,
        JS_TS_LANGUAGE_PACK_ID
    );
    assert!(module_binding_mutating_method_contract(Lang::JavaScript, "addAll", 1).is_none());
    assert!(module_binding_mutating_method_contract(Lang::Java, "addAll", 1).is_some());
    assert!(module_binding_mutating_method_contract(Lang::Python, "append", 1).is_some());
    let swift_append = module_binding_mutating_method_contract(Lang::Swift, "append", 1)
        .expect("swift append receiver mutation contract");
    assert_eq!(swift_append.pack_id, SWIFT_LANGUAGE_PACK_ID);
    assert_eq!(swift_append.lang, Lang::Swift);
    assert_eq!(swift_append.effect, EffectEvidenceKind::ReceiverMutation);
    assert!(module_binding_mutating_method_contract(
        Lang::Swift,
        "withUnsafeMutableBufferPointer",
        1
    )
    .is_some());
    assert!(module_binding_mutating_method_contract(Lang::Go, "append", 1).is_none());
}

#[test]
fn builtin_contracts_preserve_current_special_demand_split() {
    for &builtin in ALL_BUILTINS {
        assert_eq!(builtin_tag(builtin), builtin as u32 + 1);
    }
    assert_eq!(
        builtin_demand_profile(Builtin::Reduce),
        BuiltinDemandProfile::FoldReduction
    );
    assert_eq!(
        builtin_demand_profile(Builtin::Any),
        BuiltinDemandProfile::ShortCircuitQuantifier { all: false }
    );
    assert_eq!(
        builtin_demand_profile(Builtin::All),
        BuiltinDemandProfile::ShortCircuitQuantifier { all: true }
    );
    assert_eq!(
        builtin_demand_effect_profile(Builtin::All),
        DemandEffectProfile {
            operation: DemandOperation::ShortCircuitQuantifier,
            order: EvaluationOrder::ShortCircuit,
            child_demand: ChildDemand::ShortCircuitUntilKnown,
            callback: None,
            effect_visibility: EffectVisibility::OnlyIfDemanded,
        }
    );
    assert_eq!(
        builtin_demand_effect_profile(Builtin::Reduce),
        DemandEffectProfile {
            operation: DemandOperation::FoldReduction,
            order: EvaluationOrder::PerElementSourceOrder,
            child_demand: ChildDemand::PerElementPull,
            callback: Some(CallbackDemandProfile::left_fold()),
            effect_visibility: EffectVisibility::OnlyIfDemanded,
        }
    );
    assert_eq!(
        builtin_demand_profile(Builtin::Append),
        BuiltinDemandProfile::AppendMutation
    );
    assert_eq!(
        builtin_demand_profile(Builtin::ValueOrDefault),
        BuiltinDemandProfile::NullishDefault
    );
    assert_eq!(
        builtin_demand_profile(Builtin::Len),
        BuiltinDemandProfile::Eager {
            contract: EagerBuiltinContract::Len
        }
    );
    assert_eq!(
        eager_builtin_contract(Builtin::Len),
        Some(EagerBuiltinContract::Len)
    );
    assert_eq!(eager_builtin_contract(Builtin::Append), None);
    assert_eq!(
        reduction_builtin_contract(Builtin::Max),
        Some(ReductionBuiltinContract::Selection { max: true })
    );
    assert_eq!(
        reduction_builtin_contract(Builtin::Any),
        Some(ReductionBuiltinContract::Bool { all: false })
    );
    assert_eq!(reduction_builtin_contract(Builtin::Print), None);
}

#[test]
fn hof_contracts_carry_callback_demand_profiles() {
    assert_eq!(
        hof_contract(HoFKind::FilterMap),
        HofContract {
            kind: HoFKind::FilterMap,
            demand: HofDemandProfile::FilterMap {
                callback: CallbackDemandProfile::unary_element(CallbackResultDemand::OptionalValue)
            }
        }
    );
    assert_eq!(
        hof_contract(HoFKind::Reduce).demand,
        HofDemandProfile::Reduce {
            callback: CallbackDemandProfile::left_fold()
        }
    );
    assert_eq!(
        hof_demand_effect_profile(
            HoFKind::Map,
            HofDemandSource::SourceComprehension(SourceComprehensionKind::PythonListComprehension)
        ),
        Some(DemandEffectProfile {
            operation: DemandOperation::PerElementHof,
            order: EvaluationOrder::PerElementSourceOrder,
            child_demand: ChildDemand::PerElementPull,
            callback: Some(CallbackDemandProfile::unary_element(
                CallbackResultDemand::Value
            )),
            effect_visibility: EffectVisibility::OnlyIfDemanded,
        })
    );
    assert_eq!(
        hof_demand_effect_profile(
            HoFKind::Map,
            HofDemandSource::SourceComprehension(
                SourceComprehensionKind::PythonGeneratorExpression
            )
        ),
        Some(DemandEffectProfile {
            operation: DemandOperation::PullLazyHof,
            order: EvaluationOrder::DeferredUntilObserved,
            child_demand: ChildDemand::PerElementPull,
            callback: Some(CallbackDemandProfile::unary_element(
                CallbackResultDemand::Value
            )),
            effect_visibility: EffectVisibility::DelayedUntilPull,
        })
    );
    assert_eq!(
        hof_demand_effect_profile(
            HoFKind::Map,
            HofDemandSource::SourceComprehension(SourceComprehensionKind::PythonSetComprehension)
        ),
        None
    );
    assert_eq!(
        hof_demand_effect_profile(
            HoFKind::Map,
            HofDemandSource::LibraryApi(HofDemandTiming::PullLazy)
        ),
        Some(DemandEffectProfile {
            operation: DemandOperation::PullLazyHof,
            order: EvaluationOrder::DeferredUntilObserved,
            child_demand: ChildDemand::PerElementPull,
            callback: Some(CallbackDemandProfile::unary_element(
                CallbackResultDemand::Value
            )),
            effect_visibility: EffectVisibility::DelayedUntilPull,
        })
    );
}

#[test]
fn hof_demand_effect_profiles_split_eager_and_pull_lazy_timing() {
    assert!(hof_demand_effect_profile(
        HoFKind::Map,
        HofDemandSource::SourceComprehension(SourceComprehensionKind::PythonGeneratorExpression)
    )
    .unwrap()
    .callback_effects_delayed_until_pull());
    assert!(hof_demand_effect_profile(
        HoFKind::Map,
        HofDemandSource::SourceComprehension(SourceComprehensionKind::PythonListComprehension)
    )
    .unwrap()
    .proves_eager_per_element_callback_demand());
    assert!(
        library_hof_demand_effect_profile(Lang::TypeScript, HoFKind::Map)
            .unwrap()
            .proves_eager_per_element_callback_demand()
    );
    assert!(
        library_hof_demand_effect_profile(Lang::Ruby, HoFKind::Filter)
            .unwrap()
            .proves_eager_per_element_callback_demand()
    );
    assert!(
        library_hof_demand_effect_profile(Lang::Swift, HoFKind::FlatMap)
            .unwrap()
            .proves_eager_per_element_callback_demand()
    );
    assert!(library_hof_demand_effect_profile(Lang::Rust, HoFKind::Map)
        .unwrap()
        .callback_effects_delayed_until_pull());
    assert!(
        library_hof_demand_effect_profile(Lang::Java, HoFKind::FlatMap)
            .unwrap()
            .callback_effects_delayed_until_pull()
    );
    assert!(
        library_hof_demand_effect_profile(Lang::Python, HoFKind::Map)
            .unwrap()
            .callback_effects_delayed_until_pull()
    );
    assert!(
        library_hof_demand_effect_profile(Lang::Python, HoFKind::Filter)
            .unwrap()
            .callback_effects_delayed_until_pull()
    );
}

#[test]
fn promise_and_protocol_demand_profiles_keep_async_boundaries() {
    assert_eq!(
        promise_then_demand_effect_profile(),
        DemandEffectProfile {
            operation: DemandOperation::AsyncContinuation,
            order: EvaluationOrder::RuntimeScheduled,
            child_demand: ChildDemand::AsyncContinuation,
            callback: Some(CallbackDemandProfile::async_continuation()),
            effect_visibility: EffectVisibility::AsyncBoundary,
        }
    );
    assert!(promise_then_demand_effect_profile().is_async_boundary());
    assert_eq!(
        source_protocol_demand_effect_profile(SourceProtocolKind::ChannelSend).effect_visibility,
        EffectVisibility::ChannelBoundary
    );
    assert_eq!(
        source_protocol_demand_effect_profile(SourceProtocolKind::GoRoutine).operation,
        DemandOperation::ProtocolBoundary
    );
    assert_eq!(
        source_protocol_demand_effect_profile(SourceProtocolKind::Defer).effect_visibility,
        EffectVisibility::ProtocolBoundary
    );
}

#[test]
fn free_function_builtin_contracts_are_language_and_shadow_constrained() {
    assert_eq!(
        free_function_builtin_contract(Lang::Python, "len", 1),
        Some(FreeFunctionBuiltinContract {
            name: "len",
            builtin: Builtin::Len,
            args: BuiltinArgContract::First,
            requires_unshadowed: true,
        })
    );
    assert_eq!(free_function_builtin_contract(Lang::Python, "len", 2), None);
    assert_eq!(
        free_function_builtin_contract(Lang::JavaScript, "len", 1),
        None
    );
    assert_eq!(
        free_function_builtin_contract(Lang::Python, "print", 3),
        Some(FreeFunctionBuiltinContract {
            name: "print",
            builtin: Builtin::Print,
            args: BuiltinArgContract::All,
            requires_unshadowed: true,
        })
    );
    assert_eq!(
        free_function_builtin_contract(Lang::Go, "append", 2),
        Some(FreeFunctionBuiltinContract {
            name: "append",
            builtin: Builtin::Append,
            args: BuiltinArgContract::All,
            requires_unshadowed: true,
        })
    );
    assert_eq!(free_function_builtin_contract(Lang::Go, "append", 1), None);
    assert_eq!(free_function_builtin_contract(Lang::C, "fmaxf", 2), None);
    assert_eq!(
        free_function_builtin_contract(Lang::Python, "fmaxf", 2),
        None
    );
    assert_eq!(
        free_function_builtin_contract(Lang::Python, "range", 0),
        None
    );
    assert!(free_function_builtin_contract(Lang::Python, "range", 3).is_some());
    assert_eq!(
        free_function_builtin_contract(Lang::Python, "range", 4),
        None
    );
    assert_eq!(
        free_function_builtin_contract(Lang::Python, "max", 2),
        Some(FreeFunctionBuiltinContract {
            name: "max",
            builtin: Builtin::Max,
            args: BuiltinArgContract::All,
            requires_unshadowed: true,
        })
    );
    assert_eq!(
        free_function_builtin_contract(Lang::Swift, "abs", 1),
        Some(FreeFunctionBuiltinContract {
            name: "abs",
            builtin: Builtin::Abs,
            args: BuiltinArgContract::First,
            requires_unshadowed: true,
        })
    );
    assert_eq!(
        free_function_builtin_contract(Lang::Swift, "min", 2),
        Some(FreeFunctionBuiltinContract {
            name: "min",
            builtin: Builtin::Min,
            args: BuiltinArgContract::All,
            requires_unshadowed: true,
        })
    );
    assert_eq!(free_function_builtin_contract(Lang::Swift, "min", 1), None);
    assert!(free_function_builtin_contract(Lang::Swift, "max", 3).is_some());
    assert_eq!(free_function_builtin_contract(Lang::Python, "any", 2), None);
}
