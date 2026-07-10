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

pub(super) struct RubyDynamicMethodChangeIndex {
    assignments: FxHashMap<u32, FxHashMap<String, Vec<NodeId>>>,
    method_object_targets: FxHashMap<u32, Vec<NodeId>>,
    literal_mutator_targets: FxHashMap<u32, Vec<NodeId>>,
    instruction_sequence_eval_receivers: std::cell::RefCell<FxHashMap<(u32, u32, usize), bool>>,
}

#[derive(Clone, Copy)]
pub(super) struct RubyDynamicMethodChangeQuery<'a> {
    expected_method: &'a str,
    node_name: RubyNodeNameResolver,
    occurrence_span: Span,
    index: &'a RubyDynamicMethodChangeIndex,
}

impl RubyDynamicMethodChangeIndex {
    pub(super) fn new(il: &Il, interner: &Interner, node_name: RubyNodeNameResolver) -> Self {
        let mut assignments: FxHashMap<u32, FxHashMap<String, Vec<NodeId>>> = FxHashMap::default();
        let mut method_object_targets: FxHashMap<u32, Vec<NodeId>> = FxHashMap::default();
        let mut literal_mutator_targets: FxHashMap<u32, Vec<NodeId>> = FxHashMap::default();
        for (idx, node) in il.nodes.iter().enumerate() {
            let id = NodeId(idx as u32);
            let file = node.span.file.0;
            if node.kind == NodeKind::Assign {
                if let [lhs, rhs, ..] = il.children(id) {
                    if let Some(name) = node_name(il, interner, *lhs) {
                        assignments
                            .entry(file)
                            .or_default()
                            .entry(name.to_owned())
                            .or_default()
                            .push(*rhs);
                    }
                }
            }
            if let Some(target) =
                method_objects::ruby_direct_method_object_target(il, interner, id, node_name)
            {
                method_object_targets.entry(file).or_default().push(target);
            }
            if RUBY_DYNAMIC_METHOD_CHANGE_TARGETS
                .iter()
                .any(|name| method_name_argument_is_literal(il, id, name))
            {
                literal_mutator_targets.entry(file).or_default().push(id);
            }
        }
        Self {
            assignments,
            method_object_targets,
            literal_mutator_targets,
            instruction_sequence_eval_receivers: std::cell::RefCell::new(FxHashMap::default()),
        }
    }

    pub(super) fn assigned_values(&self, file: u32, name: &str) -> &[NodeId] {
        self.assignments
            .get(&file)
            .and_then(|by_name| by_name.get(name))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub(super) fn method_object_targets(&self, file: u32) -> &[NodeId] {
        self.method_object_targets
            .get(&file)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub(super) fn literal_mutator_targets(&self, file: u32) -> &[NodeId] {
        self.literal_mutator_targets
            .get(&file)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub(super) fn instruction_sequence_eval_receiver(
        &self,
        file: u32,
        receiver: NodeId,
        depth: usize,
    ) -> Option<bool> {
        self.instruction_sequence_eval_receivers
            .borrow()
            .get(&(file, receiver.0, depth))
            .copied()
    }

    pub(super) fn cache_instruction_sequence_eval_receiver(
        &self,
        file: u32,
        receiver: NodeId,
        depth: usize,
        result: bool,
    ) {
        self.instruction_sequence_eval_receivers
            .borrow_mut()
            .insert((file, receiver.0, depth), result);
    }
}

pub(super) fn ruby_dynamic_method_change_operation(
    il: &Il,
    interner: &Interner,
    operation: NodeId,
    expected_method: &str,
    node_name: RubyNodeNameResolver,
    index: &RubyDynamicMethodChangeIndex,
) -> bool {
    match il.kind(operation) {
        NodeKind::Call => {
            ruby_dynamic_method_change_call(
                il,
                interner,
                operation,
                expected_method,
                node_name,
                index,
            ) || ruby_instruction_sequence_source_factory_dynamic_method_change_operation(
                il,
                interner,
                operation,
                node_name,
                il.node(operation).span,
                index,
            )
        }
        NodeKind::Index => {
            ruby_method_object_dynamic_method_change_index(
                il,
                interner,
                operation,
                expected_method,
                node_name,
                index,
            ) || ruby_instruction_sequence_source_factory_dynamic_method_change_operation(
                il,
                interner,
                operation,
                node_name,
                il.node(operation).span,
                index,
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
    index: &RubyDynamicMethodChangeIndex,
) -> bool {
    let [callee, args @ ..] = il.children(call) else {
        return false;
    };
    let query = RubyDynamicMethodChangeQuery {
        expected_method,
        node_name,
        occurrence_span: il.node(call).span,
        index,
    };
    let method_object_change =
        ruby_method_object_dynamic_method_change_call(il, interner, *callee, args, query)
            || ruby_method_object_wrapper_block_dynamic_method_change_call(
                il, interner, *callee, args, query,
            );
    let instruction_sequence_eval_change = ruby_instruction_sequence_eval_dynamic_method_change(
        il,
        interner,
        *callee,
        args,
        node_name,
        il.node(call).span,
        index,
    );
    let instruction_sequence_block_eval_change =
        ruby_instruction_sequence_block_eval_dynamic_method_change(
            il,
            interner,
            *callee,
            args,
            node_name,
            il.node(call).span,
            index,
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
