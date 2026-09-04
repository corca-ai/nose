use super::*;

#[cfg(unix)]
#[test]
fn unreadable_source_is_an_error_with_and_without_a_warm_cache() {
    use std::os::unix::fs::PermissionsExt;
    let project = TempProject::new("unreadable_gate");
    let source = "def f(items):\n    total = 0\n    for item in items:\n        if item > 0:\n            total += item * item\n    return total\n";
    project.write("a.py", source);
    project.write("b.py", source);
    let b = project.path().join("b.py");
    let cache = make_temp_dir("unreadable_cache");
    for cached in [false, true] {
        let run_query = || {
            let mut cmd = Command::new(bin());
            cmd.args([
                "query",
                project.path().to_str().unwrap(),
                "--mode",
                "semantic",
                "--min-size",
                "1",
                "--min-lines",
                "1",
                "all",
                "top=0",
                "--fail-on",
                "any",
            ]);
            if cached {
                cmd.arg("--cache-dir").arg(&cache);
            }
            cmd.output().unwrap()
        };
        assert!(!run_query().status.success());
        fs::set_permissions(&b, fs::Permissions::from_mode(0o0)).unwrap();
        let readable = fs::read(&b).is_ok();
        let output = run_query();
        fs::set_permissions(&b, fs::Permissions::from_mode(0o600)).unwrap();
        if readable {
            continue;
        } // Privileged test runners can bypass mode bits.
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("incomplete source analysis"), "{stderr}");
        assert!(stderr.contains("b.py"), "{stderr}");
    }
    fs::remove_dir_all(cache).unwrap();
}

#[test]
fn explicit_markdown_roots_are_independent_of_ancestor_names() {
    let project = TempProject::new("markdown_parent");
    let prose = "# Install\n\nDownload the binary from the releases page and place it on your PATH. Then run the version command to confirm the installation succeeded correctly.\n";
    for parent in ["normal/project", "target/project", "vendor/project"] {
        for file in ["a.md", "b.md"] {
            project.write(&format!("{parent}/{file}"), prose);
        }
        let root = project.path().join(parent);
        let out = run_raw(&["query", root.to_str().unwrap(), "--format", "json"]);
        let report: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(
            report["markdown"].as_array().unwrap().len(),
            1,
            "{parent}: {report}"
        );
    }
}
