use super::*;

#[test]
fn query_accepts_explicit_multi_roots() {
    let dir = make_project("multi_roots");
    let a = dir.join("a");
    let b = dir.join("b");
    let a = a.to_str().unwrap();
    let b = b.to_str().unwrap();

    let out = run_raw(&[
        "query",
        "-r",
        a,
        "-r",
        b,
        "all",
        "top=0",
        "--mode",
        "semantic",
        "--min-size",
        "1",
        "--min-lines",
        "1",
        "--format",
        "json",
    ]);
    let json = query_json(&out);
    assert_eq!(json["view"], "list");
    assert_eq!(json["path"], format!("-r {a} -r {b}"));
    assert!(
        family_contains_all(&json, &["a/f.py", "b/f.py"]),
        "multi-root query should analyze both explicit roots: {json}"
    );

    let dash = run(&[
        "query",
        "-r",
        a,
        "-r",
        b,
        "--mode",
        "semantic",
        "--min-size",
        "1",
        "--min-lines",
        "1",
    ]);
    assert!(
        dash.contains(&format!("nose query -r {a} -r {b} id=")),
        "multi-root drill links should remain runnable: {dash}"
    );
}

#[test]
fn query_dedupes_repeated_explicit_roots() {
    let dir = make_project("repeated_roots");
    let path = dir.to_str().unwrap();
    let dotted = dir.join(".");
    let dotted = dotted.to_str().unwrap();
    let child = dir.join("a");
    let child = child.to_str().unwrap();
    let direct_file = dir.join("a/f.py");
    let direct_file = direct_file.to_str().unwrap();
    let query_roots = |roots: &[&str]| {
        let mut args = vec!["query"];
        for root in roots {
            args.push("-r");
            args.push(*root);
        }
        args.extend([
            "--mode",
            "semantic",
            "--min-size",
            "1",
            "--min-lines",
            "1",
            "--format",
            "json",
        ]);
        query_json(&run_raw(&args))
    };

    let once = query_roots(&[path]);
    let repeated = query_roots(&[path, path]);
    let dotted = query_roots(&[path, dotted]);
    let overlapping = query_roots(&[path, child]);
    let direct_file = query_roots(&[path, direct_file]);

    assert_eq!(once["summary"]["scanned_files"], 4);
    for (label, json) in [
        ("same root", &repeated),
        ("dotted root", &dotted),
        ("overlapping child root", &overlapping),
        ("direct file root", &direct_file),
    ] {
        assert_eq!(
            json["summary"]["scanned_files"], once["summary"]["scanned_files"],
            "{label} must not double-count the analysis scope: {json}"
        );
        assert_eq!(
            json["families"], once["families"],
            "{label} must not change the family dataset"
        );
    }
}

#[test]
fn query_second_path_suggests_explicit_roots() {
    let dir = make_project("second_path_hint");
    let a = dir.join("a");
    let b = dir.join("b");
    let err = run_fail(&["query", a.to_str().unwrap(), b.to_str().unwrap()]);
    assert!(
        err.contains("looks like another path") && err.contains("nose query -r"),
        "second positional path should explain explicit multi-root syntax: {err}"
    );

    let explicit_err = run_fail(&["query", "-r", a.to_str().unwrap(), b.to_str().unwrap()]);
    assert!(
        explicit_err.contains("When using `--root`/`-r`")
            && explicit_err.contains("bare arguments are query terms"),
        "bare path after --root should explain that all roots need -r: {explicit_err}"
    );
}
