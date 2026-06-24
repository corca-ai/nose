use super::*;

mod materialized_result_domains;

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
            pack_id: PYTHON_BUILTIN_COLLECTION_FACTORY_PACK_ID,
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
            pack_id: PYTHON_STDLIB_COLLECTION_FACTORY_PACK_ID,
            id: LibraryApiContractId::PythonImportedCollectionFactory,
            callee: LibraryApiCalleeContract::ImportedBinding {
                module: "collections",
                exported: "deque",
            },
            result: LibraryCollectionFactoryResult::SequenceArgument,
        })
    );
    assert_eq!(
        library_free_name_collection_factory_contract(
            Lang::Rust,
            "std::collections::HashSet::from",
        ),
        Some(LibraryCollectionFactoryContract {
            pack_id: RUST_STDLIB_COLLECTION_FACTORY_PACK_ID,
            id: LibraryApiContractId::RustStdCollectionFactory,
            callee: LibraryApiCalleeContract::FreeName {
                name: "std::collections::HashSet::from",
                shadow: LibraryApiShadowPolicy::RustStdRootForStdPath,
            },
            result: LibraryCollectionFactoryResult::SequenceArgument,
        })
    );
    assert_eq!(
        library_free_name_map_factory_contract(Lang::Rust, "std::collections::HashMap::from"),
        Some(LibraryMapFactoryContract {
            pack_id: RUST_STDLIB_MAP_FACTORY_PACK_ID,
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
    assert_eq!(
        library_rust_vec_macro_factory_contract(Lang::Rust, "vec"),
        Some(LibraryCollectionFactoryContract {
            pack_id: RUST_STDLIB_VEC_PACK_ID,
            id: LibraryApiContractId::RustVecMacroFactory,
            callee: LibraryApiCalleeContract::RustMacro {
                name: "vec",
                shadow: LibraryApiShadowPolicy::SameName,
            },
            result: LibraryCollectionFactoryResult::VariadicElements {
                single_arg_spreads_array: false,
            },
        })
    );
    assert_eq!(
        library_rust_vec_new_factory_contract(Lang::Rust, "Vec::new"),
        Some(LibraryCollectionFactoryContract {
            pack_id: RUST_STDLIB_VEC_PACK_ID,
            id: LibraryApiContractId::RustVecNewFactory,
            callee: LibraryApiCalleeContract::FreeName {
                name: "Vec::new",
                shadow: LibraryApiShadowPolicy::ExplicitRoot("Vec"),
            },
            result: LibraryCollectionFactoryResult::EmptySequence,
        })
    );
}

#[test]
fn library_api_factory_contracts_cover_java_ruby_and_js_like_surfaces() {
    assert_eq!(
        library_java_collection_factory_contract(Lang::Java, "List", "of"),
        Some(LibraryCollectionFactoryContract {
            pack_id: JAVA_STDLIB_COLLECTION_FACTORY_PACK_ID,
            id: LibraryApiContractId::JavaCollectionFactory(JavaCollectionFactoryKind::ListOf),
            callee: LibraryApiCalleeContract::JavaUtilStaticMember {
                receiver: "List",
                method: "of",
            },
            result: LibraryCollectionFactoryResult::VariadicElements {
                single_arg_spreads_array: false,
            },
        })
    );
    assert_eq!(
        library_java_collection_factory_contract(Lang::Java, "Set", "of"),
        Some(LibraryCollectionFactoryContract {
            pack_id: JAVA_STDLIB_COLLECTION_FACTORY_PACK_ID,
            id: LibraryApiContractId::JavaCollectionFactory(JavaCollectionFactoryKind::SetOf),
            callee: LibraryApiCalleeContract::JavaUtilStaticMember {
                receiver: "Set",
                method: "of",
            },
            result: LibraryCollectionFactoryResult::VariadicElements {
                single_arg_spreads_array: false,
            },
        })
    );
    assert_eq!(
        library_java_collection_factory_contract(Lang::Java, "Arrays", "asList"),
        Some(LibraryCollectionFactoryContract {
            pack_id: JAVA_STDLIB_COLLECTION_FACTORY_PACK_ID,
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
        library_java_collection_factory_contract(Lang::Java, "Collections", "emptySet"),
        Some(LibraryCollectionFactoryContract {
            pack_id: JAVA_STDLIB_COLLECTION_FACTORY_PACK_ID,
            id: LibraryApiContractId::JavaCollectionFactory(
                JavaCollectionFactoryKind::CollectionsEmptySet,
            ),
            callee: LibraryApiCalleeContract::JavaUtilStaticMember {
                receiver: "Collections",
                method: "emptySet",
            },
            result: LibraryCollectionFactoryResult::EmptySequence,
        })
    );
    assert_eq!(
        library_java_collection_factory_contract(Lang::Java, "Collections", "singletonList"),
        Some(LibraryCollectionFactoryContract {
            pack_id: JAVA_STDLIB_COLLECTION_FACTORY_PACK_ID,
            id: LibraryApiContractId::JavaCollectionFactory(
                JavaCollectionFactoryKind::CollectionsSingletonList,
            ),
            callee: LibraryApiCalleeContract::JavaUtilStaticMember {
                receiver: "Collections",
                method: "singletonList",
            },
            result: LibraryCollectionFactoryResult::ElementArguments,
        })
    );
    assert_eq!(
        library_java_collection_factory_contract(Lang::Java, "ImmutableList", "of"),
        Some(LibraryCollectionFactoryContract {
            pack_id: JAVA_GUAVA_IMMUTABLE_COLLECTION_FACTORY_PACK_ID,
            id: LibraryApiContractId::JavaCollectionFactory(
                JavaCollectionFactoryKind::GuavaImmutableListOf,
            ),
            callee: LibraryApiCalleeContract::JavaStaticMember {
                module: "com.google.common.collect",
                receiver: "ImmutableList",
                method: "of",
                requires_import_for_simple_receiver: true,
                requires_no_local_type_shadow: true,
            },
            result: LibraryCollectionFactoryResult::VariadicElements {
                single_arg_spreads_array: false,
            },
        })
    );
    assert_eq!(
        library_java_collection_factory_contract(Lang::Java, "ImmutableSet", "of")
            .map(|contract| (contract.pack_id, contract.id)),
        Some((
            JAVA_GUAVA_IMMUTABLE_COLLECTION_FACTORY_PACK_ID,
            LibraryApiContractId::JavaCollectionFactory(
                JavaCollectionFactoryKind::GuavaImmutableSetOf,
            )
        ))
    );
    assert_eq!(
        library_java_collection_factory_contract(Lang::Java, "ImmutableList", "copyOf"),
        None
    );
    assert_eq!(
        library_java_collection_constructor_contract(Lang::Java, "ArrayList", 0),
        Some(LibraryCollectionFactoryContract {
            pack_id: JAVA_STDLIB_COLLECTION_CONSTRUCTOR_PACK_ID,
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
        library_java_collection_constructor_contract(Lang::Java, "java.util.LinkedList", 0)
            .map(|contract| contract.pack_id),
        Some(JAVA_STDLIB_COLLECTION_CONSTRUCTOR_PACK_ID)
    );
    assert_eq!(
        library_ruby_set_factory_contract(Lang::Ruby, "Set", "new", 1),
        Some(LibraryCollectionFactoryContract {
            pack_id: RUBY_STDLIB_SET_PACK_ID,
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
            pack_id: JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PACK_ID,
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
        library_js_like_set_constructor_contract(Lang::JavaScript, "Set")
            .map(|contract| contract.pack_id),
        Some(JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PACK_ID)
    );
    assert_eq!(
        library_java_map_factory_contract(Lang::Java, "Map", "of"),
        Some(LibraryMapFactoryContract {
            pack_id: JAVA_STDLIB_MAP_FACTORY_PACK_ID,
            id: LibraryApiContractId::JavaMapFactory(JavaMapFactoryKind::Of),
            callee: LibraryApiCalleeContract::JavaUtilStaticMember {
                receiver: "Map",
                method: "of",
            },
            result: LibraryMapFactoryResult::JavaFactory {
                kind: JavaMapFactoryKind::Of,
            },
        })
    );
    assert_eq!(
        library_java_map_factory_contract(Lang::Java, "Map", "ofEntries"),
        Some(LibraryMapFactoryContract {
            pack_id: JAVA_STDLIB_MAP_FACTORY_PACK_ID,
            id: LibraryApiContractId::JavaMapFactory(JavaMapFactoryKind::OfEntries),
            callee: LibraryApiCalleeContract::JavaUtilStaticMember {
                receiver: "Map",
                method: "ofEntries",
            },
            result: LibraryMapFactoryResult::JavaFactory {
                kind: JavaMapFactoryKind::OfEntries,
            },
        })
    );
    assert_eq!(
        library_java_map_factory_contract(Lang::Java, "Collections", "singletonMap"),
        Some(LibraryMapFactoryContract {
            pack_id: JAVA_STDLIB_MAP_FACTORY_PACK_ID,
            id: LibraryApiContractId::JavaMapFactory(JavaMapFactoryKind::CollectionsSingletonMap),
            callee: LibraryApiCalleeContract::JavaUtilStaticMember {
                receiver: "Collections",
                method: "singletonMap",
            },
            result: LibraryMapFactoryResult::JavaFactory {
                kind: JavaMapFactoryKind::CollectionsSingletonMap,
            },
        })
    );
    assert_eq!(
        library_java_map_factory_contract(Lang::Java, "ImmutableMap", "of"),
        Some(LibraryMapFactoryContract {
            pack_id: JAVA_GUAVA_IMMUTABLE_COLLECTION_FACTORY_PACK_ID,
            id: LibraryApiContractId::JavaMapFactory(JavaMapFactoryKind::GuavaImmutableMapOf),
            callee: LibraryApiCalleeContract::JavaStaticMember {
                module: "com.google.common.collect",
                receiver: "ImmutableMap",
                method: "of",
                requires_import_for_simple_receiver: true,
                requires_no_local_type_shadow: true,
            },
            result: LibraryMapFactoryResult::JavaFactory {
                kind: JavaMapFactoryKind::GuavaImmutableMapOf,
            },
        })
    );
    assert_eq!(
        library_java_map_factory_contract(Lang::Java, "ImmutableMap", "copyOf"),
        None
    );
    assert_eq!(
        library_java_map_entry_contract(Lang::Java, "Map", "entry"),
        Some(LibraryMapEntryFactoryContract {
            pack_id: JAVA_STDLIB_MAP_ENTRY_PACK_ID,
            id: LibraryApiContractId::JavaMapEntryFactory,
            callee: LibraryApiCalleeContract::JavaUtilStaticMember {
                receiver: "Map",
                method: "entry",
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
            library_java_collection_factory_contract(Lang::Java, "Collections", "emptySet")
                .unwrap()
        ),
        DomainEvidence::Set
    );
    assert_eq!(
        library_collection_factory_result_domain(
            library_java_collection_factory_contract(Lang::Java, "Collections", "singleton")
                .unwrap()
        ),
        DomainEvidence::Set
    );
    assert_eq!(
        library_collection_factory_result_domain(
            library_java_collection_factory_contract(Lang::Java, "Collections", "singletonList")
                .unwrap()
        ),
        DomainEvidence::Collection
    );
    assert_eq!(
        library_collection_factory_result_domain(
            library_java_collection_factory_contract(Lang::Java, "ImmutableList", "of").unwrap()
        ),
        DomainEvidence::Collection
    );
    assert_eq!(
        library_collection_factory_result_domain(
            library_java_collection_factory_contract(Lang::Java, "ImmutableSet", "of").unwrap()
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
            library_java_map_factory_contract(Lang::Java, "Collections", "emptyMap").unwrap()
        ),
        DomainEvidence::Map
    );
    assert_eq!(
        library_map_factory_result_domain(
            library_java_map_factory_contract(Lang::Java, "ImmutableMap", "of").unwrap()
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
