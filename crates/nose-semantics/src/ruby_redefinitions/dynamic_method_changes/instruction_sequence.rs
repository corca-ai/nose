use super::*;

const RUBY_INSTRUCTION_SEQUENCE_SOURCE_FACTORY_TARGETS: &[&str] = &[
    "compile",
    "compile_file",
    "load",
    "load_from_binary",
    "load_from_binary_extra_data",
    "new",
];

pub(super) fn ruby_instruction_sequence_eval_dynamic_method_change(
    il: &Il,
    interner: &Interner,
    callee: NodeId,
    args: &[NodeId],
    node_name: RubyNodeNameResolver,
    occurrence_span: Span,
    index: &RubyDynamicMethodChangeIndex,
) -> bool {
    let Some(callee_name) = field_name(il, interner, callee) else {
        return false;
    };
    if !matches!(callee_name, "eval" | "send" | "public_send" | "__send__") {
        return false;
    }
    let Some(receiver) = il.children(callee).first().copied() else {
        return false;
    };
    if !ruby_instruction_sequence_eval_receiver_may_redefine_method(
        il,
        interner,
        receiver,
        node_name,
        occurrence_span,
        4,
        index,
    ) {
        return false;
    }
    match callee_name {
        "eval" => true,
        "send" | "public_send" | "__send__" => {
            ruby_reflective_eval_invocation_args(il, interner, args)
        }
        _ => false,
    }
}

pub(super) fn ruby_instruction_sequence_block_eval_dynamic_method_change(
    il: &Il,
    interner: &Interner,
    callee: NodeId,
    args: &[NodeId],
    node_name: RubyNodeNameResolver,
    occurrence_span: Span,
    index: &RubyDynamicMethodChangeIndex,
) -> bool {
    let Some(callee_name) = field_name(il, interner, callee) else {
        return false;
    };
    if !matches!(
        callee_name,
        "instance_eval" | "instance_exec" | "tap" | "then" | "yield_self"
    ) {
        return false;
    }
    let Some(receiver) = il.children(callee).first().copied() else {
        return false;
    };
    ruby_instruction_sequence_eval_receiver_may_redefine_method(
        il,
        interner,
        receiver,
        node_name,
        occurrence_span,
        4,
        index,
    ) && args.iter().any(|&arg| {
        ruby_descendant_instruction_sequence_eval_invocation(il, interner, arg, node_name, 8)
    })
}

pub(super) fn ruby_instruction_sequence_source_factory_dynamic_method_change_operation(
    il: &Il,
    interner: &Interner,
    operation: NodeId,
    node_name: RubyNodeNameResolver,
    occurrence_span: Span,
    index: &RubyDynamicMethodChangeIndex,
) -> bool {
    if il.kind(operation) == NodeKind::Index {
        return method_objects::ruby_method_object_instruction_sequence_source_factory_index(
            il, interner, operation, node_name, index,
        );
    }
    ruby_instruction_sequence_eval_receiver_may_redefine_method(
        il,
        interner,
        operation,
        node_name,
        occurrence_span,
        4,
        index,
    )
}

fn ruby_descendant_instruction_sequence_eval_invocation(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    node_name: RubyNodeNameResolver,
    depth: usize,
) -> bool {
    if depth == 0 {
        return false;
    }
    if node_name(il, interner, node) == Some("eval") {
        return true;
    }
    if il.kind(node) == NodeKind::Call {
        let [callee, args @ ..] = il.children(node) else {
            return false;
        };
        let callee_name =
            node_name(il, interner, *callee).or_else(|| field_name(il, interner, *callee));
        if matches!(callee_name, Some("eval"))
            || matches!(callee_name, Some("send" | "public_send" | "__send__"))
                && ruby_reflective_eval_invocation_args(il, interner, args)
            || matches!(
                callee_name,
                Some("method" | "public_method" | "singleton_method")
            ) && args
                .first()
                .copied()
                .is_some_and(|arg| method_name_argument_may_match(il, arg, "eval"))
        {
            return true;
        }
    }
    il.children(node).iter().any(|&child| {
        ruby_descendant_instruction_sequence_eval_invocation(
            il,
            interner,
            child,
            node_name,
            depth - 1,
        )
    })
}

