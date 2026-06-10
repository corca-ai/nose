use nose_il::{
    stable_symbol_hash, Builtin, EffectEvidenceKind, EvidenceAnchor, EvidenceEmitter, EvidenceId,
    EvidenceKind, EvidenceStatus, Il, Interner, Lang, NodeId, NodeKind, Payload, PlaceEvidenceKind,
};
use nose_semantics::{
    library_api_dependency_id_for_canonical_builtin_call, module_binding_mutating_method_contract,
    FIRST_PARTY_PACK_ID,
};

pub(crate) fn run(il: &mut Il, interner: &Interner) {
    let nodes: Vec<NodeId> = il
        .nodes
        .iter()
        .enumerate()
        .map(|(idx, _)| NodeId(idx as u32))
        .collect();
    for node in nodes {
        match il.kind(node) {
            NodeKind::Var => {
                let _ = record_self_receiver(il, interner, node);
            }
            NodeKind::Field => {
                let _ = record_self_field(il, interner, node);
            }
            NodeKind::Call => {
                record_call_mutation_effects(il, interner, node);
                record_builder_append(il, node);
            }
            NodeKind::Assign => {
                record_assignment_effect(il, interner, node);
            }
            _ => {}
        }
    }
}

fn record_self_receiver(il: &mut Il, interner: &Interner, node: NodeId) -> Option<EvidenceId> {
    if il.meta.lang != Lang::Java || il.kind(node) != NodeKind::Var {
        return None;
    }
    let Payload::Name(name) = il.node(node).payload else {
        return None;
    };
    if interner.resolve(name) != "this" {
        return None;
    }
    Some(upsert(
        il,
        node,
        EvidenceKind::Place(PlaceEvidenceKind::SelfReceiver),
        "place_self_receiver_normalize",
        Vec::new(),
    ))
}

fn record_self_field(il: &mut Il, interner: &Interner, node: NodeId) -> Option<EvidenceId> {
    if il.meta.lang != Lang::Java || il.kind(node) != NodeKind::Field {
        return None;
    }
    let Payload::Name(field) = il.node(node).payload else {
        return None;
    };
    let receiver = il.children(node).first().copied()?;
    let receiver_evidence = record_self_receiver(il, interner, receiver)?;
    let field_hash = stable_symbol_hash(interner.resolve(field));
    Some(upsert(
        il,
        node,
        EvidenceKind::Place(PlaceEvidenceKind::SelfField { field_hash }),
        "place_self_field_normalize",
        vec![receiver_evidence],
    ))
}

fn record_builder_append(il: &mut Il, node: NodeId) {
    if il.kind(node) != NodeKind::Call
        || !matches!(il.node(node).payload, Payload::Builtin(Builtin::Append))
        || il.children(node).len() != 2
    {
        return;
    }
    let Some(api_dependency) =
        library_api_dependency_id_for_canonical_builtin_call(il, node, Builtin::Append)
    else {
        return;
    };
    upsert(
        il,
        node,
        EvidenceKind::Effect(EffectEvidenceKind::BuilderAppendCall),
        "effect_builder_append_normalize",
        vec![api_dependency],
    );
}

fn record_call_mutation_effects(il: &mut Il, interner: &Interner, node: NodeId) {
    if il.kind(node) != NodeKind::Call || !matches!(il.node(node).payload, Payload::None) {
        return;
    }
    let arg_count = il.children(node).len().saturating_sub(1);
    let callee = il.children(node).first().copied();
    if arg_count > 0 {
        upsert(
            il,
            node,
            EvidenceKind::Effect(EffectEvidenceKind::OpaqueArgumentEscape),
            "effect_opaque_argument_escape_normalize",
            Vec::new(),
        );
    }
    let Some(callee) = callee else {
        return;
    };
    if il.kind(callee) != NodeKind::Field {
        return;
    }
    let Payload::Name(method) = il.node(callee).payload else {
        return;
    };
    if let Some(contract) =
        module_binding_mutating_method_contract(il.meta.lang, interner.resolve(method), arg_count)
    {
        upsert(
            il,
            node,
            EvidenceKind::Effect(contract.effect),
            "effect_receiver_mutation_normalize",
            Vec::new(),
        );
    }
}

fn record_assignment_effect(il: &mut Il, interner: &Interner, node: NodeId) {
    if il.kind(node) != NodeKind::Assign {
        return;
    }
    let Some(&target) = il.children(node).first() else {
        return;
    };
    upsert(
        il,
        node,
        EvidenceKind::Effect(EffectEvidenceKind::BindingWrite),
        "effect_binding_write_normalize",
        Vec::new(),
    );
    if matches!(il.meta.lang, Lang::C | Lang::Go | Lang::Java) && il.kind(target) == NodeKind::Index
    {
        upsert(
            il,
            node,
            EvidenceKind::Effect(EffectEvidenceKind::NonOverloadableIndexWrite),
            "effect_non_overloadable_index_write_normalize",
            Vec::new(),
        );
        return;
    }
    if let Some((field_hash, place_evidence)) = self_field_target(il, interner, target) {
        upsert(
            il,
            node,
            EvidenceKind::Effect(EffectEvidenceKind::SelfFieldWrite { field_hash }),
            "effect_self_field_write_normalize",
            vec![place_evidence],
        );
    }
}

