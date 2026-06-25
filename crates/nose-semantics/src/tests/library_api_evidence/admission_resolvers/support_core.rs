use super::*;

pub(crate) fn asserted_library_api_node_record(
    id: u32,
    il: &Il,
    node: NodeId,
    contract_id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    arity: u16,
    dependencies: &[u32],
) -> EvidenceRecord {
    evidence_with_dependencies(
        id,
        EvidenceAnchor::node(il.node(node).span, il.kind(node)),
        EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
            contract_hash: library_api_contract_id_hash(contract_id),
            callee_hash: library_api_callee_contract_hash(callee),
            arity,
        }),
        EvidenceStatus::Asserted,
        dependencies.iter().copied().map(EvidenceId).collect(),
    )
}

pub(crate) fn asserted_library_api_node_record_with_provenance(
    id: u32,
    il: &Il,
    node: NodeId,
    contract_id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    arity: u16,
    dependencies: &[u32],
    pack_id: &str,
    rule: &str,
) -> EvidenceRecord {
    let mut record =
        asserted_library_api_node_record(id, il, node, contract_id, callee, arity, dependencies);
    record.provenance.pack_hash = Some(stable_symbol_hash(pack_id));
    record.provenance.rule_hash = Some(stable_symbol_hash(rule));
    record
}

#[derive(Clone, Copy)]
pub(crate) enum CallbackFixtureShape {
    InlineFunc { cid: u32 },
    Reference { name: &'static str },
    EffectfulCall { callee: &'static str, arg_cid: u32 },
    MutatingAssign { lhs_cid: u32, rhs_cid: u32 },
    Throwing { err_cid: u32 },
}

pub(crate) fn callback_fixture_node(
    b: &mut IlBuilder,
    interner: &Interner,
    shape: CallbackFixtureShape,
    span_base: u32,
) -> NodeId {
    match shape {
        CallbackFixtureShape::InlineFunc { cid } => {
            b.add(NodeKind::Func, Payload::Cid(cid), sp(span_base), &[])
        }
        CallbackFixtureShape::Reference { name } => b.add(
            NodeKind::Var,
            Payload::Name(interner.intern(name)),
            sp(span_base),
            &[],
        ),
        CallbackFixtureShape::EffectfulCall { callee, arg_cid } => {
            let callee = b.add(
                NodeKind::Var,
                Payload::Name(interner.intern(callee)),
                sp(span_base + 1),
                &[],
            );
            let arg = b.add(NodeKind::Var, Payload::Cid(arg_cid), sp(span_base + 2), &[]);
            let call = b.add(
                NodeKind::Call,
                Payload::None,
                sp(span_base + 3),
                &[callee, arg],
            );
            let ret = b.add(NodeKind::Return, Payload::None, sp(span_base + 4), &[call]);
            let body = b.add(NodeKind::Block, Payload::None, sp(span_base + 5), &[ret]);
            b.add(NodeKind::Lambda, Payload::None, sp(span_base + 6), &[body])
        }
        CallbackFixtureShape::MutatingAssign { lhs_cid, rhs_cid } => {
            let lhs = b.add(NodeKind::Var, Payload::Cid(lhs_cid), sp(span_base + 1), &[]);
            let rhs = b.add(NodeKind::Var, Payload::Cid(rhs_cid), sp(span_base + 2), &[]);
            let assign = b.add(
                NodeKind::Assign,
                Payload::None,
                sp(span_base + 3),
                &[lhs, rhs],
            );
            let body = b.add(NodeKind::Block, Payload::None, sp(span_base + 4), &[assign]);
            b.add(NodeKind::Lambda, Payload::None, sp(span_base + 5), &[body])
        }
        CallbackFixtureShape::Throwing { err_cid } => {
            let err = b.add(NodeKind::Var, Payload::Cid(err_cid), sp(span_base + 1), &[]);
            let throw = b.add(NodeKind::Throw, Payload::None, sp(span_base + 2), &[err]);
            let body = b.add(NodeKind::Block, Payload::None, sp(span_base + 3), &[throw]);
            b.add(NodeKind::Lambda, Payload::None, sp(span_base + 4), &[body])
        }
    }
}

pub(crate) fn js_length_field_il() -> (Il, Interner, NodeId, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let receiver = b.add(NodeKind::Var, Payload::Cid(0), sp(42), &[]);
    let field = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("length")),
        sp(43),
        &[receiver],
    );
    let root = b.add(NodeKind::Func, Payload::None, sp(44), &[field]);
    (
        finish_il(b, root, Lang::JavaScript),
        interner,
        field,
        receiver,
    )
}

