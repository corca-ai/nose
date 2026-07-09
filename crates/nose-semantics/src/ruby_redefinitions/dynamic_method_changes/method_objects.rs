use super::instruction_sequence::{
    ruby_instruction_sequence_class_receiver,
    ruby_instruction_sequence_eval_receiver_may_redefine_method,
    ruby_instruction_sequence_source_factory_target_may_match,
};
use super::*;

mod source_factories;

pub(super) use source_factories::{
    ruby_method_object_instruction_sequence_source_factory_call,
    ruby_method_object_instruction_sequence_source_factory_index,
};

pub(super) fn ruby_method_object_dynamic_method_change_call(
    il: &Il,
    interner: &Interner,
    callee: NodeId,
    args: &[NodeId],
    expected_method: &str,
    node_name: RubyNodeNameResolver,
    occurrence_span: Span,
) -> bool {
    let Some((receiver, method_args)) = ruby_method_object_invocation(il, interner, callee, args)
    else {
        return false;
    };
    ruby_method_object_dynamic_method_change_receiver(
        il,
        interner,
        receiver,
        method_args,
        expected_method,
        node_name,
        occurrence_span,
    )
}

pub(super) fn ruby_method_object_dynamic_method_change_index(
    il: &Il,
    interner: &Interner,
    index: NodeId,
    expected_method: &str,
    node_name: RubyNodeNameResolver,
) -> bool {
    let [receiver, args @ ..] = il.children(index) else {
        return false;
    };
    ruby_method_object_dynamic_method_change_receiver(
        il,
        interner,
        *receiver,
        args,
        expected_method,
        node_name,
        il.node(index).span,
    )
}

pub(super) fn ruby_method_object_wrapper_block_dynamic_method_change_call(
    il: &Il,
    interner: &Interner,
    callee: NodeId,
    args: &[NodeId],
    expected_method: &str,
    node_name: RubyNodeNameResolver,
    occurrence_span: Span,
) -> bool {
    let Some(wrapper_name) = field_name(il, interner, callee) else {
        return false;
    };
    if !matches!(wrapper_name, "tap" | "then" | "yield_self") {
        return false;
    }
    let Some(receiver) = il.children(callee).first().copied() else {
        return false;
    };
    let Some(target) =
        ruby_method_object_or_stored_target(il, interner, receiver, node_name, occurrence_span, 8)
    else {
        return false;
    };
    args.iter()
        .find_map(|&arg| ruby_descendant_method_name_literal(il, arg, expected_method, 8))
        .is_some_and(|method| {
            ruby_reflective_target_dynamic_method_change(il, target, &[method], expected_method)
        })
}

fn ruby_method_object_dynamic_method_change_receiver(
    il: &Il,
    interner: &Interner,
    receiver: NodeId,
    args: &[NodeId],
    expected_method: &str,
    node_name: RubyNodeNameResolver,
    occurrence_span: Span,
) -> bool {
    ruby_method_object_bound_instruction_sequence_eval(
        il,
        interner,
        receiver,
        node_name,
        occurrence_span,
        8,
    ) || ruby_method_object_or_stored_target(il, interner, receiver, node_name, occurrence_span, 8)
        .is_some_and(|target| {
            ruby_reflective_target_dynamic_method_change(il, target, args, expected_method)
        })
        || ruby_any_method_object_target_change_in_file(
            il,
            interner,
            node_name,
            occurrence_span,
            args,
            expected_method,
        )
}

fn ruby_method_object_bound_instruction_sequence_source_factory(
    il: &Il,
    interner: &Interner,
    method_object: NodeId,
    node_name: RubyNodeNameResolver,
    occurrence_span: Span,
    depth: usize,
) -> bool {
    if depth == 0 {
        return false;
    }
    if let Some((receiver, target)) =
        ruby_direct_bound_method_object_target(il, interner, method_object)
    {
        let target_may_be_source_factory =
            ruby_instruction_sequence_source_factory_target_may_match(il, target);
        if target_may_be_source_factory
            && ruby_instruction_sequence_class_receiver(il, interner, receiver, node_name)
        {
            return true;
        }
    }
    if let Some(receiver) = ruby_method_object_transparent_receiver(il, interner, method_object) {
        return ruby_method_object_bound_instruction_sequence_source_factory(
            il,
            interner,
            receiver,
            node_name,
            occurrence_span,
            depth - 1,
        );
    }
    let Some(receiver_name) = node_name(il, interner, method_object) else {
        return false;
    };
    il.nodes.iter().enumerate().any(|(idx, node)| {
        if node.kind != NodeKind::Assign || node.span.file != occurrence_span.file {
            return false;
        }
        let assign = NodeId(idx as u32);
        let [lhs, rhs, ..] = il.children(assign) else {
            return false;
        };
        node_name(il, interner, *lhs) == Some(receiver_name)
            && ruby_method_object_bound_instruction_sequence_source_factory(
                il,
                interner,
                *rhs,
                node_name,
                occurrence_span,
                depth - 1,
            )
    })
}

