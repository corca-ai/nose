//! Admitted API occurrence resolvers for semantic consumers.
//!
//! Contract row lookup and `LibraryApi` evidence admission are kept together here
//! so downstream consumers do not recombine raw selector parsing with proof checks.

use super::*;

mod admission;

use admission::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AdmittedLibraryApiCall<C> {
    pub contract: C,
    pub callee: NodeId,
    pub receiver: Option<NodeId>,
    pub arg_count: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AdmittedLibraryApiNode<C> {
    pub contract: C,
    pub node: NodeId,
    pub receiver: Option<NodeId>,
    pub arg_count: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryApiSpanCall {
    pub call_span: Option<Span>,
    pub callee_span: Option<Span>,
    pub receiver_span: Option<Span>,
    pub arg_count: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AdmittedLibraryApiSpanCall<C> {
    pub contract: C,
    pub call_span: Option<Span>,
    pub callee_span: Option<Span>,
    pub receiver_span: Option<Span>,
    pub arg_count: usize,
}

macro_rules! receiver_method_contract_resolver {
    ($name:ident, $contract:ty, $lookup:path) => {
        pub fn $name(
            il: &Il,
            interner: &Interner,
            call: NodeId,
        ) -> Option<AdmittedLibraryApiCall<$contract>> {
            admitted_receiver_method_contract_call(il, interner, call, $lookup)
        }
    };
}

receiver_method_contract_resolver!(
    admitted_library_method_call_at_call,
    LibraryMethodCallContract,
    library_method_call_contract
);

pub fn admitted_free_function_builtin_at_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<AdmittedLibraryApiCall<LibraryFreeFunctionBuiltinContract>> {
    admitted_free_name_call(il, interner, call, |name, arg_count| {
        library_free_function_builtin_contract(il.meta.lang, name, arg_count)
    })
}

pub fn admitted_property_builtin_at_field(
    il: &Il,
    interner: &Interner,
    field: NodeId,
) -> Option<AdmittedLibraryApiNode<LibraryPropertyBuiltinContract>> {
    if il.kind(field) != NodeKind::Field {
        return None;
    }
    let Payload::Name(property) = il.node(field).payload else {
        return None;
    };
    let receiver = il.children(field).first().copied()?;
    let contract = library_property_builtin_contract(il.meta.lang, interner.resolve(property))?;
    admitted_library_node(il, interner, field, Some(receiver), 0, contract)
}

receiver_method_contract_resolver!(
    admitted_map_get_at_call,
    LibraryMapGetContract,
    library_map_get_contract
);

pub fn admitted_map_get_at_call_span(
    il: &Il,
    interner: &Interner,
    occurrence: LibraryApiSpanCall,
    method_hash: u64,
) -> Option<AdmittedLibraryApiSpanCall<LibraryMapGetContract>> {
    let contract =
        library_map_get_contract_by_hash(il.meta.lang, method_hash, occurrence.arg_count)?;
    admitted_library_span_call(il, interner, occurrence, contract)
}

receiver_method_contract_resolver!(
    admitted_regex_test_at_call,
    LibraryRegexTestContract,
    library_regex_test_contract
);

pub fn admitted_js_array_is_array_at_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<AdmittedLibraryApiCall<LibraryStaticGlobalMethodContract>> {
    admitted_named_receiver_method_call(il, interner, call, |receiver_name, method, arg_count| {
        library_js_array_is_array_contract(il.meta.lang, receiver_name, method, arg_count)
    })
}

receiver_method_contract_resolver!(
    admitted_static_index_membership_at_call,
    LibraryStaticIndexMembershipContract,
    library_static_index_membership_contract
);

receiver_method_contract_resolver!(
    admitted_scalar_integer_method_at_call,
    LibraryScalarIntegerMethodContract,
    library_scalar_integer_method_contract
);

receiver_method_contract_resolver!(
    admitted_map_key_view_at_call,
    LibraryMapKeyViewContract,
    library_map_key_view_contract
);

pub fn admitted_map_key_view_at_call_span(
    il: &Il,
    interner: &Interner,
    occurrence: LibraryApiSpanCall,
    method_hash: u64,
) -> Option<AdmittedLibraryApiSpanCall<LibraryMapKeyViewContract>> {
    let contract =
        library_map_key_view_contract_by_hash(il.meta.lang, method_hash, occurrence.arg_count)?;
    admitted_library_span_call(il, interner, occurrence, contract)
}

pub fn admitted_map_key_view_wrapper_at_call_span(
    il: &Il,
    interner: &Interner,
    occurrence: LibraryApiSpanCall,
    receiver: &str,
    method_hash: u64,
) -> Option<AdmittedLibraryApiSpanCall<LibraryMapKeyViewWrapperContract>> {
    let contract = library_map_key_view_wrapper_contract_by_hash(
        il.meta.lang,
        receiver,
        method_hash,
        occurrence.arg_count,
    )?;
    admitted_library_span_call(il, interner, occurrence, contract)
}

pub fn admitted_map_key_view_wrapper_at_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<AdmittedLibraryApiCall<LibraryMapKeyViewWrapperContract>> {
    admitted_receiver_method_call(il, interner, call, |method, arg_count| {
        library_map_key_view_wrapper_contract(il.meta.lang, "Array", method, arg_count)
    })
}

pub fn admitted_free_name_collection_factory_at_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<AdmittedLibraryApiCall<LibraryCollectionFactoryContract>> {
    admitted_free_name_call(il, interner, call, |name, _arg_count| {
        library_free_name_collection_factory_contract(il.meta.lang, name)
    })
}

pub fn admitted_swift_collection_factory_at_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<AdmittedLibraryApiCall<LibraryCollectionFactoryContract>> {
    let (_, args) = call_parts(il, call)?;
    if args.len() != 1 || il.kind(args[0]) == NodeKind::KwArg {
        return None;
    }
    let admitted = admitted_free_name_collection_factory_at_call(il, interner, call)?;
    matches!(
        admitted.contract.id,
        LibraryApiContractId::SwiftCollectionFactory(_)
    )
    .then_some(admitted)
}

pub fn admitted_free_name_collection_factory_at_call_span(
    il: &Il,
    interner: &Interner,
    occurrence: LibraryApiSpanCall,
    mut matches_name: impl FnMut(&str) -> bool,
) -> Option<AdmittedLibraryApiSpanCall<LibraryCollectionFactoryContract>> {
    library_free_name_collection_factory_contracts(il.meta.lang).find_map(|contract| {
        let LibraryApiCalleeContract::FreeName { name, .. } = contract.callee else {
            return None;
        };
        matches_name(name)
            .then(|| admitted_library_span_call(il, interner, occurrence, contract))
            .flatten()
    })
}

pub fn admitted_imported_collection_factory_at_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<AdmittedLibraryApiCall<LibraryCollectionFactoryContract>> {
    let (callee, args) = call_parts(il, call)?;
    let arg_count = args.len();
    library_imported_collection_factory_contracts(il.meta.lang).find_map(|contract| {
        admitted_library_call(il, interner, call, callee, None, arg_count, contract)
    })
}

pub fn admitted_imported_collection_factory_at_call_span(
    il: &Il,
    interner: &Interner,
    occurrence: LibraryApiSpanCall,
) -> Option<AdmittedLibraryApiSpanCall<LibraryCollectionFactoryContract>> {
    library_imported_collection_factory_contracts(il.meta.lang)
        .find_map(|contract| admitted_library_span_call(il, interner, occurrence, contract))
}

pub fn admitted_ruby_set_factory_at_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<AdmittedLibraryApiCall<LibraryCollectionFactoryContract>> {
    admitted_named_receiver_method_call(il, interner, call, |receiver_name, method, arg_count| {
        library_ruby_set_factory_contract(il.meta.lang, receiver_name, method, arg_count)
    })
}

pub fn admitted_ruby_set_factory_at_call_span(
    il: &Il,
    interner: &Interner,
    occurrence: LibraryApiSpanCall,
    method_hash: u64,
) -> Option<AdmittedLibraryApiSpanCall<LibraryCollectionFactoryContract>> {
    let contract = library_ruby_set_factory_contract_by_hash(il.meta.lang, "Set", method_hash, 1)?;
    admitted_library_span_call(il, interner, occurrence, contract)
}

pub fn admitted_rust_vec_macro_factory_at_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<AdmittedLibraryApiCall<LibraryCollectionFactoryContract>> {
    let (callee, args) = call_parts(il, call)?;
    let arg_count = args.len();
    let contract = library_rust_vec_macro_factory_contract(il.meta.lang, "vec")?;
    admitted_library_call(il, interner, call, callee, None, arg_count, contract)
}

pub fn admitted_rust_vec_macro_factory_at_call_span(
    il: &Il,
    interner: &Interner,
    occurrence: LibraryApiSpanCall,
) -> Option<AdmittedLibraryApiSpanCall<LibraryCollectionFactoryContract>> {
    let contract = library_rust_vec_macro_factory_contract(il.meta.lang, "vec")?;
    admitted_library_span_call(il, interner, occurrence, contract)
}

pub fn admitted_java_collection_factory_at_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<AdmittedLibraryApiCall<LibraryCollectionFactoryContract>> {
    admitted_named_receiver_method_call(il, interner, call, |receiver_name, method, _arg_count| {
        library_java_collection_factory_contract(il.meta.lang, receiver_name, method)
    })
}

pub fn admitted_java_collection_factory_at_call_span(
    il: &Il,
    interner: &Interner,
    occurrence: LibraryApiSpanCall,
    method_hash: u64,
) -> Option<AdmittedLibraryApiSpanCall<LibraryCollectionFactoryContract>> {
    [
        "List",
        "Set",
        "Arrays",
        "Collections",
        "ImmutableList",
        "ImmutableSet",
    ]
    .into_iter()
    .find_map(|receiver| {
        let contract =
            library_java_collection_factory_contract_by_hash(il.meta.lang, receiver, method_hash)?;
        admitted_library_span_call(il, interner, occurrence, contract)
    })
}

pub fn admitted_java_collection_constructor_at_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<AdmittedLibraryApiCall<LibraryCollectionFactoryContract>> {
    admitted_free_name_call(il, interner, call, |type_name, arg_count| {
        library_java_collection_constructor_contract(il.meta.lang, type_name, arg_count)
    })
}

pub fn admitted_js_like_set_constructor_at_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<AdmittedLibraryApiCall<LibraryCollectionFactoryContract>> {
    admitted_free_name_call(il, interner, call, |name, _arg_count| {
        library_js_like_set_constructor_contract(il.meta.lang, name)
    })
}

pub fn admitted_free_name_map_factory_at_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<AdmittedLibraryApiCall<LibraryMapFactoryContract>> {
    admitted_free_name_call(il, interner, call, |name, _arg_count| {
        library_free_name_map_factory_contract(il.meta.lang, name)
    })
}

pub fn admitted_swift_map_factory_at_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<AdmittedLibraryApiCall<LibraryMapFactoryContract>> {
    let (callee, args) = call_parts(il, call)?;
    let name = node_name(il, interner, callee)?;
    let first_label = call_first_argument_label(il, interner, args.first().copied()?)?;
    let contract = library_swift_map_factory_contract(il.meta.lang, name, first_label)?;
    admitted_library_call(il, interner, call, callee, None, args.len(), contract)
}

pub fn admitted_free_name_map_factory_at_call_span(
    il: &Il,
    interner: &Interner,
    occurrence: LibraryApiSpanCall,
    mut matches_name: impl FnMut(&str) -> bool,
) -> Option<AdmittedLibraryApiSpanCall<LibraryMapFactoryContract>> {
    library_free_name_map_factory_contracts(il.meta.lang).find_map(|contract| {
        let LibraryApiCalleeContract::FreeName { name, .. } = contract.callee else {
            return None;
        };
        matches_name(name)
            .then(|| admitted_library_span_call(il, interner, occurrence, contract))
            .flatten()
    })
}

fn call_first_argument_label<'a>(il: &Il, interner: &'a Interner, arg: NodeId) -> Option<&'a str> {
    if il.kind(arg) != NodeKind::KwArg {
        return None;
    }
    let Payload::Name(name) = il.node(arg).payload else {
        return None;
    };
    Some(interner.resolve(name))
}

pub fn admitted_java_map_factory_at_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<AdmittedLibraryApiCall<LibraryMapFactoryContract>> {
    admitted_named_receiver_method_call(il, interner, call, |receiver_name, method, _arg_count| {
        library_java_map_factory_contract(il.meta.lang, receiver_name, method)
    })
}

pub fn admitted_java_map_factory_at_call_span(
    il: &Il,
    interner: &Interner,
    occurrence: LibraryApiSpanCall,
    method_hash: u64,
) -> Option<AdmittedLibraryApiSpanCall<LibraryMapFactoryContract>> {
    ["Map", "Collections", "ImmutableMap"]
        .into_iter()
        .find_map(|receiver| {
            let contract =
                library_java_map_factory_contract_by_hash(il.meta.lang, receiver, method_hash)?;
            admitted_library_span_call(il, interner, occurrence, contract)
        })
}

pub fn admitted_java_map_entry_at_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<AdmittedLibraryApiCall<LibraryMapEntryFactoryContract>> {
    admitted_named_receiver_method_call(il, interner, call, |receiver_name, method, arg_count| {
        (arg_count == 2)
            .then(|| library_java_map_entry_contract(il.meta.lang, receiver_name, method))
            .flatten()
    })
}

pub fn admitted_java_map_entry_at_call_span(
    il: &Il,
    interner: &Interner,
    occurrence: LibraryApiSpanCall,
    method_hash: u64,
) -> Option<AdmittedLibraryApiSpanCall<LibraryMapEntryFactoryContract>> {
    if occurrence.arg_count != 2 {
        return None;
    }
    let contract = library_java_map_entry_contract_by_hash(il.meta.lang, "Map", method_hash)?;
    admitted_library_span_call(il, interner, occurrence, contract)
}

pub fn admitted_js_like_map_constructor_at_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<AdmittedLibraryApiCall<LibraryMapFactoryContract>> {
    admitted_free_name_call(il, interner, call, |name, _arg_count| {
        library_js_like_map_constructor_contract(il.meta.lang, name)
    })
}

receiver_method_contract_resolver!(
    admitted_imported_namespace_function_at_call,
    LibraryImportedNamespaceFunctionContract,
    library_imported_namespace_function_contract
);

receiver_method_contract_resolver!(
    admitted_promise_then_at_call,
    LibraryPromiseThenContract,
    library_promise_then_contract
);

pub fn admitted_promise_resolve_at_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<AdmittedLibraryApiCall<LibraryPromiseFactoryContract>> {
    admitted_named_receiver_method_call(il, interner, call, |receiver_name, method, arg_count| {
        library_promise_resolve_contract(il.meta.lang, receiver_name, method, arg_count)
    })
}

receiver_method_contract_resolver!(
    admitted_iterator_identity_adapter_at_call,
    LibraryIteratorIdentityAdapterContract,
    library_iterator_identity_adapter_contract
);

pub fn admitted_static_collection_adapter_at_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<AdmittedLibraryApiCall<LibraryStaticCollectionAdapterContract>> {
    admitted_named_receiver_method_call(il, interner, call, |receiver_name, method, arg_count| {
        library_static_collection_adapter_contract(il.meta.lang, receiver_name, method, arg_count)
    })
}

receiver_method_contract_resolver!(
    admitted_rust_option_and_then_at_call,
    LibraryRustOptionAndThenContract,
    library_rust_option_and_then_contract
);

pub fn admitted_rust_option_some_constructor_at_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<AdmittedLibraryApiCall<LibraryRustOptionConstructorContract>> {
    admitted_free_name_call(il, interner, call, |name, arg_count| {
        library_rust_option_some_constructor_contract(il.meta.lang, name, arg_count)
    })
}

pub fn admitted_rust_option_some_constructor_at_node(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> Option<AdmittedLibraryApiNode<LibraryRustOptionConstructorContract>> {
    let name = node_name(il, interner, node)?;
    let contract = library_rust_option_some_constructor_contract(il.meta.lang, name, 1)?;
    admitted_library_node(il, interner, node, None, 1, contract)
}

pub fn admitted_rust_vec_new_factory_at_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<AdmittedLibraryApiCall<LibraryCollectionFactoryContract>> {
    admitted_free_name_call(il, interner, call, |name, arg_count| {
        (arg_count == 0)
            .then(|| library_rust_vec_new_factory_contract(il.meta.lang, name))
            .flatten()
    })
}

pub fn admitted_rust_option_none_sentinel_at_node(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> Option<LibraryRustOptionSentinelContract> {
    let name = node_name(il, interner, node)?;
    let contract = library_rust_option_none_sentinel_contract(il.meta.lang, name)?;
    matches!(
        library_api_contract_evidence_for_node(il, interner, node, contract.id, contract.callee, 0),
        LibraryApiEvidenceStatus::Admitted
    )
    .then_some(contract)
}
