use super::support::*;
use crate::strict_exact::{
    strict_exact_java_collection_factory_safe, strict_exact_java_map_factory_safe,
    strict_exact_python_collection_factory_safe, strict_exact_safe_tree,
    strict_exact_set_constructor_collection_safe, StrictFacts,
};
use nose_il::{
    stable_symbol_hash, EvidenceAnchor, EvidenceId, EvidenceKind, FileId, FileMeta, Il, IlBuilder,
    ImportEvidenceKind, Interner, Lang, NodeId, NodeKind, Payload, SequenceSurfaceKind, Span,
    SymbolEvidenceKind,
};
use nose_semantics::{
    library_free_name_collection_factory_contract, library_java_collection_factory_contract,
    library_java_map_factory_contract, library_js_like_map_constructor_contract,
    library_js_like_set_constructor_contract, LibraryApiCalleeContract, LibraryApiContractId,
    JAVA_GUAVA_IMMUTABLE_COLLECTION_FACTORY_PACK_ID,
    JAVA_GUAVA_IMMUTABLE_COLLECTION_FACTORY_PRODUCER_ID, JAVA_STDLIB_COLLECTION_FACTORY_PACK_ID,
    JAVA_STDLIB_COLLECTION_FACTORY_PRODUCER_ID, JAVA_STDLIB_MAP_FACTORY_PACK_ID,
    JAVA_STDLIB_MAP_FACTORY_PRODUCER_ID,
};

mod java_collections;
mod swift_factories;

#[test]
fn strict_exact_js_constructor_requires_library_api_evidence() {
    let interner = Interner::new();
    let (mut il, call) = js_new_set_il(&interner);
    let facts = StrictFacts::collect(&il, &interner);
    assert!(!strict_exact_set_constructor_collection_safe(
        &il, &interner, &facts, call
    ));

    let wrong = library_js_like_map_constructor_contract(Lang::JavaScript, "Map").unwrap();
    il.evidence.push(library_api_contract_evidence(
        3,
        sp(13),
        wrong.id,
        wrong.callee,
        1,
        vec![EvidenceId(0), EvidenceId(1)],
    ));
    let facts = StrictFacts::collect(&il, &interner);
    assert!(!strict_exact_set_constructor_collection_safe(
        &il, &interner, &facts, call
    ));

    let (mut il, call) = js_new_set_il(&interner);
    let set = library_js_like_set_constructor_contract(Lang::JavaScript, "Set").unwrap();
    il.evidence
        .push(js_like_builtin_collection_constructor_evidence(
            3,
            sp(13),
            set.id,
            set.callee,
            1,
            vec![EvidenceId(0), EvidenceId(1)],
        ));
    let facts = StrictFacts::collect(&il, &interner);
    assert!(strict_exact_set_constructor_collection_safe(
        &il, &interner, &facts, call
    ));
}

#[test]
fn strict_exact_python_builtin_factory_requires_library_api_evidence() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let callee = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("list")),
        sp(40),
        &[],
    );
    let item = b.add(NodeKind::Lit, Payload::LitInt(1), sp(41), &[]);
    let seq = b.add(
        NodeKind::Seq,
        Payload::Name(interner.intern("array")),
        sp(42),
        &[item],
    );
    let call = b.add(NodeKind::Call, Payload::None, sp(43), &[callee, seq]);
    let root = b.add(NodeKind::Block, Payload::None, sp(39), &[call]);
    let mut il = b.finish(
        root,
        FileMeta {
            path: "t.py".into(),
            lang: Lang::Python,
        },
        Vec::new(),
        Vec::new(),
    );
    il.evidence.push(sequence_surface_evidence(
        0,
        Lang::Python,
        sp(42),
        SequenceSurfaceKind::Collection,
    ));
    let facts = StrictFacts::collect(&il, &interner);
    assert!(!strict_exact_python_collection_factory_safe(
        &il, &interner, &facts, call
    ));

    let contract = library_free_name_collection_factory_contract(Lang::Python, "list").unwrap();
    il.evidence.push(language_core_symbol_evidence(
        1,
        Lang::Python,
        EvidenceAnchor::node(sp(40), NodeKind::Var),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("list"),
        },
        Vec::new(),
    ));
    il.evidence.push(python_builtin_collection_factory_evidence(
        2,
        sp(43),
        contract,
        1,
        vec![EvidenceId(1)],
    ));
    let facts = StrictFacts::collect(&il, &interner);
    assert!(strict_exact_python_collection_factory_safe(
        &il, &interner, &facts, call
    ));
}