fn ruby_method_object_bound_instruction_sequence_eval(
    il: &Il,
    interner: &Interner,
    method_object: NodeId,
    node_name: RubyNodeNameResolver,
    occurrence_span: Span,
    depth: usize,
) -> bool {
    if depth == 0 {
        return false;
    }
    if let Some((receiver, target)) =
        ruby_direct_bound_method_object_target(il, interner, method_object)
    {
        let target_may_be_eval = method_name_argument_is_literal(il, target, "eval")
            || !matches!(il.node(target).payload, Payload::LitStr(_));
        if target_may_be_eval
            && ruby_instruction_sequence_eval_receiver_may_redefine_method(
                il,
                interner,
                receiver,
                node_name,
                occurrence_span,
                4,
            )
        {
            return true;
        }
    }
    if let Some(receiver) = ruby_method_object_transparent_receiver(il, interner, method_object) {
        return ruby_method_object_bound_instruction_sequence_eval(
            il,
            interner,
            receiver,
            node_name,
            occurrence_span,
            depth - 1,
        );
    }
    let Some(receiver_name) = node_name(il, interner, method_object) else {
        return false;
    };
    il.nodes.iter().enumerate().any(|(idx, node)| {
        if node.kind != NodeKind::Assign || node.span.file != occurrence_span.file {
            return false;
        }
        let assign = NodeId(idx as u32);
        let [lhs, rhs, ..] = il.children(assign) else {
            return false;
        };
        node_name(il, interner, *lhs) == Some(receiver_name)
            && ruby_method_object_bound_instruction_sequence_eval(
                il,
                interner,
                *rhs,
                node_name,
                occurrence_span,
                depth - 1,
            )
    })
}

fn ruby_direct_bound_method_object_target(
    il: &Il,
    interner: &Interner,
    method_object: NodeId,
) -> Option<(NodeId, NodeId)> {
    let [method_callee, args @ ..] = il.children(method_object) else {
        return None;
    };
    if il.kind(method_object) != NodeKind::Call {
        return None;
    }
    let method_callee_name = field_name(il, interner, *method_callee)?;
    let bound_receiver = il.children(*method_callee).first().copied()?;
    if matches!(method_callee_name, "send" | "public_send" | "__send__") {
        return ruby_reflective_method_object_constructor_target(il, interner, args)
            .map(|target| (bound_receiver, target));
    }
    matches!(
        method_callee_name,
        "instance_method"
            | "method"
            | "public_instance_method"
            | "public_method"
            | "singleton_method"
    )
    .then(|| args.first().copied().map(|target| (bound_receiver, target)))
    .flatten()
}

fn ruby_method_object_invocation<'a>(
    il: &'a Il,
    interner: &Interner,
    callee: NodeId,
    args: &'a [NodeId],
) -> Option<(NodeId, &'a [NodeId])> {
    let receiver = il.children(callee).first().copied()?;
    let direct_invocation = field_name(il, interner, callee)
        .is_some_and(ruby_method_object_invocation_name)
        || anonymous_field(il, callee);
    if direct_invocation {
        return Some((receiver, args));
    }
    let reflective_invocation = matches!(
        field_name(il, interner, callee),
        Some("send" | "public_send" | "__send__")
    );
    if !reflective_invocation {
        return None;
    }
    let [target, method_args @ ..] = args else {
        return None;
    };
    if ruby_method_object_invocation_argument_may_match(il, *target) {
        return Some((receiver, method_args));
    }
    ruby_single_array_argument_children(il, interner, args)
        .and_then(|args| ruby_method_object_invocation_from_args(il, receiver, args))
}

