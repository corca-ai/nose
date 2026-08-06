pub(super) fn runtime_root(callee_path: &str) -> Option<&str> {
    callee_path.split("::").next()
}

pub(super) fn module_root(module: &str) -> &str {
    module.split("::").next().unwrap_or(module)
}

pub(super) fn is_spawn(callee_path: &str) -> bool {
    matches!(
        callee_path,
        "tokio::spawn"
            | "tokio::task::spawn"
            | "tokio::task::spawn_blocking"
            | "async_std::task::spawn"
            | "async_std::task::spawn_blocking"
    )
}

pub(super) fn is_join_macro(callee_path: &str) -> bool {
    matches!(
        callee_path,
        "tokio::join"
            | "tokio::try_join"
            | "futures::join"
            | "futures::try_join"
            | "futures_util::join"
            | "futures_util::try_join"
    )
}

pub(super) fn is_select_macro(callee_path: &str) -> bool {
    matches!(
        callee_path,
        "tokio::select" | "futures::select" | "futures_util::select"
    )
}
