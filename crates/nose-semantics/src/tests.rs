use super::*;
use nose_il::{
    EvidenceAnchor, EvidenceEmitter, EvidenceId, EvidenceKind, EvidenceProvenance, EvidenceRecord,
    EvidenceStatus, FileId, FileMeta, GuardEvidenceKind, IlBuilder, ImportEvidenceKind, Interner,
    JsRecordGuardComparison, JsRecordGuardNullCheck, LibraryApiEvidenceKind, ParamSemantic,
    PlaceEvidenceKind, SequenceSurfaceKind, SourceCastKind, SourceFactKind, Span, Symbol,
    SymbolEvidenceKind, Unit, UnitKind,
};

mod js_symbol_guards;

const ALL_LANGS: &[Lang] = &[
    Lang::Python,
    Lang::JavaScript,
    Lang::TypeScript,
    Lang::Go,
    Lang::Rust,
    Lang::Java,
    Lang::C,
    Lang::Ruby,
    Lang::Vue,
    Lang::Svelte,
    Lang::Html,
];

const ALL_BUILTINS: &[Builtin] = &[
    Builtin::Len,
    Builtin::Print,
    Builtin::Append,
    Builtin::Range,
    Builtin::Sum,
    Builtin::Reduce,
    Builtin::Min,
    Builtin::Max,
    Builtin::Abs,
    Builtin::Zip,
    Builtin::Enumerate,
    Builtin::Keys,
    Builtin::Any,
    Builtin::All,
    Builtin::DictEntry,
    Builtin::IsEmpty,
    Builtin::StartsWith,
    Builtin::EndsWith,
    Builtin::Contains,
    Builtin::GetOrDefault,
    Builtin::ValueOrDefault,
    Builtin::IsNull,
    Builtin::IsNotNull,
    Builtin::Join,
    Builtin::UnsignedCast32,
];

fn inferred_domains_for_added_literal(lit: Payload) -> Vec<ValueDomain> {
    let sp = Span::synthetic(FileId(0));
    let mut b = IlBuilder::new(FileId(0));
    let param = b.add(NodeKind::Param, Payload::Cid(0), sp, &[]);
    let varx = b.add(NodeKind::Var, Payload::Cid(0), sp, &[]);
    let lit = b.add(NodeKind::Lit, lit, sp, &[]);
    let add = b.add(NodeKind::BinOp, Payload::Op(Op::Add), sp, &[varx, lit]);
    let ret = b.add(NodeKind::Return, Payload::None, sp, &[add]);
    let func = b.add(NodeKind::Func, Payload::None, sp, &[param, ret]);
    let il = b.finish(
        func,
        FileMeta {
            path: "t".into(),
            lang: Lang::Python,
        },
        Vec::new(),
        Vec::new(),
    );
    semantics(Lang::Python)
        .operators()
        .infer_param_value_domains(&il, func)
}

#[test]
fn value_domain_inference_treats_retained_float_literals_as_numeric() {
    assert_eq!(
        inferred_domains_for_added_literal(Payload::LitInt(3)),
        vec![ValueDomain::Number]
    );
    assert_eq!(
        inferred_domains_for_added_literal(Payload::LitFloat(0xBEEF)),
        vec![ValueDomain::Number]
    );
}

fn sp(line: u32) -> Span {
    Span::new(FileId(0), line, line, 1, 1)
}

fn span(start: u32, end: u32, line: u32) -> Span {
    Span::new(FileId(0), start, end, line, line)
}

fn finish_il(builder: IlBuilder, root: NodeId, lang: Lang) -> Il {
    builder.finish(
        root,
        FileMeta {
            path: "t".into(),
            lang,
        },
        vec![Unit {
            root,
            kind: UnitKind::Function,
            name: None,
        }],
        Vec::new(),
    )
}

fn evidence(
    id: u32,
    anchor: EvidenceAnchor,
    kind: EvidenceKind,
    status: EvidenceStatus,
) -> EvidenceRecord {
    evidence_with_dependencies(id, anchor, kind, status, Vec::new())
}

fn evidence_with_dependencies(
    id: u32,
    anchor: EvidenceAnchor,
    kind: EvidenceKind,
    status: EvidenceStatus,
    dependencies: Vec<EvidenceId>,
) -> EvidenceRecord {
    EvidenceRecord {
        id: EvidenceId(id),
        anchor,
        kind,
        provenance: EvidenceProvenance {
            emitter: EvidenceEmitter::FirstParty,
            pack_hash: Some(stable_symbol_hash(FIRST_PARTY_PACK_ID)),
            rule_hash: Some(stable_symbol_hash("test")),
        },
        dependencies,
        status,
    }
}

fn push_node_effect(il: &mut Il, id: u32, node: NodeId, effect: EffectEvidenceKind) -> EvidenceId {
    push_node_effect_with_dependencies(il, id, node, effect, Vec::new())
}

fn push_node_effect_with_dependencies(
    il: &mut Il,
    id: u32,
    node: NodeId,
    effect: EffectEvidenceKind,
    dependencies: Vec<EvidenceId>,
) -> EvidenceId {
    let evidence_id = EvidenceId(id);
    il.evidence.push(evidence_with_dependencies(
        id,
        EvidenceAnchor::node(il.node(node).span, il.kind(node)),
        EvidenceKind::Effect(effect),
        EvidenceStatus::Asserted,
        dependencies,
    ));
    evidence_id
}

fn push_node_place(il: &mut Il, id: u32, node: NodeId, place: PlaceEvidenceKind) -> EvidenceId {
    push_node_place_with_dependencies(il, id, node, place, Vec::new())
}

fn push_node_place_with_dependencies(
    il: &mut Il,
    id: u32,
    node: NodeId,
    place: PlaceEvidenceKind,
    dependencies: Vec<EvidenceId>,
) -> EvidenceId {
    let evidence_id = EvidenceId(id);
    il.evidence.push(evidence_with_dependencies(
        id,
        EvidenceAnchor::node(il.node(node).span, il.kind(node)),
        EvidenceKind::Place(place),
        EvidenceStatus::Asserted,
        dependencies,
    ));
    evidence_id
}

#[test]
fn first_party_profile_wraps_each_language() {
    for &lang in ALL_LANGS {
        let profile = semantics(lang);
        assert_eq!(profile.lang(), lang);
        assert_eq!(profile.pack_id(), FIRST_PARTY_PACK_ID);
        assert_eq!(profile.trust(), PackTrust::DefaultFirstParty);
    }
}

#[test]
fn domain_evidence_preserves_param_semantic_boundaries() {
    assert_eq!(
        domain_evidence_from_param_semantic(ParamSemantic::Array),
        DomainEvidence::Array
    );
    assert!(DomainEvidence::Array.is_array_collection_or_set());
    assert!(DomainEvidence::Collection.is_array_or_collection());
    assert!(DomainEvidence::Set.is_collection_or_set());
    assert!(DomainEvidence::Map.is_map());
    assert!(DomainEvidence::Option.is_option());
    assert!(DomainEvidence::String.is_string());
    assert!(DomainEvidence::ByteArray.is_byte_array());
    assert!(DomainEvidence::Integer.is_integer());
    assert!(DomainEvidence::Number.is_integer_or_number());
    assert!(DomainEvidence::Integer.is_integer_or_number());
    assert!(!DomainEvidence::Number.is_integer());
    assert!(!DomainEvidence::Array.is_collection_or_set());
    assert!(!DomainEvidence::Set.is_array_or_collection());
    assert!(DomainRequirement::CollectionOrMap.accepts(DomainEvidence::Array));
    assert!(DomainRequirement::CollectionOrMap.accepts(DomainEvidence::Map));
    assert!(DomainRequirement::SetOrMap.accepts(DomainEvidence::Set));
    assert!(DomainRequirement::SetOrMap.accepts(DomainEvidence::Map));
    assert!(!DomainRequirement::SetOrMap.accepts(DomainEvidence::Collection));
}

#[test]
fn type_domain_contracts_are_language_scoped_and_exact_enough() {
    assert_eq!(
        type_domain_from_source_text(Lang::TypeScript, "xs: Array<string>"),
        Some(DomainEvidence::Array)
    );
    assert_eq!(
        type_domain_from_source_text(Lang::TypeScript, "xs: string[]"),
        Some(DomainEvidence::Array)
    );
    assert_eq!(
        type_domain_from_source_text(Lang::TypeScript, "xs: Bitmap<string, number>"),
        None
    );
    assert_eq!(
        type_domain_from_source_text(Lang::TypeScript, "xs: Blacklist<string>"),
        None
    );

    assert_eq!(
        type_domain_from_source_text(Lang::Java, "@Nonnull List<String> xs"),
        Some(DomainEvidence::Collection)
    );
    assert_eq!(
        type_domain_from_source_text(Lang::Java, "@Ann(\"...\") String value"),
        Some(DomainEvidence::String)
    );

    assert_eq!(
        type_domain_from_source_text(Lang::Rust, "std::collections::HashMap<String, i32>"),
        Some(DomainEvidence::Map)
    );
    assert_eq!(
        type_domain_from_source_text(Lang::Rust, "HashSet<i32>"),
        Some(DomainEvidence::Set)
    );

    assert_eq!(type_domain_from_source_text(Lang::C, "int *xs"), None);
    assert_eq!(
        type_domain_from_source_text(Lang::C, "int xs"),
        Some(DomainEvidence::Integer)
    );
}

#[test]
fn method_receiver_contracts_expose_only_domain_backed_obligations() {
    assert_eq!(
        method_receiver_domain_requirement(MethodReceiverContract::ExactCollection),
        Some(DomainRequirement::ArrayCollectionOrSet)
    );
    assert_eq!(
        method_receiver_domain_requirement(MethodReceiverContract::ExactProtocol),
        Some(DomainRequirement::ArrayCollectionOrSet)
    );
    assert_eq!(
        method_receiver_domain_requirement(MethodReceiverContract::ExactCollectionOrMap),
        Some(DomainRequirement::CollectionOrMap)
    );
    assert_eq!(
        method_receiver_domain_requirement(MethodReceiverContract::ExactSetOrMap),
        Some(DomainRequirement::SetOrMap)
    );
    assert_eq!(
        method_receiver_domain_requirement(MethodReceiverContract::RustMapGetOrExactOption),
        Some(DomainRequirement::Option)
    );
    assert_eq!(
        method_receiver_domain_requirement(MethodReceiverContract::ExactMapLiteral),
        None
    );
    assert_eq!(
        method_receiver_domain_requirement(MethodReceiverContract::ImportedNamespace("math")),
        None
    );
}

#[test]
fn domain_evidence_records_drive_param_domain_proof() {
    let mut b = IlBuilder::new(FileId(0));
    let param = b.add(NodeKind::Param, Payload::None, sp(3), &[]);
    let root = b.add(NodeKind::Func, Payload::None, sp(3), &[param]);
    let mut il = finish_il(b, root, Lang::TypeScript);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::param(sp(3)),
        EvidenceKind::Domain(DomainEvidence::Map),
        EvidenceStatus::Asserted,
    ));

    assert_eq!(
        domain_evidence_for_param(&il, param),
        Some(DomainEvidence::Map)
    );
}

#[test]
fn ambiguous_domain_evidence_stays_closed() {
    let mut b = IlBuilder::new(FileId(0));
    let param = b.add(NodeKind::Param, Payload::None, sp(4), &[]);
    let root = b.add(NodeKind::Func, Payload::None, sp(4), &[param]);
    let mut il = finish_il(b, root, Lang::TypeScript);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::param(sp(4)),
        EvidenceKind::Domain(DomainEvidence::Set),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(evidence(
        1,
        EvidenceAnchor::param(sp(4)),
        EvidenceKind::Domain(DomainEvidence::Map),
        EvidenceStatus::Asserted,
    ));

    assert_eq!(domain_evidence_for_param(&il, param), None);
}

#[test]
fn receiver_domain_evidence_at_node_is_preferred_over_param_evidence() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let param = b.add(NodeKind::Param, Payload::Cid(0), span(10, 12, 1), &[]);
    let receiver = b.add(NodeKind::Var, Payload::Cid(0), span(20, 22, 2), &[]);
    let stmt = b.add(
        NodeKind::ExprStmt,
        Payload::None,
        span(20, 22, 2),
        &[receiver],
    );
    let body = b.add(NodeKind::Block, Payload::None, span(18, 24, 2), &[stmt]);
    let root = b.add(
        NodeKind::Func,
        Payload::None,
        span(0, 30, 1),
        &[param, body],
    );
    let mut il = finish_il(b, root, Lang::TypeScript);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::param(span(10, 12, 1)),
        EvidenceKind::Domain(DomainEvidence::Set),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(evidence(
        1,
        EvidenceAnchor::node(span(20, 22, 2), NodeKind::Var),
        EvidenceKind::Domain(DomainEvidence::Map),
        EvidenceStatus::Asserted,
    ));

    assert_eq!(
        domain_evidence_for_param(&il, param),
        Some(DomainEvidence::Set)
    );
    assert_eq!(
        domain_evidence_for_receiver(&il, &interner, receiver),
        Some(DomainEvidence::Map)
    );
    assert!(receiver_satisfies_domain(
        &il,
        &interner,
        receiver,
        DomainRequirement::Map
    ));
    assert!(!receiver_satisfies_domain(
        &il,
        &interner,
        receiver,
        DomainRequirement::Set
    ));
}

#[test]
fn ambiguous_receiver_domain_evidence_blocks_param_fallback() {
    let mut b = IlBuilder::new(FileId(0));
    let param = b.add(NodeKind::Param, Payload::Cid(0), span(10, 12, 1), &[]);
    let receiver = b.add(NodeKind::Var, Payload::Cid(0), span(20, 22, 2), &[]);
    let body = b.add(NodeKind::Block, Payload::None, span(18, 24, 2), &[receiver]);
    let root = b.add(
        NodeKind::Func,
        Payload::None,
        span(0, 30, 1),
        &[param, body],
    );
    let mut il = finish_il(b, root, Lang::TypeScript);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::param(span(10, 12, 1)),
        EvidenceKind::Domain(DomainEvidence::Map),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(evidence(
        1,
        EvidenceAnchor::node(span(20, 22, 2), NodeKind::Var),
        EvidenceKind::Domain(DomainEvidence::Set),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(evidence(
        2,
        EvidenceAnchor::node(span(20, 22, 2), NodeKind::Var),
        EvidenceKind::Domain(DomainEvidence::Map),
        EvidenceStatus::Asserted,
    ));

    let interner = Interner::new();
    assert_eq!(domain_evidence_for_receiver(&il, &interner, receiver), None);
}

fn binding_receiver_fixture(interner: &Interner, module_receiver: bool) -> (Il, NodeId, NodeId) {
    let xs = interner.intern("xs");
    let mut b = IlBuilder::new(FileId(0));
    let lhs = b.add(NodeKind::Var, Payload::Cid(0), span(10, 12, 1), &[]);
    let rhs = b.add(NodeKind::Seq, Payload::None, span(15, 17, 1), &[]);
    let assign = b.add(
        NodeKind::Assign,
        Payload::None,
        span(10, 17, 1),
        &[lhs, rhs],
    );
    let receiver_payload = if module_receiver {
        Payload::Name(xs)
    } else {
        Payload::Cid(0)
    };
    let receiver = b.add(NodeKind::Var, receiver_payload, span(40, 42, 3), &[]);
    let root = if module_receiver {
        let stmt = b.add(
            NodeKind::ExprStmt,
            Payload::None,
            span(40, 42, 3),
            &[receiver],
        );
        let body = b.add(NodeKind::Block, Payload::None, span(38, 45, 3), &[stmt]);
        let func = b.add(NodeKind::Func, Payload::None, span(30, 50, 2), &[body]);
        b.add(
            NodeKind::Module,
            Payload::None,
            span(0, 60, 1),
            &[assign, func],
        )
    } else {
        let body = b.add(
            NodeKind::Block,
            Payload::None,
            span(10, 44, 1),
            &[assign, receiver],
        );
        b.add(NodeKind::Func, Payload::None, span(0, 50, 1), &[body])
    };
    let mut il = finish_il(b, root, Lang::TypeScript);
    il.cid_names = vec![xs];
    (il, lhs, receiver)
}

#[test]
fn binding_domain_evidence_drives_receiver_domain_proof() {
    let interner = Interner::new();
    let (mut il, lhs, receiver) = binding_receiver_fixture(&interner, false);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::binding(span(10, 12, 1), stable_symbol_hash("xs")),
        EvidenceKind::Domain(DomainEvidence::Collection),
        EvidenceStatus::Asserted,
    ));

    assert_eq!(
        domain_evidence_for_binding_lhs(&il, &interner, lhs),
        Some(DomainEvidence::Collection)
    );
    assert_eq!(
        domain_evidence_for_receiver(&il, &interner, receiver),
        Some(DomainEvidence::Collection)
    );

    il.evidence.push(evidence(
        1,
        EvidenceAnchor::binding(span(10, 12, 1), stable_symbol_hash("xs")),
        EvidenceKind::Domain(DomainEvidence::Map),
        EvidenceStatus::Asserted,
    ));
    assert_eq!(
        domain_evidence_for_receiver(&il, &interner, receiver),
        None,
        "conflicting binding-domain evidence must close receiver proof"
    );
}

#[test]
fn binding_domain_evidence_validates_dependencies() {
    let interner = Interner::new();
    let (mut il, _, receiver) = binding_receiver_fixture(&interner, false);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::sequence(span(15, 17, 1)),
        EvidenceKind::SequenceSurface(SequenceSurfaceKind::Collection),
        EvidenceStatus::Ambiguous,
    ));
    il.evidence.push(evidence_with_dependencies(
        1,
        EvidenceAnchor::binding(span(10, 12, 1), stable_symbol_hash("xs")),
        EvidenceKind::Domain(DomainEvidence::Collection),
        EvidenceStatus::Asserted,
        vec![EvidenceId(0)],
    ));

    assert_eq!(
        domain_evidence_for_receiver(&il, &interner, receiver),
        None,
        "dependency-broken binding-domain evidence must fail closed"
    );
}

#[test]
fn module_binding_domain_evidence_reaches_free_name_receiver() {
    let interner = Interner::new();
    let (mut il, _, receiver) = binding_receiver_fixture(&interner, true);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::binding(span(10, 12, 1), stable_symbol_hash("xs")),
        EvidenceKind::Domain(DomainEvidence::Collection),
        EvidenceStatus::Asserted,
    ));

    assert_eq!(
        domain_evidence_for_receiver(&il, &interner, receiver),
        Some(DomainEvidence::Collection)
    );
}

