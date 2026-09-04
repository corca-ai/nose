use super::*;

#[test]
fn directory_ignores_preserve_gate_results_across_invocation_paths() {
    let dir = make_project("ignore_directory_paths");
    fs::create_dir_all(dir.join("a/nested")).unwrap();
    fs::rename(dir.join("a/f.py"), dir.join("a/nested/f.py")).unwrap();
    fs::create_dir_all(dir.join("suppressions")).unwrap();

    let query = |cwd: &Path, root: &Path, ignore: &Path| {
        Command::new(bin())
            .arg("query")
            .arg(root)
            .args([
                "--mode",
                "semantic",
                "--min-size",
                "12",
                "--format",
                "json",
                "all",
                "top=0",
                "--fail-on",
                "any",
                "--ignore-file",
            ])
            .arg(ignore)
            .current_dir(cwd)
            .output()
            .expect("run query")
    };
    let ignore = dir.join("nose.ignore.json");
    fs::write(&ignore, "{\"ignores\":[]}").unwrap();
    let before = query(&dir, Path::new("."), &ignore);
    assert_eq!(
        before.status.code(),
        Some(1),
        "unignored family must trip gate"
    );
    let before_json = query_json(&String::from_utf8(before.stdout).unwrap());
    assert!(!query_families(&before_json).is_empty());

    let body = r#"{"ignores":[{"paths":["/a/","/b/","/tests/"],"reason":"generated"}]}"#;
    fs::write(&ignore, body).unwrap();
    let nested_ignore = dir.join("suppressions/nose.ignore.json");
    fs::write(&nested_ignore, body).unwrap();
    let parent = dir.parent().unwrap();
    let relative_root = Path::new(dir.file_name().unwrap());
    for (cwd, root, ignore) in [
        (dir.as_path(), Path::new("."), &ignore),
        (parent, relative_root, &ignore),
        (parent, dir.as_path(), &ignore),
        (dir.as_path(), Path::new("."), &nested_ignore),
    ] {
        let output = query(cwd, root, ignore);
        assert!(
            output.status.success(),
            "directory ignore should pass the gate from {}: {}",
            cwd.display(),
            String::from_utf8_lossy(&output.stderr)
        );
        let report = query_json(&String::from_utf8(output.stdout).unwrap());
        assert!(query_families(&report).is_empty(), "{report}");
    }
    fs::remove_dir_all(dir).unwrap();
}