fn ruby_method_object_invocation_from_args<'a>(
    il: &Il,
    receiver: NodeId,
    args: &'a [NodeId],
) -> Option<(NodeId, &'a [NodeId])> {
    let [target, method_args @ ..] = args else {
        return None;
    };
    ruby_method_object_invocation_argument_may_match(il, *target).then_some((receiver, method_args))
}

fn ruby_method_object_invocation_name(name: &str) -> bool {
    matches!(name, "call" | "[]" | "===" | "yield")
}

fn ruby_method_object_invocation_argument_may_match(il: &Il, target: NodeId) -> bool {
    ["call", "[]", "===", "yield"]
        .iter()
        .any(|name| method_name_argument_is_literal(il, target, name))
        || !matches!(il.node(target).payload, Payload::LitStr(_))
}

fn anonymous_field(il: &Il, node: NodeId) -> bool {
    il.kind(node) == NodeKind::Field && matches!(il.node(node).payload, Payload::None)
}

fn ruby_method_object_or_stored_target(
    il: &Il,
    interner: &Interner,
    method_object: NodeId,
    node_name: RubyNodeNameResolver,
    occurrence_span: Span,
    depth: usize,
) -> Option<NodeId> {
    if depth == 0 {
        return None;
    }
    if let Some(target) = ruby_direct_method_object_target(il, interner, method_object, node_name) {
        return Some(target);
    }
    if let Some(receiver) = ruby_method_object_transparent_receiver(il, interner, method_object) {
        return ruby_method_object_or_stored_target(
            il,
            interner,
            receiver,
            node_name,
            occurrence_span,
            depth - 1,
        );
    }
    if let Some(target) = ruby_nested_method_object_target(
        il,
        interner,
        method_object,
        node_name,
        occurrence_span,
        depth - 1,
    ) {
        return Some(target);
    }
    let receiver_name = node_name(il, interner, method_object)?;
    il.nodes.iter().enumerate().find_map(|(idx, node)| {
        if node.kind != NodeKind::Assign || node.span.file != occurrence_span.file {
            return None;
        }
        let assign = NodeId(idx as u32);
        let [lhs, rhs, ..] = il.children(assign) else {
            return None;
        };
        (node_name(il, interner, *lhs) == Some(receiver_name))
            .then(|| {
                ruby_method_object_or_stored_target(
                    il,
                    interner,
                    *rhs,
                    node_name,
                    occurrence_span,
                    depth - 1,
                )
            })
            .flatten()
    })
}

fn ruby_nested_method_object_target(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    node_name: RubyNodeNameResolver,
    occurrence_span: Span,
    depth: usize,
) -> Option<NodeId> {
    if depth == 0 {
        return None;
    }
    il.children(node).iter().find_map(|&child| {
        ruby_method_object_or_stored_target(
            il,
            interner,
            child,
            node_name,
            occurrence_span,
            depth - 1,
        )
    })
}

fn ruby_any_method_object_target_change_in_file(
    il: &Il,
    interner: &Interner,
    node_name: RubyNodeNameResolver,
    occurrence_span: Span,
    args: &[NodeId],
    expected_method: &str,
) -> bool {
    let mut saw_method_object_constructor = false;
    for (idx, node) in il.nodes.iter().enumerate() {
        if node.span.file != occurrence_span.file {
            continue;
        }
        let Some(target) =
            ruby_direct_method_object_target(il, interner, NodeId(idx as u32), node_name)
        else {
            continue;
        };
        saw_method_object_constructor = true;
        if ruby_reflective_target_dynamic_method_change(il, target, args, expected_method) {
            return true;
        }
    }
    saw_method_object_constructor
        && il.nodes.iter().enumerate().any(|(idx, node)| {
            node.span.file == occurrence_span.file
                && ruby_literal_mutator_target_change(il, NodeId(idx as u32), args, expected_method)
        })
}

fn ruby_literal_mutator_target_change(
    il: &Il,
    target: NodeId,
    args: &[NodeId],
    expected_method: &str,
) -> bool {
    RUBY_DYNAMIC_METHOD_CHANGE_TARGETS
        .iter()
        .any(|name| method_name_argument_is_literal(il, target, name))
        && ruby_reflective_target_dynamic_method_change(il, target, args, expected_method)
}

