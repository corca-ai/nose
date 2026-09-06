//! Offline projection for collections observed only through membership and size.
use nose_il::{
    Builtin, DomainEvidence, EvidenceKind, EvidenceStatus, Il, Lang, NodeId, NodeKind, Payload,
    TypeEvidenceKind,
};
use rustc_hash::FxHashSet;

pub fn keyed_membership_projection(
    il: &Il,
    interner: &nose_il::Interner,
    root: NodeId,
    param: NodeId,
) -> Option<DomainEvidence> {
    if il.meta.lang != Lang::TypeScript
        || !matches!(
            crate::domain_evidence_for_param(il, param),
            Some(DomainEvidence::Map | DomainEvidence::Set)
        )
    {
        return None;
    }
    let Payload::Cid(cid) = il.node(param).payload else {
        return None;
    };
    let mut key = None;
    for record in il.evidence_anchored_at(il.node(param).span) {
        if let EvidenceKind::Type(TypeEvidenceKind::KeyedCollectionKey { key: next }) = record.kind
        {
            if record.status != EvidenceStatus::Asserted
                || !matches!(
                    next,
                    DomainEvidence::Boolean | DomainEvidence::Number | DomainEvidence::String
                )
                || key.is_some_and(|previous| previous != next)
            {
                return None;
            }
            key = Some(next);
        }
    }
    let key = key?;
    let mut stack = vec![root];
    let mut visited = FxHashSet::default();
    let mut reads = 0;
    while let Some(parent) = stack.pop() {
        if !visited.insert(parent) {
            continue;
        }
        let kids = il.children(parent);
        if il.kind(parent) == NodeKind::Assign
            && kids
                .first()
                .is_some_and(|&target| il.kind(target) == NodeKind::Field)
        {
            return None;
        }
        for (index, &child) in kids.iter().enumerate() {
            if il.kind(child) == NodeKind::Var && il.node(child).payload == Payload::Cid(cid) {
                if il.kind(parent) == NodeKind::Field
                    && index == 0
                    && kids.len() == 1
                    && matches!(il.node(parent).payload, Payload::Name(name) if interner.resolve(name) == "size")
                {
                    reads += 1;
                    stack.push(child);
                    continue;
                }
                let builtin = match (il.node(parent).payload, index, kids.len()) {
                    (Payload::Builtin(Builtin::Len), 0, 1) => Builtin::Len,
                    (Payload::Builtin(Builtin::Contains), 1, 2) => Builtin::Contains,
                    _ => return None,
                };
                if !crate::admitted_builtin_semantics_at_call(il, parent, builtin) {
                    return None;
                }
                reads += 1;
            }
            stack.push(child);
        }
    }
    (reads > 0).then_some(key)
}
