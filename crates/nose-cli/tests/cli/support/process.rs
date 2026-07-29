use std::process::{Command, Output};

use super::fixtures::bin;

fn query_json_list_args(args: &[&str]) -> Vec<String> {
    let mut normalized = args
        .iter()
        .map(|arg| (*arg).to_string())
        .collect::<Vec<_>>();
    if args.first() != Some(&"query") || !args.windows(2).any(|w| w == ["--format", "json"]) {
        return normalized;
    }
    let mut has_term = false;
    let mut has_top = false;
    let mut skip_next = false;
    for arg in args.iter().skip(2) {
        if skip_next {
            skip_next = false;
            continue;
        }
        if matches!(
            *arg,
            "--mode"
                | "--min-size"
                | "--min-lines"
                | "--min-value"
                | "--min-members"
                | "--format"
                | "--baseline"
                | "--ignore-file"
                | "--config"
                | "--cache-dir"
                | "--semantic-pack"
                | "--exclude"
                | "--generated-path"
                | "--fail-on"
        ) {
            skip_next = true;
            continue;
        }
        if !arg.starts_with('-') {
            if arg.starts_with("top=") {
                has_top = true;
                continue;
            }
            has_term = true;
            break;
        }
    }
    if !has_term {
        if !has_top {
            normalized.insert(2, "top=0".to_string());
        }
        normalized.insert(2, "all".to_string());
    }
    normalized
}

fn owned_args(args: &[&str]) -> Vec<String> {
    args.iter().map(|arg| (*arg).to_string()).collect()
}

pub(crate) fn run(args: &[&str]) -> String {
    let normalized = query_json_list_args(args);
    let out = nose_output(&normalized);
    assert_success(&normalized, &out);
    String::from_utf8(out.stdout).unwrap()
}

pub(crate) fn run_raw(args: &[&str]) -> String {
    let args = owned_args(args);
    let out = nose_output(&args);
    assert_success(&args, &out);
    String::from_utf8(out.stdout).unwrap()
}

/// Like [`run`] but expects a non-zero exit; returns stderr (where errors print).
#[allow(dead_code)]
pub(crate) fn run_fail(args: &[&str]) -> String {
    let args = owned_args(args);
    let out = nose_output(&args);
    assert!(
        !out.status.success(),
        "{}",
        cli_output_message_with_bin(
            bin(),
            "expected nose to fail, but it succeeded",
            &args,
            &format!("{:?}", out.status),
            &out.stdout,
            &out.stderr
        )
    );
    String::from_utf8(out.stderr).unwrap()
}

fn nose_output(args: &[String]) -> Output {
    Command::new(bin())
        .args(args)
        .output()
        .unwrap_or_else(|err| {
            panic!(
                "failed to spawn nose\ncommand: {}\nerror: {err}",
                command_line_for_message(bin(), args)
            )
        })
}

fn assert_success(args: &[String], out: &Output) {
    assert!(
        out.status.success(),
        "{}",
        cli_output_message_with_bin(
            bin(),
            "nose exited non-zero",
            args,
            &format!("{:?}", out.status),
            &out.stdout,
            &out.stderr
        )
    );
}

fn cli_output_message_with_bin(
    bin: &str,
    summary: &str,
    args: &[String],
    status: &str,
    stdout: &[u8],
    stderr: &[u8],
) -> String {
    format!(
        "{summary}\nstatus: {status}\ncommand: {}\nstdout:\n{}\nstderr:\n{}",
        command_line_for_message(bin, args),
        String::from_utf8_lossy(stdout),
        String::from_utf8_lossy(stderr)
    )
}

fn command_line_for_message(bin: &str, args: &[String]) -> String {
    let mut parts = Vec::with_capacity(args.len() + 1);
    parts.push(bin.to_string());
    parts.extend(args.iter().map(|arg| format!("{arg:?}")));
    parts.join(" ")
}

#[cfg(test)]
#[path = "../support_tests.rs"]
mod tests;
