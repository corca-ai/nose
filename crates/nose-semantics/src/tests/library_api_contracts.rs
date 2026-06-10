use super::*;

#[test]
fn language_predicates_preserve_existing_gates() {
    for &lang in ALL_LANGS {
        let profile = semantics(lang);
        assert_eq!(
            profile.operators().primitive_order_comparisons(),
            matches!(lang, Lang::C | Lang::Go | Lang::Java)
        );
        let byte_pack = profile
            .operators()
            .c_integer_byte_pack_contract(CBytePackWidth::U32);
        assert_eq!(byte_pack.is_some(), lang == Lang::C);
        if let Some(contract) = byte_pack {
            assert_eq!(contract.base_domain, DomainRequirement::ByteArray);
            assert_eq!(
                contract.required_high_lane_cast,
                Some(SourceFactKind::Cast(SourceCastKind::CUnsigned32))
            );
        }
        assert_eq!(
            profile.effects().non_overloadable_index_assignment(),
            matches!(lang, Lang::C | Lang::Go | Lang::Java)
        );
        assert_eq!(
            profile.effects().java_this_field_place(),
            lang == Lang::Java
        );
        assert_eq!(
            profile.modules().js_like_shadowed_module_bindings(),
            matches!(
                lang,
                Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html
            )
        );
        assert_eq!(
            profile.modules().java_class_literal_exports(),
            lang == Lang::Java
        );
        assert_eq!(
            profile.modules().java_type_declarations_shadow_stdlib(),
            lang == Lang::Java
        );
        assert_eq!(
            profile.modules().go_import_namespace_facts(),
            lang == Lang::Go
        );
    }
}

#[test]
fn stdlib_predicates_preserve_existing_gates() {
    for &lang in ALL_LANGS {
        let stdlib = semantics(lang).stdlib();
        assert_eq!(stdlib.python_collection_factories(), lang == Lang::Python);
        assert_eq!(stdlib.python_deque_factory(), lang == Lang::Python);
        assert_eq!(stdlib.java_collection_factories(), lang == Lang::Java);
        assert_eq!(stdlib.java_map_factories(), lang == Lang::Java);
        assert_eq!(stdlib.java_primitive_integer_ops(), lang == Lang::Java);
        assert_eq!(stdlib.ruby_set_factory(), lang == Lang::Ruby);
        assert_eq!(stdlib.rust_vec_macro_factory(), lang == Lang::Rust);
        assert_eq!(stdlib.rust_vec_new_factory(), lang == Lang::Rust);
        assert_eq!(stdlib.rust_std_collection_factories(), lang == Lang::Rust);
        assert_eq!(stdlib.rust_std_map_factories(), lang == Lang::Rust);
        assert_eq!(stdlib.go_literal_zero_map_lookup(), lang == Lang::Go);
        assert_eq!(stdlib.rust_filter_map_option_contract(), lang == Lang::Rust);
    }
}

#[test]
fn free_name_contracts_are_behavior_equivalent_tables() {
    let py_names: Vec<_> = semantics(Lang::Python)
        .collections()
        .free_name_collection_factories()
        .flat_map(|factory| factory.names.iter().copied())
        .collect();
    assert!(py_names.contains(&"list"));
    assert!(py_names.contains(&"frozenset"));
    assert!(!py_names.contains(&"Set"));

    let imported_py_names: Vec<_> = semantics(Lang::Python)
        .collections()
        .imported_collection_factories()
        .map(|factory| (factory.module, factory.exported))
        .collect();
    assert_eq!(imported_py_names, vec![("collections", "deque")]);

    let rust_map_tags: Vec<_> = semantics(Lang::Rust)
        .collections()
        .free_name_map_factories()
        .map(|factory| factory.entry_seq_tag)
        .collect();
    assert_eq!(rust_map_tags, vec![2]);

    let js_map_tags: Vec<_> = semantics(Lang::JavaScript)
        .collections()
        .free_name_map_factories()
        .map(|factory| factory.entry_seq_tag)
        .collect();
    assert!(js_map_tags.is_empty());
}

#[test]
fn library_api_contracts_carry_identity_and_result_obligations() {
    assert_eq!(
        library_free_name_collection_factory_contract(Lang::Python, "list"),
        Some(LibraryCollectionFactoryContract {
            id: LibraryApiContractId::PythonBuiltinCollectionFactory,
            callee: LibraryApiCalleeContract::FreeName {
                name: "list",
                shadow: LibraryApiShadowPolicy::SameName,
            },
            result: LibraryCollectionFactoryResult::SequenceArgument,
        })
    );
    assert_eq!(
        library_free_function_builtin_contract(Lang::Python, "len", 1),
        Some(LibraryFreeFunctionBuiltinContract {
            id: LibraryApiContractId::FreeFunctionBuiltin(Builtin::Len),
            callee: LibraryApiCalleeContract::FreeName {
                name: "len",
                shadow: LibraryApiShadowPolicy::SameName,
            },
            result: FreeFunctionBuiltinContract {
                name: "len",
                builtin: Builtin::Len,
                args: BuiltinArgContract::First,
                requires_unshadowed: true,
            },
        })
    );
    assert_eq!(
        library_imported_collection_factory_contract(Lang::Python, "collections", "deque"),
        Some(LibraryCollectionFactoryContract {
            id: LibraryApiContractId::PythonImportedCollectionFactory,
            callee: LibraryApiCalleeContract::ImportedBinding {
                module: "collections",
                exported: "deque",
            },
            result: LibraryCollectionFactoryResult::SequenceArgument,
        })
    );
    assert_eq!(
        library_free_name_map_factory_contract(Lang::Rust, "std::collections::HashMap::from"),
        Some(LibraryMapFactoryContract {
            id: LibraryApiContractId::RustStdMapFactory,
            callee: LibraryApiCalleeContract::FreeName {
                name: "std::collections::HashMap::from",
                shadow: LibraryApiShadowPolicy::RustStdRootForStdPath,
            },
            result: LibraryMapFactoryResult::EntrySequence {
                entry_seq_tag: SEQ_VALUE_TUPLE,
            },
        })
    );
    assert!(!library_api_free_name_shadow_safe(
        Lang::Rust,
        "std::collections::HashMap::from",
        LibraryApiShadowPolicy::RustStdRootForStdPath,
        |name| name == "std"
    ));
    assert!(library_api_free_name_shadow_safe(
        Lang::Rust,
        "std::collections::HashMap::from",
        LibraryApiShadowPolicy::RustStdRootForStdPath,
        |_| false
    ));
}

