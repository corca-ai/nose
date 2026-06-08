//! Library API contract identities, callee coordinates, and result wrappers.
//!
//! This module names the semantic rows. Occurrence evidence admission and
//! dependency validation stay in the parent module.

use crate::{
    AsyncReceiverContract, FreeFunctionBuiltinContract, ImportedNamespaceFunctionContract,
    ImportedNamespaceFunctionSemantic, IteratorAdapterReceiverContract,
    IteratorIdentityAdapterContract, JavaCollectionConstructorKind, JavaCollectionFactoryKind,
    JavaMapFactoryKind, MapGetContract, MapKeyViewContract, MapKeyViewKind,
    MapKeyViewWrapperContract, MethodCallContract, MethodReceiverContract, MethodSemanticContract,
    PromiseThenContract, RegexTestContract, ScalarIntegerMethod, ScalarIntegerMethodContract,
    StaticCollectionAdapterContract, StaticGlobalFunctionContract, StaticGlobalMethodContract,
    StaticIndexMembershipContract, StaticIndexMembershipKind,
    StaticIndexMembershipReceiverContract,
};
use nose_il::{stable_symbol_hash, Builtin, DomainEvidence, Lang, SourceFactKind, Span};

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
    PromiseThen,
    IteratorIdentityAdapter,
    StaticCollectionAdapter,
    MethodCall(MethodSemanticContract),
}

pub fn library_api_contract_id_hash(id: LibraryApiContractId) -> u64 {
    stable_symbol_hash(&library_api_contract_id_key(id))
}

fn library_api_contract_id_key(id: LibraryApiContractId) -> String {
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
        LibraryApiContractId::FreeFunctionBuiltin(builtin) => {
            format!("free_function_builtin.{}", builtin as u32)
        }
        LibraryApiContractId::RustOptionSomeConstructor => "rust.option.some.constructor".into(),
        LibraryApiContractId::RustOptionNoneSentinel => "rust.option.none.sentinel".into(),
        LibraryApiContractId::RustOptionAndThen => "rust.option.and_then".into(),
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

fn method_semantic_contract_key(semantic: MethodSemanticContract) -> String {
    match semantic {
        MethodSemanticContract::Builtin(builtin) => format!("builtin.{}", builtin as u32),
        MethodSemanticContract::HoF(hof) => format!("hof.{}", hof as u32),
    }
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

fn library_api_callee_contract_key(callee: LibraryApiCalleeContract) -> String {
    match callee {
        LibraryApiCalleeContract::FreeName { name, .. } => format!("free_name:{name}"),
        LibraryApiCalleeContract::RustMacro { name, .. } => format!("rust_macro:{name}"),
        LibraryApiCalleeContract::ImportedBinding { module, exported } => {
            format!("imported_binding:{module}:{exported}")
        }
        LibraryApiCalleeContract::JavaUtilStaticMember { receiver, method } => {
            format!("java_util_static_member:{receiver}:{method}")
        }
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
        MethodReceiverContract::ExactCollection => "exact_collection".into(),
        MethodReceiverContract::ExactProtocol => "exact_protocol".into(),
        MethodReceiverContract::ExactProtocolPairArgument => "exact_protocol_pair_argument".into(),
        MethodReceiverContract::ExactOption => "exact_option".into(),
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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LibraryCollectionFactoryResult {
    SequenceArgument,
    VariadicElements { single_arg_spreads_array: bool },
    StaticNonFloatSequenceArgument,
    EmptySequence,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryCollectionFactoryContract {
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
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result: LibraryMapFactoryResult,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryMapEntryFactoryContract {
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryMapKeyViewContract {
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result: MapKeyViewContract,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryMapKeyViewWrapperContract {
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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryMapGetContract {
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result: MapGetContract,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryStaticGlobalMethodContract {
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result: StaticGlobalMethodContract,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryStaticGlobalFunctionContract {
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result: StaticGlobalFunctionContract,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryRegexTestContract {
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result: RegexTestContract,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryStaticIndexMembershipContract {
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result: StaticIndexMembershipContract,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryImportedNamespaceFunctionContract {
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result: ImportedNamespaceFunctionContract,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryPromiseThenContract {
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result: PromiseThenContract,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryIteratorIdentityAdapterContract {
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result: IteratorIdentityAdapterContract,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryStaticCollectionAdapterContract {
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result: StaticCollectionAdapterContract,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryMethodCallContract {
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result: MethodCallContract,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryPropertyBuiltinContract {
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result: Builtin,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryScalarIntegerMethodContract {
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result: ScalarIntegerMethodContract,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryRustOptionConstructorContract {
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result_domain: DomainEvidence,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryRustOptionSentinelContract {
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
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub rule: &'static str,
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
