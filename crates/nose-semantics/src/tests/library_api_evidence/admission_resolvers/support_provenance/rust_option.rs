use super::super::*;

#[derive(Clone, Copy)]
pub(crate) enum RustOptionSomeAdmission {
    Call,
    Node,
}

impl RustOptionSomeAdmission {
    fn label(self) -> &'static str {
        match self {
            Self::Call => "Rust Some call resolver",
            Self::Node => "Rust Some node resolver",
        }
    }

    fn occurrence_record(
        self,
        il: &Il,
        call: NodeId,
        callee: NodeId,
        contract: LibraryRustOptionConstructorContract,
        dependencies: &[u32],
        pack_id: &str,
        producer_id: &str,
    ) -> EvidenceRecord {
        match self {
            Self::Call => library_api_record_with_provenance(
                1,
                il.node(call).span,
                contract.id,
                contract.callee,
                EvidenceStatus::Asserted,
                dependencies,
                pack_id,
                producer_id,
            ),
            Self::Node => asserted_library_api_node_record_with_provenance(
                1,
                il,
                callee,
                contract.id,
                contract.callee,
                1,
                dependencies,
                pack_id,
                producer_id,
            ),
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
            Self::Call => {
                admitted_rust_option_some_constructor_at_call(il, interner, call).is_none()
            }
            Self::Node => {
                admitted_rust_option_some_constructor_at_node(il, interner, callee).is_none()
            }
        };
        assert!(rejected, "{}: {reason}", self.label());
    }

    fn assert_admitted(self, il: &Il, interner: &Interner, call: NodeId, callee: NodeId) {
        match self {
            Self::Call => {
                let occurrence =
                    admitted_rust_option_some_constructor_at_call(il, interner, call).unwrap();
                assert_eq!(
                    occurrence.contract.id,
                    LibraryApiContractId::RustOptionSomeConstructor
                );
                assert_eq!(occurrence.callee, callee);
                assert_eq!(occurrence.receiver, None);
                assert_eq!(occurrence.arg_count, 1);
            }
            Self::Node => {
                let occurrence =
                    admitted_rust_option_some_constructor_at_node(il, interner, callee).unwrap();
                assert_eq!(
                    occurrence.contract.id,
                    LibraryApiContractId::RustOptionSomeConstructor
                );
                assert_eq!(occurrence.node, callee);
                assert_eq!(occurrence.receiver, None);
                assert_eq!(occurrence.arg_count, 1);
            }
        }
    }
}

pub(crate) fn assert_rust_option_some_requires_pack_provenance(admission: RustOptionSomeAdmission) {
    let contract = library_rust_option_some_constructor_contract(Lang::Rust, "Some", 1)
        .expect("Rust Some constructor contract");

    let (raw, interner, call, callee) = rust_some_call_il();
    admission.assert_rejected(&raw, &interner, call, callee, "raw shape alone is rejected");

    let (mut missing_dependency, interner, call, callee) = rust_some_call_il();
    let record = admission.occurrence_record(
        &missing_dependency,
        call,
        callee,
        contract,
        &[],
        RUST_STDLIB_OPTION_PACK_ID,
        RUST_STDLIB_OPTION_PRODUCER_ID,
    );
    missing_dependency.push_evidence(record);
    admission.assert_rejected(
        &missing_dependency,
        &interner,
        call,
        callee,
        "occurrence without callee dependency is rejected",
    );

    for (pack_id, producer_id, reason) in [
        (
            BUILTIN_COMPAT_PACK_ID,
            RUST_STDLIB_OPTION_PRODUCER_ID,
            "compatibility-pack evidence is rejected",
        ),
        (
            RUST_STDLIB_OPTION_PACK_ID,
            "wrong.rust.stdlib.option-api",
            "wrong-producer evidence is rejected",
        ),
    ] {
        let (mut rejected, interner, call, callee) = rust_some_call_il();
        push_some_symbol_dependency(&mut rejected, callee);
        let record = admission.occurrence_record(
            &rejected,
            call,
            callee,
            contract,
            &[0],
            pack_id,
            producer_id,
        );
        rejected.push_evidence(record);
        admission.assert_rejected(&rejected, &interner, call, callee, reason);
    }

    let (mut wrong_emitter, interner, call, callee) = rust_some_call_il();
    push_some_symbol_dependency(&mut wrong_emitter, callee);
    let mut external_record = admission.occurrence_record(
        &wrong_emitter,
        call,
        callee,
        contract,
        &[0],
        RUST_STDLIB_OPTION_PACK_ID,
        RUST_STDLIB_OPTION_PRODUCER_ID,
    );
    external_record.provenance.emitter = EvidenceEmitter::External;
    wrong_emitter.push_evidence(external_record);
    admission.assert_rejected(
        &wrong_emitter,
        &interner,
        call,
        callee,
        "external-emitter evidence is rejected",
    );

    let (mut admitted, interner, call, callee) = rust_some_call_il();
    push_some_symbol_dependency(&mut admitted, callee);
    let record = admission.occurrence_record(
        &admitted,
        call,
        callee,
        contract,
        &[0],
        RUST_STDLIB_OPTION_PACK_ID,
        RUST_STDLIB_OPTION_PRODUCER_ID,
    );
    admitted.push_evidence(record);
    admission.assert_admitted(&admitted, &interner, call, callee);
}

fn push_some_symbol_dependency(il: &mut Il, callee: NodeId) {
    il.push_evidence(language_core_symbol_record(
        0,
        EvidenceAnchor::node(il.node(callee).span, NodeKind::Var),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Some"),
        },
        EvidenceStatus::Asserted,
        &[],
        Lang::Rust,
    ));
}
