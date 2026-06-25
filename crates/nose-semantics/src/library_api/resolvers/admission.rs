use super::*;
use nose_il::Lang;

pub(super) fn admitted_receiver_method_contract_call<C: LibraryApiContractParts>(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    contract_for: fn(Lang, &str, usize) -> Option<C>,
) -> Option<AdmittedLibraryApiCall<C>> {
    admitted_receiver_method_call(il, interner, call, |method, arg_count| {
        contract_for(il.meta.lang, method, arg_count)
    })
}

pub(super) fn admitted_receiver_method_contract_call_candidates<C: LibraryApiContractParts>(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    contract_for: fn(Lang, &str, usize) -> Vec<C>,
) -> Option<AdmittedLibraryApiCall<C>> {
    let (callee, receiver, method, arg_count) = receiver_method_call_parts(il, interner, call)?;
    contract_for(il.meta.lang, method, arg_count)
        .into_iter()
        .find_map(|contract| {
            admitted_library_call(
                il,
                interner,
                call,
                callee,
                Some(receiver),
                arg_count,
                contract,
            )
        })
}

pub(super) fn admitted_receiver_method_call<C: LibraryApiContractParts>(
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

pub(super) fn admitted_named_receiver_method_call<C: LibraryApiContractParts>(
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

pub(super) fn admitted_free_name_call<C: LibraryApiContractParts>(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    contract_for: impl FnOnce(&str, usize) -> Option<C>,
) -> Option<AdmittedLibraryApiCall<C>> {
    let (callee, name, arg_count) = free_name_call_parts(il, interner, call)?;
    let contract = contract_for(name, arg_count)?;
    admitted_library_call(il, interner, call, callee, None, arg_count, contract)
}

pub(super) fn admitted_library_call<C: LibraryApiContractParts>(
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

pub(super) fn admitted_library_node<C: LibraryApiContractParts>(
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

pub(super) fn admitted_library_span_call<C: LibraryApiContractParts>(
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

pub(super) fn call_parts(il: &Il, call: NodeId) -> Option<(NodeId, &[NodeId])> {
    if il.kind(call) != NodeKind::Call {
        return None;
    }
    let (callee, args) = il.children(call).split_first()?;
    Some((*callee, args))
}

pub(super) trait LibraryApiContractParts: Copy {
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

impl LibraryApiContractParts for LibraryFreeFunctionHofContract {
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

impl LibraryApiContractParts for LibraryPromiseFactoryContract {
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

impl LibraryApiContractParts for LibraryRustResultConstructorContract {
    fn contract_id(self) -> LibraryApiContractId {
        self.id
    }

    fn callee_contract(self) -> LibraryApiCalleeContract {
        self.callee
    }
}

impl LibraryApiContractParts for LibraryReceiverMethodApiContract {
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
