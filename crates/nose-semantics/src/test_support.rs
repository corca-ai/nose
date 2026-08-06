use nose_il::{
    stable_symbol_hash, Builtin, EvidenceAnchor, EvidenceId, EvidenceKind, EvidenceRecord,
    EvidenceStatus, FileId, FileMeta, HoFKind, Il, IlBuilder, Interner, Lang,
    LibraryApiEvidenceKind, NodeId, NodeKind, Payload, Span, SymbolEvidenceKind, Unit, UnitKind,
};

use crate::{
    language_core_evidence_provenance, library_api_callee_contract_hash,
    library_api_contract_id_hash, library_java_map_factory_contract, library_method_call_contract,
    LibraryApiCalleeContract, LibraryApiContractId, BUILTIN_COMPAT_PACK_ID,
    BUILTIN_METHOD_CALL_PROTOCOL_PACK_ID, BUILTIN_METHOD_CALL_PROTOCOL_PRODUCER_ID,
    FREE_FUNCTION_BUILTIN_PROTOCOL_PACK_ID, FREE_FUNCTION_BUILTIN_PROTOCOL_PRODUCER_ID,
    JAVA_GUAVA_IMMUTABLE_COLLECTION_FACTORY_PACK_ID,
    JAVA_GUAVA_IMMUTABLE_COLLECTION_FACTORY_PRODUCER_ID,
};

#[derive(Clone, Copy)]
pub struct LibraryApiTestContract {
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub arity: u16,
}

#[derive(Clone, Copy)]
pub enum GuavaImmutableMapFixtureRoot {
    Block,
    Module,
}

#[derive(Clone, Copy)]
pub enum GuavaImmutableMapFixtureSpanLines {
    MatchOffsets,
    SingleLine,
}

#[derive(Clone, Copy)]
pub enum GuavaImmutableMapFixtureImportRhs {
    EmptySeq,
    QualifiedSymbols {
        module: &'static str,
        exported: &'static str,
    },
}

pub struct GuavaImmutableMapFixtureOptions {
    pub root_kind: GuavaImmutableMapFixtureRoot,
    pub span_lines: GuavaImmutableMapFixtureSpanLines,
    pub import_rhs: GuavaImmutableMapFixtureImportRhs,
    pub include_function_unit: bool,
    pub path: &'static str,
}

pub fn compat_test_evidence_with_dependencies(
    id: u32,
    anchor: EvidenceAnchor,
    kind: EvidenceKind,
    status: EvidenceStatus,
    dependencies: Vec<EvidenceId>,
) -> EvidenceRecord {
    EvidenceRecord::builtin(
        EvidenceId(id),
        anchor,
        kind,
        BUILTIN_COMPAT_PACK_ID,
        "test",
        dependencies,
        status,
    )
}

pub fn compat_test_asserted_evidence(
    id: u32,
    anchor: EvidenceAnchor,
    kind: EvidenceKind,
    dependencies: Vec<EvidenceId>,
) -> EvidenceRecord {
    compat_test_evidence_with_dependencies(id, anchor, kind, EvidenceStatus::Asserted, dependencies)
}

pub fn compat_library_api_test_evidence_with_dependencies(
    id: u32,
    span: Span,
    contract: LibraryApiTestContract,
    status: EvidenceStatus,
    dependencies: Vec<EvidenceId>,
) -> EvidenceRecord {
    library_api_test_evidence_with_dependencies(
        id,
        span,
        contract,
        status,
        dependencies,
        (BUILTIN_COMPAT_PACK_ID, "test"),
    )
}

pub fn builtin_library_api_test_evidence_with_dependencies(
    id: u32,
    span: Span,
    contract: LibraryApiTestContract,
    status: EvidenceStatus,
    dependencies: Vec<EvidenceId>,
) -> EvidenceRecord {
    let provenance = if matches!(contract.id, LibraryApiContractId::FreeFunctionBuiltin(_)) {
        (
            FREE_FUNCTION_BUILTIN_PROTOCOL_PACK_ID,
            FREE_FUNCTION_BUILTIN_PROTOCOL_PRODUCER_ID,
        )
    } else if matches!(contract.id, LibraryApiContractId::MethodCall(_)) {
        (
            BUILTIN_METHOD_CALL_PROTOCOL_PACK_ID,
            BUILTIN_METHOD_CALL_PROTOCOL_PRODUCER_ID,
        )
    } else {
        (BUILTIN_COMPAT_PACK_ID, "test")
    };
    library_api_test_evidence_with_dependencies(
        id,
        span,
        contract,
        status,
        dependencies,
        provenance,
    )
}