pub(super) fn ruby_instruction_sequence_eval_receiver_may_redefine_method(
    il: &Il,
    interner: &Interner,
    receiver: NodeId,
    node_name: RubyNodeNameResolver,
    occurrence_span: Span,
    depth: usize,
    index: &RubyDynamicMethodChangeIndex,
) -> bool {
    let file = occurrence_span.file.0;
    if let Some(result) = index.instruction_sequence_eval_receiver(file, receiver, depth) {
        return result;
    }
    let result = ruby_instruction_sequence_eval_receiver_may_redefine_method_uncached(
        il,
        interner,
        receiver,
        node_name,
        occurrence_span,
        depth,
        index,
    );
    index.cache_instruction_sequence_eval_receiver(file, receiver, depth, result);
    result
}

fn ruby_instruction_sequence_eval_receiver_may_redefine_method_uncached(
    il: &Il,
    interner: &Interner,
    receiver: NodeId,
    node_name: RubyNodeNameResolver,
    occurrence_span: Span,
    depth: usize,
    index: &RubyDynamicMethodChangeIndex,
) -> bool {
    if depth == 0 {
        return false;
    }
    if ruby_instruction_sequence_source_factory_call(il, interner, receiver, node_name) {
        return true;
    }
    if method_objects::ruby_method_object_instruction_sequence_source_factory_call(
        il,
        interner,
        receiver,
        node_name,
        occurrence_span,
        index,
    ) {
        return true;
    }
    if il.children(receiver).iter().any(|&child| {
        ruby_instruction_sequence_eval_receiver_may_redefine_method(
            il,
            interner,
            child,
            node_name,
            occurrence_span,
            depth - 1,
            index,
        )
    }) {
        return true;
    }
    let Some(receiver_name) = node_name(il, interner, receiver) else {
        return false;
    };
    index
        .assigned_values(occurrence_span.file.0, receiver_name)
        .iter()
        .any(|&rhs| {
            ruby_instruction_sequence_eval_receiver_may_redefine_method(
                il,
                interner,
                rhs,
                node_name,
                occurrence_span,
                depth - 1,
                index,
            )
        })
}

fn ruby_instruction_sequence_source_factory_call(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    node_name: RubyNodeNameResolver,
) -> bool {
    let [callee, args @ ..] = il.children(node) else {
        return false;
    };
    if il.kind(node) != NodeKind::Call {
        return false;
    }
    let Some(factory_name) = field_name(il, interner, *callee) else {
        return false;
    };
    let Some(receiver) = il.children(*callee).first().copied() else {
        return false;
    };
    if !ruby_instruction_sequence_class_receiver(il, interner, receiver, node_name) {
        return false;
    }
    if ruby_instruction_sequence_source_factory_name(factory_name) {
        return args
            .first()
            .copied()
            .is_some_and(|arg| ruby_eval_source_may_redefine_method(il, arg));
    }
    if matches!(factory_name, "send" | "public_send" | "__send__") {
        return ruby_reflective_instruction_sequence_source_factory_args(il, interner, args);
    }
    false
}

fn ruby_instruction_sequence_source_factory_name(name: &str) -> bool {
    RUBY_INSTRUCTION_SEQUENCE_SOURCE_FACTORY_TARGETS.contains(&name)
}

pub(super) fn ruby_instruction_sequence_source_factory_target_may_match(
    il: &Il,
    target: NodeId,
) -> bool {
    RUBY_INSTRUCTION_SEQUENCE_SOURCE_FACTORY_TARGETS
        .iter()
        .any(|name| method_name_argument_is_literal(il, target, name))
        || !matches!(il.node(target).payload, Payload::LitStr(_))
}

