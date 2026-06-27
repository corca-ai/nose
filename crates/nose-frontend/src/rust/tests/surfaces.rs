use super::*;
use nose_il::{EvidenceAnchor, EvidenceKind, SequenceSurfaceKind};

#[test]
fn struct_literal_emits_rust_struct_expression_surface_evidence() {
    let src = "struct Point { x: i32, y: i32 }\nfn f(x: i32, y: i32) -> Point { Point { x, y } }";
    let (interner, il) = lower_rust(src);

    assert_eq!(
        sequence_surface_count(
            &il,
            &interner,
            "rust_struct_expression",
            SequenceSurfaceKind::RustStructExpression,
        ),
        1,
        "Rust struct literals should emit exact-safe struct-expression surface evidence"
    );
}

fn sequence_surface_count(
    il: &Il,
    interner: &Interner,
    tag: &str,
    surface: SequenceSurfaceKind,
) -> usize {
    il.nodes
        .iter()
        .filter(|node| {
            node.kind == NodeKind::Seq
                && matches!(node.payload, Payload::Name(name) if interner.resolve(name) == tag)
        })
        .map(|node| EvidenceAnchor::sequence(node.span))
        .filter(|anchor| {
            il.evidence.iter().any(|record| {
                record.anchor == *anchor && record.kind == EvidenceKind::SequenceSurface(surface)
            })
        })
        .count()
}
