//! Library API contract identities, callee coordinates, and result wrappers.
//!
//! This module names the semantic rows. Occurrence evidence admission and
//! dependency validation stay in the parent module.

use super::contract_keys::{library_api_callee_contract_key, library_api_contract_id_key};
use crate::{
    AsyncReceiverContract, FreeFunctionBuiltinContract, ImportedNamespaceFunctionContract,
    ImportedNamespaceFunctionSemantic, IteratorAdapterReceiverContract,
    IteratorIdentityAdapterContract, JavaCollectionConstructorKind, JavaCollectionFactoryKind,
    JavaMapFactoryKind, MapGetContract, MapKeyViewContract, MapKeyViewKind,
    MapKeyViewWrapperContract, MethodCallContract, MethodReceiverContract, MethodSemanticContract,
    PromiseFactoryContract, PromiseFactoryKind, PromiseThenContract, RegexTestContract,
    ScalarIntegerMethod, ScalarIntegerMethodContract, StaticCollectionAdapterContract,
    StaticGlobalFunctionContract, StaticGlobalMethodContract, StaticIndexMembershipContract,
    StaticIndexMembershipKind, StaticIndexMembershipReceiverContract,
};
use nose_il::{stable_symbol_hash, Builtin, DomainEvidence, Lang, SourceFactKind, Span};

pub const PYTHON_BUILTIN_COLLECTION_FACTORY_PACK_ID: &str =
    "nose.python.builtins.collection_factories";
pub const PYTHON_BUILTIN_COLLECTION_FACTORY_PRODUCER_ID: &str =
    "python.builtins.collection-factory-api";
pub const PYTHON_BUILTIN_COLLECTION_FACTORY_CONTRACT_ID: &str = "python.builtin.collection_factory";
pub const PYTHON_STDLIB_COLLECTION_FACTORY_PACK_ID: &str =
    "nose.python.stdlib.collection_factories";
pub const PYTHON_STDLIB_COLLECTION_FACTORY_PRODUCER_ID: &str =
    "python.stdlib.collection-factory-api";
pub const PYTHON_STDLIB_COLLECTION_FACTORY_CONTRACT_ID: &str = "python.imported.collection_factory";
pub const PYTHON_STDLIB_MATH_PACK_ID: &str = "nose.python.stdlib.math";
pub const PYTHON_STDLIB_MATH_PRODUCER_ID: &str = "python.stdlib.math-api";
pub const PYTHON_STDLIB_MATH_PROD_CONTRACT_ID: &str = "python.math.prod.reduction";
pub const JS_LIKE_BUILTIN_PROMISE_PACK_ID: &str = "nose.javascript.builtins.promise";
pub const JS_LIKE_BUILTIN_PROMISE_PRODUCER_ID: &str = "javascript.builtins.promise-api";
pub const JS_LIKE_BUILTIN_PROMISE_RESOLVE_CONTRACT_ID: &str = "js_like.promise.factory.resolve";
pub const JS_LIKE_BUILTIN_PROMISE_THEN_CONTRACT_ID: &str = "js_like.promise.then";
pub const JS_LIKE_BUILTIN_ARRAY_PACK_ID: &str = "nose.javascript.builtins.array";
pub const JS_LIKE_BUILTIN_ARRAY_PRODUCER_ID: &str = "javascript.builtins.array-api";
pub const JS_LIKE_BUILTIN_ARRAY_FROM_CONTRACT_ID: &str = "map_key_view.wrapper";
pub const JS_LIKE_BUILTIN_ARRAY_IS_ARRAY_CONTRACT_ID: &str = "js_like.array.is_array";
pub const JS_LIKE_BUILTIN_BOOLEAN_PACK_ID: &str = "nose.javascript.builtins.boolean";
pub const JS_LIKE_BUILTIN_BOOLEAN_PRODUCER_ID: &str = "javascript.builtins.boolean-api";
pub const JS_LIKE_BUILTIN_BOOLEAN_CONTRACT_ID: &str = "js_like.boolean.coercion";
pub const JS_LIKE_BUILTIN_REGEX_PACK_ID: &str = "nose.javascript.builtins.regex";
pub const JS_LIKE_BUILTIN_REGEX_PRODUCER_ID: &str = "javascript.builtins.regex-api";
pub const JS_LIKE_BUILTIN_REGEX_TEST_CONTRACT_ID: &str = "js_like.regex.test";
pub const JS_LIKE_BUILTIN_STATIC_INDEX_MEMBERSHIP_PACK_ID: &str =
    "nose.javascript.builtins.static_index_membership";
