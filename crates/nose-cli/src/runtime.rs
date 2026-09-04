/// Bounded recursive language adapters run only after the iterative parser
/// depth/node check. Common IL traversals use explicit work stacks.
pub(crate) const STACK_SIZE: usize = 64 * 1024 * 1024;

/// When a reader closes the pipe early — `nose query … | head`, quitting a pager —
/// the next write to stdout fails with `BrokenPipe`, and `println!` turns that into
/// a panic (the ugly `failed printing to stdout` message). The Unix convention for a
/// filter is to stop quietly instead. The textbook fix is to reset the `SIGPIPE`
/// disposition to `SIG_DFL`, but that needs `unsafe` and this crate is `unsafe`-free
/// (`unsafe_code = "forbid"`), so we install a panic hook that recognizes the
/// broken-pipe panic and exits 0 without a backtrace, while leaving every other panic
/// to the normal hook.
pub(crate) fn install_broken_pipe_guard() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if is_broken_pipe_panic(info) {
            std::process::exit(0);
        }
        default_hook(info);
    }));
}

/// True for the panic `println!`/`writeln!` raise when stdout (or stderr) is a broken
/// pipe. The payload is a `String` like `failed printing to stdout: Broken pipe
/// (os error 32)`; we match both the textual kind and the numeric `EPIPE` (32 on
/// Linux and macOS) so a localized `strerror` message is still caught.
fn is_broken_pipe_panic(info: &std::panic::PanicHookInfo<'_>) -> bool {
    let payload = info.payload();
    let msg = payload
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| payload.downcast_ref::<&str>().copied());
    matches!(msg, Some(m) if m.contains("Broken pipe") || m.contains("os error 32"))
}
