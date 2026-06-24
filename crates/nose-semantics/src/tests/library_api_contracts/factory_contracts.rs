use super::*;

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
            module: "java.util",
            receiver: "List",
            method: "of",
            kind: JavaCollectionFactoryKind::ListOf,
            single_arg_spreads_array: false,
        })
    );
    assert_eq!(
        java_collection_factory_contract(Lang::Java, "Arrays", "asList"),
        Some(JavaCollectionFactoryContract {
            module: "java.util",
            receiver: "Arrays",
            method: "asList",
            kind: JavaCollectionFactoryKind::ArraysAsList,
            single_arg_spreads_array: true,
        })
    );
    assert_eq!(
        java_collection_factory_contract(Lang::Java, "ImmutableList", "of"),
        Some(JavaCollectionFactoryContract {
            module: "com.google.common.collect",
            receiver: "ImmutableList",
            method: "of",
            kind: JavaCollectionFactoryKind::GuavaImmutableListOf,
            single_arg_spreads_array: false,
        })
    );
    assert_eq!(
        java_collection_factory_contract(Lang::Java, "ImmutableList", "copyOf"),
        None
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
            module: "java.util",
            receiver: "Map",
            method: "ofEntries",
            kind: JavaMapFactoryKind::OfEntries,
        })
    );
    assert_eq!(
        java_map_factory_contract(Lang::Java, "ImmutableMap", "of"),
        Some(JavaMapFactoryContract {
            module: "com.google.common.collect",
            receiver: "ImmutableMap",
            method: "of",
            kind: JavaMapFactoryKind::GuavaImmutableMapOf,
        })
    );
    assert_eq!(
        java_map_factory_contract(Lang::Java, "ImmutableMap", "copyOf"),
        None
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
    assert_eq!(
        java_map_factory_contract_by_hash(Lang::Java, "ImmutableMap", stable_symbol_hash("of"))
            .map(|contract| contract.kind),
        Some(JavaMapFactoryKind::GuavaImmutableMapOf)
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
