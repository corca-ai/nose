//! Stable textual keys for library API contract hashes.

use super::contracts::{LibraryApiCalleeContract, LibraryApiContractId};
use crate::{
    AsyncReceiverContract, ImportedNamespaceFunctionSemantic, IteratorAdapterReceiverContract,
    JavaCollectionConstructorKind, JavaCollectionFactoryKind, JavaMapFactoryKind, MapKeyViewKind,
    MethodReceiverContract, MethodSemanticContract, PromiseFactoryKind, ScalarIntegerMethod,
    StaticIndexMembershipKind, StaticIndexMembershipReceiverContract, SwiftCollectionFactoryKind,
    SwiftMapFactoryKind,
};

pub(super) fn library_api_contract_id_key(id: LibraryApiContractId) -> String {
    match id {
        LibraryApiContractId::PropertyBuiltin(builtin) => {
            format!("property_builtin.{}", builtin as u32)
        }
        LibraryApiContractId::PythonBuiltinCollectionFactory => {
            "python.builtin.collection_factory".into()
        }
        LibraryApiContractId::PythonImportedCollectionFactory => {
            "python.imported.collection_factory".into()
        }
        LibraryApiContractId::SwiftCollectionFactory(kind) => {
            format!(
                "swift.collection_factory.{}",
                swift_collection_factory_kind_key(kind)
            )
        }
        LibraryApiContractId::SwiftMapFactory(kind) => {
            format!("swift.map_factory.{}", swift_map_factory_kind_key(kind))
        }
        LibraryApiContractId::FreeFunctionBuiltin(builtin) => {
            format!("free_function_builtin.{}", builtin as u32)
        }
        LibraryApiContractId::FreeFunctionHof(kind) => {
            format!("free_function_hof.{}", kind as u32)
        }
        LibraryApiContractId::RustOptionSomeConstructor => "rust.option.some.constructor".into(),
        LibraryApiContractId::RustOptionNoneSentinel => "rust.option.none.sentinel".into(),
        LibraryApiContractId::RustOptionAndThen => "rust.option.and_then".into(),
        LibraryApiContractId::RustResultOkConstructor => "rust.result.ok.constructor".into(),
        LibraryApiContractId::RustResultErrConstructor => "rust.result.err.constructor".into(),
        LibraryApiContractId::RustResultIsOk => "rust.result.is_ok".into(),
        LibraryApiContractId::RustResultIsErr => "rust.result.is_err".into(),
        LibraryApiContractId::ScalarIntegerMethod(method) => {
            format!(
                "scalar_integer_method.{}",
                scalar_integer_method_key(method)
            )
        }
        LibraryApiContractId::RustStdCollectionFactory => "rust.std.collection_factory".into(),
        LibraryApiContractId::RustStdMapFactory => "rust.std.map_factory".into(),
        LibraryApiContractId::RustVecMacroFactory => "rust.vec.macro_factory".into(),
        LibraryApiContractId::RustVecNewFactory => "rust.vec.new_factory".into(),
        LibraryApiContractId::JavaCollectionFactory(kind) => {
            format!(
                "java.collection_factory.{}",
                java_collection_factory_kind_key(kind)
            )
        }
        LibraryApiContractId::JavaCollectionConstructor(kind) => {
            format!(
                "java.collection_constructor.{}",
                java_collection_constructor_kind_key(kind)
            )
        }
        LibraryApiContractId::JavaMapFactory(kind) => {
            format!("java.map_factory.{}", java_map_factory_kind_key(kind))
        }
        LibraryApiContractId::JavaMapEntryFactory => "java.map_entry_factory".into(),
        LibraryApiContractId::RubySetFactory => "ruby.set_factory".into(),
        LibraryApiContractId::JsLikeSetConstructor => "js_like.set.constructor".into(),
        LibraryApiContractId::JsLikeMapConstructor => "js_like.map.constructor".into(),
        LibraryApiContractId::MapKeyView(kind) => {
            format!("map_key_view.{}", map_key_view_kind_key(kind))
        }
        LibraryApiContractId::MapKeyViewWrapper => "map_key_view.wrapper".into(),
        LibraryApiContractId::MapGet => "map.get".into(),
        LibraryApiContractId::JsArrayIsArray => "js_like.array.is_array".into(),
        LibraryApiContractId::JsBooleanCoercion => "js_like.boolean.coercion".into(),
        LibraryApiContractId::RegexTest => "js_like.regex.test".into(),
        LibraryApiContractId::JsLikeStaticIndexMembership(kind) => {
            format!(
                "js_like.static_index_membership.{}",
                static_index_membership_kind_key(kind)
            )
        }
        LibraryApiContractId::ImportedNamespaceFunction(semantic) => {
            format!(
                "imported_namespace_function.{}",
                imported_namespace_function_semantic_key(semantic)
            )
        }
        LibraryApiContractId::PromiseFactory(kind) => {
            format!("js_like.promise.factory.{}", promise_factory_kind_key(kind))
        }
        LibraryApiContractId::PromiseThen => "js_like.promise.then".into(),
        LibraryApiContractId::IteratorIdentityAdapter => "iterator.identity_adapter".into(),
        LibraryApiContractId::StaticCollectionAdapter => "static.collection_adapter".into(),
        LibraryApiContractId::MethodCall(semantic) => {
            format!("method_call.{}", method_semantic_contract_key(semantic))
        }
    }
}

