use super::*;

pub(crate) fn ruby_method_object_instruction_sequence_source_factory_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    node_name: RubyNodeNameResolver,
    occurrence_span: Span,
    index: &RubyDynamicMethodChangeIndex,
) -> bool {
    let [callee, args @ ..] = il.children(call) else {
        return false;
    };
    if il.kind(call) != NodeKind::Call {
        return false;
    }
    let Some((method_object, factory_args)) =
        ruby_method_object_invocation(il, interner, *callee, args)
    else {
        return false;
    };
    ruby_method_object_bound_instruction_sequence_source_factory(
        il,
        interner,
        method_object,
        node_name,
        occurrence_span,
        8,
        index,
    ) && factory_args
        .first()
        .copied()
        .is_some_and(|arg| ruby_eval_source_may_redefine_method(il, arg))
}

pub(crate) fn ruby_method_object_instruction_sequence_source_factory_index(
    il: &Il,
    interner: &Interner,
    index: NodeId,
    node_name: RubyNodeNameResolver,
    change_index: &RubyDynamicMethodChangeIndex,
) -> bool {
    let [method_object, factory_args @ ..] = il.children(index) else {
        return false;
    };
    ruby_method_object_bound_instruction_sequence_source_factory(
        il,
        interner,
        *method_object,
        node_name,
        il.node(index).span,
        8,
        change_index,
    ) && factory_args
        .first()
        .copied()
        .is_some_and(|arg| ruby_eval_source_may_redefine_method(il, arg))
}
