use crate::verify_admission::AdmissionContext;
use nose_il::{Interner, NodeId};

mod api_paths;
mod import_identity;
mod runtime_receiver;

use api_paths::{is_join_macro, is_select_macro, is_spawn, runtime_root};
use import_identity::{
    imported_async_join_macro_member, imported_async_select_macro_member,
    imported_async_spawn_member,
};
use runtime_receiver::is_future_drive_call;

pub(super) fn push_rust_async_runtime_call_missing_evidence(
    il: &nose_il::Il,
    interner: &Interner,
    call: NodeId,
    callee: NodeId,
    callee_path: Option<&str>,
    context: &AdmissionContext,
    labels: &mut Vec<&'static str>,
) -> bool {
    if callee_path
        .and_then(runtime_root)
        .is_some_and(|root| context.rust_runtime_root_is_local_for_file(root, &il.meta.path))
    {
        return false;
    }

    let is_macro_invocation = nose_semantics::source_call_at_node(il, call)
        == Some(nose_il::SourceCallKind::MacroInvocation);
    let operation = if is_macro_invocation {
        macro_operation(il, interner, callee, callee_path, context)
    } else {
        call_operation(il, interner, callee, callee_path, context)
    };
    operation.is_some_and(|operation| {
        operation.push_missing_evidence(labels);
        true
    })
}

fn call_operation(
    il: &nose_il::Il,
    interner: &Interner,
    callee: NodeId,
    callee_path: Option<&str>,
    context: &AdmissionContext,
) -> Option<RustAsyncOperation> {
    if callee_path.is_some_and(is_spawn)
        || imported_async_spawn_member(il, interner, callee, context)
    {
        return Some(RustAsyncOperation::Spawn);
    }
    is_future_drive_call(il, interner, callee, callee_path, context)
        .then_some(RustAsyncOperation::FutureDrive)
}

fn macro_operation(
    il: &nose_il::Il,
    interner: &Interner,
    callee: NodeId,
    callee_path: Option<&str>,
    context: &AdmissionContext,
) -> Option<RustAsyncOperation> {
    if callee_path.is_some_and(is_join_macro)
        || imported_async_join_macro_member(il, interner, callee, context)
    {
        return Some(RustAsyncOperation::JoinAll);
    }
    (callee_path.is_some_and(is_select_macro)
        || imported_async_select_macro_member(il, interner, callee, context))
    .then_some(RustAsyncOperation::SelectFirst)
}

enum RustAsyncOperation {
    Spawn,
    FutureDrive,
    JoinAll,
    SelectFirst,
}

impl RustAsyncOperation {
    fn push_missing_evidence(self, labels: &mut Vec<&'static str>) {
        match self {
            Self::Spawn => super::push_task_spawn_missing_evidence(labels),
            Self::FutureDrive => {
                super::super::push_unique(labels, "future-drive-scheduling-contract");
                super::super::push_unique(labels, "future-settled-value-channel-contract");
            }
            Self::JoinAll => super::push_async_aggregate_all_missing_evidence(labels),
            Self::SelectFirst => super::push_async_aggregate_first_missing_evidence(labels),
        }
    }
}
