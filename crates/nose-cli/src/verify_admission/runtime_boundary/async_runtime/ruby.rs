use super::push_task_spawn_missing_evidence;
use nose_il::{stable_symbol_hash, Interner, NodeId, NodeKind, Payload};

pub(super) fn push_ruby_thread_fiber_runtime_call_missing_evidence(
    il: &nose_il::Il,
    interner: &Interner,
    callee_path: &str,
    labels: &mut Vec<&'static str>,
) -> bool {
    match callee_path {
        "Thread.new" | "Thread.start" | "Thread.fork"
            if ruby_runtime_root_unshadowed(il, interner, "Thread") =>
        {
            push_ruby_thread_fiber_missing_evidence(labels);
            true
        }
        "Fiber.new" | "Fiber.schedule" if ruby_runtime_root_unshadowed(il, interner, "Fiber") => {
            push_ruby_thread_fiber_missing_evidence(labels);
            true
        }
        _ => false,
    }
}

fn push_ruby_thread_fiber_missing_evidence(labels: &mut Vec<&'static str>) {
    push_task_spawn_missing_evidence(labels);
    super::super::push_unique(labels, "concurrency-scheduling-contract");
}

fn ruby_runtime_root_unshadowed(il: &nose_il::Il, interner: &Interner, root: &str) -> bool {
    !ruby_runtime_root_shadowed(il, interner, root)
}

fn ruby_runtime_root_shadowed(il: &nose_il::Il, interner: &Interner, root: &str) -> bool {
    il.units.iter().any(|unit| {
        unit.name
            .is_some_and(|symbol| name_shadows_runtime_root(interner.resolve(symbol), root))
    }) || (0..il.nodes.len()).any(|idx| {
        let node = NodeId(idx as u32);
        match il.kind(node) {
            NodeKind::Assign => il
                .children(node)
                .first()
                .copied()
                .is_some_and(|lhs| node_subtree_shadows_runtime_root(il, interner, lhs, root)),
            NodeKind::Module | NodeKind::Block | NodeKind::Param => {
                node_name_shadows_runtime_root(il, interner, node, root)
            }
            NodeKind::Call => {
                ruby_dynamic_constant_definition_shadows_runtime_root(il, interner, node, root)
            }
            _ => false,
        }
    })
}

fn ruby_dynamic_constant_definition_shadows_runtime_root(
    il: &nose_il::Il,
    interner: &Interner,
    call: NodeId,
    root: &str,
) -> bool {
    let children = il.children(call);
    let Some((&callee, args)) = children.split_first() else {
        return false;
    };
    let Some(constant_args) = ruby_dynamic_constant_definition_args(il, interner, callee, args)
    else {
        return false;
    };
    constant_args
        .first()
        .copied()
        .is_some_and(|arg| node_is_static_literal_name(il, arg, root))
}

fn ruby_dynamic_constant_definition_args<'a>(
    il: &nose_il::Il,
    interner: &Interner,
    callee: NodeId,
    args: &'a [NodeId],
) -> Option<&'a [NodeId]> {
    let Payload::Name(symbol) = il.node(callee).payload else {
        return None;
    };
    match interner.resolve(symbol) {
        "const_set" | "autoload" => Some(args),
        "send" | "public_send" | "__send__"
            if args.first().copied().is_some_and(|method| {
                node_is_static_literal_name(il, method, "const_set")
                    || node_is_static_literal_name(il, method, "autoload")
            }) =>
        {
            Some(&args[1..])
        }
        "call" if ruby_method_object_defines_constant(il, interner, callee) => Some(args),
        _ => None,
    }
}

fn ruby_method_object_defines_constant(
    il: &nose_il::Il,
    interner: &Interner,
    call_field: NodeId,
) -> bool {
    let Some(call_receiver) = il.children(call_field).first().copied() else {
        return false;
    };
    ruby_method_lookup_defines_constant(il, interner, call_receiver)
        || ruby_bound_unbound_method_defines_constant(il, interner, call_receiver)
}