fn scalar_integer_method_key(method: ScalarIntegerMethod) -> &'static str {
    match method {
        ScalarIntegerMethod::Abs => "abs",
        ScalarIntegerMethod::Min => "min",
        ScalarIntegerMethod::Max => "max",
        ScalarIntegerMethod::Clamp => "clamp",
    }
}

fn java_collection_factory_kind_key(kind: JavaCollectionFactoryKind) -> &'static str {
    match kind {
        JavaCollectionFactoryKind::ListOf => "list_of",
        JavaCollectionFactoryKind::SetOf => "set_of",
        JavaCollectionFactoryKind::ArraysAsList => "arrays_as_list",
        JavaCollectionFactoryKind::CollectionsEmptyList => "collections_empty_list",
        JavaCollectionFactoryKind::CollectionsEmptySet => "collections_empty_set",
        JavaCollectionFactoryKind::CollectionsSingleton => "collections_singleton",
        JavaCollectionFactoryKind::CollectionsSingletonList => "collections_singleton_list",
        JavaCollectionFactoryKind::GuavaImmutableListOf => "guava_immutable_list_of",
        JavaCollectionFactoryKind::GuavaImmutableSetOf => "guava_immutable_set_of",
    }
}

fn swift_collection_factory_kind_key(kind: SwiftCollectionFactoryKind) -> &'static str {
    match kind {
        SwiftCollectionFactoryKind::Array => "array",
        SwiftCollectionFactoryKind::Set => "set",
    }
}

fn swift_map_factory_kind_key(kind: SwiftMapFactoryKind) -> &'static str {
    match kind {
        SwiftMapFactoryKind::DictionaryUniqueKeysWithValues => "dictionary_unique_keys_with_values",
    }
}

fn java_collection_constructor_kind_key(kind: JavaCollectionConstructorKind) -> &'static str {
    match kind {
        JavaCollectionConstructorKind::EmptyList => "empty_list",
    }
}

fn java_map_factory_kind_key(kind: JavaMapFactoryKind) -> &'static str {
    match kind {
        JavaMapFactoryKind::Of => "of",
        JavaMapFactoryKind::OfEntries => "of_entries",
        JavaMapFactoryKind::CollectionsEmptyMap => "collections_empty_map",
        JavaMapFactoryKind::CollectionsSingletonMap => "collections_singleton_map",
        JavaMapFactoryKind::GuavaImmutableMapOf => "guava_immutable_map_of",
    }
}

fn map_key_view_kind_key(kind: MapKeyViewKind) -> &'static str {
    match kind {
        MapKeyViewKind::Collection => "collection",
        MapKeyViewKind::Iterator => "iterator",
    }
}

fn static_index_membership_kind_key(kind: StaticIndexMembershipKind) -> &'static str {
    match kind {
        StaticIndexMembershipKind::IndexOf => "index_of",
        StaticIndexMembershipKind::FindIndex => "find_index",
    }
}

fn imported_namespace_function_semantic_key(semantic: ImportedNamespaceFunctionSemantic) -> String {
    match semantic {
        ImportedNamespaceFunctionSemantic::ProductReduction { op, identity } => {
            format!("product_reduction.{}.{}", op as u32, identity)
        }
    }
}

fn promise_factory_kind_key(kind: PromiseFactoryKind) -> &'static str {
    match kind {
        PromiseFactoryKind::Resolve => "resolve",
    }
}

fn method_semantic_contract_key(semantic: MethodSemanticContract) -> String {
    match semantic {
        MethodSemanticContract::Builtin(builtin) => format!("builtin.{}", builtin as u32),
        MethodSemanticContract::HoF(hof) => format!("hof.{}", hof as u32),
    }
}

