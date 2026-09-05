use serde_json::Value;
use std::{
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

struct Project(PathBuf);
impl Project {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "nose exploration {} {}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn write(&self, path: &str, source: &str) {
        let path = self.0.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, source).unwrap();
    }
    fn query(&self, terms: &[&str]) -> Value {
        let out = Command::new(env!("CARGO_BIN_EXE_nose"))
            .current_dir(&self.0)
            .env("RAYON_NUM_THREADS", "2")
            .args([
                "query",
                ".",
                "--min-size",
                "1",
                "--min-lines",
                "1",
                "--format",
                "json",
            ])
            .args(terms)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout).unwrap()
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const RUST_BODY: &str = r#"    let values = vec![12, 24, 36, 48];
    let mut total = 0;
    for value in values {
        total += value * 3;
        if total > 100 { total -= 7; }
    }
    assert_eq!(total, 353);
"#;

#[test]
fn syntax_regions_in_attributed_rust_tests_stay_test_scoped_on_cache_hits() {
    let p = Project::new();
    p.write("src/checks.rs", &format!("#[test]\nfn checks_alpha() {{\n{RUST_BODY}}}\n#[test]\nfn checks_beta() {{\n{RUST_BODY}}}\n"));
    let args = ["--mode", "syntax", "all", "top=0", "--cache-dir", "cache"];
    let cold = p.query(&args);
    let families = cold["families"].as_array().unwrap();
    assert!(!families.is_empty());
    assert!(families.iter().all(|f| f["scope"] == "test"), "{cold}");
    let warm = p.query(&args);
    assert_eq!(cold["families"], warm["families"]);
    let product = p.query(&["--mode", "syntax", "scope=prod", "all", "top=0"]);
    assert_eq!(product["summary"]["families"], 0);
}

#[test]
fn display_sorts_preserve_family_ids_and_fold_relationships() {
    let p = Project::new();
    for file in ["a.rs", "b.rs", "c.rs"] {
        p.write(
            file,
            &format!("fn compute() {{\n{RUST_BODY}}}\nfn again() {{\n{RUST_BODY}}}\n"),
        );
    }
    let signature = |report: &Value| {
        let mut rows: Vec<_> = report["families"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| (f["id"].to_string(), f["folds"].to_string()))
            .collect();
        rows.sort();
        rows
    };
    let first = signature(&p.query(&["top=0"]));
    assert!(!first.is_empty());
    for sort in [
        "sort=value",
        "sort=members",
        "sort=hazard",
        "sort=extractability",
    ] {
        assert_eq!(first, signature(&p.query(&["top=0", sort])), "{sort}");
    }
}

#[test]
fn dashboard_offers_executable_scope_and_evaluation_routes_in_json() {
    let p = Project::new();
    for file in ["src/a.rs", "src/b.rs", "bench/a.rs", "bench/b.rs"] {
        p.write(file, &format!("fn compute() {{\n{RUST_BODY}}}\n"));
    }
    let dashboard = p.query(&[]);
    let total: u64 = dashboard["summary"]["by_confidence"]
        .as_object()
        .unwrap()
        .values()
        .map(|n| n.as_u64().unwrap())
        .sum();
    assert_eq!(dashboard["summary"]["families"], total);
    let commands = dashboard["next"].as_array().unwrap();
    assert!(
        commands
            .iter()
            .any(|c| c.as_str().unwrap().contains("path~bench/")),
        "{dashboard}"
    );
    let command = commands
        .iter()
        .find(|c| c.as_str().unwrap().contains("scope=prod"))
        .unwrap()
        .as_str()
        .unwrap();
    assert!(command.contains("path!~bench/") && command.contains("--format json"));
    let binary = PathBuf::from(env!("CARGO_BIN_EXE_nose"));
    let path = std::env::join_paths(
        std::iter::once(binary.parent().unwrap().to_path_buf()).chain(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        )),
    )
    .unwrap();
    let out = Command::new("sh")
        .current_dir(&p.0)
        .env("PATH", path)
        .args(["-c", command])
        .output()
        .unwrap();
    assert!(out.status.success());
    let result: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(result["view"], "list");
    assert!(result["families"]
        .as_array()
        .unwrap()
        .iter()
        .all(|f| f["locations"]
            .as_array()
            .unwrap()
            .iter()
            .all(|l| !l["file"].as_str().unwrap().contains("bench/"))));
}

