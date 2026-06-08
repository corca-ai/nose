//! Admitted API occurrence resolvers for semantic consumers.
//!
//! Contract row lookup and `LibraryApi` evidence admission are kept together here
//! so downstream consumers do not recombine raw selector parsing with proof checks.

use super::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AdmittedLibraryApiCall<C> {
    pub contract: C,
    pub callee: NodeId,
    pub receiver: Option<NodeId>,
    pub arg_count: usize,
}

pub fn admitted_library_method_call_at_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<AdmittedLibraryApiCall<LibraryMethodCallContract>> {
    let (callee, receiver, method, arg_count) = receiver_method_call_parts(il, interner, call)?;
    let contract = library_method_call_contract(il.meta.lang, method, arg_count)?;
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

pub fn admitted_map_get_at_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<AdmittedLibraryApiCall<LibraryMapGetContract>> {
    let (callee, receiver, method, arg_count) = receiver_method_call_parts(il, interner, call)?;
    let contract = library_map_get_contract(il.meta.lang, method, arg_count)?;
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

pub fn admitted_regex_test_at_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<AdmittedLibraryApiCall<LibraryRegexTestContract>> {
    let (callee, receiver, method, arg_count) = receiver_method_call_parts(il, interner, call)?;
    let contract = library_regex_test_contract(il.meta.lang, method, arg_count)?;
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

pub fn admitted_js_array_is_array_at_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<AdmittedLibraryApiCall<LibraryStaticGlobalMethodContract>> {
    let (callee, receiver, method, arg_count) = receiver_method_call_parts(il, interner, call)?;
    let receiver_name = node_name(il, interner, receiver)?;
    let contract =
        library_js_array_is_array_contract(il.meta.lang, receiver_name, method, arg_count)?;
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

pub fn admitted_static_index_membership_at_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<AdmittedLibraryApiCall<LibraryStaticIndexMembershipContract>> {
    let (callee, receiver, method, arg_count) = receiver_method_call_parts(il, interner, call)?;
    let contract = library_static_index_membership_contract(il.meta.lang, method, arg_count)?;
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

pub fn admitted_map_key_view_at_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<AdmittedLibraryApiCall<LibraryMapKeyViewContract>> {
    let (callee, receiver, method, arg_count) = receiver_method_call_parts(il, interner, call)?;
    let contract = library_map_key_view_contract(il.meta.lang, method, arg_count)?;
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

pub fn admitted_map_key_view_wrapper_at_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<AdmittedLibraryApiCall<LibraryMapKeyViewWrapperContract>> {
    let (callee, receiver, method, arg_count) = receiver_method_call_parts(il, interner, call)?;
    let contract = library_map_key_view_wrapper_contract(il.meta.lang, "Array", method, arg_count)?;
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

pub fn admitted_free_name_collection_factory_at_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<AdmittedLibraryApiCall<LibraryCollectionFactoryContract>> {
    let (callee, name, arg_count) = free_name_call_parts(il, interner, call)?;
    let contract = library_free_name_collection_factory_contract(il.meta.lang, name)?;
    admitted_library_call(il, interner, call, callee, None, arg_count, contract)
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

pub fn admitted_ruby_set_factory_at_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<AdmittedLibraryApiCall<LibraryCollectionFactoryContract>> {
    let (callee, receiver, method, arg_count) = receiver_method_call_parts(il, interner, call)?;
    let receiver_name = node_name(il, interner, receiver)?;
    let contract =
        library_ruby_set_factory_contract(il.meta.lang, receiver_name, method, arg_count)?;
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

pub fn admitted_java_collection_factory_at_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<AdmittedLibraryApiCall<LibraryCollectionFactoryContract>> {
    let (callee, receiver, method, arg_count) = receiver_method_call_parts(il, interner, call)?;
    let receiver_name = node_name(il, interner, receiver)?;
    let contract = library_java_collection_factory_contract(il.meta.lang, receiver_name, method)?;
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

pub fn admitted_java_collection_constructor_at_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<AdmittedLibraryApiCall<LibraryCollectionFactoryContract>> {
    let (callee, type_name, arg_count) = free_name_call_parts(il, interner, call)?;
    let contract =
        library_java_collection_constructor_contract(il.meta.lang, type_name, arg_count)?;
    admitted_library_call(il, interner, call, callee, None, arg_count, contract)
}

pub fn admitted_js_like_set_constructor_at_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<AdmittedLibraryApiCall<LibraryCollectionFactoryContract>> {
    let (callee, name, arg_count) = free_name_call_parts(il, interner, call)?;
    let contract = library_js_like_set_constructor_contract(il.meta.lang, name)?;
    admitted_library_call(il, interner, call, callee, None, arg_count, contract)
}

pub fn admitted_free_name_map_factory_at_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<AdmittedLibraryApiCall<LibraryMapFactoryContract>> {
    let (callee, name, arg_count) = free_name_call_parts(il, interner, call)?;
    let contract = library_free_name_map_factory_contract(il.meta.lang, name)?;
    admitted_library_call(il, interner, call, callee, None, arg_count, contract)
}

pub fn admitted_java_map_factory_at_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<AdmittedLibraryApiCall<LibraryMapFactoryContract>> {
    let (callee, receiver, method, arg_count) = receiver_method_call_parts(il, interner, call)?;
    let receiver_name = node_name(il, interner, receiver)?;
    let contract = library_java_map_factory_contract(il.meta.lang, receiver_name, method)?;
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

pub fn admitted_java_map_entry_at_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<AdmittedLibraryApiCall<LibraryMapEntryFactoryContract>> {
    let (callee, receiver, method, arg_count) = receiver_method_call_parts(il, interner, call)?;
    let receiver_name = node_name(il, interner, receiver)?;
    let contract = library_java_map_entry_contract(il.meta.lang, receiver_name, method)?;
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

pub fn admitted_js_like_map_constructor_at_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<AdmittedLibraryApiCall<LibraryMapFactoryContract>> {
    let (callee, name, arg_count) = free_name_call_parts(il, interner, call)?;
    let contract = library_js_like_map_constructor_contract(il.meta.lang, name)?;
    admitted_library_call(il, interner, call, callee, None, arg_count, contract)
}

pub fn admitted_imported_namespace_function_at_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<AdmittedLibraryApiCall<LibraryImportedNamespaceFunctionContract>> {
    let (callee, receiver, function, arg_count) = receiver_method_call_parts(il, interner, call)?;
    let contract = library_imported_namespace_function_contract(il.meta.lang, function, arg_count)?;
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

pub fn admitted_iterator_identity_adapter_at_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<AdmittedLibraryApiCall<LibraryIteratorIdentityAdapterContract>> {
    let (callee, receiver, method, arg_count) = receiver_method_call_parts(il, interner, call)?;
    let contract = library_iterator_identity_adapter_contract(il.meta.lang, method, arg_count)?;
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

pub fn admitted_rust_option_and_then_at_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<AdmittedLibraryApiCall<LibraryRustOptionAndThenContract>> {
    let (callee, receiver, method, arg_count) = receiver_method_call_parts(il, interner, call)?;
    let contract = library_rust_option_and_then_contract(il.meta.lang, method, arg_count)?;
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

pub fn admitted_rust_option_some_constructor_at_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<AdmittedLibraryApiCall<LibraryRustOptionConstructorContract>> {
    let (callee, name, arg_count) = free_name_call_parts(il, interner, call)?;
    let contract = library_rust_option_some_constructor_contract(il.meta.lang, name, arg_count)?;
    admitted_library_call(il, interner, call, callee, None, arg_count, contract)
}

pub fn admitted_rust_vec_new_factory_at_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<AdmittedLibraryApiCall<LibraryCollectionFactoryContract>> {
    let (callee, name, arg_count) = free_name_call_parts(il, interner, call)?;
    let contract = library_rust_vec_new_factory_contract(il.meta.lang, name)?;
    if arg_count != 0 {
        return None;
    }
    admitted_library_call(il, interner, call, callee, None, arg_count, contract)
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

impl LibraryApiContractParts for LibraryIteratorIdentityAdapterContract {
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
