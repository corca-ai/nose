//! Admitted API occurrence resolvers for semantic consumers.
//!
//! Contract row lookup and `LibraryApi` evidence admission are kept together here
//! so downstream consumers do not recombine raw selector parsing with proof checks.

use super::*;
use nose_il::Lang;

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
    ["List", "Set", "Arrays"].into_iter().find_map(|receiver| {
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
    let contract = library_java_map_factory_contract_by_hash(il.meta.lang, "Map", method_hash)?;
    admitted_library_span_call(il, interner, occurrence, contract)
}

pub fn admitted_java_map_entry_at_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<AdmittedLibraryApiCall<LibraryMapEntryFactoryContract>> {
    admitted_named_receiver_method_call(il, interner, call, |receiver_name, method, _arg_count| {
        library_java_map_entry_contract(il.meta.lang, receiver_name, method)
    })
}

pub fn admitted_java_map_entry_at_call_span(
    il: &Il,
    interner: &Interner,
    occurrence: LibraryApiSpanCall,
    method_hash: u64,
) -> Option<AdmittedLibraryApiSpanCall<LibraryMapEntryFactoryContract>> {
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

fn admitted_receiver_method_contract_call<C: LibraryApiContractParts>(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    contract_for: fn(Lang, &str, usize) -> Option<C>,
) -> Option<AdmittedLibraryApiCall<C>> {
    admitted_receiver_method_call(il, interner, call, |method, arg_count| {
        contract_for(il.meta.lang, method, arg_count)
    })
}

fn admitted_receiver_method_call<C: LibraryApiContractParts>(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    contract_for: impl FnOnce(&str, usize) -> Option<C>,
) -> Option<AdmittedLibraryApiCall<C>> {
    let (callee, receiver, method, arg_count) = receiver_method_call_parts(il, interner, call)?;
    let contract = contract_for(method, arg_count)?;
    admitted_library_call(
        il,
        interner,
        call,
        callee,
        Some(receiver),
        arg_count,
        contract,
    )
}

fn admitted_named_receiver_method_call<C: LibraryApiContractParts>(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    contract_for: impl FnOnce(&str, &str, usize) -> Option<C>,
) -> Option<AdmittedLibraryApiCall<C>> {
    let (callee, receiver, method, arg_count) = receiver_method_call_parts(il, interner, call)?;
    let receiver_name = node_name(il, interner, receiver)?;
    let contract = contract_for(receiver_name, method, arg_count)?;
    admitted_library_call(
        il,
        interner,
        call,
        callee,
        Some(receiver),
        arg_count,
        contract,
    )
}

fn admitted_free_name_call<C: LibraryApiContractParts>(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    contract_for: impl FnOnce(&str, usize) -> Option<C>,
) -> Option<AdmittedLibraryApiCall<C>> {
    let (callee, name, arg_count) = free_name_call_parts(il, interner, call)?;
    let contract = contract_for(name, arg_count)?;
    admitted_library_call(il, interner, call, callee, None, arg_count, contract)
}

fn admitted_library_call<C: LibraryApiContractParts>(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    callee: NodeId,
    receiver: Option<NodeId>,
    arg_count: usize,
    contract: C,
) -> Option<AdmittedLibraryApiCall<C>> {
    matches!(
        library_api_contract_evidence_for_call(
            il,
            interner,
            call,
            contract.contract_id(),
            contract.callee_contract(),
            arg_count,
        ),
        LibraryApiEvidenceStatus::Admitted
    )
    .then_some(AdmittedLibraryApiCall {
        contract,
        callee,
        receiver,
        arg_count,
    })
}

fn admitted_library_node<C: LibraryApiContractParts>(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    receiver: Option<NodeId>,
    arg_count: usize,
    contract: C,
) -> Option<AdmittedLibraryApiNode<C>> {
    matches!(
        library_api_contract_evidence_for_node(
            il,
            interner,
            node,
            contract.contract_id(),
            contract.callee_contract(),
            arg_count,
        ),
        LibraryApiEvidenceStatus::Admitted
    )
    .then_some(AdmittedLibraryApiNode {
        contract,
        node,
        receiver,
        arg_count,
    })
}

fn admitted_library_span_call<C: LibraryApiContractParts>(
    il: &Il,
    interner: &Interner,
    occurrence: LibraryApiSpanCall,
    contract: C,
) -> Option<AdmittedLibraryApiSpanCall<C>> {
    matches!(
        library_api_contract_evidence_at_call_span(
            il,
            interner,
            LibraryApiSpanEvidenceQuery {
                call_span: occurrence.call_span,
                callee_span: occurrence.callee_span,
                receiver_span: occurrence.receiver_span,
                id: contract.contract_id(),
                callee: contract.callee_contract(),
                arg_count: occurrence.arg_count,
            },
        ),
        LibraryApiEvidenceStatus::Admitted
    )
    .then_some(AdmittedLibraryApiSpanCall {
        contract,
        call_span: occurrence.call_span,
        callee_span: occurrence.callee_span,
        receiver_span: occurrence.receiver_span,
        arg_count: occurrence.arg_count,
    })
}

trait LibraryApiContractParts: Copy {
    fn contract_id(self) -> LibraryApiContractId;
    fn callee_contract(self) -> LibraryApiCalleeContract;
}

impl LibraryApiContractParts for LibraryMethodCallContract {
    fn contract_id(self) -> LibraryApiContractId {
        self.id
    }

    fn callee_contract(self) -> LibraryApiCalleeContract {
        self.callee
    }
}

impl LibraryApiContractParts for LibraryFreeFunctionBuiltinContract {
    fn contract_id(self) -> LibraryApiContractId {
        self.id
    }

    fn callee_contract(self) -> LibraryApiCalleeContract {
        self.callee
    }
}

impl LibraryApiContractParts for LibraryPropertyBuiltinContract {
    fn contract_id(self) -> LibraryApiContractId {
        self.id
    }

    fn callee_contract(self) -> LibraryApiCalleeContract {
        self.callee
    }
}

impl LibraryApiContractParts for LibraryMapGetContract {
    fn contract_id(self) -> LibraryApiContractId {
        self.id
    }

    fn callee_contract(self) -> LibraryApiCalleeContract {
        self.callee
    }
}

impl LibraryApiContractParts for LibraryRegexTestContract {
    fn contract_id(self) -> LibraryApiContractId {
        self.id
    }

    fn callee_contract(self) -> LibraryApiCalleeContract {
        self.callee
    }
}

impl LibraryApiContractParts for LibraryStaticGlobalMethodContract {
    fn contract_id(self) -> LibraryApiContractId {
        self.id
    }

    fn callee_contract(self) -> LibraryApiCalleeContract {
        self.callee
    }
}

impl LibraryApiContractParts for LibraryStaticIndexMembershipContract {
    fn contract_id(self) -> LibraryApiContractId {
        self.id
    }

    fn callee_contract(self) -> LibraryApiCalleeContract {
        self.callee
    }
}

impl LibraryApiContractParts for LibraryScalarIntegerMethodContract {
    fn contract_id(self) -> LibraryApiContractId {
        self.id
    }

    fn callee_contract(self) -> LibraryApiCalleeContract {
        self.callee
    }
}

impl LibraryApiContractParts for LibraryMapKeyViewContract {
    fn contract_id(self) -> LibraryApiContractId {
        self.id
    }

    fn callee_contract(self) -> LibraryApiCalleeContract {
        self.callee
    }
}

impl LibraryApiContractParts for LibraryMapKeyViewWrapperContract {
    fn contract_id(self) -> LibraryApiContractId {
        self.id
    }

    fn callee_contract(self) -> LibraryApiCalleeContract {
        self.callee
    }
}

impl LibraryApiContractParts for LibraryImportedNamespaceFunctionContract {
    fn contract_id(self) -> LibraryApiContractId {
        self.id
    }

    fn callee_contract(self) -> LibraryApiCalleeContract {
        self.callee
    }
}

impl LibraryApiContractParts for LibraryPromiseThenContract {
    fn contract_id(self) -> LibraryApiContractId {
        self.id
    }

    fn callee_contract(self) -> LibraryApiCalleeContract {
        self.callee
    }
}

impl LibraryApiContractParts for LibraryIteratorIdentityAdapterContract {
    fn contract_id(self) -> LibraryApiContractId {
        self.id
    }

    fn callee_contract(self) -> LibraryApiCalleeContract {
        self.callee
    }
}

impl LibraryApiContractParts for LibraryStaticCollectionAdapterContract {
    fn contract_id(self) -> LibraryApiContractId {
        self.id
    }

    fn callee_contract(self) -> LibraryApiCalleeContract {
        self.callee
    }
}

impl LibraryApiContractParts for LibraryRustOptionAndThenContract {
    fn contract_id(self) -> LibraryApiContractId {
        self.id
    }

    fn callee_contract(self) -> LibraryApiCalleeContract {
        self.callee
    }
}

impl LibraryApiContractParts for LibraryRustOptionConstructorContract {
    fn contract_id(self) -> LibraryApiContractId {
        self.id
    }

    fn callee_contract(self) -> LibraryApiCalleeContract {
        self.callee
    }
}

impl LibraryApiContractParts for LibraryCollectionFactoryContract {
    fn contract_id(self) -> LibraryApiContractId {
        self.id
    }

    fn callee_contract(self) -> LibraryApiCalleeContract {
        self.callee
    }
}

impl LibraryApiContractParts for LibraryMapFactoryContract {
    fn contract_id(self) -> LibraryApiContractId {
        self.id
    }

    fn callee_contract(self) -> LibraryApiCalleeContract {
        self.callee
    }
}

impl LibraryApiContractParts for LibraryMapEntryFactoryContract {
    fn contract_id(self) -> LibraryApiContractId {
        self.id
    }

    fn callee_contract(self) -> LibraryApiCalleeContract {
        self.callee
    }
}

fn receiver_method_call_parts<'a>(
    il: &Il,
    interner: &'a Interner,
    call: NodeId,
) -> Option<(NodeId, NodeId, &'a str, usize)> {
    if il.kind(call) != NodeKind::Call {
        return None;
    }
    let (callee, args) = il.children(call).split_first()?;
    if il.kind(*callee) != NodeKind::Field {
        return None;
    }
    let Payload::Name(method) = il.node(*callee).payload else {
        return None;
    };
    let receiver = *il.children(*callee).first()?;
    Some((*callee, receiver, interner.resolve(method), args.len()))
}

fn free_name_call_parts<'a>(
    il: &Il,
    interner: &'a Interner,
    call: NodeId,
) -> Option<(NodeId, &'a str, usize)> {
    let (callee, args) = call_parts(il, call)?;
    Some((callee, node_name(il, interner, callee)?, args.len()))
}

fn call_parts(il: &Il, call: NodeId) -> Option<(NodeId, &[NodeId])> {
    if il.kind(call) != NodeKind::Call {
        return None;
    }
    let (callee, args) = il.children(call).split_first()?;
    Some((*callee, args))
}