#[test]
fn full_family_lists_every_copy_and_compact_view_names_omissions() {
    let p = Project::new();
    for n in 0..32 {
        p.write(
            &format!("copy_{n}.rs"),
            &format!("fn compute() {{\n{RUST_BODY}}}\n"),
        );
    }
    let report = p.query(&["--mode", "syntax", "all", "top=0"]);
    let family = report["families"]
        .as_array()
        .unwrap()
        .iter()
        .max_by_key(|f| f["members"].as_u64())
        .unwrap();
    assert!(family["locations"].as_array().unwrap().len() > 30);
    let id = format!("id={}", family["id"].as_str().unwrap());
    for full in [false, true] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_nose"));
        command.current_dir(&p.0).args([
            "query",
            ".",
            "--mode",
            "syntax",
            "--min-size",
            "1",
            "--min-lines",
            "1",
            &id,
        ]);
        if full {
            command.arg("full");
        }
        let out = command.output().unwrap();
        assert!(out.status.success());
        let text = String::from_utf8(out.stdout).unwrap();
        if full {
            for loc in family["locations"].as_array().unwrap() {
                let address = format!(
                    "{}:{}-{}",
                    loc["file"].as_str().unwrap(),
                    loc["start"],
                    loc["end"]
                );
                assert!(text.contains(&address), "missing {address}");
            }
            assert!(!text.contains("more copies; add"));
        } else {
            assert!(text.contains("more copies; add `full`"));
        }
    }
}

#[test]
fn filtered_json_navigation_retains_selection_format_and_shell_quoting() {
    let p = Project::new();
    for file in ["src space/a.rs", "src space/b.rs", "bench/a.rs"] {
        p.write(file, &format!("fn compute() {{\n{RUST_BODY}}}\n"));
    }
    let report = p.query(&["all", "path~src space", "path!~bench/", "top=0"]);
    let command = report["next"][0].as_str().unwrap();
    assert!(
        command.contains("'path~src space'")
            && command.contains("'path!~bench/'")
            && command.contains("--format json")
    );
    let binary = PathBuf::from(env!("CARGO_BIN_EXE_nose"));
    let path = std::env::join_paths(
        std::iter::once(binary.parent().unwrap().to_path_buf()).chain(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        )),
    )
    .unwrap();
    let out = Command::new("sh")
        .current_dir(&p.0)
        .env("PATH", path)
        .args(["-c", command])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let grouped: Value = serde_json::from_slice(&out.stdout).unwrap();
    let count: u64 = grouped["groups"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| g["count"].as_u64().unwrap())
        .sum();
    assert_eq!(grouped["view"], "group");
    assert_eq!(report["summary"]["families"], count);
}

#[test]
fn every_advertised_evidence_kind_and_alias_is_a_valid_filter() {
    let p = Project::new();
    for file in ["a.rs", "b.rs"] {
        p.write(file, &format!("fn compute() {{\n{RUST_BODY}}}\n"));
    }
    for (alias, canonical) in [
        ("exact", "exact-value-graph"),
        ("shared-core", "shared-sub-dag"),
        ("connected", "connected-mapped-sub-dag"),
        ("bounded-window", "bounded-same-unit-window"),
        ("copy-paste", "copy-paste-run"),
        ("similar", "structural-similarity"),
    ] {
        let friendly = p.query(&[&format!("witness={alias}"), "top=0"]);
        let stable = p.query(&[&format!("witness={canonical}"), "top=0"]);
        assert_eq!(friendly["families"], stable["families"], "{alias}");
    }
}

#[test]
fn member_facets_keep_family_identity_and_scope_evidence() {
    let p = Project::new();
    for file in ["src/one.py", "src/sub/two.py", "tests/three.py"] {
        p.write(
            file,
            "def compute(x):\n    a = x * x\n    b = a + 7\n    c = b // 3\n    return c\n",
        );
    }
    let view = p.query(&["--mode", "semantic", "all", "top=0"]);
    let family = view["families"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["members"] == 3)
        .unwrap();
    let id = format!("id={}", family["id"].as_str().unwrap());
    let grouped = p.query(&["--mode", "semantic", &id, "member-group=dir"]);
    assert_eq!(grouped["member_view"]["groups_total"], 3);
    assert_eq!(grouped["family"]["id"], family["id"]);
    let narrowed = p.query(&["--mode", "semantic", &id, "member-dir=src"]);
    assert_eq!(narrowed["member_view"]["selected"], 1, "{narrowed}");
    assert_eq!(narrowed["family"]["members"], 3);
    assert_eq!(narrowed["family"]["locations"].as_array().unwrap().len(), 1);
    let tests = p.query(&["--mode", "semantic", &id, "member-scope=test"]);
    assert_eq!(tests["member_view"]["selected"], 1);
    assert_eq!(
        tests["family"]["locations"][0]["scope_evidence"]["reasons"][0],
        "test-path-convention"
    );
    assert_eq!(
        tests["family"]["assessment"]["verdict"],
        "caller-review-required"
    );
}