#[test]
fn binding_domain_evidence_requires_matching_local_hash() {
    let interner = Interner::new();
    let xs = interner.intern("xs");
    let ys = interner.intern("ys");
    let mut b = IlBuilder::new(FileId(0));
    let xs_lhs = b.add(NodeKind::Var, Payload::Cid(0), span(10, 12, 1), &[]);
    let xs_rhs = b.add(NodeKind::Seq, Payload::None, span(14, 15, 1), &[]);
    let xs_assign = b.add(
        NodeKind::Assign,
        Payload::None,
        span(10, 15, 1),
        &[xs_lhs, xs_rhs],
    );
    let ys_lhs = b.add(NodeKind::Var, Payload::Cid(1), span(10, 12, 1), &[]);
    let ys_rhs = b.add(NodeKind::Seq, Payload::None, span(18, 19, 1), &[]);
    let ys_assign = b.add(
        NodeKind::Assign,
        Payload::None,
        span(16, 19, 1),
        &[ys_lhs, ys_rhs],
    );
    let ys_receiver = b.add(NodeKind::Var, Payload::Cid(1), span(30, 32, 2), &[]);
    let body = b.add(
        NodeKind::Block,
        Payload::None,
        span(8, 34, 1),
        &[xs_assign, ys_assign, ys_receiver],
    );
    let root = b.add(NodeKind::Func, Payload::None, span(0, 40, 1), &[body]);
    let mut il = finish_il(b, root, Lang::TypeScript);
    il.cid_names = vec![xs, ys];
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::binding(span(10, 12, 1), stable_symbol_hash("xs")),
        EvidenceKind::Domain(DomainEvidence::Collection),
        EvidenceStatus::Asserted,
    ));

    assert_eq!(
        domain_evidence_for_binding_lhs(&il, &interner, xs_lhs),
        Some(DomainEvidence::Collection)
    );
    assert_eq!(
        domain_evidence_for_binding_lhs(&il, &interner, ys_lhs),
        None,
        "same-span binding evidence must not cross local_hash boundaries"
    );
    assert_eq!(
        domain_evidence_for_receiver(&il, &interner, ys_receiver),
        None
    );
}

#[test]
fn binding_domain_evidence_requires_assignment_before_receiver() {
    let interner = Interner::new();
    let xs = interner.intern("xs");
    let mut b = IlBuilder::new(FileId(0));
    let receiver = b.add(NodeKind::Var, Payload::Cid(0), span(10, 12, 1), &[]);
    let lhs = b.add(NodeKind::Var, Payload::Cid(0), span(20, 22, 2), &[]);
    let rhs = b.add(NodeKind::Seq, Payload::None, span(24, 25, 2), &[]);
    let assign = b.add(
        NodeKind::Assign,
        Payload::None,
        span(20, 25, 2),
        &[lhs, rhs],
    );
    let body = b.add(
        NodeKind::Block,
        Payload::None,
        span(8, 28, 1),
        &[receiver, assign],
    );
    let root = b.add(NodeKind::Func, Payload::None, span(0, 30, 1), &[body]);
    let mut il = finish_il(b, root, Lang::TypeScript);
    il.cid_names = vec![xs];
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::binding(span(20, 22, 2), stable_symbol_hash("xs")),
        EvidenceKind::Domain(DomainEvidence::Collection),
        EvidenceStatus::Asserted,
    ));

    assert_eq!(
        domain_evidence_for_binding_lhs(&il, &interner, lhs),
        Some(DomainEvidence::Collection)
    );
    assert_eq!(
        domain_evidence_for_receiver(&il, &interner, receiver),
        None,
        "binding-domain evidence must not prove use-before-assignment receivers"
    );
}

#[test]
fn cid_receiver_domain_uses_nearest_function_scope() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let first_param = b.add(NodeKind::Param, Payload::Cid(0), span(10, 12, 1), &[]);
    let first_body = b.add(NodeKind::Block, Payload::None, span(14, 20, 1), &[]);
    let first_func = b.add(
        NodeKind::Func,
        Payload::None,
        span(0, 30, 1),
        &[first_param, first_body],
    );
    let second_param = b.add(NodeKind::Param, Payload::Cid(0), span(50, 52, 3), &[]);
    let receiver = b.add(NodeKind::Var, Payload::Cid(0), span(60, 62, 4), &[]);
    let stmt = b.add(
        NodeKind::ExprStmt,
        Payload::None,
        span(60, 62, 4),
        &[receiver],
    );
    let second_body = b.add(NodeKind::Block, Payload::None, span(58, 66, 4), &[stmt]);
    let second_func = b.add(
        NodeKind::Func,
        Payload::None,
        span(40, 80, 3),
        &[second_param, second_body],
    );
    let root = b.add(
        NodeKind::Module,
        Payload::None,
        span(0, 90, 1),
        &[first_func, second_func],
    );
    let mut il = finish_il(b, root, Lang::TypeScript);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::param(span(10, 12, 1)),
        EvidenceKind::Domain(DomainEvidence::Collection),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(evidence(
        1,
        EvidenceAnchor::param(span(50, 52, 3)),
        EvidenceKind::Domain(DomainEvidence::Map),
        EvidenceStatus::Asserted,
    ));

    assert_eq!(
        domain_evidence_for_receiver(&il, &interner, receiver),
        Some(DomainEvidence::Map)
    );
}

#[test]
fn dependency_broken_receiver_domain_evidence_blocks_param_fallback() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let param = b.add(NodeKind::Param, Payload::Cid(0), span(10, 12, 1), &[]);
    let receiver = b.add(NodeKind::Var, Payload::Cid(0), span(20, 22, 2), &[]);
    let body = b.add(NodeKind::Block, Payload::None, span(18, 24, 2), &[receiver]);
    let root = b.add(
        NodeKind::Func,
        Payload::None,
        span(0, 30, 1),
        &[param, body],
    );
    let mut il = finish_il(b, root, Lang::TypeScript);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::param(span(10, 12, 1)),
        EvidenceKind::Domain(DomainEvidence::Set),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(evidence_with_dependencies(
        1,
        EvidenceAnchor::node(span(20, 22, 2), NodeKind::Var),
        EvidenceKind::Domain(DomainEvidence::Map),
        EvidenceStatus::Asserted,
        vec![EvidenceId(99)],
    ));

    assert_eq!(domain_evidence_for_receiver(&il, &interner, receiver), None);
}

#[test]
fn receiver_domain_index_uses_kernel_fail_closed_policy() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let param = b.add(NodeKind::Param, Payload::Cid(0), span(10, 12, 1), &[]);
    let receiver = b.add(NodeKind::Var, Payload::Cid(0), span(20, 22, 2), &[]);
    let body = b.add(NodeKind::Block, Payload::None, span(18, 24, 2), &[receiver]);
    let root = b.add(
        NodeKind::Func,
        Payload::None,
        span(0, 30, 1),
        &[param, body],
    );
    let mut il = finish_il(b, root, Lang::TypeScript);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::param(span(10, 12, 1)),
        EvidenceKind::Domain(DomainEvidence::Collection),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(evidence(
        1,
        EvidenceAnchor::node(span(20, 22, 2), NodeKind::Var),
        EvidenceKind::Domain(DomainEvidence::Map),
        EvidenceStatus::Ambiguous,
    ));

    let domains = ReceiverDomainEvidenceIndex::new(&il, &interner);
    assert_eq!(domains.domain_evidence_for_receiver(receiver), None);
    assert!(!domains.receiver_satisfies_domain(receiver, DomainRequirement::Collection));
}

#[test]
fn named_receiver_domain_requires_unassigned_param_scope() {
    let interner = Interner::new();
    let xs = interner.intern("xs");
    let mut b = IlBuilder::new(FileId(0));
    let param = b.add(NodeKind::Param, Payload::Name(xs), span(10, 12, 1), &[]);
    let receiver = b.add(NodeKind::Var, Payload::Name(xs), span(40, 42, 3), &[]);
    let stmt = b.add(
        NodeKind::ExprStmt,
        Payload::None,
        span(40, 42, 3),
        &[receiver],
    );
    let body = b.add(NodeKind::Block, Payload::None, span(20, 50, 2), &[stmt]);
    let root = b.add(
        NodeKind::Func,
        Payload::None,
        span(0, 60, 1),
        &[param, body],
    );
    let mut il = finish_il(b, root, Lang::TypeScript);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::param(span(10, 12, 1)),
        EvidenceKind::Domain(DomainEvidence::Collection),
        EvidenceStatus::Asserted,
    ));

    assert_eq!(
        domain_evidence_for_receiver(&il, &interner, receiver),
        Some(DomainEvidence::Collection)
    );

    let mut b = IlBuilder::new(FileId(0));
    let param = b.add(NodeKind::Param, Payload::Name(xs), span(10, 12, 1), &[]);
    let lhs = b.add(NodeKind::Var, Payload::Name(xs), span(24, 26, 2), &[]);
    let rhs = b.add(NodeKind::Lit, Payload::LitInt(1), span(29, 30, 2), &[]);
    let assign = b.add(
        NodeKind::Assign,
        Payload::None,
        span(24, 30, 2),
        &[lhs, rhs],
    );
    let receiver = b.add(NodeKind::Var, Payload::Name(xs), span(40, 42, 3), &[]);
    let stmt = b.add(
        NodeKind::ExprStmt,
        Payload::None,
        span(40, 42, 3),
        &[receiver],
    );
    let body = b.add(
        NodeKind::Block,
        Payload::None,
        span(20, 50, 2),
        &[assign, stmt],
    );
    let root = b.add(
        NodeKind::Func,
        Payload::None,
        span(0, 60, 1),
        &[param, body],
    );
    let mut il = finish_il(b, root, Lang::TypeScript);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::param(span(10, 12, 1)),
        EvidenceKind::Domain(DomainEvidence::Collection),
        EvidenceStatus::Asserted,
    ));

    assert_eq!(domain_evidence_for_receiver(&il, &interner, receiver), None);
}

#[test]
fn sequence_surface_contracts_keep_value_and_exact_axes_separate() {
    let array = seq_surface_contract(Lang::JavaScript, Some("array")).unwrap();
    assert_eq!(array.value_tag, SEQ_VALUE_COLLECTION);
    assert!(array.exact_tree_safe);
    assert!(array.membership_collection);

    let untagged = seq_surface_contract(Lang::JavaScript, None).unwrap();
    assert_eq!(untagged.value_tag, SEQ_VALUE_UNTAGGED);
    assert!(!untagged.exact_tree_safe);
    assert!(!untagged.membership_collection);

    let object = seq_surface_contract(Lang::JavaScript, Some("object")).unwrap();
    assert_eq!(object.value_tag, SEQ_VALUE_MAP);
    assert!(object.exact_tree_safe);
    assert!(!object.membership_collection);
    assert!(object.imported_literal);

    let go_map = seq_surface_contract(Lang::Go, Some("composite_literal")).unwrap();
    assert_eq!(
        go_map.value_tag,
        stable_symbol_hash("go_composite_map_literal")
    );
    assert!(!go_map.exact_tree_safe);
    assert!(!go_map.membership_collection);
    assert!(!go_map.imported_literal);

    let go_entry = seq_surface_contract(Lang::Go, Some("keyed_element")).unwrap();
    assert_eq!(go_entry.value_tag, stable_symbol_hash("keyed_element"));
    assert!(!go_entry.exact_tree_safe);
    assert!(!go_entry.membership_collection);

    assert!(seq_surface_contract(Lang::Python, Some("composite_literal")).is_none());
    assert!(seq_surface_contract(Lang::Python, Some("keyed_element")).is_none());
    assert!(imported_literal_seq_tag_safe(Lang::Python, "dictionary"));
    assert!(!imported_literal_seq_tag_safe(Lang::Ruby, "hash"));
}

#[test]
fn sequence_surface_evidence_must_match_the_lowered_surface() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let array = interner.intern("array");
    let seq = b.add(NodeKind::Seq, Payload::Name(array), sp(5), &[]);
    let root = b.add(NodeKind::Block, Payload::None, sp(5), &[seq]);
    let mut il = finish_il(b, root, Lang::JavaScript);

    assert_eq!(
        seq_surface_contract_for_node(&il, &interner, seq),
        None,
        "raw sequence tags do not prove semantic surfaces without evidence"
    );

    il.evidence.push(evidence(
        0,
        EvidenceAnchor::sequence(sp(5)),
        EvidenceKind::SequenceSurface(SequenceSurfaceKind::Collection),
        EvidenceStatus::Asserted,
    ));
    assert!(seq_surface_contract_for_node(&il, &interner, seq)
        .is_some_and(|contract| contract.membership_collection));

    il.evidence.push(evidence(
        1,
        EvidenceAnchor::sequence(sp(5)),
        EvidenceKind::SequenceSurface(SequenceSurfaceKind::Map),
        EvidenceStatus::Asserted,
    ));
    assert_eq!(seq_surface_contract_for_node(&il, &interner, seq), None);
}

#[test]
fn imported_literal_export_safety_requires_sequence_evidence() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let object = interner.intern("object");
    let key = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("ready")),
        sp(6),
        &[],
    );
    let value = b.add(NodeKind::Lit, Payload::LitInt(1), sp(6), &[]);
    let entry = b.add(NodeKind::Seq, Payload::Name(object), sp(6), &[key, value]);
    let root = b.add(NodeKind::Block, Payload::None, sp(6), &[entry]);
    let mut il = finish_il(b, root, Lang::JavaScript);

    assert!(!imported_literal_export_safe(&il, &interner, entry));

    il.evidence.push(evidence(
        0,
        EvidenceAnchor::sequence(sp(6)),
        EvidenceKind::SequenceSurface(SequenceSurfaceKind::Map),
        EvidenceStatus::Asserted,
    ));
    assert!(imported_literal_export_safe(&il, &interner, entry));
}

#[test]
fn imported_literal_export_safety_rejects_import_coordinate_children() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let object = interner.intern("object");
    let imported = b.add(NodeKind::Seq, Payload::None, sp(7), &[]);
    let root_value = b.add(NodeKind::Seq, Payload::Name(object), sp(8), &[imported]);
    let root = b.add(NodeKind::Block, Payload::None, sp(8), &[root_value]);
    let mut il = finish_il(b, root, Lang::JavaScript);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::sequence(sp(8)),
        EvidenceKind::SequenceSurface(SequenceSurfaceKind::Map),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(evidence(
        1,
        EvidenceAnchor::sequence(sp(7)),
        EvidenceKind::Import(ImportEvidenceKind::Binding {
            module_hash: stable_symbol_hash("provider"),
            exported_hash: stable_symbol_hash("VALUE"),
        }),
        EvidenceStatus::Asserted,
    ));

    assert!(!imported_literal_export_safe(&il, &interner, root_value));
}

#[test]
fn go_zero_map_surface_helpers_require_evidence() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let key = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("ready")),
        sp(32),
        &[],
    );
    let value = b.add(NodeKind::Lit, Payload::LitInt(1), sp(32), &[]);
    let entry = b.add(
        NodeKind::Seq,
        Payload::Name(interner.intern("keyed_element")),
        sp(32),
        &[key, value],
    );
    let map = b.add(
        NodeKind::Seq,
        Payload::Name(interner.intern("composite_literal")),
        sp(31),
        &[entry],
    );
    let root = b.add(NodeKind::Block, Payload::None, sp(31), &[map]);
    let mut il = finish_il(b, root, Lang::Go);

    assert!(go_zero_map_literal_contract_for_node(&il, &interner, map).is_none());
    assert!(go_zero_map_entry_contract_for_node(&il, &interner, entry).is_none());

    il.evidence.push(evidence(
        0,
        EvidenceAnchor::sequence(sp(31)),
        EvidenceKind::SequenceSurface(SequenceSurfaceKind::GoCompositeMapLiteral),
        EvidenceStatus::Asserted,
    ));
    assert!(go_zero_map_literal_contract_for_node(&il, &interner, map).is_some());
    assert!(go_zero_map_entry_contract_for_node(&il, &interner, entry).is_none());

    il.evidence.push(evidence(
        1,
        EvidenceAnchor::sequence(sp(32)),
        EvidenceKind::SequenceSurface(SequenceSurfaceKind::GoMapEntry),
        EvidenceStatus::Asserted,
    ));
    assert!(go_zero_map_entry_contract_for_node(&il, &interner, entry).is_some());
}

#[test]
fn import_fact_contracts_resolve_evidence_only_binding_and_namespace_proofs() {
    let mut b = IlBuilder::new(FileId(0));
    let collections = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("collections")),
        sp(1),
        &[],
    );
    let deque = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("deque")),
        sp(1),
        &[],
    );
    let binding = b.add(NodeKind::Seq, Payload::None, sp(1), &[collections, deque]);
    let math = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("math")),
        sp(2),
        &[],
    );
    let namespace = b.add(NodeKind::Seq, Payload::None, sp(2), &[math]);
    let raw_coordinates = b.add(NodeKind::Seq, Payload::None, sp(3), &[math]);
    let root = b.add(
        NodeKind::Module,
        Payload::None,
        sp(1),
        &[binding, namespace, raw_coordinates],
    );
    let mut il = finish_il(b, root, Lang::Python);

    assert_eq!(
        import_fact_contract(ImportFactKind::Binding).channel,
        ChannelEligibility::ExactProven
    );
    assert_eq!(import_fact_evidence_rhs(&il, binding), None);
    assert_eq!(import_fact_evidence_rhs(&il, namespace), None);

    il.evidence.push(evidence(
        0,
        EvidenceAnchor::sequence(sp(1)),
        EvidenceKind::Import(ImportEvidenceKind::Binding {
            module_hash: stable_symbol_hash("collections"),
            exported_hash: stable_symbol_hash("deque"),
        }),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(evidence(
        1,
        EvidenceAnchor::sequence(sp(2)),
        EvidenceKind::Import(ImportEvidenceKind::Namespace {
            module_hash: stable_symbol_hash("math"),
        }),
        EvidenceStatus::Asserted,
    ));

    assert_eq!(
        import_fact_evidence_rhs(&il, binding),
        Some(ImportFact {
            kind: ImportFactKind::Binding,
            module_hash: stable_symbol_hash("collections"),
            exported_hash: Some(stable_symbol_hash("deque")),
        })
    );
    assert_eq!(
        import_fact_evidence_rhs(&il, namespace),
        Some(ImportFact {
            kind: ImportFactKind::Namespace,
            module_hash: stable_symbol_hash("math"),
            exported_hash: None,
        })
    );
    assert_eq!(import_fact_evidence_rhs(&il, raw_coordinates), None);
}

#[test]
fn ambiguous_import_evidence_stays_closed_without_raw_seq_fallback() {
    let mut b = IlBuilder::new(FileId(0));
    let module = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("collections")),
        sp(10),
        &[],
    );
    let exported = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("deque")),
        sp(10),
        &[],
    );
    let binding = b.add(NodeKind::Seq, Payload::None, sp(10), &[module, exported]);
    let root = b.add(NodeKind::Module, Payload::None, sp(10), &[binding]);
    let mut il = finish_il(b, root, Lang::Python);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::sequence(sp(10)),
        EvidenceKind::Import(ImportEvidenceKind::Namespace {
            module_hash: stable_symbol_hash("math"),
        }),
        EvidenceStatus::Asserted,
    ));

    assert_eq!(
        import_fact_evidence_rhs(&il, binding),
        Some(ImportFact {
            kind: ImportFactKind::Namespace,
            module_hash: stable_symbol_hash("math"),
            exported_hash: None,
        })
    );

    il.evidence.push(evidence(
        1,
        EvidenceAnchor::sequence(sp(10)),
        EvidenceKind::Import(ImportEvidenceKind::Binding {
            module_hash: stable_symbol_hash("collections"),
            exported_hash: stable_symbol_hash("deque"),
        }),
        EvidenceStatus::Asserted,
    ));
    assert_eq!(import_fact_evidence_rhs(&il, binding), None);
}