fn ruby_descendant_method_name_literal(
    il: &Il,
    node: NodeId,
    expected_method: &str,
    depth: usize,
) -> Option<NodeId> {
    if depth == 0 {
        return None;
    }
    if method_name_argument_is_literal(il, node, expected_method) {
        return Some(node);
    }
    il.children(node).iter().find_map(|&child| {
        ruby_descendant_method_name_literal(il, child, expected_method, depth - 1)
    })
}

fn ruby_direct_method_object_target(
    il: &Il,
    interner: &Interner,
    method_object: NodeId,
    node_name: RubyNodeNameResolver,
) -> Option<NodeId> {
    let [method_callee, args @ ..] = il.children(method_object) else {
        return None;
    };
    if il.kind(method_object) != NodeKind::Call {
        return None;
    }
    let method_callee_name = node_name(il, interner, *method_callee)
        .or_else(|| field_name(il, interner, *method_callee))?;
    if matches!(method_callee_name, "send" | "public_send" | "__send__") {
        return ruby_reflective_method_object_constructor_target(il, interner, args);
    }
    if !matches!(
        method_callee_name,
        "instance_method"
            | "method"
            | "public_instance_method"
            | "public_method"
            | "singleton_method"
    ) {
        return None;
    }
    args.first().copied()
}

fn ruby_reflective_method_object_constructor_target(
    il: &Il,
    interner: &Interner,
    args: &[NodeId],
) -> Option<NodeId> {
    ruby_reflective_method_object_constructor_target_from_args(il, args).or_else(|| {
        ruby_single_array_argument_children(il, interner, args)
            .and_then(|args| ruby_reflective_method_object_constructor_target_from_args(il, args))
    })
}

fn ruby_reflective_method_object_constructor_target_from_args(
    il: &Il,
    args: &[NodeId],
) -> Option<NodeId> {
    let [constructor, method_target, ..] = args else {
        return None;
    };
    ruby_method_object_constructor_argument_may_match(il, *constructor).then_some(*method_target)
}

fn ruby_method_object_constructor_argument_may_match(il: &Il, target: NodeId) -> bool {
    [
        "instance_method",
        "method",
        "public_instance_method",
        "public_method",
        "singleton_method",
    ]
    .iter()
    .any(|name| method_name_argument_is_literal(il, target, name))
        || !matches!(il.node(target).payload, Payload::LitStr(_))
}

fn ruby_method_object_transparent_receiver(
    il: &Il,
    interner: &Interner,
    method_object: NodeId,
) -> Option<NodeId> {
    if il.kind(method_object) != NodeKind::Call {
        return None;
    };
    let [callee, args @ ..] = il.children(method_object) else {
        return None;
    };
    let receiver = il.children(*callee).first().copied()?;
    let wrapper_name = field_name(il, interner, *callee)?;
    if ruby_method_object_transparent_wrapper_name(wrapper_name) {
        return Some(receiver);
    }
    if !matches!(wrapper_name, "send" | "public_send" | "__send__") {
        return None;
    }
    ruby_method_object_transparent_receiver_from_args(il, receiver, args).or_else(|| {
        ruby_single_array_argument_children(il, interner, args)
            .and_then(|args| ruby_method_object_transparent_receiver_from_args(il, receiver, args))
    })
}

fn ruby_method_object_transparent_receiver_from_args(
    il: &Il,
    receiver: NodeId,
    args: &[NodeId],
) -> Option<NodeId> {
    let target = args.first().copied()?;
    ruby_method_object_transparent_wrapper_argument_may_match(il, target).then_some(receiver)
}

fn ruby_method_object_transparent_wrapper_name(name: &str) -> bool {
    matches!(
        name,
        "bind" | "clone" | "curry" | "dup" | "freeze" | "itself" | "tap" | "to_proc" | "unbind"
    )
}

fn ruby_method_object_transparent_wrapper_argument_may_match(il: &Il, target: NodeId) -> bool {
    [
        "bind", "clone", "curry", "dup", "freeze", "itself", "tap", "to_proc", "unbind",
    ]
    .iter()
    .any(|name| method_name_argument_is_literal(il, target, name))
        || !matches!(il.node(target).payload, Payload::LitStr(_))
}