fn self_field_target(
    il: &mut Il,
    interner: &Interner,
    target: NodeId,
) -> Option<(u64, EvidenceId)> {
    if il.meta.lang != Lang::Java || il.kind(target) != NodeKind::Field {
        return None;
    }
    let Payload::Name(field) = il.node(target).payload else {
        return None;
    };
    let field_hash = stable_symbol_hash(interner.resolve(field));
    let place_evidence = record_self_field(il, interner, target)?;
    Some((field_hash, place_evidence))
}

fn upsert(
    il: &mut Il,
    node: NodeId,
    kind: EvidenceKind,
    rule: &'static str,
    dependencies: Vec<EvidenceId>,
) -> EvidenceId {
    let anchor = EvidenceAnchor::node(il.node(node).span, il.kind(node));
    let pack_hash = stable_symbol_hash(FIRST_PARTY_PACK_ID);
    let rule_hash = stable_symbol_hash(rule);
    let mut found = None;
    // Index-backed: only records anchored at this node's span can match, so the
    // per-upsert whole-`evidence` scan (quadratic — this pass upserts for most
    // assignments and calls) narrows to one bucket. `rule_hash`/`dependencies`
    // are read live by the index, so in-place updates need no invalidation.
    for idx in il.evidence_indices_anchored_at(anchor.span()) {
        let record = &mut il.evidence[idx as usize];
        if record.anchor == anchor
            && record.kind == kind
            && record.status == EvidenceStatus::Asserted
            && record.provenance.emitter == EvidenceEmitter::FirstParty
            && record.provenance.pack_hash == Some(pack_hash)
        {
            if found.is_none() {
                found = Some(record.id);
            }
            record.provenance.rule_hash = Some(rule_hash);
            record.dependencies = dependencies.clone();
        }
    }
    found.unwrap_or_else(|| {
        il.find_or_push_first_party_evidence(anchor, kind, FIRST_PARTY_PACK_ID, rule, dependencies)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use nose_il::{FileId, FileMeta, IlBuilder, LibraryApiEvidenceKind, Span};
    use nose_semantics::{
        library_api_callee_contract_hash, library_api_contract_id_hash,
        library_free_function_builtin_contract,
    };

    fn append_il() -> (Il, NodeId) {
        let span = Span::synthetic(FileId(0));
        let mut builder = IlBuilder::new(FileId(0));
        let receiver = builder.add(NodeKind::Var, Payload::Cid(0), span, &[]);
        let value = builder.add(NodeKind::Var, Payload::Cid(1), span, &[]);
        let append = builder.add(
            NodeKind::Call,
            Payload::Builtin(Builtin::Append),
            span,
            &[receiver, value],
        );
        let root = builder.add(NodeKind::Func, Payload::None, span, &[append]);
        (
            builder.finish(
                root,
                FileMeta {
                    path: "append.go".into(),
                    lang: Lang::Go,
                },
                Vec::new(),
                Vec::new(),
            ),
            append,
        )
    }

    fn builder_append_effect(il: &Il, call: NodeId) -> Option<&nose_il::EvidenceRecord> {
        il.evidence.iter().find(|record| {
            record.anchor == EvidenceAnchor::node(il.node(call).span, NodeKind::Call)
                && record.kind == EvidenceKind::Effect(EffectEvidenceKind::BuilderAppendCall)
        })
    }

    #[test]
    fn canonical_append_payload_does_not_emit_effect_without_api_proof() {
        let interner = Interner::new();
        let (mut il, append) = append_il();

        run(&mut il, &interner);

        assert!(
            builder_append_effect(&il, append).is_none(),
            "raw canonical append payload must not mint BuilderAppendCall evidence"
        );
    }

    #[test]
    fn canonical_append_effect_depends_on_library_api_proof() {
        let interner = Interner::new();
        let (mut il, append) = append_il();
        let contract =
            library_free_function_builtin_contract(Lang::Go, "append", 2).expect("Go append");
        let api = il.find_or_push_first_party_evidence(
            EvidenceAnchor::node(il.node(append).span, NodeKind::Call),
            EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
                contract_hash: library_api_contract_id_hash(contract.id),
                callee_hash: library_api_callee_contract_hash(contract.callee),
                arity: 2,
            }),
            FIRST_PARTY_PACK_ID,
            "test_go_append",
            Vec::new(),
        );

        run(&mut il, &interner);

        let effect = builder_append_effect(&il, append).expect("append effect");
        assert_eq!(effect.dependencies, vec![api]);
    }
}
