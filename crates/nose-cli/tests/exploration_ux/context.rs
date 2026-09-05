use super::*;

#[test]
fn every_exploration_view_reports_the_same_effective_population() {
    let p = Project::new();
    for path in ["a.rs", "b.rs", "ignored/c.rs"] {
        p.write(path, &format!("fn compute() {{\n{RUST_BODY}}}\n"));
    }
    p.write(
        "custom.toml",
        "[query]\nmode = [\"syntax\"]\nexclude = [\"ignored/**\"]\nmin-size = 500\n",
    );
    let options = ["--config", "custom.toml", "--max-candidate-pairs", "1024"];
    let dashboard = p.query(&options);
    let expected = &dashboard["analysis"];
    assert_eq!(expected["roots"], serde_json::json!(["."]));
    assert_eq!(expected["modes"], serde_json::json!(["syntax"]));
    assert_eq!(expected["exclude"], serde_json::json!(["ignored/**"]));
    assert_eq!(expected["scanned_files"], 2);
    assert_eq!(
        expected["min_size"], 1,
        "explicit CLI floor overrides config"
    );
    assert_eq!(expected["max_candidate_pairs"], 1024);
    let id = format!("id={}", dashboard["families"][0]["id"].as_str().unwrap());
    for terms in [
        vec!["group=dir"],
        vec!["files>1"],
        vec![&id, "full"],
        vec![&id, "member-path~b.rs", "full"],
        vec!["reinvented"],
    ] {
        let args = options
            .iter()
            .chain(terms.iter())
            .copied()
            .collect::<Vec<_>>();
        let view = p.query(&args);
        assert_eq!(&view["analysis"], expected, "{terms:?}");
        let human = Command::new(env!("CARGO_BIN_EXE_nose"))
            .current_dir(&p.0)
            .args(["query", ".", "--min-size", "1", "--min-lines", "1"])
            .args(&args)
            .output()
            .unwrap();
        assert!(human.status.success());
        let text = String::from_utf8(human.stdout).unwrap();
        assert!(
            text.contains("analysis: 2 files")
                && text.contains("modes: syntax")
                && text.contains("ignored/**"),
            "{text}"
        );
    }
    for command in dashboard["next"].as_array().unwrap() {
        assert!(command
            .as_str()
            .unwrap()
            .contains("'--max-candidate-pairs' '1024'"));
    }
}

#[test]
fn source_windows_expose_unknown_declaration_boundaries() {
    let p = Project::new();
    let source = format!("fn first() {{\n{RUST_BODY}}}\nfn second() {{\n{RUST_BODY}}}\n");
    p.write("a.rs", &source);
    p.write("b.rs", &source);
    let list = p.query(&["--mode", "syntax", "all", "top=0"]);
    let family = list["families"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| {
            f["locations"]
                .as_array()
                .unwrap()
                .iter()
                .any(|l| l["boundary"]["kind"] == "unclassified-region")
        })
        .expect("cross-declaration run");
    let id = format!("id={}", family["id"].as_str().unwrap());
    let view = p.query(&["--mode", "syntax", &id, "member-path~a.rs", "full"]);
    let boundary = &view["member_view"]["source_bodies"]["members"][0]["boundary"];
    assert_eq!(boundary["extraction_safety"], "unassessed");
    assert!(boundary["enclosing_unit"].is_null());
    assert!(boundary["meaning"]
        .as_str()
        .unwrap()
        .contains("may cross declaration boundaries"));
}

#[test]
fn explicit_work_limit_overrides_environment_and_rejects_zero() {
    let p = Project::new();
    for path in ["a.py", "b.py", "c.py"] {
        p.write(
            path,
            "def compute(x):\n    a = x * x\n    b = a + 7\n    return b // 3\n",
        );
    }
    let run = |limit: &str| {
        Command::new(env!("CARGO_BIN_EXE_nose"))
            .current_dir(&p.0)
            .env("NOSE_MAX_CANDIDATE_PAIRS", "1")
            .args([
                "query",
                ".",
                "--mode",
                "semantic",
                "--min-size",
                "1",
                "--min-lines",
                "1",
                "--max-candidate-pairs",
                limit,
                "--format",
                "json",
            ])
            .output()
            .unwrap()
    };
    assert!(run("3").status.success());
    assert!(!run("2").status.success());
    let zero = run("0");
    assert!(!zero.status.success());
    assert!(zero.stdout.is_empty());
    assert!(String::from_utf8_lossy(&zero.stderr).contains("positive integer"));
}

#[test]
fn token_overlap_without_shared_whole_lines_explains_both_measurements() {
    let p = Project::new();
    let body = RUST_BODY.replace('\n', " ");
    p.write("a.rs", &format!("fn alpha() {{ {body} }}\n"));
    p.write("b.rs", &format!("fn beta() {{ {body} }}\n"));
    let list = p.query(&["--mode", "syntax", "files>1", "all"]);
    let family = &list["families"][0];
    assert_eq!(family["witness"], "copy-paste");
    assert_eq!(family["shared"], 0);
    let explanation = family["assessment"]["relation"]["explanation"]
        .as_str()
        .unwrap();
    assert!(
        explanation.contains("matching token run") && explanation.contains("whole source lines")
    );
    let output = Command::new(env!("CARGO_BIN_EXE_nose"))
        .current_dir(&p.0)
        .args([
            "query",
            ".",
            "--mode",
            "syntax",
            "--min-size",
            "1",
            "--min-lines",
            "1",
            "files>1",
            "all",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout)
        .contains("matching tokens; no invariant whole lines"));
}