#[test]
fn imported_symbol_identity_does_not_fall_back_to_raw_import_seq() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let local = interner.intern("deque");
    let module = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("collections")),
        sp(30),
        &[],
    );
    let exported = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("deque")),
        sp(30),
        &[],
    );
    let lhs = b.add(NodeKind::Var, Payload::Name(local), sp(30), &[]);
    let rhs = b.add(NodeKind::Seq, Payload::None, sp(30), &[module, exported]);
    let assignment = b.add(NodeKind::Assign, Payload::None, sp(30), &[lhs, rhs]);
    let use_site = b.add(NodeKind::Var, Payload::Name(local), sp(31), &[]);
    let root = b.add(
        NodeKind::Module,
        Payload::None,
        sp(30),
        &[assignment, use_site],
    );
    let mut il = finish_il(b, root, Lang::Python);

    assert_eq!(import_fact_evidence_rhs(&il, rhs), None);
    assert!(!imported_binding_symbol(
        &il,
        &interner,
        use_site,
        "collections",
        "deque"
    ));

    il.evidence.push(evidence(
        0,
        EvidenceAnchor::binding(sp(30), stable_symbol_hash("deque")),
        EvidenceKind::Symbol(SymbolEvidenceKind::ImportedBinding {
            module_hash: stable_symbol_hash("collections"),
            exported_hash: stable_symbol_hash("deque"),
        }),
        EvidenceStatus::Asserted,
    ));
    assert!(imported_binding_symbol(
        &il,
        &interner,
        use_site,
        "collections",
        "deque"
    ));
}

#[test]
fn imported_occurrence_symbol_evidence_requires_binding_dependency() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let local_hash = stable_symbol_hash("m");
    let receiver = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("m")),
        sp(20),
        &[],
    );
    let root = b.add(NodeKind::Module, Payload::None, sp(20), &[receiver]);
    let mut il = finish_il(b, root, Lang::Python);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::node(sp(20), NodeKind::Var),
        EvidenceKind::Symbol(SymbolEvidenceKind::ImportedNamespace {
            module_hash: stable_symbol_hash("math"),
        }),
        EvidenceStatus::Asserted,
    ));

    assert!(!imported_namespace_symbol(&il, &interner, receiver, "math"));

    il.evidence.clear();
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::binding(sp(19), local_hash),
        EvidenceKind::Symbol(SymbolEvidenceKind::ImportedNamespace {
            module_hash: stable_symbol_hash("math"),
        }),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(evidence_with_dependencies(
        1,
        EvidenceAnchor::node(sp(20), NodeKind::Var),
        EvidenceKind::Symbol(SymbolEvidenceKind::ImportedNamespace {
            module_hash: stable_symbol_hash("math"),
        }),
        EvidenceStatus::Asserted,
        vec![EvidenceId(0)],
    ));

    assert!(imported_namespace_symbol(&il, &interner, receiver, "math"));
    assert!(!imported_namespace_symbol(
        &il,
        &interner,
        receiver,
        "collections"
    ));
}

#[test]
fn symbol_evidence_blocks_import_assignment_fallback() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let local = interner.intern("math");
    let lhs = b.add(NodeKind::Var, Payload::Name(local), sp(21), &[]);
    let module = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("math")),
        sp(21),
        &[],
    );
    let rhs = b.add(NodeKind::Seq, Payload::None, sp(21), &[module]);
    let assign = b.add(NodeKind::Assign, Payload::None, sp(21), &[lhs, rhs]);
    let receiver = b.add(NodeKind::Var, Payload::Name(local), sp(22), &[]);
    let root = b.add(NodeKind::Module, Payload::None, sp(21), &[assign, receiver]);
    let mut il = finish_il(b, root, Lang::Python);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::binding(sp(21), stable_symbol_hash("math")),
        EvidenceKind::Symbol(SymbolEvidenceKind::ImportedNamespace {
            module_hash: stable_symbol_hash("other"),
        }),
        EvidenceStatus::Asserted,
    ));

    assert!(!imported_namespace_symbol(&il, &interner, receiver, "math"));
}

#[test]
fn binding_symbol_evidence_does_not_prove_rebound_alias_uses() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let local = interner.intern("math");
    let lhs = b.add(NodeKind::Var, Payload::Name(local), sp(24), &[]);
    let module = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("math")),
        sp(24),
        &[],
    );
    let rhs = b.add(NodeKind::Seq, Payload::None, sp(24), &[module]);
    let import_assign = b.add(NodeKind::Assign, Payload::None, sp(24), &[lhs, rhs]);
    let rebound_lhs = b.add(NodeKind::Var, Payload::Name(local), sp(25), &[]);
    let rebound_rhs = b.add(NodeKind::Lit, Payload::LitInt(0), sp(25), &[]);
    let rebound = b.add(
        NodeKind::Assign,
        Payload::None,
        sp(25),
        &[rebound_lhs, rebound_rhs],
    );
    let receiver = b.add(NodeKind::Var, Payload::Name(local), sp(26), &[]);
    let root = b.add(
        NodeKind::Module,
        Payload::None,
        sp(24),
        &[import_assign, rebound, receiver],
    );
    let mut il = finish_il(b, root, Lang::Python);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::binding(sp(24), stable_symbol_hash("math")),
        EvidenceKind::Symbol(SymbolEvidenceKind::ImportedNamespace {
            module_hash: stable_symbol_hash("math"),
        }),
        EvidenceStatus::Asserted,
    ));

    assert!(!imported_namespace_symbol(&il, &interner, receiver, "math"));
}

#[test]
fn ambiguous_global_symbol_evidence_blocks_name_fallback() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let math = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("Math")),
        sp(23),
        &[],
    );
    let root = b.add(NodeKind::Module, Payload::None, sp(23), &[math]);
    let mut il = finish_il(b, root, Lang::JavaScript);

    assert!(unshadowed_global_symbol(&il, &interner, math, "Math"));

    il.evidence.push(evidence(
        0,
        EvidenceAnchor::node(sp(23), NodeKind::Var),
        EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Math"),
        }),
        EvidenceStatus::Ambiguous,
    ));
    assert!(!unshadowed_global_symbol(&il, &interner, math, "Math"));
}

fn js_array_is_array_call_il(interner: &Interner) -> (Il, NodeId, NodeId, NodeId) {
    let mut b = IlBuilder::new(FileId(0));
    let array = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("Array")),
        sp(29),
        &[],
    );
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("isArray")),
        sp(30),
        &[array],
    );
    let value = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("value")),
        sp(31),
        &[],
    );
    let call = b.add(NodeKind::Call, Payload::None, sp(32), &[callee, value]);
    let root = b.add(NodeKind::Module, Payload::None, sp(29), &[call]);
    (finish_il(b, root, Lang::JavaScript), call, callee, array)
}

fn library_api_record(
    id: u32,
    span: Span,
    contract_id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    status: EvidenceStatus,
    dependencies: &[u32],
) -> EvidenceRecord {
    evidence_with_dependencies(
        id,
        EvidenceAnchor::node(span, NodeKind::Call),
        EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
            contract_hash: library_api_contract_id_hash(contract_id),
            callee_hash: library_api_callee_contract_hash(callee),
            arity: 1,
        }),
        status,
        dependencies.iter().copied().map(EvidenceId).collect(),
    )
}

fn canonical_builtin_call_il(
    lang: Lang,
    builtin: Builtin,
    args: &[NodeId],
    builder: IlBuilder,
    root: NodeId,
) -> (Il, NodeId) {
    let mut builder = builder;
    let call = builder.add(NodeKind::Call, Payload::Builtin(builtin), sp(40), args);
    let root = builder.add(NodeKind::Func, Payload::None, sp(41), &[root, call]);
    (finish_il(builder, root, lang), call)
}

fn python_len_canonical_call_il() -> (Il, NodeId) {
    let mut b = IlBuilder::new(FileId(0));
    let arg = b.add(NodeKind::Var, Payload::Cid(0), sp(39), &[]);
    canonical_builtin_call_il(Lang::Python, Builtin::Len, &[arg], b, arg)
}

#[test]
fn canonical_builtin_admission_requires_language_core_or_library_api_evidence() {
    let (mut il, call) = python_len_canonical_call_il();

    assert!(!admitted_builtin_semantics_at_call(&il, call, Builtin::Len));

    let contract = library_free_function_builtin_contract(Lang::Python, "len", 1)
        .expect("Python len contract");
    il.evidence.push(library_api_record(
        10,
        il.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[],
    ));
    assert!(admitted_builtin_semantics_at_call(&il, call, Builtin::Len));
    assert!(!admitted_builtin_semantics_at_call(&il, call, Builtin::Abs));
}

#[test]
fn rust_map_get_unwrap_or_canonical_builtin_uses_map_get_dependency() {
    let mut b = IlBuilder::new(FileId(0));
    let map = b.add(NodeKind::Var, Payload::Cid(0), sp(38), &[]);
    let key = b.add(NodeKind::Var, Payload::Cid(1), sp(39), &[]);
    let default = b.add(NodeKind::Lit, Payload::LitInt(0), sp(40), &[]);
    let (mut il, call) = canonical_builtin_call_il(
        Lang::Rust,
        Builtin::GetOrDefault,
        &[map, key, default],
        b,
        map,
    );
    let map_get = library_map_get_contract(Lang::Rust, "get", 1).expect("Rust map get contract");
    let unwrap_or =
        library_method_call_contract(Lang::Rust, "unwrap_or", 1).expect("Rust unwrap_or contract");

    il.evidence.push(library_api_record(
        10,
        sp(39),
        map_get.id,
        map_get.callee,
        EvidenceStatus::Asserted,
        &[],
    ));
    il.evidence.push(library_api_record(
        11,
        il.node(call).span,
        unwrap_or.id,
        unwrap_or.callee,
        EvidenceStatus::Asserted,
        &[10],
    ));

    assert!(admitted_builtin_semantics_at_call(
        &il,
        call,
        Builtin::GetOrDefault
    ));
    assert!(!admitted_builtin_semantics_at_call(
        &il,
        call,
        Builtin::ValueOrDefault
    ));
}

#[test]
fn rust_value_default_contract_alone_does_not_admit_map_default_builtin() {
    let mut b = IlBuilder::new(FileId(0));
    let value = b.add(NodeKind::Var, Payload::Cid(0), sp(38), &[]);
    let default = b.add(NodeKind::Lit, Payload::LitInt(0), sp(39), &[]);
    let (mut il, call) = canonical_builtin_call_il(
        Lang::Rust,
        Builtin::GetOrDefault,
        &[value, default],
        b,
        value,
    );
    let unwrap_or =
        library_method_call_contract(Lang::Rust, "unwrap_or", 1).expect("Rust unwrap_or contract");

    il.evidence.push(library_api_record(
        10,
        il.node(call).span,
        unwrap_or.id,
        unwrap_or.callee,
        EvidenceStatus::Asserted,
        &[],
    ));

    assert!(!admitted_builtin_semantics_at_call(
        &il,
        call,
        Builtin::GetOrDefault
    ));
}

#[test]
fn canonical_builtin_admission_fails_closed_on_bad_library_api_evidence() {
    let contract = library_free_function_builtin_contract(Lang::Python, "len", 1)
        .expect("Python len contract");

    let (mut broken, broken_call) = python_len_canonical_call_il();
    broken.evidence.push(library_api_record(
        10,
        broken.node(broken_call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[99],
    ));
    assert!(!admitted_builtin_semantics_at_call(
        &broken,
        broken_call,
        Builtin::Len
    ));

    let (mut ambiguous, ambiguous_call) = python_len_canonical_call_il();
    ambiguous.evidence.push(library_api_record(
        10,
        ambiguous.node(ambiguous_call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Ambiguous,
        &[],
    ));
    assert!(!admitted_builtin_semantics_at_call(
        &ambiguous,
        ambiguous_call,
        Builtin::Len
    ));

    let (mut conflicting, conflicting_call) = python_len_canonical_call_il();
    let abs = library_free_function_builtin_contract(Lang::Python, "abs", 1)
        .expect("Python abs contract");
    conflicting.evidence.push(library_api_record(
        10,
        conflicting.node(conflicting_call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[],
    ));
    conflicting.evidence.push(library_api_record(
        11,
        conflicting.node(conflicting_call).span,
        abs.id,
        abs.callee,
        EvidenceStatus::Asserted,
        &[],
    ));
    assert!(!admitted_builtin_semantics_at_call(
        &conflicting,
        conflicting_call,
        Builtin::Len
    ));
}

#[test]
fn canonical_property_builtin_admission_accepts_field_span_evidence() {
    let mut b = IlBuilder::new(FileId(0));
    let collection = b.add(NodeKind::Var, Payload::Cid(0), sp(39), &[]);
    let call = b.add(
        NodeKind::Call,
        Payload::Builtin(Builtin::Len),
        sp(40),
        &[collection],
    );
    let root = b.add(NodeKind::Func, Payload::None, sp(41), &[call]);
    let mut il = finish_il(b, root, Lang::JavaScript);
    let contract =
        library_property_builtin_contract(Lang::JavaScript, "length").expect("length contract");
    il.evidence.push(evidence_with_dependencies(
        10,
        EvidenceAnchor::node(il.node(call).span, NodeKind::Field),
        EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
            contract_hash: library_api_contract_id_hash(contract.id),
            callee_hash: library_api_callee_contract_hash(contract.callee),
            arity: 0,
        }),
        EvidenceStatus::Asserted,
        Vec::new(),
    ));

    assert!(admitted_builtin_semantics_at_call(&il, call, Builtin::Len));
}

#[test]
fn canonical_builtin_admission_keeps_language_core_exceptions_narrow() {
    let mut go = IlBuilder::new(FileId(0));
    let key = go.add(NodeKind::Var, Payload::Cid(0), sp(39), &[]);
    let map = go.add(NodeKind::Var, Payload::Cid(1), sp(40), &[]);
    let (go_il, contains) =
        canonical_builtin_call_il(Lang::Go, Builtin::Contains, &[key, map], go, map);
    assert!(admitted_builtin_semantics_at_call(
        &go_il,
        contains,
        Builtin::Contains
    ));

    let mut py = IlBuilder::new(FileId(0));
    let dict_key = py.add(NodeKind::Var, Payload::Cid(0), sp(39), &[]);
    let dict_value = py.add(NodeKind::Var, Payload::Cid(1), sp(40), &[]);
    let (py_il, dict_entry) = canonical_builtin_call_il(
        Lang::Python,
        Builtin::DictEntry,
        &[dict_key, dict_value],
        py,
        dict_value,
    );
    assert!(admitted_builtin_semantics_at_call(
        &py_il,
        dict_entry,
        Builtin::DictEntry
    ));

    let mut raw_len = IlBuilder::new(FileId(0));
    let arg = raw_len.add(NodeKind::Var, Payload::Cid(0), sp(39), &[]);
    let (go_len_il, go_len) =
        canonical_builtin_call_il(Lang::Go, Builtin::Len, &[arg], raw_len, arg);
    assert!(!admitted_builtin_semantics_at_call(
        &go_len_il,
        go_len,
        Builtin::Len
    ));
}

#[test]
fn c_unsigned_cast_builtin_admission_requires_source_cast_evidence() {
    let mut b = IlBuilder::new(FileId(0));
    let arg = b.add(NodeKind::Var, Payload::Cid(0), sp(39), &[]);
    let (mut il, call) =
        canonical_builtin_call_il(Lang::C, Builtin::UnsignedCast32, &[arg], b, arg);

    assert!(!admitted_builtin_semantics_at_call(
        &il,
        call,
        Builtin::UnsignedCast32
    ));

    il.evidence.push(evidence(
        10,
        EvidenceAnchor::node(il.node(call).span, NodeKind::Call),
        EvidenceKind::Source(SourceFactKind::Cast(SourceCastKind::CUnsigned32)),
        EvidenceStatus::Asserted,
    ));
    assert!(admitted_builtin_semantics_at_call(
        &il,
        call,
        Builtin::UnsignedCast32
    ));
}

fn add_with_len_rhs_il() -> (Il, NodeId) {
    let mut b = IlBuilder::new(FileId(0));
    let px = b.add(NodeKind::Param, Payload::Cid(0), sp(50), &[]);
    let py = b.add(NodeKind::Param, Payload::Cid(1), sp(51), &[]);
    let x = b.add(NodeKind::Var, Payload::Cid(0), sp(52), &[]);
    let y = b.add(NodeKind::Var, Payload::Cid(1), sp(53), &[]);
    let len = b.add(NodeKind::Call, Payload::Builtin(Builtin::Len), sp(54), &[y]);
    let add = b.add(NodeKind::BinOp, Payload::Op(Op::Add), sp(55), &[x, len]);
    let ret = b.add(NodeKind::Return, Payload::None, sp(56), &[add]);
    let root = b.add(NodeKind::Func, Payload::None, sp(57), &[px, py, ret]);
    (finish_il(b, root, Lang::Python), len)
}

#[test]
fn value_domain_inference_requires_admitted_builtin_result_domains() {
    let (mut il, len) = add_with_len_rhs_il();
    assert_eq!(
        semantics(Lang::Python)
            .operators()
            .infer_param_value_domains(&il, il.root),
        vec![ValueDomain::Unknown, ValueDomain::Unknown],
        "raw canonical Len payload must not prove a numeric result domain"
    );

    let contract = library_free_function_builtin_contract(Lang::Python, "len", 1)
        .expect("Python len contract");
    il.evidence.push(library_api_record(
        10,
        il.node(len).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[],
    ));
    assert_eq!(
        semantics(Lang::Python)
            .operators()
            .infer_param_value_domains(&il, il.root),
        vec![ValueDomain::Number, ValueDomain::Unknown],
        "admitted Python len can contribute its numeric result domain"
    );
}

fn java_list_of_import_evidence_il(
    interner: &Interner,
    import_in_root: bool,
) -> (Il, NodeId, NodeId, Symbol, LibraryCollectionFactoryContract) {
    let mut b = IlBuilder::new(FileId(0));
    let local = interner.intern("List");
    let lhs = b.add(NodeKind::Var, Payload::Name(local), sp(30), &[]);
    let rhs = b.add(NodeKind::Seq, Payload::None, sp(30), &[]);
    let import = b.add(NodeKind::Assign, Payload::None, sp(30), &[lhs, rhs]);
    let receiver = b.add(NodeKind::Var, Payload::Name(local), sp(31), &[]);
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("of")),
        sp(32),
        &[receiver],
    );
    let arg = b.add(NodeKind::Lit, Payload::LitInt(1), sp(33), &[]);
    let call = b.add(NodeKind::Call, Payload::None, sp(34), &[callee, arg]);
    let root = if import_in_root {
        b.add(NodeKind::Module, Payload::None, sp(29), &[import, call])
    } else {
        b.add(NodeKind::Func, Payload::None, sp(35), &[call])
    };
    let mut il = finish_il(b, root, Lang::Java);
    let contract = library_java_collection_factory_contract(Lang::Java, "List", "of")
        .expect("List.of contract");
    let binding_symbol = EvidenceKind::Symbol(SymbolEvidenceKind::ImportedBinding {
        module_hash: stable_symbol_hash("java.util"),
        exported_hash: stable_symbol_hash("List"),
    });
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::binding(sp(30), stable_symbol_hash("List")),
        binding_symbol,
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(evidence_with_dependencies(
        1,
        EvidenceAnchor::node(sp(31), NodeKind::Var),
        binding_symbol,
        EvidenceStatus::Asserted,
        vec![EvidenceId(0)],
    ));
    il.evidence.push(library_api_record(
        2,
        sp(34),
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[1],
    ));
    (il, call, root, local, contract)
}