#[test]
fn library_api_factory_contracts_cover_java_ruby_and_js_like_surfaces() {
    assert_eq!(
        library_java_collection_factory_contract(Lang::Java, "Arrays", "asList"),
        Some(LibraryCollectionFactoryContract {
            id: LibraryApiContractId::JavaCollectionFactory(
                JavaCollectionFactoryKind::ArraysAsList,
            ),
            callee: LibraryApiCalleeContract::JavaUtilStaticMember {
                receiver: "Arrays",
                method: "asList",
            },
            result: LibraryCollectionFactoryResult::VariadicElements {
                single_arg_spreads_array: true,
            },
        })
    );
    assert_eq!(
        library_java_collection_constructor_contract(Lang::Java, "ArrayList", 0),
        Some(LibraryCollectionFactoryContract {
            id: LibraryApiContractId::JavaCollectionConstructor(
                JavaCollectionConstructorKind::EmptyList,
            ),
            callee: LibraryApiCalleeContract::JavaUtilConstructor {
                simple_type: "ArrayList",
                qualified_type: "java.util.ArrayList",
                module: "java.util",
                requires_import_for_simple_type: true,
                requires_no_local_type_shadow: true,
            },
            result: LibraryCollectionFactoryResult::EmptySequence,
        })
    );
    assert_eq!(
        library_ruby_set_factory_contract(Lang::Ruby, "Set", "new", 1),
        Some(LibraryCollectionFactoryContract {
            id: LibraryApiContractId::RubySetFactory,
            callee: LibraryApiCalleeContract::RubyRequireStaticMember {
                receiver: "Set",
                method: "new",
                required_module: "set",
                shadow_root: "Set",
            },
            result: LibraryCollectionFactoryResult::SequenceArgument,
        })
    );
    assert_eq!(
        library_js_like_map_constructor_contract(Lang::TypeScript, "Map"),
        Some(LibraryMapFactoryContract {
            id: LibraryApiContractId::JsLikeMapConstructor,
            callee: LibraryApiCalleeContract::JsGlobalConstructor {
                receiver: "Map",
                requires_unshadowed_global: true,
            },
            result: LibraryMapFactoryResult::EntrySequence {
                entry_seq_tag: SEQ_VALUE_COLLECTION,
            },
        })
    );
    assert_eq!(
        library_free_name_collection_factory_contract(Lang::JavaScript, "list"),
        None
    );
    assert_eq!(
        library_java_map_factory_contract(Lang::Java, "List", "of"),
        None
    );
}

#[test]
fn library_api_result_domain_mapping_is_contract_scoped() {
    assert_eq!(
        library_collection_factory_result_domain(
            library_free_name_collection_factory_contract(Lang::Python, "list").unwrap()
        ),
        DomainEvidence::Collection
    );
    assert_eq!(
        library_collection_factory_result_domain(
            library_free_name_collection_factory_contract(Lang::Python, "set").unwrap()
        ),
        DomainEvidence::Set
    );
    assert_eq!(
        library_collection_factory_result_domain(
            library_free_name_collection_factory_contract(Lang::Python, "frozenset").unwrap()
        ),
        DomainEvidence::Set
    );
    assert_eq!(
        library_collection_factory_result_domain(
            library_imported_collection_factory_contract(Lang::Python, "collections", "deque",)
                .unwrap()
        ),
        DomainEvidence::Collection
    );
    assert_eq!(
        library_collection_factory_result_domain(
            library_free_name_collection_factory_contract(
                Lang::Rust,
                "std::collections::HashSet::from",
            )
            .unwrap()
        ),
        DomainEvidence::Set
    );
    assert_eq!(
        library_collection_factory_result_domain(
            library_free_name_collection_factory_contract(
                Lang::Rust,
                "std::collections::VecDeque::from",
            )
            .unwrap()
        ),
        DomainEvidence::Collection
    );
    assert_eq!(
        library_collection_factory_result_domain(
            library_rust_vec_macro_factory_contract(Lang::Rust, "vec").unwrap()
        ),
        DomainEvidence::Collection
    );
    assert_eq!(
        library_collection_factory_result_domain(
            library_java_collection_factory_contract(Lang::Java, "List", "of").unwrap()
        ),
        DomainEvidence::Collection
    );
    let as_list = library_java_collection_factory_contract(Lang::Java, "Arrays", "asList").unwrap();
    assert_eq!(
        library_collection_factory_result_domain_for_arity(as_list, 0),
        Some(DomainEvidence::Collection)
    );
    assert_eq!(
        library_collection_factory_result_domain_for_arity(as_list, 1),
        None,
        "single-argument Arrays.asList has ambiguous element provenance"
    );
    assert_eq!(
        library_collection_factory_result_domain_for_arity(as_list, 2),
        Some(DomainEvidence::Collection)
    );
    assert_eq!(
        library_collection_factory_result_domain(
            library_java_collection_factory_contract(Lang::Java, "Set", "of").unwrap()
        ),
        DomainEvidence::Set
    );
    assert_eq!(
        library_collection_factory_result_domain(
            library_ruby_set_factory_contract(Lang::Ruby, "Set", "new", 1).unwrap()
        ),
        DomainEvidence::Set
    );
    assert_eq!(
        library_collection_factory_result_domain(
            library_js_like_set_constructor_contract(Lang::JavaScript, "Set").unwrap()
        ),
        DomainEvidence::Set
    );
}

#[test]
fn library_map_factory_result_domain_mapping_is_contract_scoped() {
    assert_eq!(
        library_map_factory_result_domain(
            library_free_name_map_factory_contract(Lang::Rust, "std::collections::HashMap::from",)
                .unwrap()
        ),
        DomainEvidence::Map
    );
    assert_eq!(
        library_map_factory_result_domain(
            library_java_map_factory_contract(Lang::Java, "Map", "of").unwrap()
        ),
        DomainEvidence::Map
    );
    assert_eq!(
        library_map_factory_result_domain(
            library_js_like_map_constructor_contract(Lang::JavaScript, "Map").unwrap()
        ),
        DomainEvidence::Map
    );
    assert_eq!(
        library_map_key_view_wrapper_result_domain(
            library_map_key_view_wrapper_contract(Lang::JavaScript, "Array", "from", 1).unwrap()
        ),
        DomainEvidence::Array
    );
}

#[test]
fn library_non_factory_api_contracts_carry_identity_and_result_obligations() {
    assert_eq!(
        library_map_key_view_contract(Lang::TypeScript, "keys", 0),
        Some(LibraryMapKeyViewContract {
            id: LibraryApiContractId::MapKeyView(MapKeyViewKind::Iterator),
            callee: LibraryApiCalleeContract::Method {
                method: "keys",
                receiver: MethodReceiverContract::ExactMap,
            },
            result: MapKeyViewContract {
                method: "keys",
                kind: MapKeyViewKind::Iterator,
            },
        })
    );
    assert_eq!(
        library_map_key_view_wrapper_contract(Lang::JavaScript, "Array", "from", 1),
        Some(LibraryMapKeyViewWrapperContract {
            id: LibraryApiContractId::MapKeyViewWrapper,
            callee: LibraryApiCalleeContract::StaticGlobalMethod {
                receiver: "Array",
                method: "from",
                qualified_path: "Array.from",
                requires_unshadowed_receiver: true,
            },
            result: MapKeyViewWrapperContract {
                receiver: "Array",
                method: "from",
                qualified_path: "Array.from",
            },
        })
    );
    assert_eq!(
        library_map_get_contract(Lang::Rust, "get", 1),
        Some(LibraryMapGetContract {
            id: LibraryApiContractId::MapGet,
            callee: LibraryApiCalleeContract::Method {
                method: "get",
                receiver: MethodReceiverContract::ExactMap,
            },
            result: MapGetContract {
                method: "get",
                receiver: MethodReceiverContract::ExactMap,
            },
        })
    );
    assert_eq!(
        library_js_array_is_array_contract(Lang::JavaScript, "Array", "isArray", 1),
        Some(LibraryStaticGlobalMethodContract {
            id: LibraryApiContractId::JsArrayIsArray,
            callee: LibraryApiCalleeContract::StaticGlobalMethod {
                receiver: "Array",
                method: "isArray",
                qualified_path: "Array.isArray",
                requires_unshadowed_receiver: true,
            },
            result: StaticGlobalMethodContract {
                receiver: "Array",
                method: "isArray",
                qualified_path: "Array.isArray",
                requires_unshadowed_receiver: true,
            },
        })
    );
}