pub(super) fn library_api_callee_contract_key(callee: LibraryApiCalleeContract) -> String {
    match callee {
        LibraryApiCalleeContract::FreeName { name, .. } => format!("free_name:{name}"),
        LibraryApiCalleeContract::LabeledFreeName {
            name, first_label, ..
        } => {
            format!("labeled_free_name:{name}:{first_label}")
        }
        LibraryApiCalleeContract::RustMacro { name, .. } => format!("rust_macro:{name}"),
        LibraryApiCalleeContract::ImportedBinding { module, exported } => {
            format!("imported_binding:{module}:{exported}")
        }
        LibraryApiCalleeContract::JavaUtilStaticMember { receiver, method } => {
            format!("java_util_static_member:{receiver}:{method}")
        }
        LibraryApiCalleeContract::JavaStaticMember {
            module,
            receiver,
            method,
            ..
        } => format!("java_static_member:{module}:{receiver}:{method}"),
        LibraryApiCalleeContract::JavaUtilConstructor {
            simple_type,
            qualified_type,
            module,
            ..
        } => format!("java_util_constructor:{module}:{simple_type}:{qualified_type}"),
        LibraryApiCalleeContract::RubyRequireStaticMember {
            receiver,
            method,
            required_module,
            ..
        } => format!("ruby_require_static_member:{required_module}:{receiver}:{method}"),
        LibraryApiCalleeContract::JsGlobalConstructor { receiver, .. } => {
            format!("js_global_constructor:{receiver}")
        }
        LibraryApiCalleeContract::Method { method, receiver } => {
            format!("method:{method}:{}", method_receiver_contract_key(receiver))
        }
        LibraryApiCalleeContract::StaticGlobalMethod { qualified_path, .. } => {
            format!("static_global_method:{qualified_path}")
        }
        LibraryApiCalleeContract::StaticGlobalFunction { function, .. } => {
            format!("static_global_function:{function}")
        }
        LibraryApiCalleeContract::RegexLiteralMethod { method, .. } => {
            format!("regex_literal_method:{method}")
        }
        LibraryApiCalleeContract::Property { property, receiver } => {
            format!(
                "property:{property}:{}",
                method_receiver_contract_key(receiver)
            )
        }
        LibraryApiCalleeContract::StaticIndexMembershipMethod { method, receiver } => {
            format!(
                "static_index_membership_method:{method}:{}",
                static_index_membership_receiver_contract_key(receiver)
            )
        }
        LibraryApiCalleeContract::ImportedNamespaceFunction { module, function } => {
            format!("imported_namespace_function:{module}:{function}")
        }
        LibraryApiCalleeContract::AsyncMethod { method, receiver } => {
            format!(
                "async_method:{method}:{}",
                async_receiver_contract_key(receiver)
            )
        }
        LibraryApiCalleeContract::IteratorAdapterMethod { method, receiver } => {
            format!(
                "iterator_adapter_method:{method}:{}",
                iterator_adapter_receiver_contract_key(receiver)
            )
        }
    }
}

fn method_receiver_contract_key(receiver: MethodReceiverContract) -> String {
    match receiver {
        MethodReceiverContract::ExactArray => "exact_array".into(),
        MethodReceiverContract::ExactArrayOrCollection => "exact_array_or_collection".into(),
        MethodReceiverContract::ExactCollection => "exact_collection".into(),
        MethodReceiverContract::ExactProtocol => "exact_protocol".into(),
        MethodReceiverContract::ExactProtocolPairArgument => "exact_protocol_pair_argument".into(),
        MethodReceiverContract::ExactOption => "exact_option".into(),
        MethodReceiverContract::ExactResult => "exact_result".into(),
        MethodReceiverContract::ExactString => "exact_string".into(),
        MethodReceiverContract::ExactInteger => "exact_integer".into(),
        MethodReceiverContract::ExactMap => "exact_map".into(),
        MethodReceiverContract::ExactMapLiteral => "exact_map_literal".into(),
        MethodReceiverContract::ExactCollectionOrMap => "exact_collection_or_map".into(),
        MethodReceiverContract::ExactCollectionOrMapLiteral => {
            "exact_collection_or_map_literal".into()
        }
        MethodReceiverContract::ExactCollectionOrJavaKeySet => {
            "exact_collection_or_java_key_set".into()
        }
        MethodReceiverContract::ExactSetOrMap => "exact_set_or_map".into(),
        MethodReceiverContract::LiteralString => "literal_string".into(),
        MethodReceiverContract::UnshadowedGlobal(name) => format!("unshadowed_global:{name}"),
        MethodReceiverContract::ImportedNamespace(module) => {
            format!("imported_namespace:{module}")
        }
        MethodReceiverContract::RustMapGetOrExactOption => "rust_map_get_or_exact_option".into(),
    }
}

fn async_receiver_contract_key(receiver: AsyncReceiverContract) -> &'static str {
    match receiver {
        AsyncReceiverContract::ExactPromiseLike => "exact_promise_like",
    }
}

fn iterator_adapter_receiver_contract_key(
    receiver: IteratorAdapterReceiverContract,
) -> &'static str {
    match receiver {
        IteratorAdapterReceiverContract::ExactIterableValue => "exact_iterable_value",
    }
}

fn static_index_membership_receiver_contract_key(
    receiver: StaticIndexMembershipReceiverContract,
) -> &'static str {
    match receiver {
        StaticIndexMembershipReceiverContract::StaticNonFloatLiteralCollection => {
            "static_non_float_literal_collection"
        }
    }
}
