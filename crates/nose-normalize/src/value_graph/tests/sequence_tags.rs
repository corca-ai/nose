use super::support::*;

#[test]
fn import_binding_value_requires_sequence_evidence() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let module = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("collections")),
        sp(40),
        &[],
    );
    let exported = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("deque")),
        sp(40),
        &[],
    );
    let binding = b.add(NodeKind::Seq, Payload::None, sp(40), &[module, exported]);
    let root = b.add(NodeKind::Block, Payload::None, sp(40), &[binding]);
    let mut il = finish_test_il(b, root, Lang::Python);

    let mut builder = Builder::new(&il, &interner);
    let raw = builder.eval(binding, &FxHashMap::default());
    assert!(matches!(
        builder.nodes[raw as usize].op,
        ValOp::Seq(SEQ_VALUE_UNTAGGED)
    ));
    assert!(!builder.is_import_binding_value(raw, "collections", "deque"));

    il.push_evidence(language_core_evidence(
        0,
        Lang::Python,
        EvidenceAnchor::sequence(sp(40)),
        EvidenceKind::Import(ImportEvidenceKind::Binding {
            module_hash: stable_symbol_hash("collections"),
            exported_hash: stable_symbol_hash("deque"),
        }),
    ));
    let mut builder = Builder::new(&il, &interner);
    let proven = builder.eval(binding, &FxHashMap::default());
    assert!(matches!(
        builder.nodes[proven as usize].op,
        ValOp::ImportBinding { .. }
    ));
    assert!(builder.is_import_binding_value(proven, "collections", "deque"));
}

fn seq_value_tag_for(
    interner: &Interner,
    raw_tag: &str,
    lang: Lang,
    evidence_records: Vec<EvidenceRecord>,
) -> u64 {
    let mut b = IlBuilder::new(FileId(0));
    let seq = b.add(
        NodeKind::Seq,
        Payload::Name(interner.intern(raw_tag)),
        sp(44),
        &[],
    );
    let root = b.add(NodeKind::Block, Payload::None, sp(44), &[seq]);
    let mut il = finish_test_il(b, root, lang);
    il.evidence.extend(evidence_records);

    let mut builder = Builder::new(&il, interner);
    let value = builder.eval(seq, &FxHashMap::default());
    let ValOp::Seq(tag) = builder.nodes[value as usize].op else {
        panic!("expected a sequence value op");
    };
    tag
}

#[test]
fn raw_sequence_name_tags_without_surface_evidence_are_untagged() {
    let interner = Interner::new();

    for raw_tag in ["array", "record_guard", "own_property_guard"] {
        let value_tag = seq_value_tag_for(&interner, raw_tag, Lang::JavaScript, Vec::new());
        assert_eq!(
            value_tag, SEQ_VALUE_UNTAGGED,
            "raw Seq({raw_tag:?}) must not enter the value graph as a semantic tag"
        );
        assert_ne!(
            value_tag,
            interner.symbol_hash(interner.intern(raw_tag)),
            "raw Seq({raw_tag:?}) must not fall back to its spelling hash"
        );
    }
}

#[test]
fn swift_internal_default_subscript_marker_survives_sequence_tag_normalization() {
    let interner = Interner::new();
    let tag = seq_value_tag_for(
        &interner,
        "swift_subscript_default",
        Lang::Swift,
        Vec::new(),
    );
    assert_eq!(tag, stable_symbol_hash("swift_subscript_default"));
}

#[test]
fn admitted_sequence_surface_controls_sequence_value_tag() {
    let interner = Interner::new();
    let tag = seq_value_tag_for(
        &interner,
        "array",
        Lang::JavaScript,
        vec![language_core_evidence(
            0,
            Lang::JavaScript,
            EvidenceAnchor::sequence(sp(44)),
            EvidenceKind::SequenceSurface(SequenceSurfaceKind::Collection),
        )],
    );

    assert_eq!(tag, SEQ_VALUE_COLLECTION);
}

#[test]
fn namespace_member_import_binding_requires_proven_namespace_value() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let prod = interner.intern("prod");
    let module = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("math")),
        sp(50),
        &[],
    );
    let namespace = b.add(NodeKind::Seq, Payload::None, sp(50), &[module]);
    let field = b.add(NodeKind::Field, Payload::Name(prod), sp(51), &[namespace]);
    let root = b.add(NodeKind::Block, Payload::None, sp(50), &[field]);
    let mut il = finish_test_il(b, root, Lang::Python);

    let mut builder = Builder::new(&il, &interner);
    let raw = builder.eval(field, &FxHashMap::default());
    assert!(matches!(builder.nodes[raw as usize].op, ValOp::Field(_)));
    assert!(!builder.is_import_binding_value(raw, "math", "prod"));

    il.push_evidence(language_core_evidence(
        0,
        Lang::Python,
        EvidenceAnchor::sequence(sp(50)),
        EvidenceKind::Import(ImportEvidenceKind::Namespace {
            module_hash: stable_symbol_hash("math"),
        }),
    ));
    let mut builder = Builder::new(&il, &interner);
    let proven = builder.eval(field, &FxHashMap::default());
    assert!(builder.is_import_binding_value(proven, "math", "prod"));
}
