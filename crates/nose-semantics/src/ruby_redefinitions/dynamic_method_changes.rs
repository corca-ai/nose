use super::*;

mod instruction_sequence;
mod method_objects;

use instruction_sequence::{
    ruby_instruction_sequence_block_eval_dynamic_method_change,
    ruby_instruction_sequence_eval_dynamic_method_change,
    ruby_instruction_sequence_source_factory_dynamic_method_change_operation,
};
use method_objects::{
    ruby_method_object_dynamic_method_change_call, ruby_method_object_dynamic_method_change_index,
    ruby_method_object_wrapper_block_dynamic_method_change_call,
};

const RUBY_DYNAMIC_METHOD_CHANGE_TARGETS: &[&str] = &[
    "alias_method",
    "class_eval",
    "define_method",
    "define_singleton_method",
    "eval",
    "instance_eval",
    "module_eval",
    "remove_method",
    "undef_method",
];

pub(super) fn ruby_dynamic_method_change_operation(
    il: &Il,
    interner: &Interner,
    operation: NodeId,
    expected_method: &str,
    node_name: RubyNodeNameResolver,
) -> bool {
    match il.kind(operation) {
        NodeKind::Call => {
            ruby_dynamic_method_change_call(il, interner, operation, expected_method, node_name)
                || ruby_instruction_sequence_source_factory_dynamic_method_change_operation(
                    il,
                    interner,
                    operation,
                    node_name,
                    il.node(operation).span,
                )
        }
        NodeKind::Index => {
            ruby_method_object_dynamic_method_change_index(
                il,
                interner,
                operation,
                expected_method,
                node_name,
            ) || ruby_instruction_sequence_source_factory_dynamic_method_change_operation(
                il,
                interner,
                operation,
                node_name,
                il.node(operation).span,
            )
        }
        _ => false,
    }
}

fn ruby_dynamic_method_change_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    expected_method: &str,
    node_name: RubyNodeNameResolver,
) -> bool {
    let [callee, args @ ..] = il.children(call) else {
        return false;
    };
    let method_object_change = ruby_method_object_dynamic_method_change_call(
        il,
        interner,
        *callee,
        args,
        expected_method,
        node_name,
        il.node(call).span,
    ) || ruby_method_object_wrapper_block_dynamic_method_change_call(
        il,
        interner,
        *callee,
        args,
        expected_method,
        node_name,
        il.node(call).span,
    );
    let instruction_sequence_eval_change = ruby_instruction_sequence_eval_dynamic_method_change(
        il,
        interner,
        *callee,
        args,
        node_name,
        il.node(call).span,
    );
    let instruction_sequence_block_eval_change =
        ruby_instruction_sequence_block_eval_dynamic_method_change(
            il,
            interner,
            *callee,
            args,
            node_name,
            il.node(call).span,
        );
    let Some(callee_name) =
        node_name(il, interner, *callee).or_else(|| field_name(il, interner, *callee))
    else {
        return method_object_change
            || instruction_sequence_eval_change
            || instruction_sequence_block_eval_change;
    };
    ruby_dynamic_method_change_by_name(il, callee_name, args, expected_method)
        || ruby_reflective_dynamic_method_change_call(
            il,
            interner,
            callee_name,
            args,
            expected_method,
        )
        || instruction_sequence_eval_change
        || instruction_sequence_block_eval_change
        || method_object_change
}

fn ruby_dynamic_method_change_by_name(
    il: &Il,
    callee_name: &str,
    args: &[NodeId],
    expected_method: &str,
) -> bool {
    match callee_name {
        "alias_method" | "define_method" | "define_singleton_method" => args
            .first()
            .copied()
            .is_some_and(|arg| method_name_argument_may_match(il, arg, expected_method)),
        "undef_method" | "remove_method" => args
            .iter()
            .any(|&arg| method_name_argument_may_match(il, arg, expected_method)),
        "class_eval" | "eval" | "instance_eval" | "module_eval" => args
            .first()
            .copied()
            .is_some_and(|arg| ruby_eval_source_may_redefine_method(il, arg)),
        _ => false,
    }
}

fn ruby_eval_source_may_redefine_method(il: &Il, node: NodeId) -> bool {
    matches!(il.node(node).payload, Payload::LitStr(_))
        || !matches!(il.node(node).kind, NodeKind::Lit)
}

fn ruby_reflective_eval_invocation_args(il: &Il, interner: &Interner, args: &[NodeId]) -> bool {
    ruby_reflective_eval_invocation_args_direct(il, args)
        || ruby_single_array_argument_children(il, interner, args)
            .is_some_and(|args| ruby_reflective_eval_invocation_args_direct(il, args))
        || ruby_single_dynamic_argument_may_be_erased_splat(il, args)
}

fn ruby_reflective_eval_invocation_args_direct(il: &Il, args: &[NodeId]) -> bool {
    let [target, ..] = args else {
        return false;
    };
    method_name_argument_is_literal(il, *target, "eval")
        || !matches!(il.node(*target).payload, Payload::LitStr(_))
}

fn ruby_reflective_dynamic_method_change_call(
    il: &Il,
    interner: &Interner,
    callee_name: &str,
    args: &[NodeId],
    expected_method: &str,
) -> bool {
    if !matches!(callee_name, "send" | "public_send" | "__send__") {
        return false;
    }
    ruby_reflective_args_dynamic_method_change(il, args, expected_method)
        || ruby_single_array_argument_children(il, interner, args).is_some_and(|args| {
            ruby_reflective_args_dynamic_method_change(il, args, expected_method)
        })
        || ruby_single_dynamic_argument_may_be_erased_splat(il, args)
}

fn ruby_reflective_args_dynamic_method_change(
    il: &Il,
    args: &[NodeId],
    expected_method: &str,
) -> bool {
    let [target, method_args @ ..] = args else {
        return false;
    };
    ruby_reflective_target_dynamic_method_change(il, *target, method_args, expected_method)
}

fn ruby_single_array_argument_children<'a>(
    il: &'a Il,
    interner: &Interner,
    args: &'a [NodeId],
) -> Option<&'a [NodeId]> {
    let [arg] = args else {
        return None;
    };
    ruby_array_seq_children(il, interner, *arg)
}

fn ruby_array_seq_children<'a>(
    il: &'a Il,
    interner: &Interner,
    node: NodeId,
) -> Option<&'a [NodeId]> {
    if il.kind(node) != NodeKind::Seq {
        return None;
    }
    let Payload::Name(tag) = il.node(node).payload else {
        return None;
    };
    (interner.resolve(tag) == "array").then_some(il.children(node))
}

fn ruby_single_dynamic_argument_may_be_erased_splat(il: &Il, args: &[NodeId]) -> bool {
    let [arg] = args else {
        return false;
    };
    !matches!(il.node(*arg).payload, Payload::LitStr(_))
}

fn ruby_reflective_target_dynamic_method_change(
    il: &Il,
    target: NodeId,
    method_args: &[NodeId],
    expected_method: &str,
) -> bool {
    for &target_name in RUBY_DYNAMIC_METHOD_CHANGE_TARGETS {
        if method_name_argument_is_literal(il, target, target_name) {
            return ruby_dynamic_method_change_by_name(
                il,
                target_name,
                method_args,
                expected_method,
            );
        }
    }
    if matches!(il.node(target).payload, Payload::LitStr(_)) {
        return false;
    }
    method_args
        .iter()
        .any(|&arg| method_name_argument_may_match(il, arg, expected_method))
        || method_args
            .first()
            .copied()
            .is_some_and(|arg| ruby_eval_source_may_redefine_method(il, arg))
}