fn ruby_reflective_instruction_sequence_source_factory_args(
    il: &Il,
    interner: &Interner,
    args: &[NodeId],
) -> bool {
    ruby_reflective_instruction_sequence_source_factory_args_direct(il, args)
        || ruby_single_array_argument_children(il, interner, args).is_some_and(|args| {
            ruby_reflective_instruction_sequence_source_factory_args_direct(il, args)
        })
        || ruby_single_dynamic_argument_may_be_erased_splat(il, args)
}

fn ruby_reflective_instruction_sequence_source_factory_args_direct(
    il: &Il,
    args: &[NodeId],
) -> bool {
    let [target, source, ..] = args else {
        return false;
    };
    ruby_instruction_sequence_source_factory_target_may_match(il, *target)
        && ruby_eval_source_may_redefine_method(il, *source)
}

pub(super) fn ruby_instruction_sequence_class_receiver(
    il: &Il,
    interner: &Interner,
    receiver: NodeId,
    node_name: RubyNodeNameResolver,
) -> bool {
    node_name(il, interner, receiver) == Some("RubyVM::InstructionSequence")
        || ruby_instruction_sequence_const_get_receiver(il, interner, receiver, node_name)
        || ruby_instruction_sequence_singleton_class_receiver(il, interner, receiver, node_name)
}

fn ruby_instruction_sequence_singleton_class_receiver(
    il: &Il,
    interner: &Interner,
    receiver: NodeId,
    node_name: RubyNodeNameResolver,
) -> bool {
    let [callee] = il.children(receiver) else {
        return false;
    };
    if il.kind(receiver) != NodeKind::Call
        || field_name(il, interner, *callee) != Some("singleton_class")
    {
        return false;
    }
    il.children(*callee)
        .first()
        .copied()
        .is_some_and(|receiver| {
            ruby_instruction_sequence_class_receiver(il, interner, receiver, node_name)
        })
}

fn ruby_instruction_sequence_const_get_receiver(
    il: &Il,
    interner: &Interner,
    receiver: NodeId,
    node_name: RubyNodeNameResolver,
) -> bool {
    let [callee, args @ ..] = il.children(receiver) else {
        return false;
    };
    if il.kind(receiver) != NodeKind::Call {
        return false;
    }
    let Some(callee_name) = field_name(il, interner, *callee) else {
        return false;
    };
    let Some(const_lookup_receiver) = il.children(*callee).first().copied() else {
        return false;
    };
    match callee_name {
        "const_get" => ruby_const_get_instruction_sequence_args(
            il,
            interner,
            const_lookup_receiver,
            args,
            node_name,
        ),
        "send" | "public_send" | "__send__" => ruby_reflective_const_get_instruction_sequence_args(
            il,
            interner,
            const_lookup_receiver,
            args,
            node_name,
        ),
        _ => false,
    }
}

fn ruby_const_get_instruction_sequence_args(
    il: &Il,
    interner: &Interner,
    const_lookup_receiver: NodeId,
    args: &[NodeId],
    node_name: RubyNodeNameResolver,
) -> bool {
    let Some(const_name) = args.first().copied() else {
        return false;
    };
    (ruby_instruction_sequence_rubyvm_receiver(il, interner, const_lookup_receiver, node_name, 4)
        && method_name_argument_may_match(il, const_name, "InstructionSequence"))
        || (ruby_global_const_lookup_receiver(il, interner, const_lookup_receiver, node_name)
            && method_name_argument_may_match(il, const_name, "RubyVM::InstructionSequence"))
}

fn ruby_reflective_const_get_instruction_sequence_args(
    il: &Il,
    interner: &Interner,
    const_lookup_receiver: NodeId,
    args: &[NodeId],
    node_name: RubyNodeNameResolver,
) -> bool {
    ruby_reflective_const_get_instruction_sequence_args_direct(
        il,
        interner,
        const_lookup_receiver,
        args,
        node_name,
    ) || ruby_single_array_argument_children(il, interner, args).is_some_and(|args| {
        ruby_reflective_const_get_instruction_sequence_args_direct(
            il,
            interner,
            const_lookup_receiver,
            args,
            node_name,
        )
    }) || ruby_single_dynamic_argument_may_be_erased_splat(il, args)
}

