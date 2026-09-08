use crate::lower::Lowering;
use nose_il::{EvidenceAnchor, EvidenceKind, NodeKind};
use tree_sitter::Node as TsNode;

pub(super) fn record(lo: &mut Lowering, param: TsNode) {
    let span = lo.span(param);
    if lo.lang == nose_il::Lang::TypeScript {
        if let Some(annotation) = param.child_by_field_name("type") {
            let annotation = lo
                .text(annotation)
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect::<String>();
            let element = match annotation.as_str() {
                ":boolean[]" => Some(nose_il::DomainEvidence::Boolean),
                ":number[]" => Some(nose_il::DomainEvidence::Number),
                ":string[]" => Some(nose_il::DomainEvidence::String),
                _ => None,
            };
            if let Some(element) = element {
                lo.record_evidence(
                    EvidenceAnchor::node(span, NodeKind::Param),
                    EvidenceKind::Type(nose_il::TypeEvidenceKind::ArrayElementDomain { element }),
                    "typescript_primitive_array_parameter",
                );
            }
        }
    }
}