#[test]
fn library_api_evidence_resolution_is_dependency_backed_and_fail_closed() {
    let interner = Interner::new();
    let (mut il, call, callee, array) = js_array_is_array_call_il(&interner);
    let contract = library_js_array_is_array_contract(Lang::JavaScript, "Array", "isArray", 1)
        .expect("test contract");

    assert_eq!(
        library_api_contract_evidence_for_call(
            &il,
            &interner,
            call,
            contract.id,
            contract.callee,
            1,
        ),
        LibraryApiEvidenceStatus::Missing
    );

    il.evidence.push(evidence(
        0,
        EvidenceAnchor::node(il.node(array).span, NodeKind::Var),
        EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Array"),
        }),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(evidence(
        1,
        EvidenceAnchor::source_span(il.node(callee).span),
        EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Array"),
        }),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(evidence_with_dependencies(
        2,
        EvidenceAnchor::node(il.node(callee).span, NodeKind::Field),
        EvidenceKind::Symbol(SymbolEvidenceKind::QualifiedGlobal {
            path_hash: stable_symbol_hash("Array.isArray"),
        }),
        EvidenceStatus::Asserted,
        vec![EvidenceId(1)],
    ));
    il.evidence.push(library_api_record(
        3,
        il.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[0, 2],
    ));
    assert_eq!(
        library_api_contract_evidence_for_call(
            &il,
            &interner,
            call,
            contract.id,
            contract.callee,
            1,
        ),
        LibraryApiEvidenceStatus::Admitted
    );
    assert_eq!(
        library_api_contract_evidence_at_call_span(
            &il,
            &interner,
            LibraryApiSpanEvidenceQuery {
                call_span: Some(il.node(call).span),
                callee_span: Some(il.node(callee).span),
                receiver_span: Some(il.node(array).span),
                id: contract.id,
                callee: contract.callee,
                arg_count: 1,
            },
        ),
        LibraryApiEvidenceStatus::Admitted
    );
    assert_eq!(
        library_api_contract_evidence_at_call_span(
            &il,
            &interner,
            LibraryApiSpanEvidenceQuery {
                call_span: Some(il.node(call).span),
                callee_span: Some(sp(99)),
                receiver_span: Some(il.node(array).span),
                id: contract.id,
                callee: contract.callee,
                arg_count: 1,
            },
        ),
        LibraryApiEvidenceStatus::Rejected
    );
    assert_eq!(
        library_api_contract_evidence_at_call_span(
            &il,
            &interner,
            LibraryApiSpanEvidenceQuery {
                call_span: Some(il.node(call).span),
                callee_span: Some(il.node(callee).span),
                receiver_span: Some(sp(99)),
                id: contract.id,
                callee: contract.callee,
                arg_count: 1,
            },
        ),
        LibraryApiEvidenceStatus::Rejected
    );

    let (mut missing_dep, call, _callee, _array) = js_array_is_array_call_il(&interner);
    missing_dep.evidence.push(library_api_record(
        0,
        missing_dep.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[],
    ));
    assert_eq!(
        library_api_contract_evidence_for_call(
            &missing_dep,
            &interner,
            call,
            contract.id,
            contract.callee,
            1,
        ),
        LibraryApiEvidenceStatus::Rejected
    );

    let (mut ambiguous_dep, call, callee, array) = js_array_is_array_call_il(&interner);
    ambiguous_dep.evidence.push(evidence(
        0,
        EvidenceAnchor::node(ambiguous_dep.node(array).span, NodeKind::Var),
        EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Array"),
        }),
        EvidenceStatus::Ambiguous,
    ));
    ambiguous_dep.evidence.push(evidence(
        1,
        EvidenceAnchor::node(ambiguous_dep.node(callee).span, NodeKind::Field),
        EvidenceKind::Symbol(SymbolEvidenceKind::QualifiedGlobal {
            path_hash: stable_symbol_hash("Array.isArray"),
        }),
        EvidenceStatus::Asserted,
    ));
    ambiguous_dep.evidence.push(library_api_record(
        2,
        ambiguous_dep.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[0, 1],
    ));
    assert_eq!(
        library_api_contract_evidence_for_call(
            &ambiguous_dep,
            &interner,
            call,
            contract.id,
            contract.callee,
            1,
        ),
        LibraryApiEvidenceStatus::Rejected
    );

    let (mut conflicting_dep, call, callee, array) = js_array_is_array_call_il(&interner);
    conflicting_dep.evidence.push(evidence(
        0,
        EvidenceAnchor::node(conflicting_dep.node(array).span, NodeKind::Var),
        EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Array"),
        }),
        EvidenceStatus::Asserted,
    ));
    conflicting_dep.evidence.push(evidence(
        1,
        EvidenceAnchor::node(conflicting_dep.node(callee).span, NodeKind::Field),
        EvidenceKind::Symbol(SymbolEvidenceKind::QualifiedGlobal {
            path_hash: stable_symbol_hash("Array.isArray"),
        }),
        EvidenceStatus::Asserted,
    ));
    conflicting_dep.evidence.push(evidence(
        2,
        EvidenceAnchor::node(conflicting_dep.node(array).span, NodeKind::Var),
        EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Map"),
        }),
        EvidenceStatus::Asserted,
    ));
    conflicting_dep.evidence.push(library_api_record(
        3,
        conflicting_dep.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[0, 1],
    ));
    assert_eq!(
        library_api_contract_evidence_for_call(
            &conflicting_dep,
            &interner,
            call,
            contract.id,
            contract.callee,
            1,
        ),
        LibraryApiEvidenceStatus::Rejected
    );

    let boolean = library_js_boolean_coercion_contract(Lang::JavaScript, "Boolean", 1).unwrap();
    il.evidence.push(library_api_record(
        3,
        il.node(call).span,
        boolean.id,
        boolean.callee,
        EvidenceStatus::Asserted,
        &[0],
    ));
    assert_eq!(
        library_api_contract_evidence_for_call(
            &il,
            &interner,
            call,
            contract.id,
            contract.callee,
            1,
        ),
        LibraryApiEvidenceStatus::Rejected
    );

    let (mut wrong_anchor, call, _callee, _array) = js_array_is_array_call_il(&interner);
    wrong_anchor.evidence.push(library_api_record(
        0,
        sp(99),
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[],
    ));
    assert_eq!(
        library_api_contract_evidence_for_call(
            &wrong_anchor,
            &interner,
            call,
            contract.id,
            contract.callee,
            1,
        ),
        LibraryApiEvidenceStatus::Missing
    );
}

#[test]
fn library_api_evidence_resolution_accepts_import_and_source_backed_callees() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let local = interner.intern("Values");
    let lhs = b.add(NodeKind::Var, Payload::Name(local), sp(10), &[]);
    let rhs = b.add(NodeKind::Seq, Payload::None, sp(10), &[]);
    let import = b.add(NodeKind::Assign, Payload::None, sp(10), &[lhs, rhs]);
    let callee = b.add(NodeKind::Var, Payload::Name(local), sp(11), &[]);
    let arg = b.add(
        NodeKind::Seq,
        Payload::Name(interner.intern("array")),
        sp(12),
        &[],
    );
    let call = b.add(NodeKind::Call, Payload::None, sp(13), &[callee, arg]);
    let root = b.add(NodeKind::Module, Payload::None, sp(9), &[import, call]);
    let mut il = finish_il(b, root, Lang::Python);
    let contract =
        library_imported_collection_factory_contract(Lang::Python, "collections", "deque")
            .expect("deque contract");
    let binding_symbol = EvidenceKind::Symbol(SymbolEvidenceKind::ImportedBinding {
        module_hash: stable_symbol_hash("collections"),
        exported_hash: stable_symbol_hash("deque"),
    });
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::binding(sp(10), stable_symbol_hash("Values")),
        binding_symbol,
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(evidence_with_dependencies(
        1,
        EvidenceAnchor::node(sp(11), NodeKind::Var),
        binding_symbol,
        EvidenceStatus::Asserted,
        vec![EvidenceId(0)],
    ));
    il.evidence.push(library_api_record(
        2,
        sp(13),
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[1],
    ));
    assert_eq!(
        library_api_contract_evidence_for_call(
            &il,
            &interner,
            call,
            contract.id,
            contract.callee,
            1,
        ),
        LibraryApiEvidenceStatus::Admitted
    );

    let mut b = IlBuilder::new(FileId(0));
    let regex = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("/x/")),
        sp(20),
        &[],
    );
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("test")),
        sp(21),
        &[regex],
    );
    let arg = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("s")),
        sp(22),
        &[],
    );
    let call = b.add(NodeKind::Call, Payload::None, sp(23), &[callee, arg]);
    let root = b.add(NodeKind::Module, Payload::None, sp(19), &[call]);
    let mut il = finish_il(b, root, Lang::JavaScript);
    let contract =
        library_regex_test_contract(Lang::JavaScript, "test", 1).expect("regex contract");
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::source_span(sp(20)),
        EvidenceKind::Source(SourceFactKind::Literal(SourceLiteralKind::Regex)),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(library_api_record(
        1,
        sp(23),
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[0],
    ));
    assert_eq!(
        library_api_contract_evidence_for_call(
            &il,
            &interner,
            call,
            contract.id,
            contract.callee,
            1,
        ),
        LibraryApiEvidenceStatus::Admitted
    );
}

#[test]
fn library_api_evidence_resolution_accepts_free_name_and_require_backed_callees() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let callee = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("list")),
        sp(40),
        &[],
    );
    let arg = b.add(
        NodeKind::Seq,
        Payload::Name(interner.intern("array")),
        sp(41),
        &[],
    );
    let call = b.add(NodeKind::Call, Payload::None, sp(42), &[callee, arg]);
    let root = b.add(NodeKind::Module, Payload::None, sp(39), &[call]);
    let mut il = finish_il(b, root, Lang::Python);
    let contract = library_free_name_collection_factory_contract(Lang::Python, "list")
        .expect("Python list contract");
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::node(sp(40), NodeKind::Var),
        EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("list"),
        }),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(library_api_record(
        1,
        sp(42),
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[0],
    ));
    assert_eq!(
        library_api_contract_evidence_for_call(
            &il,
            &interner,
            call,
            contract.id,
            contract.callee,
            1,
        ),
        LibraryApiEvidenceStatus::Admitted
    );
    assert_eq!(
        library_api_contract_evidence_at_call_span(
            &il,
            &interner,
            LibraryApiSpanEvidenceQuery {
                call_span: Some(sp(42)),
                callee_span: Some(sp(40)),
                receiver_span: None,
                id: contract.id,
                callee: contract.callee,
                arg_count: 1,
            },
        ),
        LibraryApiEvidenceStatus::Admitted
    );

    let mut b = IlBuilder::new(FileId(0));
    let callee = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("len")),
        sp(45),
        &[],
    );
    let arg = b.add(
        NodeKind::Seq,
        Payload::Name(interner.intern("array")),
        sp(46),
        &[],
    );
    let call = b.add(NodeKind::Call, Payload::None, sp(47), &[callee, arg]);
    let root = b.add(NodeKind::Module, Payload::None, sp(44), &[call]);
    let mut il = finish_il(b, root, Lang::Python);
    let contract = library_free_function_builtin_contract(Lang::Python, "len", 1)
        .expect("Python len contract");
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::node(sp(45), NodeKind::Var),
        EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("len"),
        }),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(library_api_record(
        1,
        sp(47),
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[0],
    ));
    assert_eq!(
        library_api_contract_evidence_for_call(
            &il,
            &interner,
            call,
            contract.id,
            contract.callee,
            1,
        ),
        LibraryApiEvidenceStatus::Admitted
    );

    let mut b = IlBuilder::new(FileId(0));
    let require_callee = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("require")),
        sp(48),
        &[],
    );
    let require_arg = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("set")),
        sp(48),
        &[],
    );
    let require_call = b.add(
        NodeKind::Call,
        Payload::None,
        sp(48),
        &[require_callee, require_arg],
    );
    let set = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("Set")),
        sp(50),
        &[],
    );
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("new")),
        sp(51),
        &[set],
    );
    let arg = b.add(
        NodeKind::Seq,
        Payload::Name(interner.intern("array")),
        sp(52),
        &[],
    );
    let call = b.add(NodeKind::Call, Payload::None, sp(53), &[callee, arg]);
    let root = b.add(
        NodeKind::Module,
        Payload::None,
        sp(49),
        &[require_call, call],
    );
    let mut il = finish_il(b, root, Lang::Ruby);
    let contract = library_ruby_set_factory_contract(Lang::Ruby, "Set", "new", 1).expect("Set.new");
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::node(sp(50), NodeKind::Var),
        EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Set"),
        }),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(evidence(
        1,
        EvidenceAnchor::node(sp(48), NodeKind::Var),
        EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("require"),
        }),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(evidence_with_dependencies(
        2,
        EvidenceAnchor::source_span(sp(48)),
        EvidenceKind::Import(ImportEvidenceKind::Require {
            module_hash: stable_symbol_hash("set"),
        }),
        EvidenceStatus::Asserted,
        vec![EvidenceId(1)],
    ));
    il.evidence.push(library_api_record(
        3,
        sp(53),
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[0, 2],
    ));
    assert_eq!(
        library_api_contract_evidence_for_call(
            &il,
            &interner,
            call,
            contract.id,
            contract.callee,
            1,
        ),
        LibraryApiEvidenceStatus::Admitted
    );
}

#[test]
fn library_api_evidence_resolution_accepts_import_binding_outside_unit_root() {
    let interner = Interner::new();
    let (mut il, call, _root, _local, contract) = java_list_of_import_evidence_il(&interner, false);
    assert_eq!(
        library_api_contract_evidence_for_call(
            &il,
            &interner,
            call,
            contract.id,
            contract.callee,
            1,
        ),
        LibraryApiEvidenceStatus::Admitted
    );
    assert_eq!(
        library_api_contract_evidence_at_call_span(
            &il,
            &interner,
            LibraryApiSpanEvidenceQuery {
                call_span: Some(sp(34)),
                callee_span: Some(sp(32)),
                receiver_span: Some(sp(30)),
                id: contract.id,
                callee: contract.callee,
                arg_count: 1,
            },
        ),
        LibraryApiEvidenceStatus::Rejected
    );
    assert_eq!(
        library_api_contract_evidence_at_call_span(
            &il,
            &interner,
            LibraryApiSpanEvidenceQuery {
                call_span: Some(sp(34)),
                callee_span: Some(sp(32)),
                receiver_span: None,
                id: contract.id,
                callee: contract.callee,
                arg_count: 1,
            },
        ),
        LibraryApiEvidenceStatus::Admitted
    );

    il.evidence.push(evidence(
        3,
        EvidenceAnchor::binding(sp(36), stable_symbol_hash("List")),
        EvidenceKind::Symbol(SymbolEvidenceKind::ImportedBinding {
            module_hash: stable_symbol_hash("other.module"),
            exported_hash: stable_symbol_hash("List"),
        }),
        EvidenceStatus::Asserted,
    ));
    assert_eq!(
        library_api_contract_evidence_for_call(
            &il,
            &interner,
            call,
            contract.id,
            contract.callee,
            1,
        ),
        LibraryApiEvidenceStatus::Rejected
    );
}