#[test]
fn strict_exact_java_collection_factory_uses_library_api_evidence() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let list = interner.intern("List");
    let lhs = b.add(NodeKind::Var, Payload::Name(list), sp(20), &[]);
    let rhs = b.add(NodeKind::Seq, Payload::None, sp(20), &[]);
    let import = b.add(NodeKind::Assign, Payload::None, sp(20), &[lhs, rhs]);
    let receiver = b.add(NodeKind::Var, Payload::Name(list), sp(21), &[]);
    let factory_callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("of")),
        sp(22),
        &[receiver],
    );
    let left = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("red")),
        sp(23),
        &[],
    );
    let right = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("blue")),
        sp(24),
        &[],
    );
    let factory = b.add(
        NodeKind::Call,
        Payload::None,
        sp(25),
        &[factory_callee, left, right],
    );
    let contains_callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("contains")),
        sp(26),
        &[factory],
    );
    let value = b.add(NodeKind::Var, Payload::Cid(0), sp(27), &[]);
    let contains = b.add(
        NodeKind::Call,
        Payload::None,
        sp(28),
        &[contains_callee, value],
    );
    let root = b.add(NodeKind::Block, Payload::None, sp(20), &[import, contains]);
    let mut il = b.finish(
        root,
        FileMeta {
            path: "t.java".into(),
            lang: Lang::Java,
        },
        Vec::new(),
        Vec::new(),
    );
    let contract = library_java_collection_factory_contract(Lang::Java, "List", "of")
        .expect("List.of contract");
    let binding_symbol = EvidenceKind::Symbol(SymbolEvidenceKind::ImportedBinding {
        module_hash: stable_symbol_hash("java.util"),
        exported_hash: stable_symbol_hash("List"),
    });
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::binding(sp(20), stable_symbol_hash("List")),
        binding_symbol,
        Vec::new(),
    ));
    il.evidence.push(evidence(
        1,
        EvidenceAnchor::node(sp(21), NodeKind::Var),
        binding_symbol,
        vec![EvidenceId(0)],
    ));
    let facts = StrictFacts::collect(&il, &interner);
    assert!(!strict_exact_java_collection_factory_safe(
        &il, &interner, &facts, factory
    ));
    assert!(!strict_exact_safe_tree(&il, &interner, &facts, contains));

    push_java_factory_contract_evidence(&mut il, contract.id, contract.callee);
    let facts = StrictFacts::collect(&il, &interner);
    assert!(strict_exact_java_collection_factory_safe(
        &il, &interner, &facts, factory
    ));
    assert!(strict_exact_safe_tree(&il, &interner, &facts, contains));

    let wrong = library_js_like_set_constructor_contract(Lang::JavaScript, "Set").unwrap();
    il.evidence.pop();
    il.evidence.pop();
    push_java_factory_contract_evidence(&mut il, wrong.id, wrong.callee);
    let facts = StrictFacts::collect(&il, &interner);
    assert!(!strict_exact_java_collection_factory_safe(
        &il, &interner, &facts, factory
    ));
    assert!(!strict_exact_safe_tree(&il, &interner, &facts, contains));
}

#[test]
fn strict_exact_java_map_provider_proof_does_not_replace_receiver_identity() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let receiver = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("FakeMap")),
        sp(30),
        &[],
    );
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("of")),
        sp(31),
        &[receiver],
    );
    let key = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("k")),
        sp(32),
        &[],
    );
    let value = b.add(NodeKind::Lit, Payload::LitInt(1), sp(33), &[]);
    let call = b.add(NodeKind::Call, Payload::None, sp(34), &[callee, key, value]);
    let root = b.add(NodeKind::Block, Payload::None, sp(34), &[call]);
    let mut il = b.finish(
        root,
        FileMeta {
            path: "t.java".into(),
            lang: Lang::Java,
        },
        Vec::new(),
        Vec::new(),
    );
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::node(sp(34), NodeKind::Call),
        EvidenceKind::Import(ImportEvidenceKind::ImmutableLiteralExport {
            module_hash: stable_symbol_hash("t"),
            exported_hash: stable_symbol_hash("VALUES"),
            root_kind: NodeKind::Call,
        }),
        Vec::new(),
    ));

    let facts = StrictFacts::collect(&il, &interner);
    assert!(!strict_exact_java_map_factory_safe(
        &il, &interner, &facts, call
    ));
}

