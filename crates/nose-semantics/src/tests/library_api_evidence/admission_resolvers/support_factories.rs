use super::*;

pub(crate) fn java_arrays_stream_call_il() -> (Il, Interner, NodeId, NodeId) {
    java_arrays_stream_call_il_with_arg_count(1)
}

pub(crate) fn java_arrays_stream_call_il_with_arg_count(
    arg_count: usize,
) -> (Il, Interner, NodeId, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let local = interner.intern("Arrays");
    let lhs = b.add(NodeKind::Var, Payload::Name(local), sp(66), &[]);
    let rhs = b.add(NodeKind::Seq, Payload::None, sp(66), &[]);
    let import = b.add(NodeKind::Assign, Payload::None, sp(66), &[lhs, rhs]);
    let receiver = b.add(NodeKind::Var, Payload::Name(local), sp(67), &[]);
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("stream")),
        sp(68),
        &[receiver],
    );
    let mut children = vec![callee];
    for idx in 0..arg_count {
        children.push(b.add(
            NodeKind::Var,
            Payload::Cid(idx as u32),
            sp(69 + idx as u32),
            &[],
        ));
    }
    let call = b.add(NodeKind::Call, Payload::None, sp(70), &children);
    let root = b.add(NodeKind::Module, Payload::None, sp(71), &[import, call]);
    (finish_il(b, root, Lang::Java), interner, call, receiver)
}

pub(crate) fn java_map_factory_call_il() -> (Il, Interner, NodeId, NodeId, NodeId) {
    java_static_binary_factory_call_il("Map", "of", 80)
}

pub(crate) fn java_guava_map_factory_call_il() -> (Il, Interner, NodeId, NodeId, NodeId) {
    java_static_binary_factory_call_il("ImmutableMap", "of", 180)
}

fn java_static_binary_factory_call_il(
    receiver_name: &str,
    method: &str,
    base_line: u32,
) -> (Il, Interner, NodeId, NodeId, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let local = interner.intern(receiver_name);
    let lhs = b.add(NodeKind::Var, Payload::Name(local), sp(base_line), &[]);
    let rhs = b.add(NodeKind::Seq, Payload::None, sp(base_line), &[]);
    let import = b.add(NodeKind::Assign, Payload::None, sp(base_line), &[lhs, rhs]);
    let receiver = b.add(NodeKind::Var, Payload::Name(local), sp(base_line + 1), &[]);
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern(method)),
        sp(base_line + 2),
        &[receiver],
    );
    let key = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("red")),
        sp(base_line + 3),
        &[],
    );
    let value = b.add(NodeKind::Lit, Payload::LitInt(1), sp(base_line + 4), &[]);
    let call = b.add(
        NodeKind::Call,
        Payload::None,
        sp(base_line + 5),
        &[callee, key, value],
    );
    let root = b.add(
        NodeKind::Module,
        Payload::None,
        sp(base_line + 6),
        &[import, call],
    );
    (
        finish_il(b, root, Lang::Java),
        interner,
        call,
        callee,
        receiver,
    )
}

pub(crate) fn push_java_guava_import_dependencies(il: &mut Il, receiver: NodeId) {
    let binding_symbol = EvidenceKind::Symbol(SymbolEvidenceKind::ImportedBinding {
        module_hash: stable_symbol_hash("com.google.common.collect"),
        exported_hash: stable_symbol_hash("ImmutableMap"),
    });
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::binding(sp(180), stable_symbol_hash("ImmutableMap")),
        binding_symbol,
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(evidence_with_dependencies(
        1,
        EvidenceAnchor::node(il.node(receiver).span, NodeKind::Var),
        binding_symbol,
        EvidenceStatus::Asserted,
        vec![EvidenceId(0)],
    ));
}

pub(crate) fn java_map_entry_call_il() -> (Il, Interner, NodeId, NodeId, NodeId) {
    java_map_entry_call_il_with_arg_count(2)
}