pub fn method_call_library_api_test_evidence_with_dependencies(
    id: u32,
    lang: Lang,
    method: &str,
    span: Span,
    arity: usize,
    dependencies: Vec<EvidenceId>,
) -> EvidenceRecord {
    let contract = library_method_call_contract(lang, method, arity).expect("method call contract");
    library_api_test_evidence_with_dependencies(
        id,
        span,
        LibraryApiTestContract {
            id: contract.id,
            callee: contract.callee,
            arity: arity as u16,
        },
        EvidenceStatus::Asserted,
        dependencies,
        (contract.pack_id, contract.producer_id),
    )
}

pub fn library_api_test_evidence_with_dependencies(
    id: u32,
    span: Span,
    contract: LibraryApiTestContract,
    status: EvidenceStatus,
    dependencies: Vec<EvidenceId>,
    provenance: (&str, &str),
) -> EvidenceRecord {
    EvidenceRecord::builtin(
        EvidenceId(id),
        EvidenceAnchor::node(span, NodeKind::Call),
        EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
            contract_hash: library_api_contract_id_hash(contract.id),
            callee_hash: library_api_callee_contract_hash(contract.callee),
            arity: contract.arity,
        }),
        provenance.0,
        provenance.1,
        dependencies,
        status,
    )
}

pub fn guava_immutable_map_eleven_entry_payloads() -> Vec<Payload> {
    (0..11)
        .flat_map(|idx| {
            [
                Payload::LitStr(stable_symbol_hash(&format!("k{idx}"))),
                Payload::LitInt(idx),
            ]
        })
        .collect()
}

pub fn guava_immutable_map_of_test_il(
    args: &[Payload],
    base_line: u32,
    options: GuavaImmutableMapFixtureOptions,
) -> (Il, Interner, NodeId) {
    let span = fixture_span(options.span_lines);
    let interner = Interner::new();
    let mut builder = IlBuilder::new(FileId(0));
    let import = guava_immutable_map_import(
        &mut builder,
        &interner,
        base_line,
        &span,
        options.import_rhs,
    );
    let callee = guava_immutable_map_of_callee(&mut builder, &interner, base_line, &span);
    let call_span = span(base_line + 3 + args.len() as u32);
    let call = guava_immutable_map_of_call(&mut builder, &span, args, base_line, callee, call_span);
    let root = builder.add(
        match options.root_kind {
            GuavaImmutableMapFixtureRoot::Block => NodeKind::Block,
            GuavaImmutableMapFixtureRoot::Module => NodeKind::Module,
        },
        Payload::None,
        span(base_line),
        &[import, call],
    );
    let units = options
        .include_function_unit
        .then(|| Unit {
            root,
            kind: UnitKind::Function,
            name: None,
            origin: Default::default(),
        })
        .into_iter()
        .collect();
    let mut il = builder.finish(
        root,
        FileMeta {
            path: options.path.into(),
            lang: Lang::Java,
        },
        units,
        Vec::new(),
    );
    push_guava_immutable_map_of_evidence(&mut il, &span, args.len(), base_line, call_span);
    (il, interner, call)
}

fn fixture_span(span_lines: GuavaImmutableMapFixtureSpanLines) -> impl Fn(u32) -> Span {
    move |line| match span_lines {
        GuavaImmutableMapFixtureSpanLines::MatchOffsets => {
            Span::new(FileId(0), line, line, line, line)
        }
        GuavaImmutableMapFixtureSpanLines::SingleLine => Span::new(FileId(0), line, line, 1, 1),
    }
}