pub const JS_LIKE_BUILTIN_STATIC_INDEX_MEMBERSHIP_PRODUCER_ID: &str =
    "javascript.builtins.static-index-membership-api";
pub const JS_LIKE_BUILTIN_STATIC_INDEX_MEMBERSHIP_INDEX_OF_CONTRACT_ID: &str =
    "js_like.static_index_membership.index_of";
pub const JS_LIKE_BUILTIN_STATIC_INDEX_MEMBERSHIP_FIND_INDEX_CONTRACT_ID: &str =
    "js_like.static_index_membership.find_index";
pub const JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PACK_ID: &str =
    "nose.javascript.builtins.collection_constructors";
pub const JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PRODUCER_ID: &str =
    "javascript.builtins.collection-constructor-api";
pub const JS_LIKE_BUILTIN_SET_CONSTRUCTOR_CONTRACT_ID: &str = "js_like.set.constructor";
pub const JS_LIKE_BUILTIN_MAP_CONSTRUCTOR_CONTRACT_ID: &str = "js_like.map.constructor";
pub const MAP_GET_PROTOCOL_PACK_ID: &str = "nose.protocols.map_get";
pub const MAP_GET_PROTOCOL_PRODUCER_ID: &str = "protocols.map-get-api";
pub const MAP_GET_CONTRACT_ID: &str = "map.get";
pub const MAP_GET_DEFAULT_PROTOCOL_PACK_ID: &str = "nose.protocols.map_get_default";
pub const MAP_GET_DEFAULT_PROTOCOL_PRODUCER_ID: &str = "protocols.map-get-default-api";
pub const MAP_GET_DEFAULT_CONTRACT_ID: &str = "map.get_default";
pub const FREE_FUNCTION_BUILTIN_PROTOCOL_PACK_ID: &str = "nose.protocols.free_function_builtins";
pub const FREE_FUNCTION_BUILTIN_PROTOCOL_PRODUCER_ID: &str = "protocols.free-function-builtin-api";
pub const FREE_FUNCTION_BUILTIN_CONTRACT_ID: &str = "free_function_builtin.call";
pub const RECEIVER_MEMBERSHIP_PROTOCOL_PACK_ID: &str = "nose.protocols.receiver_membership";
pub const RECEIVER_MEMBERSHIP_PROTOCOL_PRODUCER_ID: &str = "protocols.receiver-membership-api";
pub const RECEIVER_MEMBERSHIP_CONTRACT_ID: &str = "receiver_membership.contains";
pub const MAP_KEY_VIEW_PROTOCOL_PACK_ID: &str = "nose.protocols.map_key_views";
pub const MAP_KEY_VIEW_PROTOCOL_PRODUCER_ID: &str = "protocols.map-key-view-api";
pub const MAP_KEY_VIEW_COLLECTION_CONTRACT_ID: &str = "map_key_view.collection";
pub const MAP_KEY_VIEW_ITERATOR_CONTRACT_ID: &str = "map_key_view.iterator";
pub const PROPERTY_BUILTIN_PROTOCOL_PACK_ID: &str = "nose.protocols.property_builtins";
pub const PROPERTY_BUILTIN_PROTOCOL_PRODUCER_ID: &str = "protocols.property-builtin-api";
pub const PROPERTY_BUILTIN_LEN_CONTRACT_ID: &str = "property_builtin.len";
pub const PROPERTY_BUILTIN_IS_EMPTY_CONTRACT_ID: &str = "property_builtin.is_empty";
pub const BUILTIN_METHOD_CALL_PROTOCOL_PACK_ID: &str = "nose.protocols.builtin_method_calls";
pub const BUILTIN_METHOD_CALL_PROTOCOL_PRODUCER_ID: &str = "protocols.builtin-method-call-api";
pub const BUILTIN_METHOD_CALL_CONTRACT_ID: &str = "builtin_method.call";
pub const GO_STDLIB_NAMESPACE_CALL_PACK_ID: &str = "nose.go.stdlib.namespace_calls";
pub const GO_STDLIB_NAMESPACE_CALL_PRODUCER_ID: &str = "go.stdlib.namespace-call-api";
pub const GO_STDLIB_NAMESPACE_CALL_CONTRACT_ID: &str = "go.stdlib.namespace_call";
pub const SCALAR_INTEGER_METHOD_ABS_CONTRACT_ID: &str = "scalar_integer_method.abs";
pub const SCALAR_INTEGER_METHOD_MIN_CONTRACT_ID: &str = "scalar_integer_method.min";
pub const SCALAR_INTEGER_METHOD_MAX_CONTRACT_ID: &str = "scalar_integer_method.max";
pub const SCALAR_INTEGER_METHOD_CLAMP_CONTRACT_ID: &str = "scalar_integer_method.clamp";
pub const JAVA_STDLIB_MATH_PACK_ID: &str = "nose.java.stdlib.math";
pub const JAVA_STDLIB_MATH_PRODUCER_ID: &str = "java.stdlib.math-api";
pub const RUBY_STDLIB_SET_PACK_ID: &str = "nose.ruby.stdlib.set";
pub const RUBY_STDLIB_SET_PRODUCER_ID: &str = "ruby.stdlib.set-factory-api";
pub const RUBY_STDLIB_SET_CONTRACT_ID: &str = "ruby.set_factory";
pub const RUST_STDLIB_VEC_PACK_ID: &str = "nose.rust.stdlib.vec";
pub const RUST_STDLIB_VEC_PRODUCER_ID: &str = "rust.stdlib.vec-factory-api";
pub const RUST_STDLIB_VEC_MACRO_CONTRACT_ID: &str = "rust.vec.macro_factory";
pub const RUST_STDLIB_VEC_NEW_CONTRACT_ID: &str = "rust.vec.new_factory";
pub const RUST_STDLIB_OPTION_PACK_ID: &str = "nose.rust.stdlib.option";
pub const RUST_STDLIB_OPTION_PRODUCER_ID: &str = "rust.stdlib.option-api";
pub const RUST_STDLIB_OPTION_SOME_CONTRACT_ID: &str = "rust.option.some.constructor";
pub const RUST_STDLIB_OPTION_NONE_CONTRACT_ID: &str = "rust.option.none.sentinel";
pub const RUST_STDLIB_OPTION_AND_THEN_CONTRACT_ID: &str = "rust.option.and_then";
pub const RUST_STDLIB_INTEGER_METHOD_PACK_ID: &str = "nose.rust.stdlib.integer_methods";
pub const RUST_STDLIB_INTEGER_METHOD_PRODUCER_ID: &str = "rust.stdlib.integer-method-api";
pub const ITERATOR_IDENTITY_ADAPTER_PACK_ID: &str = "nose.protocols.iterator_identity_adapters";
pub const ITERATOR_IDENTITY_ADAPTER_PRODUCER_ID: &str = "protocols.iterator-identity-adapter-api";
pub const ITERATOR_IDENTITY_ADAPTER_CONTRACT_ID: &str = "iterator.identity_adapter";
pub const RUST_STDLIB_COLLECTION_FACTORY_PACK_ID: &str = "nose.rust.stdlib.collection_factories";
pub const RUST_STDLIB_COLLECTION_FACTORY_PRODUCER_ID: &str = "rust.stdlib.collection-factory-api";
pub const RUST_STDLIB_COLLECTION_FACTORY_CONTRACT_ID: &str = "rust.std.collection_factory";
pub const RUST_STDLIB_MAP_FACTORY_PACK_ID: &str = "nose.rust.stdlib.map_factories";
pub const RUST_STDLIB_MAP_FACTORY_PRODUCER_ID: &str = "rust.stdlib.map-factory-api";
pub const RUST_STDLIB_MAP_FACTORY_CONTRACT_ID: &str = "rust.std.map_factory";
pub const JAVA_STDLIB_MAP_FACTORY_PACK_ID: &str = "nose.java.stdlib.map_factories";
pub const JAVA_STDLIB_MAP_FACTORY_PRODUCER_ID: &str = "java.stdlib.map-factory-api";
pub const JAVA_STDLIB_MAP_FACTORY_OF_CONTRACT_ID: &str = "java.map_factory.of";
pub const JAVA_STDLIB_MAP_FACTORY_OF_ENTRIES_CONTRACT_ID: &str = "java.map_factory.of_entries";
pub const JAVA_STDLIB_MAP_ENTRY_PACK_ID: &str = "nose.java.stdlib.map_entries";
pub const JAVA_STDLIB_MAP_ENTRY_PRODUCER_ID: &str = "java.stdlib.map-entry-api";
pub const JAVA_STDLIB_MAP_ENTRY_CONTRACT_ID: &str = "java.map_entry_factory";
pub const JAVA_STDLIB_COLLECTION_FACTORY_PACK_ID: &str = "nose.java.stdlib.collection_factories";
pub const JAVA_STDLIB_COLLECTION_FACTORY_PRODUCER_ID: &str = "java.stdlib.collection-factory-api";
pub const JAVA_STDLIB_COLLECTION_FACTORY_LIST_OF_CONTRACT_ID: &str =
    "java.collection_factory.list_of";