fn ruby_reflective_const_get_instruction_sequence_args_direct(
    il: &Il,
    interner: &Interner,
    const_lookup_receiver: NodeId,
    args: &[NodeId],
    node_name: RubyNodeNameResolver,
) -> bool {
    let [target, const_name, ..] = args else {
        return false;
    };
    (method_name_argument_is_literal(il, *target, "const_get")
        || !matches!(il.node(*target).payload, Payload::LitStr(_)))
        && ((ruby_instruction_sequence_rubyvm_receiver(
            il,
            interner,
            const_lookup_receiver,
            node_name,
            4,
        ) && method_name_argument_may_match(il, *const_name, "InstructionSequence"))
            || (ruby_global_const_lookup_receiver(il, interner, const_lookup_receiver, node_name)
                && method_name_argument_may_match(il, *const_name, "RubyVM::InstructionSequence")))
}

fn ruby_instruction_sequence_rubyvm_receiver(
    il: &Il,
    interner: &Interner,
    receiver: NodeId,
    node_name: RubyNodeNameResolver,
    depth: usize,
) -> bool {
    if node_name(il, interner, receiver) == Some("RubyVM") {
        return true;
    }
    if depth == 0 {
        return false;
    }
    let [callee, args @ ..] = il.children(receiver) else {
        return false;
    };
    if il.kind(receiver) != NodeKind::Call {
        return false;
    }
    let Some(callee_name) = field_name(il, interner, *callee) else {
        return false;
    };
    let Some(const_lookup_receiver) = il.children(*callee).first().copied() else {
        return false;
    };
    match callee_name {
        "const_get" => args.first().copied().is_some_and(|arg| {
            ruby_global_const_lookup_receiver(il, interner, const_lookup_receiver, node_name)
                && method_name_argument_may_match(il, arg, "RubyVM")
        }),
        "send" | "public_send" | "__send__" => ruby_reflective_const_get_rubyvm_args(
            il,
            interner,
            const_lookup_receiver,
            args,
            node_name,
        ),
        _ => false,
    }
}

fn ruby_reflective_const_get_rubyvm_args(
    il: &Il,
    interner: &Interner,
    const_lookup_receiver: NodeId,
    args: &[NodeId],
    node_name: RubyNodeNameResolver,
) -> bool {
    ruby_reflective_const_get_rubyvm_args_direct(
        il,
        interner,
        const_lookup_receiver,
        args,
        node_name,
    ) || ruby_single_array_argument_children(il, interner, args).is_some_and(|args| {
        ruby_reflective_const_get_rubyvm_args_direct(
            il,
            interner,
            const_lookup_receiver,
            args,
            node_name,
        )
    }) || ruby_single_dynamic_argument_may_be_erased_splat(il, args)
}

fn ruby_reflective_const_get_rubyvm_args_direct(
    il: &Il,
    interner: &Interner,
    const_lookup_receiver: NodeId,
    args: &[NodeId],
    node_name: RubyNodeNameResolver,
) -> bool {
    let [target, const_name, ..] = args else {
        return false;
    };
    (method_name_argument_is_literal(il, *target, "const_get")
        || !matches!(il.node(*target).payload, Payload::LitStr(_)))
        && ruby_global_const_lookup_receiver(il, interner, const_lookup_receiver, node_name)
        && method_name_argument_may_match(il, *const_name, "RubyVM")
}

fn ruby_global_const_lookup_receiver(
    il: &Il,
    interner: &Interner,
    receiver: NodeId,
    node_name: RubyNodeNameResolver,
) -> bool {
    matches!(
        node_name(il, interner, receiver),
        Some("Object" | "Module" | "Kernel")
    )
}
