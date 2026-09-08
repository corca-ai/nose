use super::super::*;

#[derive(Clone, Copy)]
pub(in crate::value_graph::tests) enum PromiseContinuation {
    Then,
    Catch,
    Finally,
}

/// Test-only evidence writer for Promise settlement scenarios.
///
/// Fixture builders own IL shape; this DSL owns the dependency-closed evidence
/// required to admit that shape. Assertions intentionally remain in each test.
pub(in crate::value_graph::tests) struct PromiseEvidenceDsl<'a> {
    il: &'a mut Il,
    interner: &'a Interner,
    next_id: u32,
}

impl<'a> PromiseEvidenceDsl<'a> {
    pub(in crate::value_graph::tests) fn new(il: &'a mut Il, interner: &'a Interner) -> Self {
        let next_id = il
            .evidence
            .iter()
            .map(|record| record.id.0)
            .max()
            .map_or(0, |id| id + 1);
        Self {
            il,
            interner,
            next_id,
        }
    }

    pub(in crate::value_graph::tests) fn factory(
        &mut self,
        call: NodeId,
        channel: PromiseSettlementChannel,
    ) {
        let method = match channel {
            PromiseSettlementChannel::Fulfilled => "resolve",
            PromiseSettlementChannel::Rejected => "reject",
        };
        let base_id = self.allocate(5);
        push_promise_factory_evidence(self.il, call, base_id, method);
    }

    pub(in crate::value_graph::tests) fn continuation(
        &mut self,
        call: NodeId,
        continuation: PromiseContinuation,
    ) {
        let id = self.allocate(1);
        push_promise_continuation_evidence(self.il, self.interner, call, id, continuation);
    }

    pub(in crate::value_graph::tests) fn promise_like(&mut self, node: NodeId) {
        self.domain(node, DomainEvidence::PromiseLike);
    }

    pub(in crate::value_graph::tests) fn domain(&mut self, node: NodeId, domain: DomainEvidence) {
        let id = self.allocate(1);
        self.il.push_evidence(evidence(
            id,
            EvidenceAnchor::node(self.il.node(node).span, self.il.kind(node)),
            EvidenceKind::Domain(domain),
        ));
    }

    pub(in crate::value_graph::tests) fn imported_settlement(
        &mut self,
        call: NodeId,
        payload: NodeId,
        channel: PromiseSettlementChannel,
    ) {
        let base_id = self.allocate(3);
        push_imported_function_promise_settlement_evidence(
            self.il,
            self.interner,
            call,
            payload,
            channel,
            base_id,
        );
    }

    fn allocate(&mut self, width: u32) -> u32 {
        let id = self.next_id;
        self.next_id += width;
        id
    }
}

fn push_promise_factory_evidence(il: &mut Il, call: NodeId, base_id: u32, method: &str) {
    let [callee, _arg] = il.children(call) else {
        panic!("Promise factory test call must have one argument");
    };
    let callee = *callee;
    let [promise] = il.children(callee) else {
        panic!("Promise factory test callee must have Promise receiver");
    };
    let promise = *promise;
    let callee_span = il.node(callee).span;
    let promise_span = il.node(promise).span;
    let call_span = il.node(call).span;
    let root_id = EvidenceId(base_id);
    let qualified_id = EvidenceId(base_id + 1);
    let receiver_id = EvidenceId(base_id + 2);
    let api_id = EvidenceId(base_id + 3);
    let qualified_path = match method {
        "resolve" => "Promise.resolve",
        "reject" => "Promise.reject",
        _ => panic!("unsupported Promise factory test method"),
    };
    il.push_evidence(language_core_symbol_evidence(
        root_id.0,
        Lang::JavaScript,
        EvidenceAnchor::source_span(callee_span),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Promise"),
        },
    ));
    il.push_evidence(evidence_with_dependencies(
        qualified_id.0,
        EvidenceAnchor::node(callee_span, NodeKind::Field),
        EvidenceKind::Symbol(SymbolEvidenceKind::QualifiedGlobal {
            path_hash: stable_symbol_hash(qualified_path),
        }),
        vec![root_id],
    ));
    il.push_evidence(language_core_symbol_evidence(
        receiver_id.0,
        Lang::JavaScript,
        EvidenceAnchor::node(promise_span, NodeKind::Var),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Promise"),
        },
    ));
    let contract = library_promise_resolve_contract(il.meta.lang, "Promise", method, 1).unwrap();
    il.push_evidence(js_like_promise_evidence_with_dependencies(
        api_id.0,
        EvidenceAnchor::node(call_span, NodeKind::Call),
        EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
            contract_hash: library_api_contract_id_hash(contract.id),
            callee_hash: library_api_callee_contract_hash(contract.callee),
            arity: 1,
        }),
        vec![qualified_id, receiver_id],
    ));
    il.push_evidence(evidence_with_dependencies(
        base_id + 4,
        EvidenceAnchor::node(call_span, NodeKind::Call),
        EvidenceKind::Domain(DomainEvidence::PromiseLike),
        vec![api_id],
    ));
}