pub(crate) fn rust_some_call_il() -> (Il, Interner, NodeId, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let callee = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("Some")),
        sp(45),
        &[],
    );
    let value = b.add(NodeKind::Var, Payload::Cid(0), sp(46), &[]);
    let call = b.add(NodeKind::Call, Payload::None, sp(47), &[callee, value]);
    let root = b.add(NodeKind::Func, Payload::None, sp(48), &[call]);
    (finish_il(b, root, Lang::Rust), interner, call, callee)
}

pub(crate) fn rust_none_node_il() -> (Il, Interner, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let none = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("None")),
        sp(49),
        &[],
    );
    let root = b.add(NodeKind::Func, Payload::None, sp(50), &[none]);
    (finish_il(b, root, Lang::Rust), interner, none)
}

pub(crate) fn rust_option_and_then_call_il() -> (Il, Interner, NodeId, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let receiver = b.add(NodeKind::Var, Payload::Cid(0), sp(51), &[]);
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("and_then")),
        sp(52),
        &[receiver],
    );
    let callback = b.add(NodeKind::Func, Payload::None, sp(53), &[]);
    let call = b.add(NodeKind::Call, Payload::None, sp(54), &[callee, callback]);
    let root = b.add(NodeKind::Func, Payload::None, sp(55), &[call]);
    (finish_il(b, root, Lang::Rust), interner, call, receiver)
}

pub(crate) fn rust_vec_new_call_il() -> (Il, Interner, NodeId, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let callee = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("Vec::new")),
        sp(49),
        &[],
    );
    let call = b.add(NodeKind::Call, Payload::None, sp(50), &[callee]);
    let root = b.add(NodeKind::Func, Payload::None, sp(51), &[call]);
    (finish_il(b, root, Lang::Rust), interner, call, callee)
}

pub(crate) fn rust_vec_macro_call_il() -> (Il, Interner, NodeId, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let callee = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("vec")),
        sp(52),
        &[],
    );
    let value = b.add(NodeKind::Lit, Payload::LitInt(1), sp(53), &[]);
    let call = b.add(NodeKind::Call, Payload::None, sp(54), &[callee, value]);
    let root = b.add(NodeKind::Func, Payload::None, sp(55), &[call]);
    (finish_il(b, root, Lang::Rust), interner, call, callee)
}

pub(crate) fn rust_std_collection_factory_call_il() -> (Il, Interner, NodeId, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let callee = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("std::collections::HashSet::from")),
        sp(56),
        &[],
    );
    let value = b.add(NodeKind::Seq, Payload::None, sp(57), &[]);
    let call = b.add(NodeKind::Call, Payload::None, sp(58), &[callee, value]);
    let root = b.add(NodeKind::Func, Payload::None, sp(59), &[call]);
    (finish_il(b, root, Lang::Rust), interner, call, callee)
}

pub(crate) fn rust_std_map_factory_call_il() -> (Il, Interner, NodeId, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let callee = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("std::collections::HashMap::from")),
        sp(66),
        &[],
    );
    let value = b.add(NodeKind::Seq, Payload::None, sp(67), &[]);
    let call = b.add(NodeKind::Call, Payload::None, sp(68), &[callee, value]);
    let root = b.add(NodeKind::Func, Payload::None, sp(69), &[call]);
    (finish_il(b, root, Lang::Rust), interner, call, callee)
}

pub(crate) fn python_list_factory_call_il() -> (Il, Interner, NodeId, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let callee = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("list")),
        sp(58),
        &[],
    );
    let value = b.add(NodeKind::Var, Payload::Cid(0), sp(59), &[]);
    let call = b.add(NodeKind::Call, Payload::None, sp(60), &[callee, value]);
    let root = b.add(NodeKind::Func, Payload::None, sp(61), &[call]);
    (finish_il(b, root, Lang::Python), interner, call, callee)
}

pub(crate) fn python_deque_factory_call_il() -> (Il, Interner, NodeId, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let callee = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("Values")),
        sp(62),
        &[],
    );
    let value = b.add(NodeKind::Var, Payload::Cid(0), sp(63), &[]);
    let call = b.add(NodeKind::Call, Payload::None, sp(64), &[callee, value]);
    let root = b.add(NodeKind::Func, Payload::None, sp(65), &[call]);
    (finish_il(b, root, Lang::Python), interner, call, callee)
}

