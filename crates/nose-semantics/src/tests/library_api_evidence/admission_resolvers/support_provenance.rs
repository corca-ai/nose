use super::*;

mod java_collection;
pub(crate) use java_collection::*;
mod rust_option;
pub(crate) use rust_option::*;

#[derive(Clone, Copy)]
pub(crate) enum RustStdMapFactoryAdmission {
    Call,
    Span,
}

impl RustStdMapFactoryAdmission {
    fn label(self) -> &'static str {
        match self {
            Self::Call => "Rust stdlib map factory call resolver",
            Self::Span => "Rust stdlib map factory span resolver",
        }
    }

    fn occurrence(self, il: &Il, call: NodeId, callee: NodeId) -> LibraryApiSpanCall {
        LibraryApiSpanCall {
            call_span: Some(il.node(call).span),
            callee_span: Some(il.node(callee).span),
            receiver_span: None,
            arg_count: 1,
        }
    }

    fn assert_rejected(
        self,
        il: &Il,
        interner: &Interner,
        call: NodeId,
        callee: NodeId,
        reason: &str,
    ) {
        let rejected = match self {
            Self::Call => admitted_free_name_map_factory_at_call(il, interner, call).is_none(),
            Self::Span => admitted_free_name_map_factory_at_call_span(
                il,
                interner,
                self.occurrence(il, call, callee),
                |name| name == "std::collections::HashMap::from",
            )
            .is_none(),
        };
        assert!(rejected, "{}: {reason}", self.label());
    }

    fn assert_admitted(self, il: &Il, interner: &Interner, call: NodeId, callee: NodeId) {
        match self {
            Self::Call => {
                let occurrence =
                    admitted_free_name_map_factory_at_call(il, interner, call).unwrap();
                assert_eq!(
                    occurrence.contract.id,
                    LibraryApiContractId::RustStdMapFactory
                );
                assert_eq!(occurrence.callee, callee);
                assert_eq!(occurrence.receiver, None);
                assert_eq!(occurrence.arg_count, 1);
            }
            Self::Span => {
                let occurrence = admitted_free_name_map_factory_at_call_span(
                    il,
                    interner,
                    self.occurrence(il, call, callee),
                    |name| name == "std::collections::HashMap::from",
                )
                .unwrap();
                assert_eq!(
                    occurrence.contract.id,
                    LibraryApiContractId::RustStdMapFactory
                );
                assert_eq!(occurrence.call_span, Some(il.node(call).span));
                assert_eq!(occurrence.callee_span, Some(il.node(callee).span));
                assert_eq!(occurrence.receiver_span, None);
                assert_eq!(occurrence.arg_count, 1);
            }
        }
    }
}

pub(crate) fn assert_rust_std_map_factory_requires_pack_provenance(
    admission: RustStdMapFactoryAdmission,
) {
    let (il, interner, call, callee) = rust_std_map_factory_call_il();
    admission.assert_rejected(
        &il,
        &interner,
        call,
        callee,
        "raw std::collections HashMap::from shape alone is rejected",
    );

    let contract =
        library_free_name_map_factory_contract(Lang::Rust, "std::collections::HashMap::from")
            .expect("Rust std::collections HashMap::from contract");

    let (mut missing_dependency, interner, call, callee) = rust_std_map_factory_call_il();
    missing_dependency.push_evidence(rust_stdlib_map_factory_record(
        0,
        missing_dependency.node(call).span,
        contract,
        EvidenceStatus::Asserted,
        &[],
    ));
    admission.assert_rejected(
        &missing_dependency,
        &interner,
        call,
        callee,
        "same-span stdlib map evidence without callee dependency is rejected",
    );

    let (mut wrong_pack, interner, call, callee) = rust_std_map_factory_call_il();
    push_rust_std_map_factory_symbol_dependency(&mut wrong_pack, callee);
    wrong_pack.push_evidence(library_api_record_with_provenance(
        1,
        wrong_pack.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[0],
        BUILTIN_COMPAT_PACK_ID,
        RUST_STDLIB_MAP_FACTORY_PRODUCER_ID,
    ));
    admission.assert_rejected(
        &wrong_pack,
        &interner,
        call,
        callee,
        "compatibility-pack stdlib map evidence is rejected",
    );

    let (mut wrong_producer, interner, call, callee) = rust_std_map_factory_call_il();
    push_rust_std_map_factory_symbol_dependency(&mut wrong_producer, callee);
    wrong_producer.push_evidence(library_api_record_with_provenance(
        1,
        wrong_producer.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[0],
        RUST_STDLIB_MAP_FACTORY_PACK_ID,
        "wrong.rust.stdlib.map-factory-api",
    ));
    admission.assert_rejected(
        &wrong_producer,
        &interner,
        call,
        callee,
        "wrong-producer stdlib map evidence is rejected",
    );

    let (mut admitted, interner, call, callee) = rust_std_map_factory_call_il();
    push_rust_std_map_factory_symbol_dependency(&mut admitted, callee);
    admitted.push_evidence(rust_stdlib_map_factory_record(
        1,
        admitted.node(call).span,
        contract,
        EvidenceStatus::Asserted,
        &[0],
    ));
    admission.assert_admitted(&admitted, &interner, call, callee);
}