pub const JAVA_STDLIB_COLLECTION_FACTORY_SET_OF_CONTRACT_ID: &str =
    "java.collection_factory.set_of";
pub const JAVA_STDLIB_COLLECTION_FACTORY_ARRAYS_AS_LIST_CONTRACT_ID: &str =
    "java.collection_factory.arrays_as_list";
pub const JAVA_GUAVA_IMMUTABLE_COLLECTION_FACTORY_PACK_ID: &str =
    "nose.java.ecosystem.guava.immutable_collection_factories";
pub const JAVA_GUAVA_IMMUTABLE_COLLECTION_FACTORY_PRODUCER_ID: &str =
    "java.guava.immutable-collection-factory-api";
pub const JAVA_GUAVA_IMMUTABLE_LIST_OF_CONTRACT_ID: &str =
    "java.collection_factory.guava_immutable_list_of";
pub const JAVA_GUAVA_IMMUTABLE_SET_OF_CONTRACT_ID: &str =
    "java.collection_factory.guava_immutable_set_of";
pub const JAVA_GUAVA_IMMUTABLE_MAP_OF_CONTRACT_ID: &str = "java.map_factory.guava_immutable_map_of";
pub const JAVA_STDLIB_COLLECTION_CONSTRUCTOR_PACK_ID: &str =
    "nose.java.stdlib.collection_constructors";