#[test]
fn library_coercion_regex_namespace_and_promise_contracts_carry_obligations() {
    assert_eq!(
        library_js_boolean_coercion_contract(Lang::TypeScript, "Boolean", 1),
        Some(LibraryStaticGlobalFunctionContract {
            id: LibraryApiContractId::JsBooleanCoercion,
            callee: LibraryApiCalleeContract::StaticGlobalFunction {
                function: "Boolean",
                requires_unshadowed_function: true,
            },
            result: StaticGlobalFunctionContract {
                function: "Boolean",
                requires_unshadowed_function: true,
            },
        })
    );
    assert_eq!(
        library_regex_test_contract(Lang::JavaScript, "test", 1),
        Some(LibraryRegexTestContract {
            id: LibraryApiContractId::RegexTest,
            callee: LibraryApiCalleeContract::RegexLiteralMethod {
                method: "test",
                required_receiver_fact: SourceFactKind::Literal(SourceLiteralKind::Regex),
            },
            result: RegexTestContract {
                method: "test",
                required_receiver_fact: SourceFactKind::Literal(SourceLiteralKind::Regex),
            },
        })
    );
    assert_eq!(
        library_imported_namespace_function_contract(Lang::Python, "prod", 2),
        Some(LibraryImportedNamespaceFunctionContract {
            id: LibraryApiContractId::ImportedNamespaceFunction(
                ImportedNamespaceFunctionSemantic::ProductReduction {
                    op: Op::Mul,
                    identity: 1,
                },
            ),
            callee: LibraryApiCalleeContract::ImportedNamespaceFunction {
                module: "math",
                function: "prod",
            },
            result: ImportedNamespaceFunctionContract {
                module: "math",
                function: "prod",
                receiver: MethodReceiverContract::ImportedNamespace("math"),
                semantic: ImportedNamespaceFunctionSemantic::ProductReduction {
                    op: Op::Mul,
                    identity: 1,
                },
            },
        })
    );
    assert_eq!(
        library_promise_then_contract(Lang::Vue, "then", 1),
        Some(LibraryPromiseThenContract {
            id: LibraryApiContractId::PromiseThen,
            callee: LibraryApiCalleeContract::AsyncMethod {
                method: "then",
                receiver: AsyncReceiverContract::ExactPromiseLike,
            },
            result: PromiseThenContract {
                receiver: AsyncReceiverContract::ExactPromiseLike,
                demand: promise_then_demand_effect_profile(),
            },
        })
    );
    assert_eq!(
        library_promise_resolve_contract(Lang::TypeScript, "Promise", "resolve", 1),
        Some(LibraryPromiseFactoryContract {
            id: LibraryApiContractId::PromiseFactory(PromiseFactoryKind::Resolve),
            callee: LibraryApiCalleeContract::StaticGlobalMethod {
                receiver: "Promise",
                method: "resolve",
                qualified_path: "Promise.resolve",
                requires_unshadowed_receiver: true,
            },
            result: PromiseFactoryContract {
                receiver: "Promise",
                method: "resolve",
                qualified_path: "Promise.resolve",
                kind: PromiseFactoryKind::Resolve,
                result_domain: DomainEvidence::PromiseLike,
            },
        })
    );
}

#[test]
fn library_iterator_adapter_and_method_call_contracts_carry_obligations() {
    assert_eq!(
        library_iterator_identity_adapter_contract(Lang::Rust, "collect", 0),
        Some(LibraryIteratorIdentityAdapterContract {
            id: LibraryApiContractId::IteratorIdentityAdapter,
            callee: LibraryApiCalleeContract::IteratorAdapterMethod {
                method: "collect",
                receiver: IteratorAdapterReceiverContract::ExactIterableValue,
            },
            result: IteratorIdentityAdapterContract {
                receiver: IteratorAdapterReceiverContract::ExactIterableValue,
            },
        })
    );
    assert_eq!(
        library_static_collection_adapter_contract(Lang::Java, "Arrays", "stream", 1),
        Some(LibraryStaticCollectionAdapterContract {
            id: LibraryApiContractId::StaticCollectionAdapter,
            callee: LibraryApiCalleeContract::JavaUtilStaticMember {
                receiver: "Arrays",
                method: "stream",
            },
            result: StaticCollectionAdapterContract {
                module: "java.util",
                exported: "Arrays",
            },
        })
    );
    assert_eq!(
        library_method_call_contract(Lang::Go, "Contains", 2),
        Some(LibraryMethodCallContract {
            id: LibraryApiContractId::MethodCall(MethodSemanticContract::Builtin(
                Builtin::Contains,
            )),
            callee: LibraryApiCalleeContract::Method {
                method: "Contains",
                receiver: MethodReceiverContract::ImportedNamespace("slices"),
            },
            result: MethodCallContract {
                semantic: MethodSemanticContract::Builtin(Builtin::Contains),
                receiver: MethodReceiverContract::ImportedNamespace("slices"),
                args: MethodBuiltinArgs::GoSliceContains,
            },
        })
    );
}

#[test]
fn library_non_factory_api_contracts_reject_raw_name_only_matches() {
    assert_eq!(
        library_map_key_view_contract(Lang::JavaScript, "keySet", 0),
        None
    );
    assert_eq!(library_map_key_view_contract(Lang::Python, "keys", 1), None);
    assert_eq!(
        library_map_key_view_wrapper_contract(Lang::Python, "Array", "from", 1),
        None
    );
    assert_eq!(
        library_map_key_view_wrapper_contract(Lang::TypeScript, "Array", "from", 2),
        None
    );
    assert_eq!(library_map_get_contract(Lang::Python, "get", 1), None);
    assert_eq!(library_map_get_contract(Lang::Rust, "get", 2), None);
    assert_eq!(
        library_js_array_is_array_contract(Lang::Python, "Array", "isArray", 1),
        None
    );
    assert_eq!(
        library_js_array_is_array_contract(Lang::TypeScript, "Array", "isArray", 2),
        None
    );
    assert_eq!(
        library_js_boolean_coercion_contract(Lang::Python, "Boolean", 1),
        None
    );
    assert_eq!(
        library_js_boolean_coercion_contract(Lang::JavaScript, "Boolean", 2),
        None
    );
}