#[test]
fn library_api_evidence_resolution_rejects_shadowed_java_static_members() {
    let interner = Interner::new();
    let (mut il, call, root, local, contract) = java_list_of_import_evidence_il(&interner, true);
    assert_eq!(
        library_api_contract_evidence_for_call(
            &il,
            &interner,
            call,
            contract.id,
            contract.callee,
            1,
        ),
        LibraryApiEvidenceStatus::Admitted
    );

    il.units.push(Unit {
        root,
        kind: UnitKind::Class,
        name: Some(local),
    });
    assert_eq!(
        library_api_contract_evidence_for_call(
            &il,
            &interner,
            call,
            contract.id,
            contract.callee,
            1,
        ),
        LibraryApiEvidenceStatus::Rejected
    );
}

#[test]
fn language_predicates_preserve_existing_gates() {
    for &lang in ALL_LANGS {
        let profile = semantics(lang);
        assert_eq!(
            profile.operators().primitive_order_comparisons(),
            matches!(lang, Lang::C | Lang::Go | Lang::Java)
        );
        let byte_pack = profile
            .operators()
            .c_integer_byte_pack_contract(CBytePackWidth::U32);
        assert_eq!(byte_pack.is_some(), lang == Lang::C);
        if let Some(contract) = byte_pack {
            assert_eq!(contract.base_domain, DomainRequirement::ByteArray);
            assert_eq!(
                contract.required_high_lane_cast,
                Some(SourceFactKind::Cast(SourceCastKind::CUnsigned32))
            );
        }
        assert_eq!(
            profile.effects().non_overloadable_index_assignment(),
            matches!(lang, Lang::C | Lang::Go | Lang::Java)
        );
        assert_eq!(
            profile.effects().java_this_field_place(),
            lang == Lang::Java
        );
        assert_eq!(
            profile.modules().js_like_shadowed_module_bindings(),
            matches!(
                lang,
                Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html
            )
        );
        assert_eq!(
            profile.modules().java_class_literal_exports(),
            lang == Lang::Java
        );
        assert_eq!(
            profile.modules().java_type_declarations_shadow_stdlib(),
            lang == Lang::Java
        );
        assert_eq!(
            profile.modules().go_import_namespace_facts(),
            lang == Lang::Go
        );
    }
}

#[test]
fn stdlib_predicates_preserve_existing_gates() {
    for &lang in ALL_LANGS {
        let stdlib = semantics(lang).stdlib();
        assert_eq!(stdlib.python_collection_factories(), lang == Lang::Python);
        assert_eq!(stdlib.python_deque_factory(), lang == Lang::Python);
        assert_eq!(stdlib.java_collection_factories(), lang == Lang::Java);
        assert_eq!(stdlib.java_map_factories(), lang == Lang::Java);
        assert_eq!(stdlib.java_primitive_integer_ops(), lang == Lang::Java);
        assert_eq!(stdlib.ruby_set_factory(), lang == Lang::Ruby);
        assert_eq!(stdlib.rust_vec_macro_factory(), lang == Lang::Rust);
        assert_eq!(stdlib.rust_vec_new_factory(), lang == Lang::Rust);
        assert_eq!(stdlib.rust_std_collection_factories(), lang == Lang::Rust);
        assert_eq!(stdlib.rust_std_map_factories(), lang == Lang::Rust);
        assert_eq!(stdlib.go_literal_zero_map_lookup(), lang == Lang::Go);
        assert_eq!(stdlib.rust_filter_map_option_contract(), lang == Lang::Rust);
    }
}

#[test]
fn free_name_contracts_are_behavior_equivalent_tables() {
    let py_names: Vec<_> = semantics(Lang::Python)
        .collections()
        .free_name_collection_factories()
        .flat_map(|factory| factory.names.iter().copied())
        .collect();
    assert!(py_names.contains(&"list"));
    assert!(py_names.contains(&"frozenset"));
    assert!(!py_names.contains(&"Set"));

    let imported_py_names: Vec<_> = semantics(Lang::Python)
        .collections()
        .imported_collection_factories()
        .map(|factory| (factory.module, factory.exported))
        .collect();
    assert_eq!(imported_py_names, vec![("collections", "deque")]);

    let rust_map_tags: Vec<_> = semantics(Lang::Rust)
        .collections()
        .free_name_map_factories()
        .map(|factory| factory.entry_seq_tag)
        .collect();
    assert_eq!(rust_map_tags, vec![2]);

    let js_map_tags: Vec<_> = semantics(Lang::JavaScript)
        .collections()
        .free_name_map_factories()
        .map(|factory| factory.entry_seq_tag)
        .collect();
    assert!(js_map_tags.is_empty());
}

#[test]
fn library_api_contracts_carry_identity_and_result_obligations() {
    assert_eq!(
        library_free_name_collection_factory_contract(Lang::Python, "list"),
        Some(LibraryCollectionFactoryContract {
            id: LibraryApiContractId::PythonBuiltinCollectionFactory,
            callee: LibraryApiCalleeContract::FreeName {
                name: "list",
                shadow: LibraryApiShadowPolicy::SameName,
            },
            result: LibraryCollectionFactoryResult::SequenceArgument,
        })
    );
    assert_eq!(
        library_free_function_builtin_contract(Lang::Python, "len", 1),
        Some(LibraryFreeFunctionBuiltinContract {
            id: LibraryApiContractId::FreeFunctionBuiltin(Builtin::Len),
            callee: LibraryApiCalleeContract::FreeName {
                name: "len",
                shadow: LibraryApiShadowPolicy::SameName,
            },
            result: FreeFunctionBuiltinContract {
                name: "len",
                builtin: Builtin::Len,
                args: BuiltinArgContract::First,
                requires_unshadowed: true,
            },
        })
    );
    assert_eq!(
        library_imported_collection_factory_contract(Lang::Python, "collections", "deque"),
        Some(LibraryCollectionFactoryContract {
            id: LibraryApiContractId::PythonImportedCollectionFactory,
            callee: LibraryApiCalleeContract::ImportedBinding {
                module: "collections",
                exported: "deque",
            },
            result: LibraryCollectionFactoryResult::SequenceArgument,
        })
    );
    assert_eq!(
        library_free_name_map_factory_contract(Lang::Rust, "std::collections::HashMap::from"),
        Some(LibraryMapFactoryContract {
            id: LibraryApiContractId::RustStdMapFactory,
            callee: LibraryApiCalleeContract::FreeName {
                name: "std::collections::HashMap::from",
                shadow: LibraryApiShadowPolicy::RustStdRootForStdPath,
            },
            result: LibraryMapFactoryResult::EntrySequence {
                entry_seq_tag: SEQ_VALUE_TUPLE,
            },
        })
    );
    assert!(!library_api_free_name_shadow_safe(
        Lang::Rust,
        "std::collections::HashMap::from",
        LibraryApiShadowPolicy::RustStdRootForStdPath,
        |name| name == "std"
    ));
    assert!(library_api_free_name_shadow_safe(
        Lang::Rust,
        "std::collections::HashMap::from",
        LibraryApiShadowPolicy::RustStdRootForStdPath,
        |_| false
    ));
    assert_eq!(
        library_java_collection_factory_contract(Lang::Java, "Arrays", "asList"),
        Some(LibraryCollectionFactoryContract {
            id: LibraryApiContractId::JavaCollectionFactory(
                JavaCollectionFactoryKind::ArraysAsList,
            ),
            callee: LibraryApiCalleeContract::JavaUtilStaticMember {
                receiver: "Arrays",
                method: "asList",
            },
            result: LibraryCollectionFactoryResult::VariadicElements {
                single_arg_spreads_array: true,
            },
        })
    );
    assert_eq!(
        library_java_collection_constructor_contract(Lang::Java, "ArrayList", 0),
        Some(LibraryCollectionFactoryContract {
            id: LibraryApiContractId::JavaCollectionConstructor(
                JavaCollectionConstructorKind::EmptyList,
            ),
            callee: LibraryApiCalleeContract::JavaUtilConstructor {
                simple_type: "ArrayList",
                qualified_type: "java.util.ArrayList",
                module: "java.util",
                requires_import_for_simple_type: true,
                requires_no_local_type_shadow: true,
            },
            result: LibraryCollectionFactoryResult::EmptySequence,
        })
    );
    assert_eq!(
        library_ruby_set_factory_contract(Lang::Ruby, "Set", "new", 1),
        Some(LibraryCollectionFactoryContract {
            id: LibraryApiContractId::RubySetFactory,
            callee: LibraryApiCalleeContract::RubyRequireStaticMember {
                receiver: "Set",
                method: "new",
                required_module: "set",
                shadow_root: "Set",
            },
            result: LibraryCollectionFactoryResult::SequenceArgument,
        })
    );
    assert_eq!(
        library_js_like_map_constructor_contract(Lang::TypeScript, "Map"),
        Some(LibraryMapFactoryContract {
            id: LibraryApiContractId::JsLikeMapConstructor,
            callee: LibraryApiCalleeContract::JsGlobalConstructor {
                receiver: "Map",
                requires_unshadowed_global: true,
            },
            result: LibraryMapFactoryResult::EntrySequence {
                entry_seq_tag: SEQ_VALUE_COLLECTION,
            },
        })
    );
    assert_eq!(
        library_free_name_collection_factory_contract(Lang::JavaScript, "list"),
        None
    );
    assert_eq!(
        library_java_map_factory_contract(Lang::Java, "List", "of"),
        None
    );
}

#[test]
fn library_api_result_domain_mapping_is_contract_scoped() {
    assert_eq!(
        library_collection_factory_result_domain(
            library_free_name_collection_factory_contract(Lang::Python, "list").unwrap()
        ),
        DomainEvidence::Collection
    );
    assert_eq!(
        library_collection_factory_result_domain(
            library_free_name_collection_factory_contract(Lang::Python, "set").unwrap()
        ),
        DomainEvidence::Set
    );
    assert_eq!(
        library_collection_factory_result_domain(
            library_free_name_collection_factory_contract(Lang::Python, "frozenset").unwrap()
        ),
        DomainEvidence::Set
    );
    assert_eq!(
        library_collection_factory_result_domain(
            library_imported_collection_factory_contract(Lang::Python, "collections", "deque",)
                .unwrap()
        ),
        DomainEvidence::Collection
    );
    assert_eq!(
        library_collection_factory_result_domain(
            library_free_name_collection_factory_contract(
                Lang::Rust,
                "std::collections::HashSet::from",
            )
            .unwrap()
        ),
        DomainEvidence::Set
    );
    assert_eq!(
        library_collection_factory_result_domain(
            library_free_name_collection_factory_contract(
                Lang::Rust,
                "std::collections::VecDeque::from",
            )
            .unwrap()
        ),
        DomainEvidence::Collection
    );
    assert_eq!(
        library_collection_factory_result_domain(
            library_rust_vec_macro_factory_contract(Lang::Rust, "vec").unwrap()
        ),
        DomainEvidence::Collection
    );
    assert_eq!(
        library_collection_factory_result_domain(
            library_java_collection_factory_contract(Lang::Java, "List", "of").unwrap()
        ),
        DomainEvidence::Collection
    );
    let as_list = library_java_collection_factory_contract(Lang::Java, "Arrays", "asList").unwrap();
    assert_eq!(
        library_collection_factory_result_domain_for_arity(as_list, 0),
        Some(DomainEvidence::Collection)
    );
    assert_eq!(
        library_collection_factory_result_domain_for_arity(as_list, 1),
        None,
        "single-argument Arrays.asList has ambiguous element provenance"
    );
    assert_eq!(
        library_collection_factory_result_domain_for_arity(as_list, 2),
        Some(DomainEvidence::Collection)
    );
    assert_eq!(
        library_collection_factory_result_domain(
            library_java_collection_factory_contract(Lang::Java, "Set", "of").unwrap()
        ),
        DomainEvidence::Set
    );
    assert_eq!(
        library_collection_factory_result_domain(
            library_ruby_set_factory_contract(Lang::Ruby, "Set", "new", 1).unwrap()
        ),
        DomainEvidence::Set
    );
    assert_eq!(
        library_collection_factory_result_domain(
            library_js_like_set_constructor_contract(Lang::JavaScript, "Set").unwrap()
        ),
        DomainEvidence::Set
    );
    assert_eq!(
        library_map_factory_result_domain(
            library_free_name_map_factory_contract(Lang::Rust, "std::collections::HashMap::from",)
                .unwrap()
        ),
        DomainEvidence::Map
    );
    assert_eq!(
        library_map_factory_result_domain(
            library_java_map_factory_contract(Lang::Java, "Map", "of").unwrap()
        ),
        DomainEvidence::Map
    );
    assert_eq!(
        library_map_factory_result_domain(
            library_js_like_map_constructor_contract(Lang::JavaScript, "Map").unwrap()
        ),
        DomainEvidence::Map
    );
    assert_eq!(
        library_map_key_view_wrapper_result_domain(
            library_map_key_view_wrapper_contract(Lang::JavaScript, "Array", "from", 1).unwrap()
        ),
        DomainEvidence::Array
    );
}

#[test]
fn library_non_factory_api_contracts_carry_identity_and_result_obligations() {
    assert_eq!(
        library_map_key_view_contract(Lang::TypeScript, "keys", 0),
        Some(LibraryMapKeyViewContract {
            id: LibraryApiContractId::MapKeyView(MapKeyViewKind::Iterator),
            callee: LibraryApiCalleeContract::Method {
                method: "keys",
                receiver: MethodReceiverContract::ExactMap,
            },
            result: MapKeyViewContract {
                method: "keys",
                kind: MapKeyViewKind::Iterator,
            },
        })
    );
    assert_eq!(
        library_map_key_view_wrapper_contract(Lang::JavaScript, "Array", "from", 1),
        Some(LibraryMapKeyViewWrapperContract {
            id: LibraryApiContractId::MapKeyViewWrapper,
            callee: LibraryApiCalleeContract::StaticGlobalMethod {
                receiver: "Array",
                method: "from",
                qualified_path: "Array.from",
                requires_unshadowed_receiver: true,
            },
            result: MapKeyViewWrapperContract {
                receiver: "Array",
                method: "from",
                qualified_path: "Array.from",
            },
        })
    );
    assert_eq!(
        library_map_get_contract(Lang::Rust, "get", 1),
        Some(LibraryMapGetContract {
            id: LibraryApiContractId::MapGet,
            callee: LibraryApiCalleeContract::Method {
                method: "get",
                receiver: MethodReceiverContract::ExactMap,
            },
            result: MapGetContract {
                method: "get",
                receiver: MethodReceiverContract::ExactMap,
            },
        })
    );
    assert_eq!(
        library_js_array_is_array_contract(Lang::JavaScript, "Array", "isArray", 1),
        Some(LibraryStaticGlobalMethodContract {
            id: LibraryApiContractId::JsArrayIsArray,
            callee: LibraryApiCalleeContract::StaticGlobalMethod {
                receiver: "Array",
                method: "isArray",
                qualified_path: "Array.isArray",
                requires_unshadowed_receiver: true,
            },
            result: StaticGlobalMethodContract {
                receiver: "Array",
                method: "isArray",
                qualified_path: "Array.isArray",
                requires_unshadowed_receiver: true,
            },
        })
    );
    assert_eq!(
        library_js_boolean_coercion_contract(Lang::TypeScript, "Boolean", 1),
        Some(LibraryStaticGlobalFunctionContract {
            id: LibraryApiContractId::JsBooleanCoercion,
            callee: LibraryApiCalleeContract::StaticGlobalFunction {
                function: "Boolean",
                requires_unshadowed_function: true,
            },
            result: StaticGlobalFunctionContract {
                function: "Boolean",
                requires_unshadowed_function: true,
            },
        })
    );
    assert_eq!(
        library_regex_test_contract(Lang::JavaScript, "test", 1),
        Some(LibraryRegexTestContract {
            id: LibraryApiContractId::RegexTest,
            callee: LibraryApiCalleeContract::RegexLiteralMethod {
                method: "test",
                required_receiver_fact: SourceFactKind::Literal(SourceLiteralKind::Regex),
            },
            result: RegexTestContract {
                method: "test",
                required_receiver_fact: SourceFactKind::Literal(SourceLiteralKind::Regex),
            },
        })
    );
    assert_eq!(
        library_imported_namespace_function_contract(Lang::Python, "prod", 2),
        Some(LibraryImportedNamespaceFunctionContract {
            id: LibraryApiContractId::ImportedNamespaceFunction(
                ImportedNamespaceFunctionSemantic::ProductReduction {
                    op: Op::Mul,
                    identity: 1,
                },
            ),
            callee: LibraryApiCalleeContract::ImportedNamespaceFunction {
                module: "math",
                function: "prod",
            },
            result: ImportedNamespaceFunctionContract {
                module: "math",
                function: "prod",
                receiver: MethodReceiverContract::ImportedNamespace("math"),
                semantic: ImportedNamespaceFunctionSemantic::ProductReduction {
                    op: Op::Mul,
                    identity: 1,
                },
            },
        })
    );
    assert_eq!(
        library_promise_then_contract(Lang::Vue, "then", 1),
        Some(LibraryPromiseThenContract {
            id: LibraryApiContractId::PromiseThen,
            callee: LibraryApiCalleeContract::AsyncMethod {
                method: "then",
                receiver: AsyncReceiverContract::ExactPromiseLike,
            },
            result: PromiseThenContract {
                receiver: AsyncReceiverContract::ExactPromiseLike,
            },
        })
    );
    assert_eq!(
        library_iterator_identity_adapter_contract(Lang::Rust, "collect", 0),
        Some(LibraryIteratorIdentityAdapterContract {
            id: LibraryApiContractId::IteratorIdentityAdapter,
            callee: LibraryApiCalleeContract::IteratorAdapterMethod {
                method: "collect",
                receiver: IteratorAdapterReceiverContract::ExactIterableValue,
            },
            result: IteratorIdentityAdapterContract {
                receiver: IteratorAdapterReceiverContract::ExactIterableValue,
            },
        })
    );
    assert_eq!(
        library_static_collection_adapter_contract(Lang::Java, "Arrays", "stream", 1),
        Some(LibraryStaticCollectionAdapterContract {
            id: LibraryApiContractId::StaticCollectionAdapter,
            callee: LibraryApiCalleeContract::JavaUtilStaticMember {
                receiver: "Arrays",
                method: "stream",
            },
            result: StaticCollectionAdapterContract {
                module: "java.util",
                exported: "Arrays",
            },
        })
    );
    assert_eq!(
        library_method_call_contract(Lang::Go, "Contains", 2),
        Some(LibraryMethodCallContract {
            id: LibraryApiContractId::MethodCall(MethodSemanticContract::Builtin(
                Builtin::Contains,
            )),
            callee: LibraryApiCalleeContract::Method {
                method: "Contains",
                receiver: MethodReceiverContract::ImportedNamespace("slices"),
            },
            result: MethodCallContract {
                semantic: MethodSemanticContract::Builtin(Builtin::Contains),
                receiver: MethodReceiverContract::ImportedNamespace("slices"),
                args: MethodBuiltinArgs::GoSliceContains,
            },
        })
    );
}

