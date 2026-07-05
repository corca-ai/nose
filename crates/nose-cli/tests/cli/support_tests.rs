use super::*;
use std::panic::{self, AssertUnwindSafe};

fn panic_message(panic: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = panic.downcast_ref::<String>() {
        return message.clone();
    }
    if let Some(message) = panic.downcast_ref::<&str>() {
        return (*message).to_string();
    }
    panic!("expected string panic payload");
}

fn assert_message_contains(message: &str, needles: &[&str]) {
    for needle in needles {
        assert!(message.contains(needle), "{message}");
    }
}

#[test]
fn cli_failure_formatter_names_command_status_and_streams() {
    let args = vec!["query".to_string(), "/tmp/nose project".to_string()];
    let message = cli_output_message_with_bin(
        "nose-bin",
        "nose exited non-zero",
        &args,
        "ExitStatus(unix_wait_status(256))",
        b"partial stdout",
        b"diagnostic stderr",
    );

    assert_message_contains(
        &message,
        &[
            "nose exited non-zero",
            "status: ExitStatus(unix_wait_status(256))",
            "command: nose-bin \"query\" \"/tmp/nose project\"",
            "stdout:\npartial stdout",
            "stderr:\ndiagnostic stderr",
        ],
    );
}

#[test]
fn assert_success_failure_message_uses_shared_cli_context() {
    let args = owned_args(&["__nose_missing_subcommand__"]);
    let out = nose_output(&args);
    assert!(!out.status.success(), "fixture command should fail");

    let panic = panic::catch_unwind(AssertUnwindSafe(|| assert_success(&args, &out)))
        .expect_err("assert_success should panic for failed output");
    let message = panic_message(panic);

    assert_message_contains(
        &message,
        &[
            "nose exited non-zero",
            "status:",
            "command:",
            "\"__nose_missing_subcommand__\"",
            "stdout:\n",
            "stderr:\n",
        ],
    );
}

#[test]
fn run_fail_success_message_uses_shared_cli_context() {
    let panic = panic::catch_unwind(|| {
        let _ = run_fail(&["--version"]);
    })
    .expect_err("run_fail should panic when the command succeeds");
    let message = panic_message(panic);

    assert_message_contains(
        &message,
        &[
            "expected nose to fail, but it succeeded",
            "status:",
            "command:",
            "\"--version\"",
            "stdout:\n",
            "stderr:\n",
        ],
    );
}