#[test]
fn library_promise_adapter_and_method_contracts_reject_raw_name_only_matches() {
    assert_eq!(library_regex_test_contract(Lang::Ruby, "test", 1), None);
    assert_eq!(
        library_imported_namespace_function_contract(Lang::JavaScript, "prod", 1),
        None
    );
    assert_eq!(
        library_imported_namespace_function_contract(Lang::Python, "prod", 3),
        None
    );
    assert_eq!(library_promise_then_contract(Lang::Python, "then", 1), None);
    assert_eq!(
        library_promise_then_contract(Lang::TypeScript, "then", 2),
        None
    );
    assert_eq!(
        library_iterator_identity_adapter_contract(Lang::JavaScript, "collect", 0),
        None
    );
    assert_eq!(
        library_iterator_identity_adapter_contract(Lang::Rust, "collect", 1),
        None
    );
    assert_eq!(
        library_static_collection_adapter_contract(Lang::JavaScript, "Arrays", "stream", 1),
        None
    );
    assert_eq!(
        library_static_collection_adapter_contract(Lang::Java, "Arrays", "stream", 0),
        None
    );
    assert_eq!(library_method_call_contract(Lang::Python, "min", 2), None);
    assert_eq!(
        library_method_call_contract(Lang::JavaScript, "min", 1),
        None
    );
    assert_eq!(
        library_method_call_contract(Lang::JavaScript, "Contains", 2),
        None
    );
}

