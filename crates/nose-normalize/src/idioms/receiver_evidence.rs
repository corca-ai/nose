use super::*;

pub(super) fn exact_collection_receiver(
    old: &Il,
    interner: &Interner,
    domains: &ReceiverDomainEvidenceIndex<'_>,
    node: NodeId,
) -> bool {
    exact_protocol_receiver(old, interner, domains, node)
}

pub(super) fn exact_array_receiver(
    old: &Il,
    interner: &Interner,
    domains: &ReceiverDomainEvidenceIndex<'_>,
    node: NodeId,
) -> bool {
    if domains.receiver_satisfies_domain(node, DomainRequirement::ARRAY)
        || exact_array_literal(old, interner, node)
    {
        return true;
    }
    if old.kind(node) != NodeKind::Call {
        return false;
    }
    let Some(admitted) = admitted_library_method_call_at_call(old, interner, node) else {
        return false;
    };
    if !matches!(
        admitted.contract.result.semantic,
        MethodSemanticContract::HoF(HoFKind::Map | HoFKind::Filter | HoFKind::FlatMap)
    ) || admitted.contract.result.receiver != MethodReceiverContract::ExactArray
    {
        return false;
    }
    let Some(receiver) = admitted.receiver else {
        return false;
    };
    exact_array_receiver(old, interner, domains, receiver)
}

pub(super) fn exact_protocol_receiver(
    old: &Il,
    interner: &Interner,
    domains: &ReceiverDomainEvidenceIndex<'_>,
    node: NodeId,
) -> bool {
    if exact_collection_literal(old, interner, node) || exact_collection_param(domains, node) {
        return true;
    }
    if old.kind(node) != NodeKind::Call {
        return false;
    }
    let kids = old.children(node);
    let Some(&callee) = kids.first() else {
        return false;
    };
    if old.kind(callee) != NodeKind::Field {
        return false;
    }
    let Some(&receiver) = old.children(callee).first() else {
        return false;
    };
    if let Some(arg) = static_collection_adapter_arg(old, interner, node, kids) {
        return exact_collection_receiver(old, interner, domains, arg);
    }
    let admitted_method = admitted_library_method_call_at_call(old, interner, node);
    if let Some(admitted) = admitted_method {
        let contract = admitted.contract;
        if contract.result.semantic == MethodSemanticContract::Builtin(Builtin::Zip)
            && contract.result.receiver == MethodReceiverContract::ExactProtocolPairArgument
            && contract.result.args == MethodBuiltinArgs::RustZip
        {
            return exact_protocol_receiver(old, interner, domains, receiver)
                && exact_protocol_receiver(old, interner, domains, kids[1]);
        }
    }
    if admitted_iterator_identity_adapter_at_call(old, interner, node).is_some() {
        return exact_protocol_receiver(old, interner, domains, receiver);
    }
    if let Some(admitted) = admitted_method {
        let contract = admitted.contract;
        if matches!(contract.result.semantic, MethodSemanticContract::HoF(_))
            && admitted.arg_count >= 1
        {
            return exact_protocol_receiver(old, interner, domains, receiver);
        }
    }
    false
}

pub(super) fn exact_collection_param(
    domains: &ReceiverDomainEvidenceIndex<'_>,
    node: NodeId,
) -> bool {
    domains.receiver_satisfies_domain(node, DomainRequirement::ARRAY_COLLECTION_OR_SET)
}

pub(super) fn static_collection_adapter_arg(
    old: &Il,
    interner: &Interner,
    call: NodeId,
    call_kids: &[NodeId],
) -> Option<NodeId> {
    admitted_static_collection_adapter_at_call(old, interner, call)?;
    call_kids.get(1).copied()
}

pub(super) fn exact_set_param(domains: &ReceiverDomainEvidenceIndex<'_>, node: NodeId) -> bool {
    domains.receiver_satisfies_domain(node, DomainRequirement::SET)
}

pub(super) fn exact_map_param(domains: &ReceiverDomainEvidenceIndex<'_>, node: NodeId) -> bool {
    domains.receiver_satisfies_domain(node, DomainRequirement::MAP)
}

pub(super) fn exact_map_receiver(
    old: &Il,
    interner: &Interner,
    domains: &ReceiverDomainEvidenceIndex<'_>,
    node: NodeId,
) -> bool {
    exact_map_param(domains, node) || map_like_literal(old, interner, node)
}

pub(super) fn exact_option_param(domains: &ReceiverDomainEvidenceIndex<'_>, node: NodeId) -> bool {
    domains.receiver_satisfies_domain(node, DomainRequirement::OPTION)
}

pub(super) fn exact_collection_literal(old: &Il, interner: &Interner, node: NodeId) -> bool {
    if old.kind(node) != NodeKind::Seq {
        return false;
    }
    seq_surface_contract_for_node(old, interner, node)
        .is_some_and(|contract| contract.membership_collection)
}