fn ruby_method_lookup_defines_constant(
    il: &nose_il::Il,
    interner: &Interner,
    method_call: NodeId,
) -> bool {
    if il.kind(method_call) != NodeKind::Call {
        return false;
    }
    let method_children = il.children(method_call);
    let Some((&method_callee, method_args)) = method_children.split_first() else {
        return false;
    };
    ruby_method_lookup_callee(il, interner, method_callee)
        && method_args.first().copied().is_some_and(|method| {
            node_is_static_literal_name(il, method, "const_set")
                || node_is_static_literal_name(il, method, "autoload")
        })
}

fn ruby_method_lookup_callee(il: &nose_il::Il, interner: &Interner, callee: NodeId) -> bool {
    matches!(
        il.node(callee).payload,
        Payload::Name(symbol) if matches!(interner.resolve(symbol), "method" | "public_method")
    )
}

fn ruby_bound_unbound_method_defines_constant(
    il: &nose_il::Il,
    interner: &Interner,
    bind_call: NodeId,
) -> bool {
    if il.kind(bind_call) != NodeKind::Call {
        return false;
    }
    let bind_children = il.children(bind_call);
    let Some((&bind_callee, _bind_args)) = bind_children.split_first() else {
        return false;
    };
    if !callee_named(il, interner, bind_callee, "bind") {
        return false;
    }
    let Some(unbound_method_call) = il.children(bind_callee).first().copied() else {
        return false;
    };
    if il.kind(unbound_method_call) != NodeKind::Call {
        return false;
    }
    let unbound_method_children = il.children(unbound_method_call);
    let Some((&unbound_method_callee, unbound_method_args)) = unbound_method_children.split_first()
    else {
        return false;
    };
    ruby_unbound_method_lookup_callee(il, interner, unbound_method_callee)
        && unbound_method_args.first().copied().is_some_and(|method| {
            node_is_static_literal_name(il, method, "const_set")
                || node_is_static_literal_name(il, method, "autoload")
        })
}

fn ruby_unbound_method_lookup_callee(
    il: &nose_il::Il,
    interner: &Interner,
    callee: NodeId,
) -> bool {
    matches!(
        il.node(callee).payload,
        Payload::Name(symbol)
            if matches!(interner.resolve(symbol), "instance_method" | "public_instance_method")
    )
}

fn callee_named(il: &nose_il::Il, interner: &Interner, callee: NodeId, expected: &str) -> bool {
    matches!(
        il.node(callee).payload,
        Payload::Name(symbol) if interner.resolve(symbol) == expected
    )
}

fn node_is_static_literal_name(il: &nose_il::Il, node: NodeId, expected: &str) -> bool {
    il.kind(node) == NodeKind::Lit
        && matches!(
            il.node(node).payload,
            Payload::LitStr(hash) if hash == stable_symbol_hash(expected)
        )
}

fn node_name_shadows_runtime_root(
    il: &nose_il::Il,
    interner: &Interner,
    node: NodeId,
    root: &str,
) -> bool {
    match il.node(node).payload {
        Payload::Name(symbol) => name_shadows_runtime_root(interner.resolve(symbol), root),
        Payload::Cid(cid) => il
            .cid_names
            .get(cid as usize)
            .is_some_and(|symbol| name_shadows_runtime_root(interner.resolve(*symbol), root)),
        _ => false,
    }
}

fn node_subtree_shadows_runtime_root(
    il: &nose_il::Il,
    interner: &Interner,
    node: NodeId,
    root: &str,
) -> bool {
    node_name_shadows_runtime_root(il, interner, node, root)
        || il
            .children(node)
            .iter()
            .copied()
            .any(|child| node_subtree_shadows_runtime_root(il, interner, child, root))
}

fn name_shadows_runtime_root(name: &str, root: &str) -> bool {
    name == root || name.rsplit("::").next() == Some(root)
}
