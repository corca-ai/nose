use super::support::*;

pub(super) fn method_call_il(
    lang: Lang,
    method: &str,
    literal_receiver: bool,
) -> (Il, Interner, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let field = method_field(&mut b, &interner, method, literal_receiver);
    let func_param = b.add(NodeKind::Param, Payload::Cid(0), sp(), &[]);
    let func_body = b.add(NodeKind::Block, Payload::None, sp(), &[]);
    let func = b.add(
        NodeKind::Lambda,
        Payload::None,
        sp(),
        &[func_param, func_body],
    );
    let call = b.add(NodeKind::Call, Payload::None, sp(), &[field, func]);
    let mut il = finish_method_call_il(b, lang, call);
    push_literal_receiver_api_evidence(&mut il, &interner, call, literal_receiver);
    (il, interner, call)
}

pub(super) fn method_call_no_arg_il(
    lang: Lang,
    method: &str,
    literal_receiver: bool,
) -> (Il, Interner, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let field = method_field(&mut b, &interner, method, literal_receiver);
    let call = b.add(NodeKind::Call, Payload::None, sp(), &[field]);
    let mut il = finish_method_call_il(b, lang, call);
    push_literal_receiver_api_evidence(&mut il, &interner, call, literal_receiver);
    (il, interner, call)
}

pub(super) fn method_call_with_arg_il(
    lang: Lang,
    method: &str,
    literal_receiver: bool,
    literal_arg: bool,
) -> (Il, Interner, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let field = method_field(&mut b, &interner, method, literal_receiver);
    let arg = collection_or_var(&mut b, &interner, literal_arg, "ys");
    let call = b.add(NodeKind::Call, Payload::None, sp(), &[field, arg]);
    let mut il = finish_method_call_il(b, lang, call);
    if literal_receiver {
        push_receiver_sequence_surface_evidence(&mut il, call, SequenceSurfaceKind::Collection);
    }
    if literal_arg {
        push_sequence_surface_evidence(&mut il, arg, SequenceSurfaceKind::Collection);
    }
    let _ = push_receiver_method_library_api_evidence(&mut il, &interner, call);
    (il, interner, call)
}

fn method_field(
    b: &mut IlBuilder,
    interner: &Interner,
    method: &str,
    literal_receiver: bool,
) -> NodeId {
    let receiver = collection_or_var(b, interner, literal_receiver, "xs");
    b.add(
        NodeKind::Field,
        Payload::Name(interner.intern(method)),
        sp(),
        &[receiver],
    )
}

fn collection_or_var(
    b: &mut IlBuilder,
    interner: &Interner,
    literal: bool,
    var_name: &str,
) -> NodeId {
    if literal {
        b.add(
            NodeKind::Seq,
            Payload::Name(interner.intern("array")),
            sp(),
            &[],
        )
    } else {
        b.add(
            NodeKind::Var,
            Payload::Name(interner.intern(var_name)),
            sp(),
            &[],
        )
    }
}

fn finish_method_call_il(b: IlBuilder, lang: Lang, call: NodeId) -> Il {
    finish_module_il(b, lang, vec![call], Vec::new())
}

fn push_literal_receiver_api_evidence(
    il: &mut Il,
    interner: &Interner,
    call: NodeId,
    literal_receiver: bool,
) {
    if literal_receiver {
        push_receiver_sequence_surface_evidence(il, call, SequenceSurfaceKind::Collection);
        let _ = push_receiver_method_library_api_evidence(il, interner, call);
    }
}
