use super::*;
use nose_il::{
    CallTargetEvidenceKind, EvidenceAnchor, EvidenceEmitter, EvidenceId, EvidenceKind,
    EvidenceProvenance, EvidenceRecord, EvidenceStatus, FileId, FileMeta, GuardEvidenceKind,
    IlBuilder, ImportEvidenceKind, Interner, JsRecordGuardComparison, JsRecordGuardNullCheck,
    LibraryApiEvidenceKind, ParamSemantic, PlaceEvidenceKind, SequenceSurfaceKind, SourceCastKind,
    SourceFactKind, SourcePatternKind, SourceRangeKind, Span, Symbol, SymbolEvidenceKind,
    TypeEvidenceKind, Unit, UnitKind,
};

mod call_targets;
mod effects_and_places;
mod js_symbol_guards;
mod library_api_contracts;
mod library_api_evidence;
mod semantic_evidence;
mod source_facts;

const ALL_LANGS: &[Lang] = &[
    Lang::Python,
    Lang::JavaScript,
    Lang::TypeScript,
    Lang::Go,
    Lang::Rust,
    Lang::Java,
    Lang::C,
    Lang::Ruby,
    Lang::Swift,
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
    Builtin::StringContains,
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
            origin: Default::default(),
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
            emitter: EvidenceEmitter::Builtin,
            pack_hash: Some(stable_symbol_hash(BUILTIN_COMPAT_PACK_ID)),
            rule_hash: Some(stable_symbol_hash("test")),
        },
        dependencies,
        status,
    }
}

fn language_core_evidence(
    id: u32,
    anchor: EvidenceAnchor,
    kind: EvidenceKind,
    status: EvidenceStatus,
    lang: Lang,
) -> EvidenceRecord {
    language_core_evidence_with_dependencies(id, anchor, kind, status, Vec::new(), lang)
}

fn language_core_evidence_with_dependencies(
    id: u32,
    anchor: EvidenceAnchor,
    kind: EvidenceKind,
    status: EvidenceStatus,
    dependencies: Vec<EvidenceId>,
    lang: Lang,
) -> EvidenceRecord {
    let (pack_id, producer_id) = language_core_evidence_provenance(lang);
    EvidenceRecord {
        id: EvidenceId(id),
        anchor,
        kind,
        provenance: EvidenceProvenance {
            emitter: EvidenceEmitter::Builtin,
            pack_hash: Some(stable_symbol_hash(pack_id)),
            rule_hash: Some(stable_symbol_hash(producer_id)),
        },
        dependencies,
        status,
    }
}

fn c_unsigned_32_source_cast_evidence(
    id: u32,
    anchor: EvidenceAnchor,
    status: EvidenceStatus,
    dependencies: Vec<EvidenceId>,
) -> EvidenceRecord {
    EvidenceRecord {
        id: EvidenceId(id),
        anchor,
        kind: EvidenceKind::Source(SourceFactKind::Cast(SourceCastKind::CUnsigned32)),
        provenance: EvidenceProvenance {
            emitter: EvidenceEmitter::Builtin,
            pack_hash: Some(stable_symbol_hash(C_LANGUAGE_PACK_ID)),
            rule_hash: Some(stable_symbol_hash(C_UNSIGNED_32_CAST_SOURCE_PRODUCER_ID)),
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