fn push_promise_continuation_evidence(
    il: &mut Il,
    interner: &Interner,
    call: NodeId,
    id: u32,
    continuation: PromiseContinuation,
) {
    let arg_count = il.children(call).len().saturating_sub(1);
    let (contract_id, callee) = match continuation {
        PromiseContinuation::Then => {
            let contract = library_promise_then_contract(il.meta.lang, "then", arg_count).unwrap();
            (contract.id, contract.callee)
        }
        PromiseContinuation::Catch => {
            let contract =
                library_promise_catch_contract(il.meta.lang, "catch", arg_count).unwrap();
            (contract.id, contract.callee)
        }
        PromiseContinuation::Finally => {
            let contract =
                library_promise_finally_contract(il.meta.lang, "finally", arg_count).unwrap();
            (contract.id, contract.callee)
        }
    };
    let dependencies =
        nose_semantics::library_api_receiver_dependencies_for_call(il, interner, call, callee)
            .expect("Promise continuation receiver dependencies");
    il.push_evidence(js_like_promise_evidence_with_dependencies(
        id,
        EvidenceAnchor::node(il.node(call).span, NodeKind::Call),
        EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
            contract_hash: library_api_contract_id_hash(contract_id),
            callee_hash: library_api_callee_contract_hash(callee),
            arity: arg_count as u16,
        }),
        dependencies,
    ));
}

fn push_imported_function_promise_settlement_evidence(
    il: &mut Il,
    interner: &Interner,
    call: NodeId,
    payload: NodeId,
    channel: PromiseSettlementChannel,
    base_id: u32,
) {
    let [callee, ..] = il.children(call) else {
        panic!("imported Promise producer test call must have a callee");
    };
    let Payload::Name(local) = il.node(*callee).payload else {
        panic!("imported Promise producer test callee must be a named local");
    };
    let call_span = il.node(call).span;
    let target_id = EvidenceId(base_id);
    let domain_id = EvidenceId(base_id + 1);
    il.push_evidence(language_core_evidence(
        target_id.0,
        il.meta.lang,
        EvidenceAnchor::node(call_span, NodeKind::Call),
        EvidenceKind::CallTarget(CallTargetEvidenceKind::ImportedFunction {
            module_hash: stable_symbol_hash("./service"),
            exported_hash: stable_symbol_hash("load"),
            local_hash: interner.symbol_hash(local),
        }),
    ));
    il.push_evidence(evidence_with_dependencies(
        domain_id.0,
        EvidenceAnchor::node(call_span, NodeKind::Call),
        EvidenceKind::Domain(DomainEvidence::PromiseLike),
        vec![target_id],
    ));
    il.push_evidence(js_like_promise_evidence_with_dependencies(
        base_id + 2,
        EvidenceAnchor::node(call_span, NodeKind::Call),
        EvidenceKind::PromiseSettledValue(PromiseSettledValueEvidenceKind {
            channel,
            payload_span: il.node(payload).span,
            payload_kind: il.kind(payload),
        }),
        vec![target_id, domain_id],
    ));
}
