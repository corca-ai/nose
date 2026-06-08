use nose_il::{
    stable_symbol_hash, Builtin, EffectEvidenceKind, EvidenceAnchor, EvidenceEmitter, EvidenceId,
    EvidenceKind, EvidenceStatus, Il, Interner, Lang, NodeId, NodeKind, Payload, PlaceEvidenceKind,
};
use nose_semantics::FIRST_PARTY_PACK_ID;

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
    upsert(
        il,
        node,
        EvidenceKind::Effect(EffectEvidenceKind::BuilderAppendCall),
        "effect_builder_append_normalize",
        Vec::new(),
    );
}

fn record_assignment_effect(il: &mut Il, interner: &Interner, node: NodeId) {
    if il.kind(node) != NodeKind::Assign {
        return;
    }
    let Some(&target) = il.children(node).first() else {
        return;
    };
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
    for record in &mut il.evidence {
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