fn guava_immutable_map_import(
    builder: &mut IlBuilder,
    interner: &Interner,
    base_line: u32,
    span: &impl Fn(u32) -> Span,
    import_rhs: GuavaImmutableMapFixtureImportRhs,
) -> NodeId {
    let local = interner.intern("ImmutableMap");
    let imported = builder.add(NodeKind::Var, Payload::Name(local), span(base_line), &[]);
    let import_rhs = match import_rhs {
        GuavaImmutableMapFixtureImportRhs::EmptySeq => {
            builder.add(NodeKind::Seq, Payload::None, span(base_line), &[])
        }
        GuavaImmutableMapFixtureImportRhs::QualifiedSymbols { module, exported } => {
            let module = builder.add(
                NodeKind::Lit,
                Payload::LitStr(stable_symbol_hash(module)),
                span(base_line),
                &[],
            );
            let exported = builder.add(
                NodeKind::Lit,
                Payload::LitStr(stable_symbol_hash(exported)),
                span(base_line),
                &[],
            );
            builder.add(
                NodeKind::Seq,
                Payload::None,
                span(base_line),
                &[module, exported],
            )
        }
    };
    builder.add(
        NodeKind::Assign,
        Payload::None,
        span(base_line),
        &[imported, import_rhs],
    )
}

fn guava_immutable_map_of_callee(
    builder: &mut IlBuilder,
    interner: &Interner,
    base_line: u32,
    span: &impl Fn(u32) -> Span,
) -> NodeId {
    let local = interner.intern("ImmutableMap");
    let receiver = builder.add(
        NodeKind::Var,
        Payload::Name(local),
        span(base_line + 1),
        &[],
    );
    builder.add(
        NodeKind::Field,
        Payload::Name(interner.intern("of")),
        span(base_line + 2),
        &[receiver],
    )
}

fn guava_immutable_map_of_call(
    builder: &mut IlBuilder,
    span: &impl Fn(u32) -> Span,
    args: &[Payload],
    base_line: u32,
    callee: NodeId,
    call_span: Span,
) -> NodeId {
    let arg_nodes: Vec<_> = args
        .iter()
        .enumerate()
        .map(|(idx, &payload)| {
            builder.add(
                NodeKind::Lit,
                payload,
                span(base_line + 3 + idx as u32),
                &[],
            )
        })
        .collect();
    let mut children = Vec::with_capacity(arg_nodes.len() + 1);
    children.push(callee);
    children.extend(arg_nodes);
    builder.add(NodeKind::Call, Payload::None, call_span, &children)
}

fn push_guava_immutable_map_of_evidence(
    il: &mut Il,
    span: &impl Fn(u32) -> Span,
    arity: usize,
    base_line: u32,
    call_span: Span,
) {
    let symbol = SymbolEvidenceKind::ImportedBinding {
        module_hash: stable_symbol_hash("com.google.common.collect"),
        exported_hash: stable_symbol_hash("ImmutableMap"),
    };
    il.evidence.push(language_core_test_evidence(
        0,
        Lang::Java,
        EvidenceAnchor::binding(span(base_line), stable_symbol_hash("ImmutableMap")),
        EvidenceKind::Symbol(symbol),
        EvidenceStatus::Asserted,
    ));
    il.evidence
        .push(language_core_test_evidence_with_dependencies(
            1,
            Lang::Java,
            EvidenceAnchor::node(span(base_line + 1), NodeKind::Var),
            EvidenceKind::Symbol(symbol),
            EvidenceStatus::Asserted,
            vec![EvidenceId(0)],
        ));
    let contract = library_java_map_factory_contract(Lang::Java, "ImmutableMap", "of")
        .expect("ImmutableMap.of contract");
    il.evidence
        .push(library_api_test_evidence_with_dependencies(
            2,
            call_span,
            LibraryApiTestContract {
                id: contract.id,
                callee: contract.callee,
                arity: arity as u16,
            },
            EvidenceStatus::Asserted,
            vec![EvidenceId(1)],
            (
                JAVA_GUAVA_IMMUTABLE_COLLECTION_FACTORY_PACK_ID,
                JAVA_GUAVA_IMMUTABLE_COLLECTION_FACTORY_PRODUCER_ID,
            ),
        ));
}

pub fn language_core_test_evidence(
    id: u32,
    lang: Lang,
    anchor: EvidenceAnchor,
    kind: EvidenceKind,
    status: EvidenceStatus,
) -> EvidenceRecord {
    language_core_test_evidence_with_dependencies(id, lang, anchor, kind, status, Vec::new())
}

