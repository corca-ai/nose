use super::push_task_spawn_missing_evidence;
use nose_il::{Interner, NodeId, NodeKind};

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
            .is_some_and(|symbol| interner.resolve(symbol) == root)
    }) || (0..il.nodes.len()).any(|idx| {
        let node = NodeId(idx as u32);
        match il.kind(node) {
            NodeKind::Assign => il
                .children(node)
                .first()
                .copied()
                .is_some_and(|lhs| super::super::node_defines_name(il, interner, lhs, root)),
            NodeKind::Module | NodeKind::Block | NodeKind::Param => {
                super::super::node_defines_name(il, interner, node, root)
            }
            _ => false,
        }
    })
}