pub const JAVA_STDLIB_COLLECTION_CONSTRUCTOR_PRODUCER_ID: &str =
    "java.stdlib.collection-constructor-api";
pub const JAVA_STDLIB_COLLECTION_CONSTRUCTOR_EMPTY_LIST_CONTRACT_ID: &str =
    "java.collection_constructor.empty_list";
pub const JAVA_STDLIB_STATIC_COLLECTION_ADAPTER_PACK_ID: &str =
    "nose.java.stdlib.static_collection_adapters";
pub const JAVA_STDLIB_STATIC_COLLECTION_ADAPTER_PRODUCER_ID: &str =
    "java.stdlib.static-collection-adapter-api";
pub const JAVA_STDLIB_STATIC_COLLECTION_ADAPTER_CONTRACT_ID: &str = "static.collection_adapter";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LibraryApiContractId {
    PropertyBuiltin(Builtin),
    PythonBuiltinCollectionFactory,
    PythonImportedCollectionFactory,
    FreeFunctionBuiltin(Builtin),
    RustOptionSomeConstructor,
    RustOptionNoneSentinel,
    RustOptionAndThen,
    ScalarIntegerMethod(ScalarIntegerMethod),
    RustStdCollectionFactory,
    RustStdMapFactory,
    RustVecMacroFactory,
    RustVecNewFactory,
    JavaCollectionFactory(JavaCollectionFactoryKind),
    JavaCollectionConstructor(JavaCollectionConstructorKind),
    JavaMapFactory(JavaMapFactoryKind),
    JavaMapEntryFactory,
    RubySetFactory,
    JsLikeSetConstructor,
    JsLikeMapConstructor,
    MapKeyView(MapKeyViewKind),
    MapKeyViewWrapper,
    MapGet,
    JsArrayIsArray,
    JsBooleanCoercion,
    RegexTest,
    JsLikeStaticIndexMembership(StaticIndexMembershipKind),
    ImportedNamespaceFunction(ImportedNamespaceFunctionSemantic),
    PromiseFactory(PromiseFactoryKind),
    PromiseThen,
    IteratorIdentityAdapter,
    StaticCollectionAdapter,
    MethodCall(MethodSemanticContract),
}