pub fn language_core_test_evidence_with_dependencies(
    id: u32,
    lang: Lang,
    anchor: EvidenceAnchor,
    kind: EvidenceKind,
    status: EvidenceStatus,
    dependencies: Vec<EvidenceId>,
) -> EvidenceRecord {
    let (pack_id, producer_id) = language_core_evidence_provenance(lang);
    EvidenceRecord::builtin(
        EvidenceId(id),
        anchor,
        kind,
        pack_id,
        producer_id,
        dependencies,
        status,
    )
}

pub fn language_core_test_asserted_evidence(
    id: u32,
    lang: Lang,
    anchor: EvidenceAnchor,
    kind: EvidenceKind,
    dependencies: Vec<EvidenceId>,
) -> EvidenceRecord {
    language_core_test_evidence_with_dependencies(
        id,
        lang,
        anchor,
        kind,
        EvidenceStatus::Asserted,
        dependencies,
    )
}

pub fn map_len_test_il_with_lambda(
    lambda: impl FnOnce(&mut IlBuilder) -> NodeId,
    lang: Lang,
) -> (Il, NodeId, NodeId) {
    let span = |line| Span::new(FileId(0), line, line, line, line);
    let mut builder = IlBuilder::new(FileId(0));
    let item = builder.add(NodeKind::Lit, Payload::LitInt(1), span(1), &[]);
    let collection = builder.add(NodeKind::Seq, Payload::None, span(1), &[item]);
    let lambda = lambda(&mut builder);
    let hof = builder.add(
        NodeKind::HoF,
        Payload::HoF(HoFKind::Map),
        span(3),
        &[collection, lambda],
    );
    let len = builder.add(
        NodeKind::Call,
        Payload::Builtin(Builtin::Len),
        span(4),
        &[hof],
    );
    let il = builder.finish(
        len,
        FileMeta {
            path: "t.rs".into(),
            lang,
        },
        Vec::new(),
        Vec::new(),
    );
    (il, hof, len)
}

pub fn rust_pull_lazy_map_len_test_il() -> (Il, NodeId, NodeId) {
    let (mut il, hof, len) = map_len_test_il_with_lambda(
        |builder| {
            let span = Span::new(FileId(0), 2, 2, 2, 2);
            let param = builder.add(NodeKind::Param, Payload::Cid(0), span, &[]);
            let value = builder.add(NodeKind::Var, Payload::Cid(0), span, &[]);
            let ret = builder.add(NodeKind::Return, Payload::None, span, &[value]);
            let body = builder.add(NodeKind::Block, Payload::None, span, &[ret]);
            builder.add(NodeKind::Lambda, Payload::None, span, &[param, body])
        },
        Lang::Rust,
    );
    il.evidence
        .push(method_call_library_api_test_evidence_with_dependencies(
            0,
            Lang::Rust,
            "map",
            il.node(hof).span,
            1,
            Vec::new(),
        ));
    il.evidence
        .push(method_call_library_api_test_evidence_with_dependencies(
            1,
            Lang::Rust,
            "len",
            il.node(len).span,
            0,
            Vec::new(),
        ));
    (il, hof, len)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SEQUENCE_HOF_ADAPTER_PROTOCOL_PACK_ID, SEQUENCE_HOF_ADAPTER_PROTOCOL_PRODUCER_ID};
    use nose_il::stable_symbol_hash;

    #[test]
    fn method_call_library_api_test_evidence_uses_contract_provenance() {
        let record = method_call_library_api_test_evidence_with_dependencies(
            7,
            Lang::Rust,
            "map",
            Span::new(FileId(0), 1, 1, 1, 1),
            1,
            Vec::new(),
        );

        assert_eq!(
            record.provenance.pack_hash,
            Some(stable_symbol_hash(SEQUENCE_HOF_ADAPTER_PROTOCOL_PACK_ID))
        );
        assert_eq!(
            record.provenance.rule_hash,
            Some(stable_symbol_hash(
                SEQUENCE_HOF_ADAPTER_PROTOCOL_PRODUCER_ID
            ))
        );
    }
}
