use super::*;

fn run(p: &Project, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_nose"))
        .current_dir(&p.0)
        .args(args)
        .output()
        .unwrap()
}
fn follow(p: &Project, command: &str) -> Value {
    let binary_dir = PathBuf::from(env!("CARGO_BIN_EXE_nose"));
    let path = format!(
        "{}:{}",
        binary_dir.parent().unwrap().display(),
        std::env::var("PATH").unwrap()
    );
    let output = Command::new("sh")
        .current_dir(&p.0)
        .env("PATH", path)
        .args(["-c", command])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{command}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
fn project() -> Project {
    let p = Project::new();
    for path in ["src/a.rs", "src/b.rs", "ignored/c.rs"] {
        p.write(path, &format!("fn compute() {{\n{RUST_BODY}}}\n"));
    }
    p
}

#[test]
fn numeric_filter_errors_cannot_masquerade_as_empty_results() {
    let p = project();
    for term in [
        "files>=2",
        "files<=2",
        "files=abc",
        "files!=NaN",
        "files>inf",
        "files=1,",
        "files>1,2",
        "files~2",
    ] {
        let output = run(&p, &["query", ".", term, "--format", "json"]);
        assert!(!output.status.success(), "{term} must be rejected");
        assert!(
            output.stdout.is_empty(),
            "{term} must not emit successful findings"
        );
        assert!(String::from_utf8_lossy(&output.stderr).contains("numeric"));
    }
    let valid = p.query(&["files>1", "--mode", "syntax"]);
    assert!(!valid["families"].as_array().unwrap().is_empty());
}

#[test]
fn dashboard_group_and_list_commands_preserve_detection_settings() {
    let p = project();
    let options = [
        "--mode",
        "syntax",
        "--exclude",
        "ignored/**",
        "--cache-dir",
        "analysis cache",
    ];
    let dashboard = p.query(&options);
    assert_eq!(dashboard["path"], ".");
    for command in dashboard["next"].as_array().unwrap() {
        let command = command.as_str().unwrap();
        assert!(
            command.contains("'--mode' 'syntax'") && command.contains("'--exclude' 'ignored/**'"),
            "{command}"
        );
        assert!(
            command.contains("'--cache-dir' 'analysis cache'"),
            "{command}"
        );
        follow(&p, command);
    }
    let command = dashboard["next"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .find(|c| c.contains("group=dir"))
        .unwrap();
    let groups = follow(&p, command);
    let slice = follow(&p, groups["groups"][0]["next"][0].as_str().unwrap());
    assert!(!slice["families"].as_array().unwrap().is_empty());
    for family in slice["families"].as_array().unwrap() {
        assert!(family["locations"]
            .as_array()
            .unwrap()
            .iter()
            .all(|l| !l["file"].as_str().unwrap().contains("ignored")));
    }
    for command in slice["next"].as_array().unwrap() {
        follow(&p, command.as_str().unwrap());
    }
    let human = run(
        &p,
        &["query", ".", "--mode", "syntax", "--exclude", "ignored/**"],
    );
    let human = String::from_utf8(human.stdout).unwrap();
    for command in human
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("nose query "))
    {
        assert!(
            command.contains("'--mode' 'syntax'") && command.contains("'--exclude' 'ignored/**'"),
            "{command}"
        );
    }
}

#[test]
fn selected_full_member_view_contains_only_selected_source_bodies() {
    let p = project();
    let list = p.query(&["--mode", "syntax", "files>1"]);
    let family = &list["families"][0];
    let id = format!("id={}", family["id"].as_str().unwrap());
    let selected = p.query(&["--mode", "syntax", &id, "member-path~src/b.rs", "full"]);
    assert_eq!(selected["family"]["review_key"], family["review_key"]);
    let bodies = &selected["member_view"]["source_bodies"];
    assert_eq!(bodies["scope"], "selected-members");
    assert_eq!(bodies["selected"], 1);
    assert_eq!(bodies["members"][0]["file"], "src/b.rs");
    assert_eq!(bodies["members"][0]["status"], "available");
    assert!(!bodies["members"][0]["lines"].as_array().unwrap().is_empty());
    let human = run(
        &p,
        &[
            "query",
            ".",
            "--mode",
            "syntax",
            &id,
            "member-path~src/b.rs",
            "full",
        ],
    );
    let text = String::from_utf8(human.stdout).unwrap();
    assert!(
        text.contains("selected source: 1 / 1") && text.contains("│"),
        "{text}"
    );
}

#[test]
fn candidate_limit_failure_has_inventory_and_executable_recovery() {
    let p = project();
    p.write("src/d.rs", &format!("fn compute() {{\n{RUST_BODY}}}\n"));
    let output = Command::new(env!("CARGO_BIN_EXE_nose"))
        .current_dir(&p.0)
        .env("NOSE_MAX_CANDIDATE_PAIRS", "1")
        .args([
            "query",
            ".",
            "--mode",
            "near",
            "--exclude",
            "ignored/**",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(
        error.contains("Analysis incomplete") && error.contains("3 supported files"),
        "{error}"
    );
    let command = error
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("nose query "))
        .unwrap();
    assert!(command.contains("'--exclude' 'ignored/**'"), "{command}");
    // Remove the artificially tiny limit when testing the advertised command's syntax.
    follow(&p, command);
}

#[test]
fn help_teaches_basic_navigation_before_saved_analysis() {
    let p = Project::new();
    let output = run(&p, &["query", "--help"]);
    let help = String::from_utf8(output.stdout).unwrap();
    for term in [
        "files>1",
        "at=FILE:LINE",
        "group=dir",
        "member-path~TEXT full",
        ">= and <=",
    ] {
        assert!(help.contains(term), "missing {term}");
    }
    assert!(help.find("Filters:").unwrap() < help.find("Save with").unwrap());
}

#[test]
fn detail_roundtrip_keeps_filters_and_opens_one_member() {
    let p = project();
    let list = p.query(&[
        "--mode",
        "syntax",
        "scope=prod",
        "path~src/",
        "files>1",
        "top=5",
    ]);
    let detail = follow(&p, list["actions"][0]["command"].as_str().unwrap());
    let back = follow(
        &p,
        detail["member_view"]["actions"][0]["command"]
            .as_str()
            .unwrap(),
    );
    assert_eq!(list["families"], back["families"]);
    let loc = &detail["member_view"]["locations"][0];
    let member = follow(&p, loc["next"][0].as_str().unwrap());
    assert_eq!(member["member_view"]["selected"], 1);
    assert_eq!(member["member_view"]["locations"][0]["id"], loc["id"]);
    let parent = member["member_view"]["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|action| action["kind"] == "return-family")
        .expect("a member offers a direct return to its complete family");
    let family = follow(&p, parent["command"].as_str().unwrap());
    assert_eq!(family["family"]["id"], detail["family"]["id"]);
    assert_eq!(
        family["member_view"]["selected"],
        detail["member_view"]["total"]
    );
    assert_eq!(
        member["family"]["review_key"],
        detail["family"]["review_key"]
    );
    // A whole journey must preserve the list limit, not only its first back link.
    let grouped = follow(&p, detail["member_view"]["next"][1].as_str().unwrap());
    let narrowed = follow(
        &p,
        grouped["member_view"]["groups"][0]["next"][0]
            .as_str()
            .unwrap(),
    );
    let copy = follow(
        &p,
        narrowed["member_view"]["locations"][0]["next"][0]
            .as_str()
            .unwrap(),
    );
    for view in [&member, &family, &grouped, &narrowed, &copy] {
        let command = view["member_view"]["actions"][0]["command"]
            .as_str()
            .unwrap();
        let returned = follow(&p, command);
        assert!(command.contains("top=5"), "{command}");
        // Generated commands spell the same positional root as --root.
        for (key, expected) in back.as_object().unwrap() {
            assert_eq!(expected, &returned[key], "returned {key}: {command}");
        }
    }
}

#[test]
fn grouped_top_limits_rows_and_expands_without_changing_counts() {
    let p = Project::new();
    for (directory, source) in [
        (
            "a",
            "def f(xs):\n    acc = 0\n    for v in xs:\n        acc += v * 3\n    return acc\n",
        ),
        (
            "b",
            "def g(x):\n    a = x * 11\n    b = a + 29\n    c = b * b\n    return c + 5\n",
        ),
    ] {
        for file in ["one.py", "two.py"] {
            p.write(&format!("{directory}/{file}"), source);
        }
    }
    let full = p.query(&["--mode", "semantic", "all", "group=dir", "top=0"]);
    assert!(full["groups"].as_array().unwrap().len() > 1, "{full}");
    let limited = p.query(&["--mode", "semantic", "all", "group=dir", "top=1"]);
    assert_eq!(limited["groups"].as_array().unwrap().len(), 1);
    assert_eq!(
        limited["summary"]["groups_total"],
        full["summary"]["groups_total"]
    );
    let expanded = follow(&p, limited["next"][0].as_str().unwrap());
    assert_eq!(expanded["groups"], full["groups"]);
}

#[test]
fn member_context_opens_surrounding_contract_without_changing_the_member() {
    let p = Project::new();
    for (file, name, guard) in [
        ("a.rs", "left", "value < 0"),
        ("b.rs", "right", "value > 99"),
    ] {
        p.write(
            file,
            &format!("// Contract for {name}: preserve validation.\nfn {name}(value: i32) {{\n    if {guard} {{ return; }}\n{RUST_BODY}}}\n"),
        );
    }
    let list = p.query(&["--mode", "syntax", "all", "top=0"]);
    let family = list["families"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| {
            f["locations"].as_array().unwrap().iter().any(|loc| {
                loc["start"].as_u64().unwrap() > 1 && !loc["boundary"]["enclosing_unit"].is_null()
            })
        })
        .expect("an inner repeated region with enclosing control flow");
    let id = format!("id={}", family["id"].as_str().unwrap());
    let detail = p.query(&["--mode", "syntax", "all", "top=5", &id]);
    let member = follow(
        &p,
        detail["member_view"]["locations"][0]["next"][0]
            .as_str()
            .unwrap(),
    );
    let inspect = member["member_view"]["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["kind"] == "inspect-context")
        .expect("member offers surrounding source");
    let context = follow(&p, inspect["command"].as_str().unwrap());
    assert_eq!(context["family"]["id"], member["family"]["id"]);
    assert_eq!(
        context["family"]["review_key"],
        member["family"]["review_key"]
    );
    for field in [
        "id",
        "file",
        "start",
        "end",
        "region",
        "boundary",
        "scope_evidence",
    ] {
        assert_eq!(
            context["member_view"]["locations"][0][field],
            member["member_view"]["locations"][0][field]
        );
    }
    assert!(context["member_view"]["locations"][0]["next"][0]
        .as_str()
        .unwrap()
        .contains("member-context=20"));
    let body = &context["member_view"]["source_bodies"]["members"][0];
    assert!(
        body["lines"].as_array().unwrap().iter().any(|line| {
            line["text"]
                .as_str()
                .unwrap()
                .starts_with("// Contract for")
                && line["line"] == 1
                && line["in_member"] == false
        }),
        "surrounding source must include the adjacent contract comment: {body}"
    );
    assert!(body["context"]["start"].as_u64().unwrap() < body["start"].as_u64().unwrap());
    assert!(body["lines"]
        .as_array()
        .unwrap()
        .iter()
        .any(|line| line["text"].as_str().unwrap().contains("value: i32")
            && line["in_member"] == false));
    assert!(body["lines"]
        .as_array()
        .unwrap()
        .iter()
        .any(|line| line["in_member"] == true));
    let resume = context["member_view"]["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["kind"] == "resume-selection")
        .unwrap();
    assert_eq!(follow(&p, resume["command"].as_str().unwrap()), context);
}

#[test]
fn dashboard_accounts_for_folded_and_other_surface_families_before_row_limits() {
    let p = Project::new();
    for file in ["a.rs", "b.rs", "c.rs"] {
        p.write(
            file,
            &format!("fn compute() {{\n{RUST_BODY}}}\nfn again() {{\n{RUST_BODY}}}\n"),
        );
    }
    for file in ["generated-a.rs", "generated-b.rs"] {
        p.write(file, "// Code generated by population fixture. DO NOT EDIT.\nfn calculated(value: i32) -> i32 {\n let shifted = value + 200;\n shifted * shifted - 30\n}\n");
    }
    let dashboard = p.query(&[]);
    let population = &dashboard["population"];
    assert_eq!(population["families"], 3);
    assert_eq!(
        population["default_unfolded"],
        dashboard["summary"]["families"]
    );
    assert_eq!(population["default_unfolded"], 1);
    assert_eq!(population["default_folded"], 1);
    assert_eq!(population["other_surfaces"], 1);
    assert_eq!(population["all_unfolded"], 2);
    assert_eq!(population["all_folded"], 1);
    let all = p.query(&["all", "top=0"]);
    assert_eq!(all["summary"]["families"], population["all_unfolded"]);
    let limited = p.query(&["all", "top=1"]);
    assert_eq!(limited["summary"]["shown"], 1);
    assert_eq!(limited["summary"]["families"], population["all_unfolded"]);
    let output = p.query(&["--save-analysis", "capture.json"]);
    assert_eq!(output["families"], population["families"]);
    let empty = Project::new().query(&[]);
    assert_eq!(empty["population"]["families"], 0);
    assert_eq!(empty["population"]["default_folded"], 0);
    assert_eq!(empty["population"]["other_surfaces"], 0);
    assert_eq!(empty["population"]["all_unfolded"], 0);
}