#[test]
fn strict_exact_java_guava_collection_factory_rejects_static_null_elements() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let list = interner.intern("ImmutableList");
    let import = imported_binding_assignment(&mut b, &interner, "ImmutableList", sp(50));
    let receiver = b.add(NodeKind::Var, Payload::Name(list), sp(51), &[]);
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("of")),
        sp(52),
        &[receiver],
    );
    let null = b.add(
        NodeKind::Lit,
        Payload::Lit(nose_il::LitClass::Null),
        sp(53),
        &[],
    );
    let value = b.add(NodeKind::Lit, Payload::LitInt(1), sp(54), &[]);
    let call = b.add(
        NodeKind::Call,
        Payload::None,
        sp(55),
        &[callee, null, value],
    );
    let root = b.add(NodeKind::Block, Payload::None, sp(50), &[import, call]);
    let mut il = finish_java_il(b, root);
    push_guava_import_symbol(&mut il, "ImmutableList", sp(50), sp(51));
    let contract = library_java_collection_factory_contract(Lang::Java, "ImmutableList", "of")
        .expect("ImmutableList.of contract");
    push_guava_api_evidence(&mut il, 2, sp(55), contract.id, contract.callee, 2);

    let facts = StrictFacts::collect(&il, &interner);
    assert!(!strict_exact_java_collection_factory_safe(
        &il, &interner, &facts, call
    ));
}

#[test]
fn strict_exact_java_guava_map_factory_rejects_throwing_or_unsupported_shapes() {
    let valid = [
        Payload::LitStr(stable_symbol_hash("red")),
        Payload::LitInt(1),
        Payload::LitStr(stable_symbol_hash("blue")),
        Payload::LitInt(2),
    ];
    assert_guava_map_strict_exact(&valid, 60, true);

    let duplicate = [
        Payload::LitStr(stable_symbol_hash("red")),
        Payload::LitInt(1),
        Payload::LitStr(stable_symbol_hash("red")),
        Payload::LitInt(2),
    ];
    assert_guava_map_strict_exact(&duplicate, 70, false);

    let null_key = [
        Payload::Lit(nose_il::LitClass::Null),
        Payload::LitInt(1),
        Payload::LitStr(stable_symbol_hash("blue")),
        Payload::LitInt(2),
    ];
    assert_guava_map_strict_exact(&null_key, 80, false);

    let unsupported_arity = eleven_entry_payloads();
    assert_guava_map_strict_exact(&unsupported_arity, 90, false);
}

fn assert_guava_map_strict_exact(args: &[Payload], base_line: u32, expected: bool) {
    let (il, interner, call) = guava_map_factory_il(args, base_line);
    let facts = StrictFacts::collect(&il, &interner);
    assert_eq!(
        strict_exact_java_map_factory_safe(&il, &interner, &facts, call),
        expected
    );
}

fn guava_map_factory_il(args: &[Payload], base_line: u32) -> (Il, Interner, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let import = imported_binding_assignment(&mut b, &interner, "ImmutableMap", sp(base_line));
    let receiver = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("ImmutableMap")),
        sp(base_line + 1),
        &[],
    );
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("of")),
        sp(base_line + 2),
        &[receiver],
    );
    let arg_nodes: Vec<_> = args
        .iter()
        .enumerate()
        .map(|(idx, &payload)| b.add(NodeKind::Lit, payload, sp(base_line + 3 + idx as u32), &[]))
        .collect();
    let mut children = Vec::with_capacity(arg_nodes.len() + 1);
    children.push(callee);
    children.extend(arg_nodes);
    let call_span = sp(base_line + 3 + args.len() as u32);
    let call = b.add(NodeKind::Call, Payload::None, call_span, &children);
    let root = b.add(
        NodeKind::Block,
        Payload::None,
        sp(base_line),
        &[import, call],
    );
    let mut il = finish_java_il(b, root);
    push_guava_import_symbol(&mut il, "ImmutableMap", sp(base_line), sp(base_line + 1));
    let contract = library_java_map_factory_contract(Lang::Java, "ImmutableMap", "of")
        .expect("ImmutableMap.of contract");
    push_guava_api_evidence(
        &mut il,
        2,
        call_span,
        contract.id,
        contract.callee,
        args.len() as u16,
    );
    (il, interner, call)
}