pub fn library_api_contract_id_hash(id: LibraryApiContractId) -> u64 {
    stable_symbol_hash(&library_api_contract_id_key(id))
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LibraryApiShadowPolicy {
    None,
    SameName,
    RustStdRootForStdPath,
    ExplicitRoot(&'static str),
}

pub fn library_api_free_name_shadow_safe(
    lang: Lang,
    name: &str,
    policy: LibraryApiShadowPolicy,
    defines_name: impl Fn(&str) -> bool,
) -> bool {
    match policy {
        LibraryApiShadowPolicy::None => true,
        LibraryApiShadowPolicy::SameName => !defines_name(name),
        LibraryApiShadowPolicy::RustStdRootForStdPath => {
            !(lang == Lang::Rust && name.starts_with("std::") && defines_name("std"))
        }
        LibraryApiShadowPolicy::ExplicitRoot(root) => !defines_name(root),
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LibraryApiCalleeContract {
    FreeName {
        name: &'static str,
        shadow: LibraryApiShadowPolicy,
    },
    RustMacro {
        name: &'static str,
        shadow: LibraryApiShadowPolicy,
    },
    ImportedBinding {
        module: &'static str,
        exported: &'static str,
    },
    JavaUtilStaticMember {
        receiver: &'static str,
        method: &'static str,
    },
    JavaStaticMember {
        module: &'static str,
        receiver: &'static str,
        method: &'static str,
        requires_import_for_simple_receiver: bool,
        requires_no_local_type_shadow: bool,
    },
    JavaUtilConstructor {
        simple_type: &'static str,
        qualified_type: &'static str,
        module: &'static str,
        requires_import_for_simple_type: bool,
        requires_no_local_type_shadow: bool,
    },
    RubyRequireStaticMember {
        receiver: &'static str,
        method: &'static str,
        required_module: &'static str,
        shadow_root: &'static str,
    },
    JsGlobalConstructor {
        receiver: &'static str,
        requires_unshadowed_global: bool,
    },
    Method {
        method: &'static str,
        receiver: MethodReceiverContract,
    },
    StaticGlobalMethod {
        receiver: &'static str,
        method: &'static str,
        qualified_path: &'static str,
        requires_unshadowed_receiver: bool,
    },
    StaticGlobalFunction {
        function: &'static str,
        requires_unshadowed_function: bool,
    },
    RegexLiteralMethod {
        method: &'static str,
        required_receiver_fact: SourceFactKind,
    },
    Property {
        property: &'static str,
        receiver: MethodReceiverContract,
    },
    StaticIndexMembershipMethod {
        method: &'static str,
        receiver: StaticIndexMembershipReceiverContract,
    },
    ImportedNamespaceFunction {
        module: &'static str,
        function: &'static str,
    },
    AsyncMethod {
        method: &'static str,
        receiver: AsyncReceiverContract,
    },
    IteratorAdapterMethod {
        method: &'static str,
        receiver: IteratorAdapterReceiverContract,
    },
}

pub fn library_api_callee_contract_hash(callee: LibraryApiCalleeContract) -> u64 {
    stable_symbol_hash(&library_api_callee_contract_key(callee))
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LibraryCollectionFactoryResult {
    SequenceArgument,
    VariadicElements { single_arg_spreads_array: bool },
    StaticNonFloatSequenceArgument,
    EmptySequence,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryCollectionFactoryContract {
    pub pack_id: &'static str,
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result: LibraryCollectionFactoryResult,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LibraryMapFactoryResult {
    EntrySequence { entry_seq_tag: u64 },
    JavaFactory { kind: JavaMapFactoryKind },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryMapFactoryContract {
    pub pack_id: &'static str,
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result: LibraryMapFactoryResult,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryMapEntryFactoryContract {
    pub pack_id: &'static str,
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryMapKeyViewContract {
    pub pack_id: &'static str,
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result: MapKeyViewContract,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryMapKeyViewWrapperContract {
    pub pack_id: &'static str,
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result: MapKeyViewWrapperContract,
}

pub fn library_collection_factory_result_domain(
    contract: LibraryCollectionFactoryContract,
) -> DomainEvidence {
    match contract.id {
        LibraryApiContractId::PythonBuiltinCollectionFactory => match contract.callee {
            LibraryApiCalleeContract::FreeName {
                name: "set" | "frozenset",
                ..
            } => DomainEvidence::Set,
            _ => DomainEvidence::Collection,
        },
        LibraryApiContractId::RustStdCollectionFactory => match contract.callee {
            LibraryApiCalleeContract::FreeName {
                name: "std::collections::HashSet::from" | "std::collections::BTreeSet::from",
                ..
            } => DomainEvidence::Set,
            _ => DomainEvidence::Collection,
        },
        LibraryApiContractId::JavaCollectionFactory(JavaCollectionFactoryKind::SetOf)
        | LibraryApiContractId::JavaCollectionFactory(
            JavaCollectionFactoryKind::GuavaImmutableSetOf,
        )
        | LibraryApiContractId::RubySetFactory
        | LibraryApiContractId::JsLikeSetConstructor => DomainEvidence::Set,
        _ => DomainEvidence::Collection,
    }
}

pub fn library_collection_factory_result_domain_for_arity(
    contract: LibraryCollectionFactoryContract,
    arg_count: usize,
) -> Option<DomainEvidence> {
    match contract.id {
        LibraryApiContractId::JavaCollectionFactory(JavaCollectionFactoryKind::ArraysAsList)
            if arg_count == 1 =>
        {
            None
        }
        _ => Some(library_collection_factory_result_domain(contract)),
    }
}

pub fn library_map_factory_result_domain(_contract: LibraryMapFactoryContract) -> DomainEvidence {
    DomainEvidence::Map
}

pub fn library_map_key_view_wrapper_result_domain(
    _contract: LibraryMapKeyViewWrapperContract,
) -> DomainEvidence {
    DomainEvidence::Array
}

pub(in crate::library_api) fn library_receiver_method_api_result_domain(
    id: LibraryApiContractId,
) -> Option<DomainEvidence> {
    match id {
        LibraryApiContractId::MapKeyView(MapKeyViewKind::Collection) => {
            Some(DomainEvidence::Collection)
        }
        LibraryApiContractId::MapKeyView(MapKeyViewKind::Iterator) => {
            Some(DomainEvidence::Iterator)
        }
        LibraryApiContractId::ScalarIntegerMethod(_) => Some(DomainEvidence::Integer),
        LibraryApiContractId::RustOptionAndThen => Some(DomainEvidence::Option),
        LibraryApiContractId::PromiseThen => Some(DomainEvidence::PromiseLike),
        _ => None,
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryMapGetContract {
    pub pack_id: &'static str,
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result: MapGetContract,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryStaticGlobalMethodContract {
    pub pack_id: &'static str,
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result: StaticGlobalMethodContract,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryStaticGlobalFunctionContract {
    pub pack_id: &'static str,
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result: StaticGlobalFunctionContract,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryRegexTestContract {
    pub pack_id: &'static str,
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result: RegexTestContract,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryStaticIndexMembershipContract {
    pub pack_id: &'static str,
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result: StaticIndexMembershipContract,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryImportedNamespaceFunctionContract {
    pub pack_id: &'static str,
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result: ImportedNamespaceFunctionContract,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryPromiseFactoryContract {
    pub pack_id: &'static str,
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result: PromiseFactoryContract,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryPromiseThenContract {
    pub pack_id: &'static str,
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result: PromiseThenContract,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryIteratorIdentityAdapterContract {
    pub pack_id: &'static str,
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result: IteratorIdentityAdapterContract,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryStaticCollectionAdapterContract {
    pub pack_id: &'static str,
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result: StaticCollectionAdapterContract,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryMethodCallContract {
    pub pack_id: &'static str,
    pub producer_id: &'static str,
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result: MethodCallContract,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryPropertyBuiltinContract {
    pub pack_id: &'static str,
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result: Builtin,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryScalarIntegerMethodContract {
    pub pack_id: &'static str,
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result: ScalarIntegerMethodContract,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryRustOptionConstructorContract {
    pub pack_id: &'static str,
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result_domain: DomainEvidence,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryRustOptionSentinelContract {
    pub pack_id: &'static str,
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result_domain: DomainEvidence,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RustOptionAndThenContract {
    pub receiver: MethodReceiverContract,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryRustOptionAndThenContract {
    pub pack_id: &'static str,
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result: RustOptionAndThenContract,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryFreeFunctionBuiltinContract {
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result: FreeFunctionBuiltinContract,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryReceiverMethodApiContract {
    pub pack_id: &'static str,
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub rule: &'static str,
    pub result_domain: Option<DomainEvidence>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LibraryApiEvidenceStatus {
    Missing,
    Admitted,
    Rejected,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryApiSpanEvidenceQuery {
    pub call_span: Option<Span>,
    pub callee_span: Option<Span>,
    pub receiver_span: Option<Span>,
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub arg_count: usize,
}
