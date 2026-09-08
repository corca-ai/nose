use super::*;

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

fn go_print_canonical_call_il() -> (Il, NodeId) {
    let mut b = IlBuilder::new(FileId(0));
    let arg = b.add(NodeKind::Var, Payload::Cid(0), sp(49), &[]);
    canonical_builtin_call_il(Lang::Go, Builtin::Print, &[arg], b, arg)
}

fn push_canonical_unshadowed_symbol_dependency(il: &mut Il, id: u32, call: NodeId, name: &str) {
    il.push_evidence(language_core_symbol_record(
        id,
        EvidenceAnchor::node(il.node(call).span, NodeKind::Var),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash(name),
        },
        EvidenceStatus::Asserted,
        &[],
        il.meta.lang,
    ));
}

fn push_canonical_imported_namespace_dependency(
    il: &mut Il,
    binding_id: u32,
    occurrence_id: u32,
    call: NodeId,
    module: &str,
) {
    let symbol = SymbolEvidenceKind::ImportedNamespace {
        module_hash: stable_symbol_hash(module),
    };
    il.push_evidence(language_core_symbol_record(
        binding_id,
        EvidenceAnchor::binding(sp(48), stable_symbol_hash(module)),
        symbol,
        EvidenceStatus::Asserted,
        &[],
        il.meta.lang,
    ));
    il.push_evidence(language_core_symbol_record(
        occurrence_id,
        EvidenceAnchor::node(il.node(call).span, NodeKind::Var),
        symbol,
        EvidenceStatus::Asserted,
        &[binding_id],
        il.meta.lang,
    ));
}

fn rust_integer_canonical_builtin_call_il(builtin: Builtin, arg_count: usize) -> (Il, NodeId) {
    let mut b = IlBuilder::new(FileId(0));
    let args = (0..arg_count)
        .map(|idx| {
            b.add(
                NodeKind::Var,
                Payload::Cid(idx as u32),
                sp(90 + idx as u32),
                &[],
            )
        })
        .collect::<Vec<_>>();
    let root = args[0];
    canonical_builtin_call_il(Lang::Rust, builtin, &args, b, root)
}

fn java_math_canonical_builtin_call_il(builtin: Builtin, arg_count: usize) -> (Il, NodeId) {
    let mut b = IlBuilder::new(FileId(0));
    let args = (0..arg_count)
        .map(|idx| {
            b.add(
                NodeKind::Var,
                Payload::Cid(idx as u32),
                sp(120 + idx as u32),
                &[],
            )
        })
        .collect::<Vec<_>>();
    let root = args[0];
    canonical_builtin_call_il(Lang::Java, builtin, &args, b, root)
}

fn push_java_math_canonical_dependencies(il: &mut Il, call: NodeId) -> Vec<u32> {
    let call_span = il.node(call).span;
    il.push_evidence(language_core_symbol_record(
        0,
        EvidenceAnchor::node(call_span, NodeKind::Var),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Math"),
        },
        EvidenceStatus::Asserted,
        &[],
        Lang::Java,
    ));
    let args = il.children(call).to_vec();
    let mut dependencies = vec![0];
    for (idx, arg) in args.into_iter().enumerate() {
        let id = 1 + idx as u32;
        il.push_evidence(evidence(
            id,
            EvidenceAnchor::node(il.node(arg).span, il.kind(arg)),
            EvidenceKind::Domain(DomainEvidence::Integer),
            EvidenceStatus::Asserted,
        ));
        dependencies.push(id);
    }
    dependencies
}

mod advanced;
mod core;