#[test]
fn library_non_factory_api_contracts_reject_raw_name_only_matches() {
    assert_eq!(
        library_map_key_view_contract(Lang::JavaScript, "keySet", 0),
        None
    );
    assert_eq!(library_map_key_view_contract(Lang::Python, "keys", 1), None);
    assert_eq!(
        library_map_key_view_wrapper_contract(Lang::Python, "Array", "from", 1),
        None
    );
    assert_eq!(
        library_map_key_view_wrapper_contract(Lang::TypeScript, "Array", "from", 2),
        None
    );
    assert_eq!(library_map_get_contract(Lang::Python, "get", 1), None);
    assert_eq!(library_map_get_contract(Lang::Rust, "get", 2), None);
    assert_eq!(
        library_js_array_is_array_contract(Lang::Python, "Array", "isArray", 1),
        None
    );
    assert_eq!(
        library_js_array_is_array_contract(Lang::TypeScript, "Array", "isArray", 2),
        None
    );
    assert_eq!(
        library_js_boolean_coercion_contract(Lang::Python, "Boolean", 1),
        None
    );
    assert_eq!(
        library_js_boolean_coercion_contract(Lang::JavaScript, "Boolean", 2),
        None
    );
    assert_eq!(library_regex_test_contract(Lang::Ruby, "test", 1), None);
    assert_eq!(
        library_imported_namespace_function_contract(Lang::JavaScript, "prod", 1),
        None
    );
    assert_eq!(
        library_imported_namespace_function_contract(Lang::Python, "prod", 3),
        None
    );
    assert_eq!(library_promise_then_contract(Lang::Python, "then", 1), None);
    assert_eq!(
        library_promise_then_contract(Lang::TypeScript, "then", 2),
        None
    );
    assert_eq!(
        library_iterator_identity_adapter_contract(Lang::JavaScript, "collect", 0),
        None
    );
    assert_eq!(
        library_iterator_identity_adapter_contract(Lang::Rust, "collect", 1),
        None
    );
    assert_eq!(
        library_static_collection_adapter_contract(Lang::JavaScript, "Arrays", "stream", 1),
        None
    );
    assert_eq!(
        library_static_collection_adapter_contract(Lang::Java, "Arrays", "stream", 0),
        None
    );
    assert_eq!(library_method_call_contract(Lang::Python, "min", 2), None);
    assert_eq!(
        library_method_call_contract(Lang::JavaScript, "min", 1),
        None
    );
    assert_eq!(
        library_method_call_contract(Lang::JavaScript, "Contains", 2),
        None
    );
}

#[test]
fn receiver_mutation_contracts_are_language_scoped_rows() {
    let js_push = module_binding_mutating_method_contract(Lang::JavaScript, "push", 1)
        .expect("js push receiver mutation contract");
    assert_eq!(js_push.pack_id, FIRST_PARTY_PACK_ID);
    assert_eq!(js_push.lang, Lang::JavaScript);
    assert_eq!(js_push.effect, EffectEvidenceKind::ReceiverMutation);
    assert_eq!(
        js_push.receiver,
        MethodEffectReceiverContract::PotentiallyMutableReceiver
    );
    assert!(module_binding_mutating_method_contract(Lang::TypeScript, "push", 1).is_some());
    assert!(module_binding_mutating_method_contract(Lang::JavaScript, "addAll", 1).is_none());
    assert!(module_binding_mutating_method_contract(Lang::Java, "addAll", 1).is_some());
    assert!(module_binding_mutating_method_contract(Lang::Python, "append", 1).is_some());
    assert!(module_binding_mutating_method_contract(Lang::Go, "append", 1).is_none());
}

#[test]
fn builtin_contracts_preserve_current_special_demand_split() {
    for &builtin in ALL_BUILTINS {
        assert_eq!(builtin_tag(builtin), builtin as u32 + 1);
    }
    assert_eq!(builtin_demand(Builtin::Reduce), BuiltinDemand::Reduce);
    assert_eq!(
        builtin_demand_profile(Builtin::Reduce),
        BuiltinDemandProfile::FoldReduction
    );
    assert_eq!(
        builtin_demand(Builtin::Any),
        BuiltinDemand::AnyAll { all: false }
    );
    assert_eq!(
        builtin_demand(Builtin::All),
        BuiltinDemand::AnyAll { all: true }
    );
    assert_eq!(
        builtin_demand_profile(Builtin::All),
        BuiltinDemandProfile::ShortCircuitQuantifier { all: true }
    );
    assert_eq!(builtin_demand(Builtin::Append), BuiltinDemand::Append);
    assert_eq!(
        builtin_demand_profile(Builtin::Append),
        BuiltinDemandProfile::AppendMutation
    );
    assert_eq!(
        builtin_demand(Builtin::ValueOrDefault),
        BuiltinDemand::ValueOrDefault
    );
    assert_eq!(
        builtin_demand_profile(Builtin::ValueOrDefault),
        BuiltinDemandProfile::NullishDefault
    );
    assert_eq!(builtin_demand(Builtin::Len), BuiltinDemand::Eager);
    assert_eq!(
        builtin_demand_profile(Builtin::Len),
        BuiltinDemandProfile::Eager {
            contract: EagerBuiltinContract::Len
        }
    );
    assert_eq!(
        eager_builtin_contract(Builtin::Len),
        Some(EagerBuiltinContract::Len)
    );
    assert_eq!(eager_builtin_contract(Builtin::Append), None);
    assert_eq!(
        reduction_builtin_contract(Builtin::Max),
        Some(ReductionBuiltinContract::Selection { max: true })
    );
    assert_eq!(
        reduction_builtin_contract(Builtin::Any),
        Some(ReductionBuiltinContract::Bool { all: false })
    );
    assert_eq!(reduction_builtin_contract(Builtin::Print), None);
    assert_eq!(
        hof_contract(HoFKind::FilterMap),
        HofContract {
            kind: HoFKind::FilterMap,
            demand: HofDemandProfile::FilterMap {
                callback: CallbackDemandProfile::unary_element(CallbackResultDemand::OptionalValue)
            }
        }
    );
    assert_eq!(
        hof_contract(HoFKind::Reduce).demand,
        HofDemandProfile::Reduce {
            callback: CallbackDemandProfile::left_fold()
        }
    );
}

#[test]
fn free_function_builtin_contracts_are_language_and_shadow_constrained() {
    assert_eq!(
        free_function_builtin_contract(Lang::Python, "len", 1),
        Some(FreeFunctionBuiltinContract {
            name: "len",
            builtin: Builtin::Len,
            args: BuiltinArgContract::First,
            requires_unshadowed: true,
        })
    );
    assert_eq!(free_function_builtin_contract(Lang::Python, "len", 2), None);
    assert_eq!(
        free_function_builtin_contract(Lang::JavaScript, "len", 1),
        None
    );
    assert_eq!(
        free_function_builtin_contract(Lang::Python, "print", 3),
        Some(FreeFunctionBuiltinContract {
            name: "print",
            builtin: Builtin::Print,
            args: BuiltinArgContract::All,
            requires_unshadowed: true,
        })
    );
    assert_eq!(
        free_function_builtin_contract(Lang::Go, "append", 2),
        Some(FreeFunctionBuiltinContract {
            name: "append",
            builtin: Builtin::Append,
            args: BuiltinArgContract::All,
            requires_unshadowed: true,
        })
    );
    assert_eq!(free_function_builtin_contract(Lang::Go, "append", 1), None);
    assert_eq!(free_function_builtin_contract(Lang::C, "fmaxf", 2), None);
    assert_eq!(
        free_function_builtin_contract(Lang::Python, "fmaxf", 2),
        None
    );
    assert_eq!(
        free_function_builtin_contract(Lang::Python, "range", 0),
        None
    );
    assert!(free_function_builtin_contract(Lang::Python, "range", 3).is_some());
    assert_eq!(
        free_function_builtin_contract(Lang::Python, "range", 4),
        None
    );
    assert_eq!(
        free_function_builtin_contract(Lang::Python, "max", 2),
        Some(FreeFunctionBuiltinContract {
            name: "max",
            builtin: Builtin::Max,
            args: BuiltinArgContract::All,
            requires_unshadowed: true,
        })
    );
    assert_eq!(free_function_builtin_contract(Lang::Python, "any", 2), None);
}

#[test]
fn method_protocol_contracts_are_language_constrained() {
    assert!(method_fold_name(Lang::Ruby, "inject"));
    assert!(!method_fold_name(Lang::Python, "inject"));
    assert!(!method_fold_name(Lang::Ruby, "map"));
    assert_eq!(
        method_bool_reduction_builtin(Lang::Java, "anyMatch"),
        Some(Builtin::Any)
    );
    assert_eq!(
        method_bool_reduction_builtin(Lang::JavaScript, "every"),
        Some(Builtin::All)
    );
    assert_eq!(method_bool_reduction_builtin(Lang::Python, "every"), None);
    assert_eq!(
        method_hof_contract(Lang::Ruby, "collect"),
        Some(HoFKind::Map)
    );
    assert_eq!(
        method_hof_contract(Lang::Rust, "flat_map"),
        Some(HoFKind::FlatMap)
    );
    assert_eq!(
        method_hof_contract(Lang::Ruby, "select"),
        Some(HoFKind::Filter)
    );
    assert_eq!(method_hof_contract(Lang::Python, "select"), None);
    assert_eq!(
        method_collection_reduction_builtin(Lang::Rust, "count"),
        Some(Builtin::Len)
    );
    assert_eq!(
        method_collection_reduction_builtin(Lang::Java, "count"),
        Some(Builtin::Len)
    );
    assert_eq!(
        method_collection_reduction_builtin(Lang::JavaScript, "count"),
        None
    );
    assert_eq!(
        property_builtin_contract(Lang::JavaScript, "length"),
        Some(Builtin::Len)
    );
    assert_eq!(property_builtin_contract(Lang::Python, "length"), None);
}

#[test]
fn method_call_contracts_carry_receiver_and_resolution_obligations() {
    assert_eq!(
        method_call_contract(Lang::Python, "append", 1),
        Some(MethodCallContract {
            semantic: MethodSemanticContract::Builtin(Builtin::Append),
            receiver: MethodReceiverContract::ExactCollection,
            args: MethodBuiltinArgs::ReceiverThenAll,
        })
    );
    assert_eq!(method_call_contract(Lang::Python, "append", 0), None);
    assert_eq!(
        method_call_contract(Lang::JavaScript, "log", 1),
        Some(MethodCallContract {
            semantic: MethodSemanticContract::Builtin(Builtin::Print),
            receiver: MethodReceiverContract::UnshadowedGlobal("console"),
            args: MethodBuiltinArgs::All,
        })
    );
    assert_eq!(
        method_call_contract(Lang::JavaScript, "min", 2),
        Some(MethodCallContract {
            semantic: MethodSemanticContract::Builtin(Builtin::Min),
            receiver: MethodReceiverContract::UnshadowedGlobal("Math"),
            args: MethodBuiltinArgs::All,
        })
    );
    assert_eq!(method_call_contract(Lang::JavaScript, "min", 1), None);
    assert_eq!(method_call_contract(Lang::Python, "min", 2), None);
    assert_eq!(
        method_call_contract(Lang::Go, "Abs", 1),
        Some(MethodCallContract {
            semantic: MethodSemanticContract::Builtin(Builtin::Abs),
            receiver: MethodReceiverContract::ImportedNamespace("math"),
            args: MethodBuiltinArgs::First,
        })
    );
    assert_eq!(
        method_call_contract(Lang::Go, "Contains", 2),
        Some(MethodCallContract {
            semantic: MethodSemanticContract::Builtin(Builtin::Contains),
            receiver: MethodReceiverContract::ImportedNamespace("slices"),
            args: MethodBuiltinArgs::GoSliceContains,
        })
    );
    assert_eq!(
        method_call_contract(Lang::Java, "abs", 1),
        Some(MethodCallContract {
            semantic: MethodSemanticContract::Builtin(Builtin::Abs),
            receiver: MethodReceiverContract::UnshadowedGlobal("Math"),
            args: MethodBuiltinArgs::First,
        })
    );
    assert_eq!(
        method_call_contract(Lang::Java, "min", 2),
        Some(MethodCallContract {
            semantic: MethodSemanticContract::Builtin(Builtin::Min),
            receiver: MethodReceiverContract::UnshadowedGlobal("Math"),
            args: MethodBuiltinArgs::All,
        })
    );
    assert_eq!(
        method_call_contract(Lang::Python, "__contains__", 1),
        Some(MethodCallContract {
            semantic: MethodSemanticContract::Builtin(Builtin::Contains),
            receiver: MethodReceiverContract::ExactCollectionOrMap,
            args: MethodBuiltinArgs::FirstThenReceiver,
        })
    );
    assert_eq!(
        method_call_contract(Lang::TypeScript, "has", 1),
        Some(MethodCallContract {
            semantic: MethodSemanticContract::Builtin(Builtin::Contains),
            receiver: MethodReceiverContract::ExactSetOrMap,
            args: MethodBuiltinArgs::FirstThenReceiver,
        })
    );
    assert_eq!(
        method_call_contract(Lang::Ruby, "member?", 1),
        Some(MethodCallContract {
            semantic: MethodSemanticContract::Builtin(Builtin::Contains),
            receiver: MethodReceiverContract::ExactCollectionOrJavaKeySet,
            args: MethodBuiltinArgs::FirstThenReceiver,
        })
    );
    assert_eq!(method_call_contract(Lang::JavaScript, "contains", 1), None);
    assert_eq!(
        method_call_contract(Lang::Java, "getOrDefault", 2),
        Some(MethodCallContract {
            semantic: MethodSemanticContract::Builtin(Builtin::GetOrDefault),
            receiver: MethodReceiverContract::ExactMap,
            args: MethodBuiltinArgs::MapGetDefault,
        })
    );
    assert_eq!(
        method_call_contract(Lang::Python, "get", 2),
        Some(MethodCallContract {
            semantic: MethodSemanticContract::Builtin(Builtin::GetOrDefault),
            receiver: MethodReceiverContract::ExactMap,
            args: MethodBuiltinArgs::MapGetDefault,
        })
    );
    assert_eq!(
        method_call_contract(Lang::Ruby, "fetch", 2),
        Some(MethodCallContract {
            semantic: MethodSemanticContract::Builtin(Builtin::GetOrDefault),
            receiver: MethodReceiverContract::ExactMap,
            args: MethodBuiltinArgs::MapGetDefaultOrZeroArgLambda,
        })
    );
    assert_eq!(method_call_contract(Lang::JavaScript, "abs", 0), None);
}

#[test]
fn scalar_integer_methods_are_language_and_signature_constrained() {
    assert_eq!(
        scalar_integer_method_contract(Lang::Rust, "clamp", 2),
        Some(ScalarIntegerMethodContract {
            semantic: ScalarIntegerMethod::Clamp,
            receiver: MethodReceiverContract::ExactInteger,
        })
    );
    assert_eq!(
        scalar_integer_method_contract(Lang::Rust, "min", 1),
        Some(ScalarIntegerMethodContract {
            semantic: ScalarIntegerMethod::Min,
            receiver: MethodReceiverContract::ExactInteger,
        })
    );
    assert_eq!(scalar_integer_method_contract(Lang::Rust, "clamp", 1), None);
    assert_eq!(
        scalar_integer_method_contract(Lang::TypeScript, "clamp", 2),
        None
    );
    assert_eq!(
        scalar_integer_method_contract(Lang::JavaScript, "abs", 0),
        None
    );
}

#[test]
fn promise_then_contract_requires_js_like_surface_and_receiver_proof() {
    assert_eq!(
        promise_then_contract(Lang::TypeScript, "then", 1),
        Some(PromiseThenContract {
            receiver: AsyncReceiverContract::ExactPromiseLike,
        })
    );
    assert_eq!(promise_then_contract(Lang::TypeScript, "then", 2), None);
    assert_eq!(promise_then_contract(Lang::Python, "then", 1), None);
}

#[test]
fn iterator_identity_adapters_are_rust_and_receiver_proof_constrained() {
    assert_eq!(
        iterator_identity_adapter_contract(Lang::Rust, "iter", 0),
        Some(IteratorIdentityAdapterContract {
            receiver: IteratorAdapterReceiverContract::ExactIterableValue,
        })
    );
    assert_eq!(
        iterator_identity_adapter_contract(Lang::Rust, "collect", 0),
        Some(IteratorIdentityAdapterContract {
            receiver: IteratorAdapterReceiverContract::ExactIterableValue,
        })
    );
    assert_eq!(
        iterator_identity_adapter_contract(Lang::Java, "stream", 0),
        Some(IteratorIdentityAdapterContract {
            receiver: IteratorAdapterReceiverContract::ExactIterableValue,
        })
    );
    assert_eq!(
        iterator_identity_adapter_contract(Lang::JavaScript, "collect", 0),
        None
    );
    assert_eq!(
        iterator_identity_adapter_contract(Lang::Rust, "collect", 1),
        None
    );
}

#[test]
fn static_collection_adapters_are_import_binding_constrained() {
    assert_eq!(
        static_collection_adapter_contract(Lang::Java, "Arrays", "stream", 1),
        Some(StaticCollectionAdapterContract {
            module: "java.util",
            exported: "Arrays",
        })
    );
    assert_eq!(
        static_collection_adapter_contract(Lang::Java, "Arrays", "stream", 0),
        None
    );
    assert_eq!(
        static_collection_adapter_contract(Lang::JavaScript, "Arrays", "stream", 1),
        None
    );
}