pub(crate) fn java_map_entry_call_il_with_arg_count(
    arg_count: usize,
) -> (Il, Interner, NodeId, NodeId, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let local = interner.intern("Map");
    let lhs = b.add(NodeKind::Var, Payload::Name(local), sp(80), &[]);
    let rhs = b.add(NodeKind::Seq, Payload::None, sp(80), &[]);
    let import = b.add(NodeKind::Assign, Payload::None, sp(80), &[lhs, rhs]);
    let receiver = b.add(NodeKind::Var, Payload::Name(local), sp(81), &[]);
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("entry")),
        sp(82),
        &[receiver],
    );
    let key = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("red")),
        sp(83),
        &[],
    );
    let value = b.add(NodeKind::Lit, Payload::LitInt(1), sp(84), &[]);
    let extra = b.add(NodeKind::Lit, Payload::LitInt(2), sp(85), &[]);
    let mut children = vec![callee, key, value];
    if arg_count > 2 {
        children.push(extra);
    }
    let call = b.add(NodeKind::Call, Payload::None, sp(86), &children);
    let root = b.add(NodeKind::Module, Payload::None, sp(86), &[import, call]);
    (
        finish_il(b, root, Lang::Java),
        interner,
        call,
        callee,
        receiver,
    )
}

pub(crate) fn java_collection_constructor_call_il() -> (Il, Interner, NodeId, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let callee = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("ArrayList")),
        sp(87),
        &[],
    );
    let call = b.add(NodeKind::Call, Payload::None, sp(88), &[callee]);
    let root = b.add(NodeKind::Module, Payload::None, sp(89), &[call]);
    (finish_il(b, root, Lang::Java), interner, call, callee)
}

pub(crate) fn js_global_constructor_call_il(receiver: &str) -> (Il, Interner, NodeId, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let callee = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern(receiver)),
        sp(90),
        &[],
    );
    let values = b.add(
        NodeKind::Seq,
        Payload::Name(interner.intern("array")),
        sp(91),
        &[],
    );
    let call = b.add(NodeKind::Call, Payload::None, sp(92), &[callee, values]);
    let root = b.add(NodeKind::Module, Payload::None, sp(93), &[call]);
    (finish_il(b, root, Lang::JavaScript), interner, call, callee)
}

pub(crate) fn push_java_collection_constructor_dependencies(
    il: &mut Il,
    call: NodeId,
    callee: NodeId,
) {
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::source_span(il.node(call).span),
        EvidenceKind::Source(SourceFactKind::Call(SourceCallKind::Construct)),
        EvidenceStatus::Asserted,
    ));
    let binding_symbol = EvidenceKind::Symbol(SymbolEvidenceKind::ImportedBinding {
        module_hash: stable_symbol_hash("java.util"),
        exported_hash: stable_symbol_hash("ArrayList"),
    });
    il.evidence.push(evidence(
        1,
        EvidenceAnchor::binding(sp(87), stable_symbol_hash("ArrayList")),
        binding_symbol,
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(evidence_with_dependencies(
        2,
        EvidenceAnchor::node(il.node(callee).span, NodeKind::Var),
        binding_symbol,
        EvidenceStatus::Asserted,
        vec![EvidenceId(1)],
    ));
}

pub(crate) fn push_js_global_constructor_dependencies(
    il: &mut Il,
    call: NodeId,
    callee: NodeId,
    name: &str,
) {
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::source_span(il.node(call).span),
        EvidenceKind::Source(SourceFactKind::Call(SourceCallKind::Construct)),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(language_core_symbol_record(
        1,
        EvidenceAnchor::node(il.node(callee).span, NodeKind::Var),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash(name),
        },
        EvidenceStatus::Asserted,
        &[],
        Lang::JavaScript,
    ));
}

pub(crate) fn push_java_map_import_dependencies(il: &mut Il, receiver: NodeId) {
    let binding_symbol = EvidenceKind::Symbol(SymbolEvidenceKind::ImportedBinding {
        module_hash: stable_symbol_hash("java.util"),
        exported_hash: stable_symbol_hash("Map"),
    });
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::binding(sp(80), stable_symbol_hash("Map")),
        binding_symbol,
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(evidence_with_dependencies(
        1,
        EvidenceAnchor::node(il.node(receiver).span, NodeKind::Var),
        binding_symbol,
        EvidenceStatus::Asserted,
        vec![EvidenceId(0)],
    ));
}