pub(super) fn exact_array_literal(old: &Il, interner: &Interner, node: NodeId) -> bool {
    if old.kind(node) != NodeKind::Seq {
        return false;
    }
    seq_surface_contract_for_node(old, interner, node).is_some_and(|contract| {
        contract.imported_literal && contract.value_tag == SEQ_VALUE_COLLECTION
    })
}

pub(super) fn exact_option_receiver(
    old: &Il,
    interner: &Interner,
    domains: &ReceiverDomainEvidenceIndex<'_>,
    node: NodeId,
) -> bool {
    if exact_option_param(domains, node) {
        return true;
    }
    if matches!(
        old.node(node).payload,
        Payload::Lit(nose_il::LitClass::Null)
    ) {
        return true;
    }
    if old.kind(node) != NodeKind::Call {
        return false;
    }
    admitted_rust_option_some_constructor_at_call(old, interner, node).is_some()
}

pub(super) fn exact_string_receiver(
    old: &Il,
    domains: &ReceiverDomainEvidenceIndex<'_>,
    node: NodeId,
) -> bool {
    matches!(
        old.node(node).payload,
        Payload::LitStr(_) | Payload::Lit(nose_il::LitClass::Str)
    ) || domains.receiver_satisfies_domain(node, DomainRequirement::STRING)
}

pub(super) fn literal_string_receiver(
    old: &Il,
    domains: &ReceiverDomainEvidenceIndex<'_>,
    node: NodeId,
) -> bool {
    exact_string_receiver(old, domains, node)
}

pub(super) fn exact_integer_receiver(
    old: &Il,
    domains: &ReceiverDomainEvidenceIndex<'_>,
    node: NodeId,
) -> bool {
    matches!(
        old.node(node).payload,
        Payload::LitInt(_) | Payload::Lit(nose_il::LitClass::Int)
    ) || domains.receiver_satisfies_domain(node, DomainRequirement::INTEGER)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nose_il::{
        stable_symbol_hash, EvidenceAnchor, EvidenceEmitter, EvidenceId, EvidenceKind,
        EvidenceProvenance, EvidenceRecord, EvidenceStatus, FileId, FileMeta, IlBuilder, Lang,
        Payload, SequenceSurfaceKind, Span,
    };
    use nose_semantics::language_core_evidence_provenance;

    fn span(start: u32, end: u32) -> Span {
        Span {
            file: FileId(0),
            start_byte: start,
            end_byte: end,
            start_line: start,
            end_line: end,
        }
    }

    fn seq_with_surface(tag: &str, surface: SequenceSurfaceKind) -> (Il, Interner, NodeId) {
        let interner = Interner::new();
        let mut builder = IlBuilder::new(FileId(0));
        let seq = builder.add(
            NodeKind::Seq,
            Payload::Name(interner.intern(tag)),
            span(1, 2),
            &[],
        );
        let root = builder.add(NodeKind::Module, Payload::None, span(0, 3), &[seq]);
        let mut il = builder.finish(
            root,
            FileMeta {
                path: "literal.js".to_string(),
                lang: Lang::JavaScript,
            },
            Vec::new(),
            Vec::new(),
        );
        let (pack_id, producer_id) = language_core_evidence_provenance(Lang::JavaScript);
        il.evidence.push(EvidenceRecord {
            id: EvidenceId(0),
            anchor: EvidenceAnchor::sequence(il.node(seq).span),
            kind: EvidenceKind::SequenceSurface(surface),
            provenance: EvidenceProvenance {
                emitter: EvidenceEmitter::Builtin,
                pack_hash: Some(stable_symbol_hash(pack_id)),
                rule_hash: Some(stable_symbol_hash(producer_id)),
            },
            dependencies: Vec::new(),
            status: EvidenceStatus::Asserted,
        });
        (il, interner, seq)
    }

    #[test]
    fn exact_array_literal_requires_imported_collection_literal() {
        let (array_il, array_interner, array) =
            seq_with_surface("array", SequenceSurfaceKind::Collection);
        assert!(exact_array_literal(&array_il, &array_interner, array));

        let (object_il, object_interner, object) =
            seq_with_surface("object", SequenceSurfaceKind::Map);
        assert!(
            !exact_array_literal(&object_il, &object_interner, object),
            "object literals are imported literals, but not Array receivers"
        );

        let (tuple_il, tuple_interner, tuple) =
            seq_with_surface("tuple_expression", SequenceSurfaceKind::Tuple);
        assert!(
            !exact_array_literal(&tuple_il, &tuple_interner, tuple),
            "tuple literals are imported literals, but not JS Array receivers"
        );
    }
}