fn push_rust_std_map_factory_symbol_dependency(il: &mut Il, callee: NodeId) {
    il.push_evidence(language_core_symbol_record(
        0,
        EvidenceAnchor::node(il.node(callee).span, NodeKind::Var),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("std::collections::HashMap::from"),
        },
        EvidenceStatus::Asserted,
        &[],
        Lang::Rust,
    ));
}

#[derive(Clone, Copy)]
pub(crate) enum JavaMapFactoryAdmission {
    Call,
    Span,
}

impl JavaMapFactoryAdmission {
    fn label(self) -> &'static str {
        match self {
            Self::Call => "Java Map.of call resolver",
            Self::Span => "Java Map.of span resolver",
        }
    }

    fn occurrence(
        self,
        il: &Il,
        call: NodeId,
        callee: NodeId,
        receiver: NodeId,
    ) -> LibraryApiSpanCall {
        LibraryApiSpanCall {
            call_span: Some(il.node(call).span),
            callee_span: Some(il.node(callee).span),
            receiver_span: Some(il.node(receiver).span),
            arg_count: 2,
        }
    }

    fn assert_rejected(
        self,
        il: &Il,
        interner: &Interner,
        call: NodeId,
        callee: NodeId,
        receiver: NodeId,
        reason: &str,
    ) {
        let rejected = match self {
            Self::Call => admitted_java_map_factory_at_call(il, interner, call).is_none(),
            Self::Span => admitted_java_map_factory_at_call_span(
                il,
                interner,
                self.occurrence(il, call, callee, receiver),
                stable_symbol_hash("of"),
            )
            .is_none(),
        };
        assert!(rejected, "{}: {reason}", self.label());
    }

    fn assert_admitted(
        self,
        il: &Il,
        interner: &Interner,
        call: NodeId,
        callee: NodeId,
        receiver: NodeId,
    ) {
        match self {
            Self::Call => {
                let occurrence = admitted_java_map_factory_at_call(il, interner, call).unwrap();
                assert_eq!(
                    occurrence.contract.id,
                    LibraryApiContractId::JavaMapFactory(JavaMapFactoryKind::Of)
                );
                assert_eq!(occurrence.callee, callee);
                assert_eq!(occurrence.receiver, Some(receiver));
                assert_eq!(occurrence.arg_count, 2);
            }
            Self::Span => {
                let occurrence = admitted_java_map_factory_at_call_span(
                    il,
                    interner,
                    self.occurrence(il, call, callee, receiver),
                    stable_symbol_hash("of"),
                )
                .unwrap();
                assert_eq!(
                    occurrence.contract.id,
                    LibraryApiContractId::JavaMapFactory(JavaMapFactoryKind::Of)
                );
                assert_eq!(occurrence.call_span, Some(il.node(call).span));
                assert_eq!(occurrence.callee_span, Some(il.node(callee).span));
                assert_eq!(occurrence.receiver_span, Some(il.node(receiver).span));
                assert_eq!(occurrence.arg_count, 2);
            }
        }
    }
}