#[test]
fn rust_std_path_contracts_carry_shadow_roots() {
    assert_eq!(
        rust_option_some_constructor_contract(Lang::Rust, "Option::Some"),
        Some(ShadowedPathContract {
            shadow_root: "Option",
        })
    );
    assert_eq!(
        rust_option_some_constructor_contract(Lang::Rust, "std::option::Option::Some"),
        Some(ShadowedPathContract { shadow_root: "std" })
    );
    assert_eq!(
        rust_option_some_constructor_contract(Lang::Python, "Some"),
        None
    );
    assert_eq!(
        rust_option_none_sentinel_contract(Lang::Rust, "None"),
        Some(ShadowedPathContract {
            shadow_root: "None",
        })
    );
    assert_eq!(
        rust_option_none_sentinel_contract(Lang::Rust, "core::option::Option::None"),
        Some(ShadowedPathContract {
            shadow_root: "core",
        })
    );
    assert_eq!(
        rust_option_none_sentinel_contract(Lang::JavaScript, "None"),
        None
    );
    assert_eq!(
        rust_vec_new_factory_contract(Lang::Rust, "alloc::vec::Vec::new"),
        Some(ShadowedPathContract {
            shadow_root: "alloc",
        })
    );
    assert_eq!(
        rust_vec_new_factory_contract(Lang::Rust, "Vec::with_capacity"),
        None
    );
    assert!(rust_option_and_then_contract(Lang::Rust, "and_then", 1));
    assert!(!rust_option_and_then_contract(Lang::Rust, "and_then", 0));
    assert!(!rust_option_and_then_contract(
        Lang::JavaScript,
        "and_then",
        1
    ));
}

#[test]
fn java_factory_contracts_are_language_receiver_and_selector_constrained() {
    assert_eq!(
        java_collection_factory_contract(Lang::Java, "List", "of"),
        Some(JavaCollectionFactoryContract {
            receiver: "List",
            method: "of",
            kind: JavaCollectionFactoryKind::ListOf,
            single_arg_spreads_array: false,
        })
    );
    assert_eq!(
        java_collection_factory_contract(Lang::Java, "Arrays", "asList"),
        Some(JavaCollectionFactoryContract {
            receiver: "Arrays",
            method: "asList",
            kind: JavaCollectionFactoryKind::ArraysAsList,
            single_arg_spreads_array: true,
        })
    );
    assert_eq!(
        java_collection_factory_contract(Lang::JavaScript, "List", "of"),
        None
    );
    assert_eq!(
        java_collection_factory_contract(Lang::Java, "Map", "of"),
        None
    );
    assert_eq!(
        java_collection_constructor_contract(Lang::Java, "ArrayList", 0),
        Some(JavaCollectionConstructorContract {
            simple_type: "ArrayList",
            qualified_type: "java.util.ArrayList",
            module: "java.util",
            kind: JavaCollectionConstructorKind::EmptyList,
            requires_import_for_simple_type: true,
            requires_no_local_type_shadow: true,
        })
    );
    assert_eq!(
        java_collection_constructor_contract(Lang::Java, "java.util.LinkedList", 0)
            .map(|contract| contract.kind),
        Some(JavaCollectionConstructorKind::EmptyList)
    );
    assert_eq!(
        java_collection_constructor_contract(Lang::Java, "ArrayList", 1),
        None
    );
    assert_eq!(
        java_collection_constructor_contract(Lang::JavaScript, "ArrayList", 0),
        None
    );
    assert_eq!(
        library_java_collection_constructor_contract(Lang::Java, "ArrayList", 1),
        None
    );
    assert_eq!(
        library_java_collection_constructor_contract(Lang::JavaScript, "ArrayList", 0),
        None
    );
    assert_eq!(
        java_map_factory_contract(Lang::Java, "Map", "ofEntries"),
        Some(JavaMapFactoryContract {
            receiver: "Map",
            method: "ofEntries",
            kind: JavaMapFactoryKind::OfEntries,
        })
    );
    assert_eq!(java_map_factory_contract(Lang::Java, "List", "of"), None);
    assert!(java_map_entry_contract(Lang::Java, "Map", "entry"));
    assert!(!java_map_entry_contract(Lang::Java, "Entry", "entry"));
    assert_eq!(
        java_collection_factory_contract_by_hash(Lang::Java, "Set", stable_symbol_hash("of"))
            .map(|contract| contract.kind),
        Some(JavaCollectionFactoryKind::SetOf)
    );
    assert_eq!(
        java_map_factory_contract_by_hash(Lang::Java, "Map", stable_symbol_hash("of"))
            .map(|contract| contract.kind),
        Some(JavaMapFactoryKind::Of)
    );
    assert!(java_map_entry_contract_by_hash(
        Lang::Java,
        "Map",
        stable_symbol_hash("entry")
    ));
}

#[test]
fn ruby_and_closed_js_like_factory_contracts_keep_proof_obligations_explicit() {
    assert_eq!(
        ruby_set_factory_contract(Lang::Ruby, "Set", "new", 1),
        Some(RubySetFactoryContract {
            receiver: "Set",
            method: "new",
            required_module: "set",
            shadow_root: "Set",
        })
    );
    assert_eq!(ruby_set_factory_contract(Lang::Ruby, "Set", "new", 2), None);
    assert_eq!(
        ruby_set_factory_contract(Lang::Python, "Set", "new", 1),
        None
    );
    assert!(
        ruby_set_factory_contract_by_hash(Lang::Ruby, "Set", stable_symbol_hash("new"), 1)
            .is_some()
    );

    assert_eq!(
        js_like_set_constructor_contract(Lang::TypeScript, "Set"),
        Some(ClosedConstructorContract {
            receiver: "Set",
            required_proof: ConstructorProofRequirement::ConstructSyntax,
            requires_unshadowed_global: true,
            entry_seq_tag: None,
        })
    );
    assert_eq!(
        js_like_map_constructor_contract(Lang::JavaScript, "Map"),
        Some(ClosedConstructorContract {
            receiver: "Map",
            required_proof: ConstructorProofRequirement::ConstructSyntax,
            requires_unshadowed_global: true,
            entry_seq_tag: Some(SEQ_VALUE_COLLECTION),
        })
    );
    assert_eq!(js_like_map_constructor_contract(Lang::Java, "Map"), None);
    assert_eq!(
        js_like_set_constructor_contract(Lang::JavaScript, "WeakSet"),
        None
    );
}

#[test]
fn map_key_view_contracts_distinguish_collection_and_iterator_views() {
    assert_eq!(
        map_key_view_contract(Lang::Python, "keys", 0),
        Some(MapKeyViewContract {
            method: "keys",
            kind: MapKeyViewKind::Collection,
        })
    );
    assert_eq!(
        map_key_view_contract(Lang::Java, "keySet", 0),
        Some(MapKeyViewContract {
            method: "keySet",
            kind: MapKeyViewKind::Collection,
        })
    );
    assert_eq!(
        map_key_view_contract(Lang::TypeScript, "keys", 0),
        Some(MapKeyViewContract {
            method: "keys",
            kind: MapKeyViewKind::Iterator,
        })
    );
    assert_eq!(map_key_view_contract(Lang::JavaScript, "keySet", 0), None);
    assert_eq!(map_key_view_contract(Lang::Python, "keys", 1), None);
    assert_eq!(
        map_key_view_wrapper_contract(Lang::JavaScript, "Array", "from", 1),
        Some(MapKeyViewWrapperContract {
            receiver: "Array",
            method: "from",
            qualified_path: "Array.from",
        })
    );
    assert_eq!(
        map_key_view_wrapper_contract(Lang::Python, "Array", "from", 1),
        None
    );
    assert_eq!(
        map_key_view_contract_by_hash(Lang::Java, stable_symbol_hash("keySet"), 0)
            .map(|contract| contract.kind),
        Some(MapKeyViewKind::Collection)
    );
    assert!(map_key_view_wrapper_contract_by_hash(
        Lang::TypeScript,
        "Array",
        stable_symbol_hash("from"),
        1,
    )
    .is_some());
}

#[test]
fn go_zero_map_contracts_are_go_surface_and_default_constrained() {
    assert_eq!(
        go_zero_map_lookup_contract(Lang::Go),
        Some(GoZeroMapLookupContract {
            map_literal_tag: "composite_literal",
            entry_tag: "keyed_element",
            canonical_value_tag: "go_literal_zero_map",
        })
    );
    assert_eq!(go_zero_map_lookup_contract(Lang::Python), None);
    assert_eq!(
        go_zero_map_default_kind(Lang::Go, Payload::LitInt(1)),
        Some(GoZeroMapDefaultKind::Int)
    );
    assert_eq!(
        go_zero_map_default_kind(Lang::Go, Payload::LitStr(stable_symbol_hash("x"))),
        Some(GoZeroMapDefaultKind::String)
    );
    assert_eq!(
        go_zero_map_default_kind(Lang::Go, Payload::Lit(LitClass::Null)),
        Some(GoZeroMapDefaultKind::Null)
    );
    assert_eq!(
        go_zero_map_default_kind(Lang::JavaScript, Payload::LitInt(1)),
        None
    );
    assert_eq!(go_zero_map_default_kind(Lang::Go, Payload::None), None);
}

#[test]
fn map_get_contracts_are_language_and_arity_constrained() {
    assert_eq!(
        map_get_contract(Lang::Rust, "get", 1),
        Some(MapGetContract {
            method: "get",
            receiver: MethodReceiverContract::ExactMap,
        })
    );
    assert_eq!(
        map_get_contract_by_hash(Lang::Java, stable_symbol_hash("get"), 1),
        Some(MapGetContract {
            method: "get",
            receiver: MethodReceiverContract::ExactMap,
        })
    );
    assert_eq!(
        map_get_contract(Lang::TypeScript, "get", 1),
        Some(MapGetContract {
            method: "get",
            receiver: MethodReceiverContract::ExactMap,
        })
    );
    assert_eq!(map_get_contract(Lang::Python, "get", 1), None);
    assert_eq!(map_get_contract(Lang::Rust, "get", 2), None);
    assert_eq!(map_get_contract(Lang::Java, "getOrDefault", 1), None);
}

#[test]
fn js_static_builtin_contracts_are_language_and_arity_constrained() {
    assert_eq!(
        static_global_symbol_contract(Lang::JavaScript, "Math"),
        Some(StaticGlobalSymbolContract {
            name: "Math",
            requires_unshadowed: true,
        })
    );
    assert_eq!(
        static_global_symbol_contract(Lang::TypeScript, "undefined"),
        Some(StaticGlobalSymbolContract {
            name: "undefined",
            requires_unshadowed: true,
        })
    );
    assert_eq!(static_global_symbol_contract(Lang::Python, "Math"), None);
    assert_eq!(
        static_global_symbol_contract(Lang::JavaScript, "WeakMap"),
        None
    );
    assert_eq!(
        typeof_operator_contract(Lang::TypeScript, "typeof", 1),
        Some(TypeofOperatorContract {
            name: "typeof",
            required_source_fact: SourceFactKind::Operator(SourceOperatorKind::Typeof),
        })
    );
    assert_eq!(typeof_operator_contract(Lang::Python, "typeof", 1), None);
    assert_eq!(
        typeof_operator_contract(Lang::JavaScript, "typeof", 2),
        None
    );
    assert_eq!(
        js_array_is_array_contract(Lang::JavaScript, "Array", "isArray", 1),
        Some(StaticGlobalMethodContract {
            receiver: "Array",
            method: "isArray",
            qualified_path: "Array.isArray",
            requires_unshadowed_receiver: true,
        })
    );
    assert_eq!(
        js_array_is_array_contract(Lang::Python, "Array", "isArray", 1),
        None
    );
    assert_eq!(
        js_array_is_array_contract(Lang::TypeScript, "Array", "isArray", 2),
        None
    );
    assert_eq!(
        js_boolean_coercion_contract(Lang::JavaScript, "Boolean", 1),
        Some(StaticGlobalFunctionContract {
            function: "Boolean",
            requires_unshadowed_function: true,
        })
    );
    assert_eq!(
        js_boolean_coercion_contract(Lang::TypeScript, "Boolean", 1),
        Some(StaticGlobalFunctionContract {
            function: "Boolean",
            requires_unshadowed_function: true,
        })
    );
    assert_eq!(
        js_boolean_coercion_contract(Lang::Python, "Boolean", 1),
        None
    );
    assert_eq!(
        js_boolean_coercion_contract(Lang::JavaScript, "Boolean", 2),
        None
    );
    assert_eq!(
        regex_test_contract(Lang::JavaScript, "test", 1),
        Some(RegexTestContract {
            method: "test",
            required_receiver_fact: SourceFactKind::Literal(SourceLiteralKind::Regex),
        })
    );
    assert_eq!(regex_test_contract(Lang::Ruby, "test", 1), None);
}

#[test]
fn operator_law_contracts_preserve_comparison_gates() {
    for &lang in ALL_LANGS {
        let profile = semantics(lang);
        assert_eq!(
            profile
                .operators()
                .comparison_law(ComparisonLaw::LatticeStrictAbsorbsNonstrict)
                .is_some(),
            matches!(lang, Lang::C | Lang::Go | Lang::Java)
        );
        assert_eq!(
            profile
                .operators()
                .comparison_law(ComparisonLaw::LatticeLeNeToLt),
            Some(OperatorLawContract {
                law: ComparisonLaw::LatticeLeNeToLt,
                channel: ChannelEligibility::ExactProven,
                evidence: OperatorEvidence::ModeledIlOperator,
            })
        );
    }
}

#[test]
fn comparison_transform_contracts_carry_outputs_and_operand_swaps() {
    let ops = semantics(Lang::Python).operators();
    assert_eq!(
        ops.comparison_direction(Op::Gt),
        Some(ComparisonTransformContract {
            law: ComparisonLaw::DirectionCanon,
            input: Op::Gt,
            output: Op::Lt,
            swap_operands: true,
            channel: ChannelEligibility::ExactProven,
            evidence: OperatorEvidence::ModeledIlOperator,
        })
    );
    assert_eq!(
        ops.comparison_complement(Op::Lt)
            .map(|contract| (contract.output, contract.swap_operands)),
        Some((Op::Ge, false))
    );
    assert_eq!(
        ops.canonical_negated_comparison(Op::Lt)
            .map(|contract| (contract.output, contract.swap_operands)),
        Some((Op::Le, true))
    );
    assert_eq!(ops.comparison_direction(Op::Eq), None);
}

#[test]
fn cardinality_threshold_contracts_name_existing_operator_shapes() {
    let ops = semantics(Lang::JavaScript).operators();
    assert_eq!(
        ops.zero_cardinality_equality(Op::Eq),
        Some(CardinalityThresholdContract {
            threshold: CardinalityThreshold::Zero,
            predicate: CardinalityPredicate::Empty,
            channel: ChannelEligibility::ExactProven,
            evidence: OperatorEvidence::StaticCardinalityThreshold,
        })
    );
    assert_eq!(ops.zero_cardinality_equality(Op::Gt), None);
    assert_eq!(
        ops.cardinality_threshold(
            Op::Gt,
            false,
            CardinalityThreshold::Zero,
            CardinalityPredicate::NonEmpty,
        )
        .map(|contract| contract.predicate),
        Some(CardinalityPredicate::NonEmpty)
    );
    assert_eq!(
        ops.cardinality_threshold(
            Op::Eq,
            false,
            CardinalityThreshold::One,
            CardinalityPredicate::NonEmpty,
        ),
        None
    );
}

#[test]
fn membership_operator_contract_is_language_scoped() {
    assert_eq!(
        semantics(Lang::Python)
            .operators()
            .membership_operator(Op::In),
        Some(MembershipOperatorContract {
            operator: Op::In,
            receiver: MembershipOperatorReceiverContract::ExactCollectionOrMap,
            channel: ChannelEligibility::ExactProven,
            evidence: OperatorEvidence::ModeledIlOperator,
        })
    );
    assert_eq!(
        semantics(Lang::JavaScript)
            .operators()
            .membership_operator(Op::In),
        None
    );
    assert_eq!(
        semantics(Lang::Python)
            .operators()
            .membership_operator(Op::Eq),
        None
    );
}

#[test]
fn static_index_membership_contracts_are_js_like_and_threshold_constrained() {
    assert_eq!(
        static_index_membership_contract(Lang::JavaScript, "indexOf", 1),
        Some(StaticIndexMembershipContract {
            method: "indexOf",
            kind: StaticIndexMembershipKind::IndexOf,
            receiver: StaticIndexMembershipReceiverContract::StaticNonFloatLiteralCollection,
        })
    );
    assert_eq!(
        static_index_membership_contract(Lang::TypeScript, "findIndex", 1),
        Some(StaticIndexMembershipContract {
            method: "findIndex",
            kind: StaticIndexMembershipKind::FindIndex,
            receiver: StaticIndexMembershipReceiverContract::StaticNonFloatLiteralCollection,
        })
    );
    assert_eq!(
        static_index_membership_contract(Lang::Python, "indexOf", 1),
        None
    );
    assert_eq!(
        static_index_membership_contract(Lang::JavaScript, "indexOf", 2),
        None
    );
    assert_eq!(
        static_index_membership_contract(Lang::JavaScript, "includes", 1),
        None
    );
    assert_eq!(
        semantics(Lang::JavaScript)
            .operators()
            .static_index_membership_threshold(Op::Ne, false, IndexMembershipThreshold::MinusOne)
            .map(|contract| contract.evidence),
        Some(OperatorEvidence::JsLikeStaticIndexMembershipThreshold)
    );
    assert!(semantics(Lang::TypeScript)
        .operators()
        .static_index_membership_threshold(Op::Le, true, IndexMembershipThreshold::Zero)
        .is_some());
    assert!(semantics(Lang::Python)
        .operators()
        .static_index_membership_threshold(Op::Ne, false, IndexMembershipThreshold::MinusOne)
        .is_none());
    assert!(semantics(Lang::JavaScript)
        .operators()
        .static_index_membership_threshold(Op::Eq, false, IndexMembershipThreshold::MinusOne)
        .is_none());
}

#[test]
fn imported_namespace_function_contracts_carry_module_and_receiver_proof() {
    assert_eq!(
        imported_namespace_function_contract(Lang::Python, "prod", 1),
        Some(ImportedNamespaceFunctionContract {
            module: "math",
            function: "prod",
            receiver: MethodReceiverContract::ImportedNamespace("math"),
            semantic: ImportedNamespaceFunctionSemantic::ProductReduction {
                op: Op::Mul,
                identity: 1,
            },
        })
    );
    assert_eq!(
        imported_namespace_function_contract(Lang::Python, "prod", 2)
            .map(|contract| contract.semantic),
        Some(ImportedNamespaceFunctionSemantic::ProductReduction {
            op: Op::Mul,
            identity: 1,
        })
    );
    assert_eq!(
        imported_namespace_function_contract(Lang::JavaScript, "prod", 1),
        None
    );
    assert_eq!(
        imported_namespace_function_contract(Lang::Python, "prod", 3),
        None
    );
    assert_eq!(
        imported_namespace_function_contract(Lang::Python, "sum", 1),
        None
    );
}