#[test]
fn receiver_mutation_contracts_are_language_scoped_rows() {
    let js_push = module_binding_mutating_method_contract(Lang::JavaScript, "push", 1)
        .expect("js push receiver mutation contract");
    assert_eq!(js_push.pack_id, FIRST_PARTY_PACK_ID);
    assert_eq!(js_push.lang, Lang::JavaScript);
    assert_eq!(js_push.effect, EffectEvidenceKind::ReceiverMutation);
    assert_eq!(
        js_push.receiver,
        MethodEffectReceiverContract::PotentiallyMutableReceiver
    );
    assert!(module_binding_mutating_method_contract(Lang::TypeScript, "push", 1).is_some());
    assert!(module_binding_mutating_method_contract(Lang::JavaScript, "addAll", 1).is_none());
    assert!(module_binding_mutating_method_contract(Lang::Java, "addAll", 1).is_some());
    assert!(module_binding_mutating_method_contract(Lang::Python, "append", 1).is_some());
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
    assert!(library_hof_demand_effect_profile(Lang::Rust, HoFKind::Map)
        .unwrap()
        .callback_effects_delayed_until_pull());
    assert!(
        library_hof_demand_effect_profile(Lang::Java, HoFKind::FlatMap)
            .unwrap()
            .callback_effects_delayed_until_pull()
    );
    assert_eq!(
        library_hof_demand_effect_profile(Lang::Python, HoFKind::Map),
        None
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
    assert_eq!(free_function_builtin_contract(Lang::Python, "any", 2), None);
}

#[test]
fn method_protocol_contracts_are_language_constrained() {
    assert!(method_fold_name(Lang::Ruby, "inject"));
    assert!(!method_fold_name(Lang::Python, "inject"));
    assert!(!method_fold_name(Lang::Ruby, "map"));
    assert_eq!(
        method_bool_reduction_builtin(Lang::Java, "anyMatch"),
        Some(Builtin::Any)
    );
    assert_eq!(
        method_bool_reduction_builtin(Lang::JavaScript, "every"),
        Some(Builtin::All)
    );
    assert_eq!(method_bool_reduction_builtin(Lang::Python, "every"), None);
    assert_eq!(
        method_hof_contract(Lang::Ruby, "collect"),
        Some(HoFKind::Map)
    );
    assert_eq!(
        method_hof_contract(Lang::Rust, "flat_map"),
        Some(HoFKind::FlatMap)
    );
    assert_eq!(
        method_hof_contract(Lang::Ruby, "select"),
        Some(HoFKind::Filter)
    );
    assert_eq!(method_hof_contract(Lang::Python, "select"), None);
    assert_eq!(
        method_collection_reduction_builtin(Lang::Rust, "count"),
        Some(Builtin::Len)
    );
    assert_eq!(
        method_collection_reduction_builtin(Lang::Java, "count"),
        Some(Builtin::Len)
    );
    assert_eq!(
        method_collection_reduction_builtin(Lang::JavaScript, "count"),
        None
    );
    assert_eq!(
        property_builtin_contract(Lang::JavaScript, "length"),
        Some(Builtin::Len)
    );
    assert_eq!(property_builtin_contract(Lang::Python, "length"), None);
}

#[test]
fn method_call_contracts_carry_receiver_and_resolution_obligations() {
    assert_eq!(
        method_call_contract(Lang::Python, "append", 1),
        Some(MethodCallContract {
            semantic: MethodSemanticContract::Builtin(Builtin::Append),
            receiver: MethodReceiverContract::ExactCollection,
            args: MethodBuiltinArgs::ReceiverThenAll,
        })
    );
    assert_eq!(method_call_contract(Lang::Python, "append", 0), None);
    assert_eq!(
        method_call_contract(Lang::JavaScript, "log", 1),
        Some(MethodCallContract {
            semantic: MethodSemanticContract::Builtin(Builtin::Print),
            receiver: MethodReceiverContract::UnshadowedGlobal("console"),
            args: MethodBuiltinArgs::All,
        })
    );
    assert_eq!(
        method_call_contract(Lang::JavaScript, "min", 2),
        Some(MethodCallContract {
            semantic: MethodSemanticContract::Builtin(Builtin::Min),
            receiver: MethodReceiverContract::UnshadowedGlobal("Math"),
            args: MethodBuiltinArgs::All,
        })
    );
    assert_eq!(method_call_contract(Lang::JavaScript, "min", 1), None);
    assert_eq!(method_call_contract(Lang::Python, "min", 2), None);
    assert_eq!(
        method_call_contract(Lang::Go, "Abs", 1),
        Some(MethodCallContract {
            semantic: MethodSemanticContract::Builtin(Builtin::Abs),
            receiver: MethodReceiverContract::ImportedNamespace("math"),
            args: MethodBuiltinArgs::First,
        })
    );
    assert_eq!(
        method_call_contract(Lang::Go, "Contains", 2),
        Some(MethodCallContract {
            semantic: MethodSemanticContract::Builtin(Builtin::Contains),
            receiver: MethodReceiverContract::ImportedNamespace("slices"),
            args: MethodBuiltinArgs::GoSliceContains,
        })
    );
    assert_eq!(
        method_call_contract(Lang::Java, "abs", 1),
        Some(MethodCallContract {
            semantic: MethodSemanticContract::Builtin(Builtin::Abs),
            receiver: MethodReceiverContract::UnshadowedGlobal("Math"),
            args: MethodBuiltinArgs::First,
        })
    );
    assert_eq!(
        method_call_contract(Lang::Java, "min", 2),
        Some(MethodCallContract {
            semantic: MethodSemanticContract::Builtin(Builtin::Min),
            receiver: MethodReceiverContract::UnshadowedGlobal("Math"),
            args: MethodBuiltinArgs::All,
        })
    );
}

#[test]
fn method_call_contracts_cover_membership_and_map_default_lookups() {
    assert_eq!(
        method_call_contract(Lang::Python, "__contains__", 1),
        Some(MethodCallContract {
            semantic: MethodSemanticContract::Builtin(Builtin::Contains),
            receiver: MethodReceiverContract::ExactCollectionOrMap,
            args: MethodBuiltinArgs::FirstThenReceiver,
        })
    );
    assert_eq!(
        method_call_contract(Lang::TypeScript, "has", 1),
        Some(MethodCallContract {
            semantic: MethodSemanticContract::Builtin(Builtin::Contains),
            receiver: MethodReceiverContract::ExactSetOrMap,
            args: MethodBuiltinArgs::FirstThenReceiver,
        })
    );
    assert_eq!(
        method_call_contract(Lang::Ruby, "member?", 1),
        Some(MethodCallContract {
            semantic: MethodSemanticContract::Builtin(Builtin::Contains),
            receiver: MethodReceiverContract::ExactCollectionOrJavaKeySet,
            args: MethodBuiltinArgs::FirstThenReceiver,
        })
    );
    assert_eq!(method_call_contract(Lang::JavaScript, "contains", 1), None);
    assert_eq!(
        method_call_contract(Lang::Java, "getOrDefault", 2),
        Some(MethodCallContract {
            semantic: MethodSemanticContract::Builtin(Builtin::GetOrDefault),
            receiver: MethodReceiverContract::ExactMap,
            args: MethodBuiltinArgs::MapGetDefault,
        })
    );
    assert_eq!(
        method_call_contract(Lang::Python, "get", 2),
        Some(MethodCallContract {
            semantic: MethodSemanticContract::Builtin(Builtin::GetOrDefault),
            receiver: MethodReceiverContract::ExactMap,
            args: MethodBuiltinArgs::MapGetDefault,
        })
    );
    assert_eq!(
        method_call_contract(Lang::Ruby, "fetch", 2),
        Some(MethodCallContract {
            semantic: MethodSemanticContract::Builtin(Builtin::GetOrDefault),
            receiver: MethodReceiverContract::ExactMap,
            args: MethodBuiltinArgs::MapGetDefaultOrZeroArgLambda,
        })
    );
    assert_eq!(method_call_contract(Lang::JavaScript, "abs", 0), None);
}

#[test]
fn scalar_integer_methods_are_language_and_signature_constrained() {
    assert_eq!(
        scalar_integer_method_contract(Lang::Rust, "clamp", 2),
        Some(ScalarIntegerMethodContract {
            semantic: ScalarIntegerMethod::Clamp,
            receiver: MethodReceiverContract::ExactInteger,
        })
    );
    assert_eq!(
        scalar_integer_method_contract(Lang::Rust, "min", 1),
        Some(ScalarIntegerMethodContract {
            semantic: ScalarIntegerMethod::Min,
            receiver: MethodReceiverContract::ExactInteger,
        })
    );
    assert_eq!(scalar_integer_method_contract(Lang::Rust, "clamp", 1), None);
    assert_eq!(
        scalar_integer_method_contract(Lang::TypeScript, "clamp", 2),
        None
    );
    assert_eq!(
        scalar_integer_method_contract(Lang::JavaScript, "abs", 0),
        None
    );
}

#[test]
fn promise_then_contract_requires_js_like_surface_and_receiver_proof() {
    assert_eq!(
        promise_then_contract(Lang::TypeScript, "then", 1),
        Some(PromiseThenContract {
            receiver: AsyncReceiverContract::ExactPromiseLike,
            demand: promise_then_demand_effect_profile(),
        })
    );
    assert_eq!(promise_then_contract(Lang::TypeScript, "then", 2), None);
    assert_eq!(promise_then_contract(Lang::Python, "then", 1), None);
}

#[test]
fn promise_resolve_contract_requires_js_like_static_global_surface() {
    assert_eq!(
        promise_resolve_contract(Lang::JavaScript, "Promise", "resolve", 1),
        Some(PromiseFactoryContract {
            receiver: "Promise",
            method: "resolve",
            qualified_path: "Promise.resolve",
            kind: PromiseFactoryKind::Resolve,
            result_domain: DomainEvidence::PromiseLike,
        })
    );
    assert_eq!(
        promise_resolve_contract(Lang::TypeScript, "Promise", "resolve", 2),
        None
    );
    assert_eq!(
        promise_resolve_contract(Lang::TypeScript, "Promise", "reject", 1),
        None
    );
    assert_eq!(
        promise_resolve_contract(Lang::Python, "Promise", "resolve", 1),
        None
    );
}

#[test]
fn iterator_identity_adapters_are_rust_and_receiver_proof_constrained() {
    assert_eq!(
        iterator_identity_adapter_contract(Lang::Rust, "iter", 0),
        Some(IteratorIdentityAdapterContract {
            receiver: IteratorAdapterReceiverContract::ExactIterableValue,
        })
    );
    assert_eq!(
        iterator_identity_adapter_contract(Lang::Rust, "collect", 0),
        Some(IteratorIdentityAdapterContract {
            receiver: IteratorAdapterReceiverContract::ExactIterableValue,
        })
    );
    assert_eq!(
        iterator_identity_adapter_contract(Lang::Java, "stream", 0),
        Some(IteratorIdentityAdapterContract {
            receiver: IteratorAdapterReceiverContract::ExactIterableValue,
        })
    );
    assert_eq!(
        iterator_identity_adapter_contract(Lang::JavaScript, "collect", 0),
        None
    );
    assert_eq!(
        iterator_identity_adapter_contract(Lang::Rust, "collect", 1),
        None
    );
}

#[test]
fn static_collection_adapters_are_import_binding_constrained() {
    assert_eq!(
        static_collection_adapter_contract(Lang::Java, "Arrays", "stream", 1),
        Some(StaticCollectionAdapterContract {
            module: "java.util",
            exported: "Arrays",
        })
    );
    assert_eq!(
        static_collection_adapter_contract(Lang::Java, "Arrays", "stream", 0),
        None
    );
    assert_eq!(
        static_collection_adapter_contract(Lang::JavaScript, "Arrays", "stream", 1),
        None
    );
}

#[test]
fn rust_std_path_contracts_carry_shadow_roots() {
    assert_eq!(
        rust_option_some_constructor_contract(Lang::Rust, "Option::Some"),
        Some(ShadowedPathContract {
            shadow_root: "Option",
        })
    );
    assert_eq!(
        rust_option_some_constructor_contract(Lang::Rust, "std::option::Option::Some"),
        Some(ShadowedPathContract { shadow_root: "std" })
    );
    assert_eq!(
        rust_option_some_constructor_contract(Lang::Python, "Some"),
        None
    );
    assert_eq!(
        rust_option_none_sentinel_contract(Lang::Rust, "None"),
        Some(ShadowedPathContract {
            shadow_root: "None",
        })
    );
    assert_eq!(
        rust_option_none_sentinel_contract(Lang::Rust, "core::option::Option::None"),
        Some(ShadowedPathContract {
            shadow_root: "core",
        })
    );
    assert_eq!(
        rust_option_none_sentinel_contract(Lang::JavaScript, "None"),
        None
    );
    assert_eq!(
        rust_vec_new_factory_contract(Lang::Rust, "alloc::vec::Vec::new"),
        Some(ShadowedPathContract {
            shadow_root: "alloc",
        })
    );
    assert_eq!(
        rust_vec_new_factory_contract(Lang::Rust, "Vec::with_capacity"),
        None
    );
    assert!(rust_option_and_then_contract(Lang::Rust, "and_then", 1));
    assert!(!rust_option_and_then_contract(Lang::Rust, "and_then", 0));
    assert!(!rust_option_and_then_contract(
        Lang::JavaScript,
        "and_then",
        1
    ));
}

#[test]
fn java_factory_contracts_are_language_receiver_and_selector_constrained() {
    assert_eq!(
        java_collection_factory_contract(Lang::Java, "List", "of"),
        Some(JavaCollectionFactoryContract {
            receiver: "List",
            method: "of",
            kind: JavaCollectionFactoryKind::ListOf,
            single_arg_spreads_array: false,
        })
    );
    assert_eq!(
        java_collection_factory_contract(Lang::Java, "Arrays", "asList"),
        Some(JavaCollectionFactoryContract {
            receiver: "Arrays",
            method: "asList",
            kind: JavaCollectionFactoryKind::ArraysAsList,
            single_arg_spreads_array: true,
        })
    );
    assert_eq!(
        java_collection_factory_contract(Lang::JavaScript, "List", "of"),
        None
    );
    assert_eq!(
        java_collection_factory_contract(Lang::Java, "Map", "of"),
        None
    );
    assert_eq!(
        java_collection_constructor_contract(Lang::Java, "ArrayList", 0),
        Some(JavaCollectionConstructorContract {
            simple_type: "ArrayList",
            qualified_type: "java.util.ArrayList",
            module: "java.util",
            kind: JavaCollectionConstructorKind::EmptyList,
            requires_import_for_simple_type: true,
            requires_no_local_type_shadow: true,
        })
    );
    assert_eq!(
        java_collection_constructor_contract(Lang::Java, "java.util.LinkedList", 0)
            .map(|contract| contract.kind),
        Some(JavaCollectionConstructorKind::EmptyList)
    );
    assert_eq!(
        java_collection_constructor_contract(Lang::Java, "ArrayList", 1),
        None
    );
    assert_eq!(
        java_collection_constructor_contract(Lang::JavaScript, "ArrayList", 0),
        None
    );
    assert_eq!(
        library_java_collection_constructor_contract(Lang::Java, "ArrayList", 1),
        None
    );
    assert_eq!(
        library_java_collection_constructor_contract(Lang::JavaScript, "ArrayList", 0),
        None
    );
    assert_eq!(
        java_map_factory_contract(Lang::Java, "Map", "ofEntries"),
        Some(JavaMapFactoryContract {
            receiver: "Map",
            method: "ofEntries",
            kind: JavaMapFactoryKind::OfEntries,
        })
    );
    assert_eq!(java_map_factory_contract(Lang::Java, "List", "of"), None);
    assert!(java_map_entry_contract(Lang::Java, "Map", "entry"));
    assert!(!java_map_entry_contract(Lang::Java, "Entry", "entry"));
    assert_eq!(
        java_collection_factory_contract_by_hash(Lang::Java, "Set", stable_symbol_hash("of"))
            .map(|contract| contract.kind),
        Some(JavaCollectionFactoryKind::SetOf)
    );
    assert_eq!(
        java_map_factory_contract_by_hash(Lang::Java, "Map", stable_symbol_hash("of"))
            .map(|contract| contract.kind),
        Some(JavaMapFactoryKind::Of)
    );
    assert!(java_map_entry_contract_by_hash(
        Lang::Java,
        "Map",
        stable_symbol_hash("entry")
    ));
}

#[test]
fn ruby_and_closed_js_like_factory_contracts_keep_proof_obligations_explicit() {
    assert_eq!(
        ruby_set_factory_contract(Lang::Ruby, "Set", "new", 1),
        Some(RubySetFactoryContract {
            receiver: "Set",
            method: "new",
            required_module: "set",
            shadow_root: "Set",
        })
    );
    assert_eq!(ruby_set_factory_contract(Lang::Ruby, "Set", "new", 2), None);
    assert_eq!(
        ruby_set_factory_contract(Lang::Python, "Set", "new", 1),
        None
    );
    assert!(
        ruby_set_factory_contract_by_hash(Lang::Ruby, "Set", stable_symbol_hash("new"), 1)
            .is_some()
    );

    assert_eq!(
        js_like_set_constructor_contract(Lang::TypeScript, "Set"),
        Some(ClosedConstructorContract {
            receiver: "Set",
            required_proof: ConstructorProofRequirement::ConstructSyntax,
            requires_unshadowed_global: true,
            entry_seq_tag: None,
        })
    );
    assert_eq!(
        js_like_map_constructor_contract(Lang::JavaScript, "Map"),
        Some(ClosedConstructorContract {
            receiver: "Map",
            required_proof: ConstructorProofRequirement::ConstructSyntax,
            requires_unshadowed_global: true,
            entry_seq_tag: Some(SEQ_VALUE_COLLECTION),
        })
    );
    assert_eq!(js_like_map_constructor_contract(Lang::Java, "Map"), None);
    assert_eq!(
        js_like_set_constructor_contract(Lang::JavaScript, "WeakSet"),
        None
    );
}

#[test]
fn map_key_view_contracts_distinguish_collection_and_iterator_views() {
    assert_eq!(
        map_key_view_contract(Lang::Python, "keys", 0),
        Some(MapKeyViewContract {
            method: "keys",
            kind: MapKeyViewKind::Collection,
        })
    );
    assert_eq!(
        map_key_view_contract(Lang::Java, "keySet", 0),
        Some(MapKeyViewContract {
            method: "keySet",
            kind: MapKeyViewKind::Collection,
        })
    );
    assert_eq!(
        map_key_view_contract(Lang::TypeScript, "keys", 0),
        Some(MapKeyViewContract {
            method: "keys",
            kind: MapKeyViewKind::Iterator,
        })
    );
    assert_eq!(map_key_view_contract(Lang::JavaScript, "keySet", 0), None);
    assert_eq!(map_key_view_contract(Lang::Python, "keys", 1), None);
    assert_eq!(
        map_key_view_wrapper_contract(Lang::JavaScript, "Array", "from", 1),
        Some(MapKeyViewWrapperContract {
            receiver: "Array",
            method: "from",
            qualified_path: "Array.from",
        })
    );
    assert_eq!(
        map_key_view_wrapper_contract(Lang::Python, "Array", "from", 1),
        None
    );
    assert_eq!(
        map_key_view_contract_by_hash(Lang::Java, stable_symbol_hash("keySet"), 0)
            .map(|contract| contract.kind),
        Some(MapKeyViewKind::Collection)
    );
    assert!(map_key_view_wrapper_contract_by_hash(
        Lang::TypeScript,
        "Array",
        stable_symbol_hash("from"),
        1,
    )
    .is_some());
}

#[test]
fn go_zero_map_contracts_are_go_surface_and_default_constrained() {
    assert_eq!(
        go_zero_map_lookup_contract(Lang::Go),
        Some(GoZeroMapLookupContract {
            map_literal_tag: "composite_literal",
            entry_tag: "keyed_element",
            canonical_value_tag: "go_literal_zero_map",
        })
    );
    assert_eq!(go_zero_map_lookup_contract(Lang::Python), None);
    assert_eq!(
        go_zero_map_default_kind(Lang::Go, Payload::LitInt(1)),
        Some(GoZeroMapDefaultKind::Int)
    );
    assert_eq!(
        go_zero_map_default_kind(Lang::Go, Payload::LitStr(stable_symbol_hash("x"))),
        Some(GoZeroMapDefaultKind::String)
    );
    assert_eq!(
        go_zero_map_default_kind(Lang::Go, Payload::Lit(LitClass::Null)),
        Some(GoZeroMapDefaultKind::Null)
    );
    assert_eq!(
        go_zero_map_default_kind(Lang::JavaScript, Payload::LitInt(1)),
        None
    );
    assert_eq!(go_zero_map_default_kind(Lang::Go, Payload::None), None);
}

#[test]
fn map_get_contracts_are_language_and_arity_constrained() {
    assert_eq!(
        map_get_contract(Lang::Rust, "get", 1),
        Some(MapGetContract {
            method: "get",
            receiver: MethodReceiverContract::ExactMap,
        })
    );
    assert_eq!(
        map_get_contract_by_hash(Lang::Java, stable_symbol_hash("get"), 1),
        Some(MapGetContract {
            method: "get",
            receiver: MethodReceiverContract::ExactMap,
        })
    );
    assert_eq!(
        map_get_contract(Lang::TypeScript, "get", 1),
        Some(MapGetContract {
            method: "get",
            receiver: MethodReceiverContract::ExactMap,
        })
    );
    assert_eq!(map_get_contract(Lang::Python, "get", 1), None);
    assert_eq!(map_get_contract(Lang::Rust, "get", 2), None);
    assert_eq!(map_get_contract(Lang::Java, "getOrDefault", 1), None);
}

#[test]
fn js_static_builtin_contracts_are_language_and_arity_constrained() {
    assert_eq!(
        static_global_symbol_contract(Lang::JavaScript, "Math"),
        Some(StaticGlobalSymbolContract {
            name: "Math",
            requires_unshadowed: true,
        })
    );
    assert_eq!(
        static_global_symbol_contract(Lang::TypeScript, "undefined"),
        Some(StaticGlobalSymbolContract {
            name: "undefined",
            requires_unshadowed: true,
        })
    );
    assert_eq!(
        static_global_symbol_contract(Lang::TypeScript, "Promise"),
        Some(StaticGlobalSymbolContract {
            name: "Promise",
            requires_unshadowed: true,
        })
    );
    assert_eq!(
        qualified_global_symbol_contract(Lang::JavaScript, "Promise.resolve"),
        Some(QualifiedGlobalSymbolContract {
            path: "Promise.resolve",
            root: "Promise",
            requires_unshadowed_root: true,
        })
    );
    assert_eq!(static_global_symbol_contract(Lang::Python, "Math"), None);
    assert_eq!(
        static_global_symbol_contract(Lang::JavaScript, "WeakMap"),
        None
    );
    assert_eq!(
        typeof_operator_contract(Lang::TypeScript, "typeof", 1),
        Some(TypeofOperatorContract {
            name: "typeof",
            required_source_fact: SourceFactKind::Operator(SourceOperatorKind::Typeof),
        })
    );
    assert_eq!(typeof_operator_contract(Lang::Python, "typeof", 1), None);
    assert_eq!(
        typeof_operator_contract(Lang::JavaScript, "typeof", 2),
        None
    );
    assert_eq!(
        js_array_is_array_contract(Lang::JavaScript, "Array", "isArray", 1),
        Some(StaticGlobalMethodContract {
            receiver: "Array",
            method: "isArray",
            qualified_path: "Array.isArray",
            requires_unshadowed_receiver: true,
        })
    );
    assert_eq!(
        js_array_is_array_contract(Lang::Python, "Array", "isArray", 1),
        None
    );
    assert_eq!(
        js_array_is_array_contract(Lang::TypeScript, "Array", "isArray", 2),
        None
    );
    assert_eq!(
        js_boolean_coercion_contract(Lang::JavaScript, "Boolean", 1),
        Some(StaticGlobalFunctionContract {
            function: "Boolean",
            requires_unshadowed_function: true,
        })
    );
    assert_eq!(
        js_boolean_coercion_contract(Lang::TypeScript, "Boolean", 1),
        Some(StaticGlobalFunctionContract {
            function: "Boolean",
            requires_unshadowed_function: true,
        })
    );
    assert_eq!(
        js_boolean_coercion_contract(Lang::Python, "Boolean", 1),
        None
    );
    assert_eq!(
        js_boolean_coercion_contract(Lang::JavaScript, "Boolean", 2),
        None
    );
    assert_eq!(
        regex_test_contract(Lang::JavaScript, "test", 1),
        Some(RegexTestContract {
            method: "test",
            required_receiver_fact: SourceFactKind::Literal(SourceLiteralKind::Regex),
        })
    );
    assert_eq!(regex_test_contract(Lang::Ruby, "test", 1), None);
}

#[test]
fn operator_law_contracts_preserve_comparison_gates() {
    for &lang in ALL_LANGS {
        let profile = semantics(lang);
        assert_eq!(
            profile
                .operators()
                .comparison_law(ComparisonLaw::LatticeStrictAbsorbsNonstrict)
                .is_some(),
            matches!(lang, Lang::C | Lang::Go | Lang::Java)
        );
        assert_eq!(
            profile
                .operators()
                .comparison_law(ComparisonLaw::LatticeLeNeToLt),
            Some(OperatorLawContract {
                law: ComparisonLaw::LatticeLeNeToLt,
                channel: ChannelEligibility::ExactProven,
                evidence: OperatorEvidence::ModeledIlOperator,
            })
        );
    }
}

#[test]
fn comparison_transform_contracts_carry_outputs_and_operand_swaps() {
    let ops = semantics(Lang::Python).operators();
    assert_eq!(
        ops.comparison_direction(Op::Gt),
        Some(ComparisonTransformContract {
            law: ComparisonLaw::DirectionCanon,
            input: Op::Gt,
            output: Op::Lt,
            swap_operands: true,
            channel: ChannelEligibility::ExactProven,
            evidence: OperatorEvidence::ModeledIlOperator,
        })
    );
    assert_eq!(
        ops.comparison_complement(Op::Lt)
            .map(|contract| (contract.output, contract.swap_operands)),
        Some((Op::Ge, false))
    );
    assert_eq!(
        ops.canonical_negated_comparison(Op::Lt)
            .map(|contract| (contract.output, contract.swap_operands)),
        Some((Op::Le, true))
    );
    assert_eq!(ops.comparison_direction(Op::Eq), None);
}

#[test]
fn cardinality_threshold_contracts_name_existing_operator_shapes() {
    let ops = semantics(Lang::JavaScript).operators();
    assert_eq!(
        ops.zero_cardinality_equality(Op::Eq),
        Some(CardinalityThresholdContract {
            threshold: CardinalityThreshold::Zero,
            predicate: CardinalityPredicate::Empty,
            channel: ChannelEligibility::ExactProven,
            evidence: OperatorEvidence::StaticCardinalityThreshold,
        })
    );
    assert_eq!(ops.zero_cardinality_equality(Op::Gt), None);
    assert_eq!(
        ops.cardinality_threshold(
            Op::Gt,
            false,
            CardinalityThreshold::Zero,
            CardinalityPredicate::NonEmpty,
        )
        .map(|contract| contract.predicate),
        Some(CardinalityPredicate::NonEmpty)
    );
    assert_eq!(
        ops.cardinality_threshold(
            Op::Eq,
            false,
            CardinalityThreshold::One,
            CardinalityPredicate::NonEmpty,
        ),
        None
    );
}

#[test]
fn membership_operator_contract_is_language_scoped() {
    assert_eq!(
        semantics(Lang::Python)
            .operators()
            .membership_operator(Op::In),
        Some(MembershipOperatorContract {
            operator: Op::In,
            receiver: MembershipOperatorReceiverContract::ExactCollectionOrMap,
            channel: ChannelEligibility::ExactProven,
            evidence: OperatorEvidence::ModeledIlOperator,
        })
    );
    assert_eq!(
        semantics(Lang::JavaScript)
            .operators()
            .membership_operator(Op::In),
        None
    );
    assert_eq!(
        semantics(Lang::Python)
            .operators()
            .membership_operator(Op::Eq),
        None
    );
}

#[test]
fn static_index_membership_contracts_are_js_like_and_threshold_constrained() {
    assert_eq!(
        static_index_membership_contract(Lang::JavaScript, "indexOf", 1),
        Some(StaticIndexMembershipContract {
            method: "indexOf",
            kind: StaticIndexMembershipKind::IndexOf,
            receiver: StaticIndexMembershipReceiverContract::StaticNonFloatLiteralCollection,
        })
    );
    assert_eq!(
        static_index_membership_contract(Lang::TypeScript, "findIndex", 1),
        Some(StaticIndexMembershipContract {
            method: "findIndex",
            kind: StaticIndexMembershipKind::FindIndex,
            receiver: StaticIndexMembershipReceiverContract::StaticNonFloatLiteralCollection,
        })
    );
    assert_eq!(
        static_index_membership_contract(Lang::Python, "indexOf", 1),
        None
    );
    assert_eq!(
        static_index_membership_contract(Lang::JavaScript, "indexOf", 2),
        None
    );
    assert_eq!(
        static_index_membership_contract(Lang::JavaScript, "includes", 1),
        None
    );
    assert_eq!(
        semantics(Lang::JavaScript)
            .operators()
            .static_index_membership_threshold(Op::Ne, false, IndexMembershipThreshold::MinusOne)
            .map(|contract| contract.evidence),
        Some(OperatorEvidence::JsLikeStaticIndexMembershipThreshold)
    );
    assert!(semantics(Lang::TypeScript)
        .operators()
        .static_index_membership_threshold(Op::Le, true, IndexMembershipThreshold::Zero)
        .is_some());
    assert!(semantics(Lang::Python)
        .operators()
        .static_index_membership_threshold(Op::Ne, false, IndexMembershipThreshold::MinusOne)
        .is_none());
    assert!(semantics(Lang::JavaScript)
        .operators()
        .static_index_membership_threshold(Op::Eq, false, IndexMembershipThreshold::MinusOne)
        .is_none());
}

#[test]
fn imported_namespace_function_contracts_carry_module_and_receiver_proof() {
    assert_eq!(
        imported_namespace_function_contract(Lang::Python, "prod", 1),
        Some(ImportedNamespaceFunctionContract {
            module: "math",
            function: "prod",
            receiver: MethodReceiverContract::ImportedNamespace("math"),
            semantic: ImportedNamespaceFunctionSemantic::ProductReduction {
                op: Op::Mul,
                identity: 1,
            },
        })
    );
    assert_eq!(
        imported_namespace_function_contract(Lang::Python, "prod", 2)
            .map(|contract| contract.semantic),
        Some(ImportedNamespaceFunctionSemantic::ProductReduction {
            op: Op::Mul,
            identity: 1,
        })
    );
    assert_eq!(
        imported_namespace_function_contract(Lang::JavaScript, "prod", 1),
        None
    );
    assert_eq!(
        imported_namespace_function_contract(Lang::Python, "prod", 3),
        None
    );
    assert_eq!(
        imported_namespace_function_contract(Lang::Python, "sum", 1),
        None
    );
}

#[test]
fn nullish_global_contracts_are_js_like_and_unshadowed() {
    assert_eq!(
        nullish_global_contract(Lang::JavaScript, "undefined"),
        Some(NullishGlobalContract {
            name: "undefined",
            requires_unshadowed: true,
        })
    );
    assert_eq!(
        nullish_global_contract(Lang::TypeScript, "undefined"),
        Some(NullishGlobalContract {
            name: "undefined",
            requires_unshadowed: true,
        })
    );
    assert_eq!(nullish_global_contract(Lang::Python, "undefined"), None);
    assert_eq!(nullish_global_contract(Lang::JavaScript, "null"), None);
}

#[test]
fn builder_append_contracts_are_language_and_arity_constrained() {
    let rust_push = builder_append_method_contract(Lang::Rust, "push", 1)
        .expect("rust push builder append contract");
    assert_eq!(rust_push.effect, EffectEvidenceKind::BuilderAppendCall);
    assert_eq!(
        rust_push.receiver,
        MethodEffectReceiverContract::ActiveCollectionBuilder
    );
    assert!(builder_append_method_contract(Lang::Rust, "push", 2).is_none());
    assert!(builder_append_method_contract(Lang::Java, "add", 1).is_some());
    assert!(builder_append_method_contract(Lang::JavaScript, "push", 1).is_some());
    assert!(builder_append_method_contract(Lang::TypeScript, "push", 1).is_some());
    assert!(builder_append_method_contract(Lang::Python, "append", 1).is_some());
    assert!(builder_append_method_contract(Lang::Ruby, "push", 1).is_none());
}

#[test]
fn unproven_membership_like_guard_is_negative_api_policy() {
    let guard = unproven_membership_like_method_contract(Lang::TypeScript, "includes", 1)
        .expect("includes guard");
    assert_eq!(guard.pack_id, FIRST_PARTY_PACK_ID);
    assert_eq!(guard.id, ApiGuardContractId::UnprovenMembershipLikeCall);
    assert_eq!(guard.lang, Lang::TypeScript);
    assert_eq!(guard.method, "includes");
    assert_eq!(guard.arg_count, 1);
    assert_eq!(guard.channel, ChannelEligibility::ExactProven);
    assert!(unproven_membership_like_method_contract(Lang::Java, "containsKey", 1).is_some());
    assert!(unproven_membership_like_method_contract(Lang::Python, "custom", 1).is_none());
}

#[test]
fn map_builder_index_write_contracts_are_language_scoped() {
    let contract =
        map_builder_index_write_contract(Lang::Python).expect("python map builder contract");
    assert_eq!(contract.pack_id, FIRST_PARTY_PACK_ID);
    assert_eq!(contract.id, IndexWriteContractId::MapBuilderEntryWrite);
    assert_eq!(
        contract.receiver,
        IndexWriteReceiverContract::ActiveMapBuilder
    );
    assert_eq!(contract.required_effect, EffectEvidenceKind::BindingWrite);
    assert_eq!(contract.channel, ChannelEligibility::ExactProven);
    assert!(map_builder_index_write_contract(Lang::Ruby).is_none());
    assert!(map_builder_index_write_contract(Lang::JavaScript).is_none());
}