pub(crate) fn assert_java_map_factory_requires_pack_provenance(admission: JavaMapFactoryAdmission) {
    let (il, interner, call, callee, receiver) = java_map_factory_call_il();
    admission.assert_rejected(
        &il,
        &interner,
        call,
        callee,
        receiver,
        "raw Map.of shape alone is rejected",
    );

    let contract =
        library_java_map_factory_contract(Lang::Java, "Map", "of").expect("Map.of contract");

    let (mut missing_dependency, interner, call, callee, receiver) = java_map_factory_call_il();
    missing_dependency.push_evidence(java_stdlib_map_factory_record(
        0,
        missing_dependency.node(call).span,
        contract,
        2,
        EvidenceStatus::Asserted,
        &[],
    ));
    admission.assert_rejected(
        &missing_dependency,
        &interner,
        call,
        callee,
        receiver,
        "same-span Map.of evidence without import dependency is rejected",
    );

    let (mut wrong_pack, interner, call, callee, receiver) = java_map_factory_call_il();
    push_java_map_import_dependencies(&mut wrong_pack, receiver);
    wrong_pack.push_evidence(library_api_record_with_provenance_and_arity(
        2,
        wrong_pack.node(call).span,
        contract.id,
        contract.callee,
        2,
        EvidenceStatus::Asserted,
        &[1],
        BUILTIN_COMPAT_PACK_ID,
        JAVA_STDLIB_MAP_FACTORY_PRODUCER_ID,
    ));
    admission.assert_rejected(
        &wrong_pack,
        &interner,
        call,
        callee,
        receiver,
        "compatibility-pack Map.of evidence is rejected",
    );

    let (mut wrong_producer, interner, call, callee, receiver) = java_map_factory_call_il();
    push_java_map_import_dependencies(&mut wrong_producer, receiver);
    wrong_producer.push_evidence(library_api_record_with_provenance_and_arity(
        2,
        wrong_producer.node(call).span,
        contract.id,
        contract.callee,
        2,
        EvidenceStatus::Asserted,
        &[1],
        JAVA_STDLIB_MAP_FACTORY_PACK_ID,
        "wrong.java.stdlib.map-factory-api",
    ));
    admission.assert_rejected(
        &wrong_producer,
        &interner,
        call,
        callee,
        receiver,
        "wrong-producer Map.of evidence is rejected",
    );

    let (mut admitted, interner, call, callee, receiver) = java_map_factory_call_il();
    push_java_map_import_dependencies(&mut admitted, receiver);
    admitted.push_evidence(java_stdlib_map_factory_record(
        2,
        admitted.node(call).span,
        contract,
        2,
        EvidenceStatus::Asserted,
        &[1],
    ));
    admission.assert_admitted(&admitted, &interner, call, callee, receiver);
}

#[derive(Clone, Copy)]
pub(crate) enum JsCollectionConstructorKind {
    Set,
    Map,
}

impl JsCollectionConstructorKind {
    fn name(self) -> &'static str {
        match self {
            Self::Set => "Set",
            Self::Map => "Map",
        }
    }

    fn expected_id(self) -> LibraryApiContractId {
        match self {
            Self::Set => LibraryApiContractId::JsLikeSetConstructor,
            Self::Map => LibraryApiContractId::JsLikeMapConstructor,
        }
    }

    fn contract_parts(self) -> (LibraryApiContractId, LibraryApiCalleeContract) {
        match self {
            Self::Set => {
                let contract = library_js_like_set_constructor_contract(Lang::JavaScript, "Set")
                    .expect("Set contract");
                (contract.id, contract.callee)
            }
            Self::Map => {
                let contract = library_js_like_map_constructor_contract(Lang::JavaScript, "Map")
                    .expect("Map contract");
                (contract.id, contract.callee)
            }
        }
    }

    fn assert_rejected(self, il: &Il, interner: &Interner, call: NodeId, reason: &str) {
        let rejected = match self {
            Self::Set => admitted_js_like_set_constructor_at_call(il, interner, call).is_none(),
            Self::Map => admitted_js_like_map_constructor_at_call(il, interner, call).is_none(),
        };
        assert!(rejected, "{} constructor: {reason}", self.name());
    }

    fn assert_admitted(self, il: &Il, interner: &Interner, call: NodeId, callee: NodeId) {
        match self {
            Self::Set => {
                let occurrence =
                    admitted_js_like_set_constructor_at_call(il, interner, call).unwrap();
                assert_eq!(occurrence.contract.id, self.expected_id());
                assert_eq!(occurrence.callee, callee);
                assert_eq!(occurrence.receiver, None);
                assert_eq!(occurrence.arg_count, 1);
            }
            Self::Map => {
                let occurrence =
                    admitted_js_like_map_constructor_at_call(il, interner, call).unwrap();
                assert_eq!(occurrence.contract.id, self.expected_id());
                assert_eq!(occurrence.callee, callee);
                assert_eq!(occurrence.receiver, None);
                assert_eq!(occurrence.arg_count, 1);
            }
        }
    }
}