fn imported_binding_assignment(
    b: &mut IlBuilder,
    interner: &Interner,
    local: &str,
    span: Span,
) -> NodeId {
    let lhs = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern(local)),
        span,
        &[],
    );
    let rhs = b.add(NodeKind::Seq, Payload::None, span, &[]);
    b.add(NodeKind::Assign, Payload::None, span, &[lhs, rhs])
}

fn finish_java_il(builder: IlBuilder, root: NodeId) -> Il {
    builder.finish(
        root,
        FileMeta {
            path: "t.java".into(),
            lang: Lang::Java,
        },
        Vec::new(),
        Vec::new(),
    )
}

fn push_guava_import_symbol(il: &mut Il, exported: &str, binding_span: Span, receiver_span: Span) {
    let symbol = SymbolEvidenceKind::ImportedBinding {
        module_hash: stable_symbol_hash("com.google.common.collect"),
        exported_hash: stable_symbol_hash(exported),
    };
    il.evidence.push(language_core_symbol_evidence(
        0,
        Lang::Java,
        EvidenceAnchor::binding(binding_span, stable_symbol_hash(exported)),
        symbol,
        Vec::new(),
    ));
    il.evidence.push(language_core_symbol_evidence(
        1,
        Lang::Java,
        EvidenceAnchor::node(receiver_span, NodeKind::Var),
        symbol,
        vec![EvidenceId(0)],
    ));
}

fn push_java_util_import_symbol(
    il: &mut Il,
    exported: &str,
    binding_span: Span,
    receiver_span: Span,
) {
    let symbol = SymbolEvidenceKind::ImportedBinding {
        module_hash: stable_symbol_hash("java.util"),
        exported_hash: stable_symbol_hash(exported),
    };
    il.evidence.push(language_core_symbol_evidence(
        0,
        Lang::Java,
        EvidenceAnchor::binding(binding_span, stable_symbol_hash(exported)),
        symbol,
        Vec::new(),
    ));
    il.evidence.push(language_core_symbol_evidence(
        1,
        Lang::Java,
        EvidenceAnchor::node(receiver_span, NodeKind::Var),
        symbol,
        vec![EvidenceId(0)],
    ));
}

#[allow(clippy::too_many_arguments)]
fn push_java_stdlib_api_evidence(
    il: &mut Il,
    id: u32,
    span: Span,
    contract_id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    arity: u16,
    pack_id: &'static str,
    producer_id: &'static str,
) {
    let mut record =
        library_api_contract_evidence(id, span, contract_id, callee, arity, vec![EvidenceId(1)]);
    record.provenance.pack_hash = Some(stable_symbol_hash(pack_id));
    record.provenance.rule_hash = Some(stable_symbol_hash(producer_id));
    il.evidence.push(record);
}

fn push_guava_api_evidence(
    il: &mut Il,
    id: u32,
    span: Span,
    contract_id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    arity: u16,
) {
    let mut record =
        library_api_contract_evidence(id, span, contract_id, callee, arity, vec![EvidenceId(1)]);
    record.provenance.pack_hash = Some(stable_symbol_hash(
        JAVA_GUAVA_IMMUTABLE_COLLECTION_FACTORY_PACK_ID,
    ));
    record.provenance.rule_hash = Some(stable_symbol_hash(
        JAVA_GUAVA_IMMUTABLE_COLLECTION_FACTORY_PRODUCER_ID,
    ));
    il.evidence.push(record);
}

fn eleven_entry_payloads() -> Vec<Payload> {
    (0..11)
        .flat_map(|idx| {
            [
                Payload::LitStr(stable_symbol_hash(&format!("k{idx}"))),
                Payload::LitInt(idx),
            ]
        })
        .collect()
}
