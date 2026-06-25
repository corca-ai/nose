use super::support::*;

pub(super) fn method_call_il(
    lang: Lang,
    method: &str,
    literal_receiver: bool,
) -> (Il, Interner, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let receiver = if literal_receiver {
        b.add(
            NodeKind::Seq,
            Payload::Name(interner.intern("array")),
            sp(),
            &[],
        )
    } else {
        b.add(
            NodeKind::Var,
            Payload::Name(interner.intern("xs")),
            sp(),
            &[],
        )
    };
    let field = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern(method)),
        sp(),
        &[receiver],
    );
    let func_body = b.add(NodeKind::Block, Payload::None, sp(), &[]);
    let func = b.add(NodeKind::Lambda, Payload::None, sp(), &[func_body]);
    let call = b.add(NodeKind::Call, Payload::None, sp(), &[field, func]);
    let root = b.add(NodeKind::Module, Payload::None, sp(), &[call]);
    let mut il = b.finish(
        root,
        FileMeta {
            path: "t".to_string(),
            lang,
        },
        Vec::new(),
        Vec::new(),
    );
    if literal_receiver {
        push_receiver_sequence_surface_evidence(&mut il, call, SequenceSurfaceKind::Collection);
        let _ = push_receiver_method_library_api_evidence(&mut il, &interner, call);
    }
    (il, interner, call)
}

pub(super) fn method_call_no_arg_il(
    lang: Lang,
    method: &str,
    literal_receiver: bool,
) -> (Il, Interner, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let receiver = if literal_receiver {
        b.add(
            NodeKind::Seq,
            Payload::Name(interner.intern("array")),
            sp(),
            &[],
        )
    } else {
        b.add(
            NodeKind::Var,
            Payload::Name(interner.intern("xs")),
            sp(),
            &[],
        )
    };
    let field = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern(method)),
        sp(),
        &[receiver],
    );
    let call = b.add(NodeKind::Call, Payload::None, sp(), &[field]);
    let root = b.add(NodeKind::Module, Payload::None, sp(), &[call]);
    let mut il = b.finish(
        root,
        FileMeta {
            path: "t".to_string(),
            lang,
        },
        Vec::new(),
        Vec::new(),
    );
    if literal_receiver {
        push_receiver_sequence_surface_evidence(&mut il, call, SequenceSurfaceKind::Collection);
        let _ = push_receiver_method_library_api_evidence(&mut il, &interner, call);
    }
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
    let receiver = if literal_receiver {
        b.add(
            NodeKind::Seq,
            Payload::Name(interner.intern("array")),
            sp(),
            &[],
        )
    } else {
        b.add(
            NodeKind::Var,
            Payload::Name(interner.intern("xs")),
            sp(),
            &[],
        )
    };
    let field = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern(method)),
        sp(),
        &[receiver],
    );
    let arg = if literal_arg {
        b.add(
            NodeKind::Seq,
            Payload::Name(interner.intern("array")),
            sp(),
            &[],
        )
    } else {
        b.add(
            NodeKind::Var,
            Payload::Name(interner.intern("ys")),
            sp(),
            &[],
        )
    };
    let call = b.add(NodeKind::Call, Payload::None, sp(), &[field, arg]);
    let root = b.add(NodeKind::Module, Payload::None, sp(), &[call]);
    let mut il = b.finish(
        root,
        FileMeta {
            path: "t".to_string(),
            lang,
        },
        Vec::new(),
        Vec::new(),
    );
    if literal_receiver {
        push_receiver_sequence_surface_evidence(&mut il, call, SequenceSurfaceKind::Collection);
    }
    if literal_arg {
        push_sequence_surface_evidence(&mut il, arg, SequenceSurfaceKind::Collection);
    }
    let _ = push_receiver_method_library_api_evidence(&mut il, &interner, call);
    (il, interner, call)
}