pub(crate) fn assert_js_collection_constructor_requires_pack_provenance(
    kind: JsCollectionConstructorKind,
) {
    let (il, interner, call, _callee) = js_global_constructor_call_il(kind.name());
    kind.assert_rejected(
        &il,
        &interner,
        call,
        "raw constructor shape alone is rejected",
    );

    let (contract_id, callee_contract) = kind.contract_parts();

    let (mut missing_dependency, interner, call, _callee) =
        js_global_constructor_call_il(kind.name());
    missing_dependency.push_evidence(js_like_builtin_collection_constructor_record(
        0,
        missing_dependency.node(call).span,
        contract_id,
        callee_contract,
        EvidenceStatus::Asserted,
        &[],
    ));
    kind.assert_rejected(
        &missing_dependency,
        &interner,
        call,
        "same-span evidence without construct/global dependencies is rejected",
    );

    let (mut wrong_pack, interner, call, callee) = js_global_constructor_call_il(kind.name());
    push_js_global_constructor_dependencies(&mut wrong_pack, call, callee, kind.name());
    wrong_pack.push_evidence(library_api_record_with_provenance(
        2,
        wrong_pack.node(call).span,
        contract_id,
        callee_contract,
        EvidenceStatus::Asserted,
        &[0, 1],
        BUILTIN_COMPAT_PACK_ID,
        JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PRODUCER_ID,
    ));
    kind.assert_rejected(
        &wrong_pack,
        &interner,
        call,
        "compatibility-pack evidence is rejected",
    );

    let (mut wrong_producer, interner, call, callee) = js_global_constructor_call_il(kind.name());
    push_js_global_constructor_dependencies(&mut wrong_producer, call, callee, kind.name());
    wrong_producer.push_evidence(library_api_record_with_provenance(
        2,
        wrong_producer.node(call).span,
        contract_id,
        callee_contract,
        EvidenceStatus::Asserted,
        &[0, 1],
        JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PACK_ID,
        "wrong.javascript.builtins.collection-constructor-api",
    ));
    kind.assert_rejected(
        &wrong_producer,
        &interner,
        call,
        "wrong-producer evidence is rejected",
    );

    let (mut wrong_emitter, interner, call, callee) = js_global_constructor_call_il(kind.name());
    push_js_global_constructor_dependencies(&mut wrong_emitter, call, callee, kind.name());
    let mut external_record = js_like_builtin_collection_constructor_record(
        2,
        wrong_emitter.node(call).span,
        contract_id,
        callee_contract,
        EvidenceStatus::Asserted,
        &[0, 1],
    );
    external_record.provenance.emitter = EvidenceEmitter::External;
    wrong_emitter.push_evidence(external_record);
    kind.assert_rejected(
        &wrong_emitter,
        &interner,
        call,
        "external-emitter evidence is rejected",
    );

    let (mut admitted, interner, call, callee) = js_global_constructor_call_il(kind.name());
    push_js_global_constructor_dependencies(&mut admitted, call, callee, kind.name());
    admitted.push_evidence(js_like_builtin_collection_constructor_record(
        2,
        admitted.node(call).span,
        contract_id,
        callee_contract,
        EvidenceStatus::Asserted,
        &[0, 1],
    ));
    kind.assert_admitted(&admitted, &interner, call, callee);
}
