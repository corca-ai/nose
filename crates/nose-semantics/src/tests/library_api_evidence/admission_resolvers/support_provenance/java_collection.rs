use super::super::*;

#[derive(Clone, Copy)]
pub(crate) enum JavaCollectionFactoryAdmission {
    Call,
    Span,
}

impl JavaCollectionFactoryAdmission {
    fn label(self) -> &'static str {
        match self {
            Self::Call => "Java List.of call resolver",
            Self::Span => "Java List.of span resolver",
        }
    }

    fn nodes(il: &Il, call: NodeId) -> (NodeId, NodeId) {
        let callee = il.children(call)[0];
        (callee, il.children(callee)[0])
    }

    fn occurrence(self, il: &Il, call: NodeId) -> LibraryApiSpanCall {
        let (callee, receiver) = Self::nodes(il, call);
        LibraryApiSpanCall {
            call_span: Some(il.node(call).span),
            callee_span: Some(il.node(callee).span),
            receiver_span: Some(il.node(receiver).span),
            arg_count: 1,
        }
    }

    fn assert_rejected(self, il: &Il, interner: &Interner, call: NodeId, reason: &str) {
        let rejected = match self {
            Self::Call => admitted_java_collection_factory_at_call(il, interner, call).is_none(),
            Self::Span => admitted_java_collection_factory_at_call_span(
                il,
                interner,
                self.occurrence(il, call),
                stable_symbol_hash("of"),
            )
            .is_none(),
        };
        assert!(rejected, "{}: {reason}", self.label());
    }

    fn assert_admitted(self, il: &Il, interner: &Interner, call: NodeId) {
        let (callee, receiver) = Self::nodes(il, call);
        match self {
            Self::Call => {
                let occurrence =
                    admitted_java_collection_factory_at_call(il, interner, call).unwrap();
                assert_eq!(
                    occurrence.contract.id,
                    LibraryApiContractId::JavaCollectionFactory(JavaCollectionFactoryKind::ListOf)
                );
                assert_eq!(occurrence.callee, callee);
                assert_eq!(occurrence.receiver, Some(receiver));
                assert_eq!(occurrence.arg_count, 1);
            }
            Self::Span => {
                let occurrence = admitted_java_collection_factory_at_call_span(
                    il,
                    interner,
                    self.occurrence(il, call),
                    stable_symbol_hash("of"),
                )
                .unwrap();
                assert_eq!(
                    occurrence.contract.id,
                    LibraryApiContractId::JavaCollectionFactory(JavaCollectionFactoryKind::ListOf)
                );
                assert_eq!(occurrence.call_span, Some(il.node(call).span));
                assert_eq!(occurrence.callee_span, Some(il.node(callee).span));
                assert_eq!(occurrence.receiver_span, Some(il.node(receiver).span));
                assert_eq!(occurrence.arg_count, 1);
            }
        }
    }
}

pub(crate) fn assert_java_collection_factory_requires_pack_provenance(
    admission: JavaCollectionFactoryAdmission,
) {
    let interner = Interner::new();

    let (mut raw, call, _root, _local, _contract) =
        java_list_of_import_evidence_il(&interner, true);
    raw.evidence.clear();
    admission.assert_rejected(&raw, &interner, call, "raw List.of shape alone is rejected");

    let (mut missing_dependency, call, _root, _local, contract) =
        java_list_of_import_evidence_il(&interner, true);
    missing_dependency.evidence.clear();
    missing_dependency
        .evidence
        .push(java_stdlib_collection_factory_record(
            0,
            missing_dependency.node(call).span,
            contract,
            1,
            EvidenceStatus::Asserted,
            &[],
        ));
    admission.assert_rejected(
        &missing_dependency,
        &interner,
        call,
        "same-span List.of evidence without import dependency is rejected",
    );

    let (mut wrong_pack, call, _root, _local, contract) =
        java_list_of_import_evidence_il(&interner, true);
    wrong_pack
        .evidence
        .retain(|record| record.id != EvidenceId(2));
    wrong_pack
        .evidence
        .push(library_api_record_with_provenance_and_arity(
            2,
            wrong_pack.node(call).span,
            contract.id,
            contract.callee,
            1,
            EvidenceStatus::Asserted,
            &[1],
            BUILTIN_COMPAT_PACK_ID,
            JAVA_STDLIB_COLLECTION_FACTORY_PRODUCER_ID,
        ));
    admission.assert_rejected(
        &wrong_pack,
        &interner,
        call,
        "compatibility-pack List.of evidence is rejected",
    );

    let (mut wrong_producer, call, _root, _local, contract) =
        java_list_of_import_evidence_il(&interner, true);
    wrong_producer
        .evidence
        .retain(|record| record.id != EvidenceId(2));
    wrong_producer
        .evidence
        .push(library_api_record_with_provenance_and_arity(
            2,
            wrong_producer.node(call).span,
            contract.id,
            contract.callee,
            1,
            EvidenceStatus::Asserted,
            &[1],
            JAVA_STDLIB_COLLECTION_FACTORY_PACK_ID,
            "wrong.java.stdlib.collection-factory-api",
        ));
    admission.assert_rejected(
        &wrong_producer,
        &interner,
        call,
        "wrong-producer List.of evidence is rejected",
    );

    let (admitted, call, _root, _local, _contract) =
        java_list_of_import_evidence_il(&interner, true);
    admission.assert_admitted(&admitted, &interner, call);
}