#[test]
fn nullish_global_contracts_are_js_like_and_unshadowed() {
    assert_eq!(
        nullish_global_contract(Lang::JavaScript, "undefined"),
        Some(NullishGlobalContract {
            name: "undefined",
            requires_unshadowed: true,
        })
    );
    assert_eq!(
        nullish_global_contract(Lang::TypeScript, "undefined"),
        Some(NullishGlobalContract {
            name: "undefined",
            requires_unshadowed: true,
        })
    );
    assert_eq!(nullish_global_contract(Lang::Python, "undefined"), None);
    assert_eq!(nullish_global_contract(Lang::JavaScript, "null"), None);
}

#[test]
fn builder_append_contracts_are_language_and_arity_constrained() {
    let rust_push = builder_append_method_contract(Lang::Rust, "push", 1)
        .expect("rust push builder append contract");
    assert_eq!(rust_push.effect, EffectEvidenceKind::BuilderAppendCall);
    assert_eq!(
        rust_push.receiver,
        MethodEffectReceiverContract::ActiveCollectionBuilder
    );
    assert!(builder_append_method_contract(Lang::Rust, "push", 2).is_none());
    assert!(builder_append_method_contract(Lang::Java, "add", 1).is_some());
    assert!(builder_append_method_contract(Lang::JavaScript, "push", 1).is_some());
    assert!(builder_append_method_contract(Lang::TypeScript, "push", 1).is_some());
    assert!(builder_append_method_contract(Lang::Python, "append", 1).is_some());
    assert!(builder_append_method_contract(Lang::Ruby, "push", 1).is_none());
}

#[test]
fn unproven_membership_like_guard_is_negative_api_policy() {
    let guard = unproven_membership_like_method_contract(Lang::TypeScript, "includes", 1)
        .expect("includes guard");
    assert_eq!(guard.pack_id, FIRST_PARTY_PACK_ID);
    assert_eq!(guard.id, ApiGuardContractId::UnprovenMembershipLikeCall);
    assert_eq!(guard.lang, Lang::TypeScript);
    assert_eq!(guard.method, "includes");
    assert_eq!(guard.arg_count, 1);
    assert_eq!(guard.channel, ChannelEligibility::ExactProven);
    assert!(unproven_membership_like_method_contract(Lang::Java, "containsKey", 1).is_some());
    assert!(unproven_membership_like_method_contract(Lang::Python, "custom", 1).is_none());
}

#[test]
fn map_builder_index_write_contracts_are_language_scoped() {
    let contract =
        map_builder_index_write_contract(Lang::Python).expect("python map builder contract");
    assert_eq!(contract.pack_id, FIRST_PARTY_PACK_ID);
    assert_eq!(contract.id, IndexWriteContractId::MapBuilderEntryWrite);
    assert_eq!(
        contract.receiver,
        IndexWriteReceiverContract::ActiveMapBuilder
    );
    assert_eq!(contract.required_effect, EffectEvidenceKind::BindingWrite);
    assert_eq!(contract.channel, ChannelEligibility::ExactProven);
    assert!(map_builder_index_write_contract(Lang::Ruby).is_none());
    assert!(map_builder_index_write_contract(Lang::JavaScript).is_none());
}

#[test]
fn exact_index_assignment_parts_require_effect_evidence() {
    let mut b = IlBuilder::new(FileId(0));
    let receiver = b.add(NodeKind::Var, Payload::Cid(1), sp(1), &[]);
    let key = b.add(NodeKind::Var, Payload::Cid(2), sp(1), &[]);
    let target = b.add(NodeKind::Index, Payload::None, sp(1), &[receiver, key]);
    let value = b.add(NodeKind::Var, Payload::Cid(3), sp(1), &[]);
    let assign = b.add(NodeKind::Assign, Payload::None, sp(1), &[target, value]);
    let mut il = finish_il(b, assign, Lang::Go);

    assert_eq!(
        exact_non_overloadable_index_assignment_parts(&il, assign),
        None
    );
    assert!(!exact_non_overloadable_index_assignment(&il, assign));

    push_node_effect(
        &mut il,
        0,
        assign,
        EffectEvidenceKind::NonOverloadableIndexWrite,
    );
    push_node_effect(&mut il, 2, assign, EffectEvidenceKind::BindingWrite);

    assert_eq!(
        exact_non_overloadable_index_assignment_parts(&il, assign),
        Some((receiver, Some(key), value))
    );
    assert!(exact_non_overloadable_index_assignment(&il, assign));

    push_node_effect(&mut il, 1, assign, EffectEvidenceKind::BuilderAppendCall);
    assert_eq!(
        exact_non_overloadable_index_assignment_parts(&il, assign),
        None
    );
    assert!(!exact_non_overloadable_index_assignment(&il, assign));
}

#[test]
fn builder_append_call_args_require_effect_evidence() {
    let interner = Interner::default();
    let append = interner.intern("append");
    let push = interner.intern("push");
    let mut b = IlBuilder::new(FileId(0));
    let receiver = b.add(NodeKind::Var, Payload::Cid(1), sp(1), &[]);
    let value = b.add(NodeKind::Var, Payload::Cid(2), sp(1), &[]);
    let builtin = b.add(
        NodeKind::Call,
        Payload::Builtin(Builtin::Append),
        sp(1),
        &[receiver, value],
    );
    let method = b.add(NodeKind::Field, Payload::Name(append), sp(2), &[receiver]);
    let call = b.add(NodeKind::Call, Payload::None, sp(2), &[method, value]);
    let push_method = b.add(NodeKind::Field, Payload::Name(push), sp(3), &[receiver]);
    let push_call = b.add(NodeKind::Call, Payload::None, sp(3), &[push_method, value]);
    let root = b.add(
        NodeKind::Block,
        Payload::None,
        sp(1),
        &[builtin, call, push_call],
    );
    let il = finish_il(b, root, Lang::Python);

    assert_eq!(builder_append_call_args(&il, &interner, builtin), None);
    let mut il = il;
    push_node_effect(&mut il, 0, builtin, EffectEvidenceKind::BuilderAppendCall);
    push_node_effect(&mut il, 3, builtin, EffectEvidenceKind::ReceiverMutation);
    assert_eq!(
        builder_append_call_args(&il, &interner, builtin),
        Some((receiver, value))
    );
    assert_eq!(builder_append_call_args(&il, &interner, call), None);
    assert_eq!(builder_append_call_args(&il, &interner, push_call), None);

    let mut rust_il = il.clone();
    rust_il.meta.lang = Lang::Rust;
    assert_eq!(
        builder_append_call_args(&rust_il, &interner, push_call),
        None
    );
}

#[test]
fn effect_evidence_can_prove_non_overloadable_index_write() {
    let mut b = IlBuilder::new(FileId(0));
    let receiver = b.add(NodeKind::Var, Payload::Cid(1), sp(1), &[]);
    let key = b.add(NodeKind::Var, Payload::Cid(2), sp(1), &[]);
    let target = b.add(NodeKind::Index, Payload::None, sp(1), &[receiver, key]);
    let value = b.add(NodeKind::Var, Payload::Cid(3), sp(1), &[]);
    let assign = b.add(NodeKind::Assign, Payload::None, sp(9), &[target, value]);
    let mut il = finish_il(b, assign, Lang::Ruby);

    assert_eq!(
        exact_non_overloadable_index_assignment_parts(&il, assign),
        None
    );

    push_node_effect(
        &mut il,
        0,
        assign,
        EffectEvidenceKind::NonOverloadableIndexWrite,
    );
    assert_eq!(
        exact_non_overloadable_index_assignment_parts(&il, assign),
        Some((receiver, Some(key), value))
    );

    push_node_effect(
        &mut il,
        1,
        assign,
        EffectEvidenceKind::SelfFieldWrite { field_hash: 1 },
    );
    assert_eq!(
        exact_non_overloadable_index_assignment_parts(&il, assign),
        None
    );

    let mut b = IlBuilder::new(FileId(0));
    let receiver = b.add(NodeKind::Var, Payload::Cid(1), sp(1), &[]);
    let key = b.add(NodeKind::Var, Payload::Cid(2), sp(1), &[]);
    let target = b.add(NodeKind::Index, Payload::None, sp(1), &[receiver, key]);
    let value = b.add(NodeKind::Var, Payload::Cid(3), sp(1), &[]);
    let call = b.add(NodeKind::Call, Payload::None, sp(10), &[target, value]);
    let mut non_assign = finish_il(b, call, Lang::Ruby);
    push_node_effect(
        &mut non_assign,
        0,
        call,
        EffectEvidenceKind::NonOverloadableIndexWrite,
    );
    assert_eq!(
        exact_non_overloadable_index_assignment_parts(&non_assign, call),
        None
    );
}

#[test]
fn append_effect_evidence_can_prove_raw_method_call() {
    let interner = Interner::default();
    let append = interner.intern("append");
    let mut b = IlBuilder::new(FileId(0));
    let receiver = b.add(NodeKind::Var, Payload::Cid(1), sp(1), &[]);
    let value = b.add(NodeKind::Var, Payload::Cid(2), sp(1), &[]);
    let method = b.add(NodeKind::Field, Payload::Name(append), sp(2), &[receiver]);
    let call = b.add(NodeKind::Call, Payload::None, sp(3), &[method, value]);
    let mut il = finish_il(b, call, Lang::Ruby);

    assert_eq!(builder_append_call_args(&il, &interner, call), None);

    push_node_effect(&mut il, 0, call, EffectEvidenceKind::BuilderAppendCall);
    assert_eq!(
        builder_append_call_args(&il, &interner, call),
        Some((receiver, value))
    );

    push_node_effect(
        &mut il,
        1,
        call,
        EffectEvidenceKind::NonOverloadableIndexWrite,
    );
    assert_eq!(builder_append_call_args(&il, &interner, call), None);
}

#[test]
fn place_evidence_is_authoritative_for_self_field_proof() {
    let interner = Interner::default();
    let this = interner.intern("this");
    let field_name = interner.intern("value");
    let field_hash = stable_symbol_hash("value");
    let mut b = IlBuilder::new(FileId(0));
    let receiver = b.add(NodeKind::Var, Payload::Name(this), sp(1), &[]);
    let field = b.add(
        NodeKind::Field,
        Payload::Name(field_name),
        sp(2),
        &[receiver],
    );
    let value = b.add(NodeKind::Var, Payload::Cid(1), sp(3), &[]);
    let assign = b.add(NodeKind::Assign, Payload::None, sp(4), &[field, value]);
    let ret = b.add(NodeKind::Return, Payload::None, sp(5), &[receiver]);
    let root = b.add(NodeKind::Block, Payload::None, sp(1), &[assign, ret]);
    let mut il = finish_il(b, root, Lang::Ruby);

    assert!(!exact_java_this_var(&il, &interner, receiver));
    assert!(!exact_java_this_field(&il, &interner, field));
    assert!(!exact_java_return_this(&il, &interner, ret));
    assert!(!exact_self_field_write_assignment(&il, &interner, assign));

    let receiver_evidence = push_node_place(&mut il, 0, receiver, PlaceEvidenceKind::SelfReceiver);
    let field_evidence = push_node_place_with_dependencies(
        &mut il,
        1,
        field,
        PlaceEvidenceKind::SelfField { field_hash },
        vec![receiver_evidence],
    );
    push_node_effect_with_dependencies(
        &mut il,
        2,
        assign,
        EffectEvidenceKind::SelfFieldWrite { field_hash },
        vec![field_evidence],
    );
    assert!(exact_java_this_var(&il, &interner, receiver));
    assert!(exact_java_this_field(&il, &interner, field));
    assert!(exact_java_return_this(&il, &interner, ret));
    assert!(exact_self_field_write_assignment(&il, &interner, assign));

    push_node_place(&mut il, 3, field, PlaceEvidenceKind::SelfReceiver);
    assert!(!exact_java_this_field(&il, &interner, field));
    assert!(!exact_self_field_write_assignment(&il, &interner, assign));

    push_node_place(
        &mut il,
        4,
        receiver,
        PlaceEvidenceKind::SelfField { field_hash },
    );
    assert!(!exact_java_this_var(&il, &interner, receiver));
    assert!(!exact_java_return_this(&il, &interner, ret));

    let other = interner.intern("other");
    let mut b = IlBuilder::new(FileId(0));
    let receiver = b.add(NodeKind::Var, Payload::Name(other), sp(5), &[]);
    let field = b.add(
        NodeKind::Field,
        Payload::Name(field_name),
        sp(6),
        &[receiver],
    );
    let value = b.add(NodeKind::Var, Payload::Cid(1), sp(7), &[]);
    let assign = b.add(NodeKind::Assign, Payload::None, sp(8), &[field, value]);
    let mut il = finish_il(b, assign, Lang::Ruby);
    push_node_place(
        &mut il,
        0,
        field,
        PlaceEvidenceKind::SelfField { field_hash },
    );
    push_node_effect(
        &mut il,
        1,
        assign,
        EffectEvidenceKind::SelfFieldWrite { field_hash },
    );
    assert!(!exact_java_this_field(&il, &interner, field));
    assert!(!exact_self_field_write_assignment(&il, &interner, assign));
}

#[test]
fn source_fact_contracts_are_span_keyed_evidence() {
    let mut b = IlBuilder::new(FileId(0));
    let call = b.add(NodeKind::Call, Payload::None, sp(7), &[]);
    let regex = b.add(NodeKind::Lit, Payload::LitStr(42), sp(8), &[]);
    let await_boundary = b.add(NodeKind::Raw, Payload::None, sp(9), &[]);
    let root = b.add(
        NodeKind::Block,
        Payload::None,
        sp(7),
        &[call, regex, await_boundary],
    );
    let mut il = finish_il(b, root, Lang::JavaScript);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::source_span(sp(7)),
        EvidenceKind::Source(SourceFactKind::Call(SourceCallKind::Construct)),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(evidence(
        1,
        EvidenceAnchor::source_span(sp(8)),
        EvidenceKind::Source(SourceFactKind::Literal(SourceLiteralKind::Regex)),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(evidence(
        2,
        EvidenceAnchor::source_span(sp(9)),
        EvidenceKind::Source(SourceFactKind::Protocol(SourceProtocolKind::Await)),
        EvidenceStatus::Asserted,
    ));

    assert!(construct_syntax_proof(&il, call));
    assert!(regex_literal_proof(&il, regex));
    assert_eq!(
        source_protocol_at_node(&il, await_boundary),
        Some(SourceProtocolKind::Await)
    );
    assert!(!construct_syntax_proof(&il, regex));
    assert_eq!(
        source_fact_contract(SourceFactKind::Call(SourceCallKind::Construct)).channel,
        ChannelEligibility::ExactProven
    );
}

#[test]
fn source_fact_evidence_conflicts_fail_closed() {
    let mut b = IlBuilder::new(FileId(0));
    let op = b.add(NodeKind::BinOp, Payload::Op(Op::Eq), sp(9), &[]);
    let root = b.add(NodeKind::Block, Payload::None, sp(9), &[op]);
    let mut il = finish_il(b, root, Lang::JavaScript);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::source_span(sp(9)),
        EvidenceKind::Source(SourceFactKind::Operator(SourceOperatorKind::StrictEquality)),
        EvidenceStatus::Asserted,
    ));
    assert_eq!(
        source_operator_at_node(&il, op),
        Some(SourceOperatorKind::StrictEquality)
    );

    il.evidence.push(evidence(
        1,
        EvidenceAnchor::source_span(sp(9)),
        EvidenceKind::Source(SourceFactKind::Operator(SourceOperatorKind::LooseEquality)),
        EvidenceStatus::Asserted,
    ));
    assert_eq!(source_operator_at_node(&il, op), None);
}

#[test]
fn source_fact_evidence_requires_live_dependencies() {
    let mut b = IlBuilder::new(FileId(0));
    let call = b.add(NodeKind::Call, Payload::None, sp(10), &[]);
    let cast = b.add(NodeKind::Call, Payload::None, sp(11), &[]);
    let root = b.add(NodeKind::Block, Payload::None, sp(10), &[call, cast]);
    let mut il = finish_il(b, root, Lang::Rust);
    il.evidence.push(evidence_with_dependencies(
        0,
        EvidenceAnchor::source_span(sp(10)),
        EvidenceKind::Source(SourceFactKind::Call(SourceCallKind::MacroInvocation)),
        EvidenceStatus::Asserted,
        vec![EvidenceId(99)],
    ));

    assert_eq!(source_call_at_node(&il, call), None);
    assert!(!source_fact_at_node(
        &il,
        call,
        SourceFactKind::Call(SourceCallKind::MacroInvocation),
    ));
    il.evidence.push(evidence_with_dependencies(
        1,
        EvidenceAnchor::source_span(sp(11)),
        EvidenceKind::Source(SourceFactKind::Cast(SourceCastKind::CUnsigned32)),
        EvidenceStatus::Asserted,
        vec![EvidenceId(100)],
    ));
    assert_eq!(source_cast_at_node(&il, cast), None);
    assert!(!source_fact_at_node(
        &il,
        cast,
        SourceFactKind::Cast(SourceCastKind::CUnsigned32),
    ));
}

#[test]
fn static_membership_predicate_operator_requires_js_strict_equality() {
    assert!(exact_static_membership_predicate_operator(
        Lang::JavaScript,
        Op::Eq,
        SourceOperatorKind::StrictEquality
    ));
    assert!(exact_static_membership_predicate_operator(
        Lang::TypeScript,
        Op::Ne,
        SourceOperatorKind::StrictInequality
    ));
    assert!(!exact_static_membership_predicate_operator(
        Lang::JavaScript,
        Op::Eq,
        SourceOperatorKind::LooseEquality
    ));
    assert!(!exact_static_membership_predicate_operator(
        Lang::Python,
        Op::Eq,
        SourceOperatorKind::ValueEquality
    ));
    assert!(!exact_static_membership_predicate_operator(
        Lang::JavaScript,
        Op::Eq,
        SourceOperatorKind::TypeMembership
    ));
}