pub(crate) fn ruby_set_factory_call_il() -> (Il, Interner, NodeId, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let require = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("require")),
        sp(70),
        &[],
    );
    let require_arg = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("set")),
        sp(71),
        &[],
    );
    let require_call = b.add(
        NodeKind::Call,
        Payload::None,
        sp(72),
        &[require, require_arg],
    );
    let receiver = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("Set")),
        sp(73),
        &[],
    );
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("new")),
        sp(74),
        &[receiver],
    );
    let value = b.add(NodeKind::Var, Payload::Cid(0), sp(75), &[]);
    let call = b.add(NodeKind::Call, Payload::None, sp(76), &[callee, value]);
    let root = b.add(
        NodeKind::Module,
        Payload::None,
        sp(77),
        &[require_call, call],
    );
    (finish_il(b, root, Lang::Ruby), interner, call, receiver)
}

pub(crate) fn push_ruby_set_require_dependencies(il: &mut Il, receiver: NodeId) {
    il.evidence.push(language_core_symbol_record(
        0,
        EvidenceAnchor::node(il.node(receiver).span, NodeKind::Var),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Set"),
        },
        EvidenceStatus::Asserted,
        &[],
        Lang::Ruby,
    ));
    il.evidence.push(language_core_symbol_record(
        1,
        EvidenceAnchor::node(sp(70), NodeKind::Var),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("require"),
        },
        EvidenceStatus::Asserted,
        &[],
        Lang::Ruby,
    ));
    il.evidence.push(evidence_with_dependencies(
        2,
        EvidenceAnchor::source_span(span(70, 72, 1)),
        EvidenceKind::Import(ImportEvidenceKind::Require {
            module_hash: stable_symbol_hash("set"),
        }),
        EvidenceStatus::Asserted,
        vec![EvidenceId(1)],
    ));
}

pub(crate) fn python_len_builtin_call_il() -> (Il, Interner, NodeId, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let callee = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("len")),
        sp(66),
        &[],
    );
    let value = b.add(NodeKind::Var, Payload::Cid(0), sp(67), &[]);
    let call = b.add(NodeKind::Call, Payload::None, sp(68), &[callee, value]);
    let root = b.add(NodeKind::Func, Payload::None, sp(69), &[call]);
    (finish_il(b, root, Lang::Python), interner, call, callee)
}

pub(crate) fn python_math_prod_call_il() -> (Il, Interner, NodeId, NodeId) {
    python_math_prod_call_il_with_arg_count(1)
}

pub(crate) fn python_math_prod_call_il_with_arg_count(
    arg_count: usize,
) -> (Il, Interner, NodeId, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let math = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("math")),
        sp(66),
        &[],
    );
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("prod")),
        sp(67),
        &[math],
    );
    let mut children = vec![callee];
    for idx in 0..arg_count {
        children.push(b.add(
            NodeKind::Var,
            Payload::Cid(idx as u32),
            sp(68 + idx as u32),
            &[],
        ));
    }
    let call = b.add(NodeKind::Call, Payload::None, sp(70), &children);
    let root = b.add(NodeKind::Func, Payload::None, sp(71), &[call]);
    (finish_il(b, root, Lang::Python), interner, call, math)
}

pub(crate) fn push_python_math_namespace_dependencies(il: &mut Il, receiver: NodeId) {
    let namespace_symbol = SymbolEvidenceKind::ImportedNamespace {
        module_hash: stable_symbol_hash("math"),
    };
    il.evidence.push(language_core_symbol_record(
        0,
        EvidenceAnchor::binding(sp(66), stable_symbol_hash("math")),
        namespace_symbol,
        EvidenceStatus::Asserted,
        &[],
        Lang::Python,
    ));
    il.evidence.push(language_core_symbol_record(
        1,
        EvidenceAnchor::node(il.node(receiver).span, NodeKind::Var),
        namespace_symbol,
        EvidenceStatus::Asserted,
        &[0],
        Lang::Python,
    ));
}

pub(crate) fn go_fmt_println_call_il() -> (Il, Interner, NodeId, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let fmt = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("fmt")),
        sp(64),
        &[],
    );
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("Println")),
        sp(65),
        &[fmt],
    );
    let arg = b.add(NodeKind::Var, Payload::Cid(0), sp(66), &[]);
    let call = b.add(NodeKind::Call, Payload::None, sp(67), &[callee, arg]);
    let root = b.add(NodeKind::Func, Payload::None, sp(68), &[call]);
    (finish_il(b, root, Lang::Go), interner, call, fmt)
}
