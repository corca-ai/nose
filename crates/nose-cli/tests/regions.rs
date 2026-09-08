//! Public source identity and explicit snapshot interoperability.
#[path = "support/analysis.rs"]
mod analysis;
use analysis::assert_same_analysis;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Project(PathBuf);
impl Project {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "nose-regions-{}-{}",
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
    fn run(&self, args: &[&str]) -> Vec<u8> {
        let output = Command::new(env!("CARGO_BIN_EXE_nose"))
            .current_dir(&self.0)
            .env("RAYON_NUM_THREADS", "2")
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        output.stdout
    }
    fn json(&self, args: &[&str]) -> Value {
        serde_json::from_slice(&self.run(args)).unwrap()
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const SOURCE: &str =
    "def compute(x):\n    a = x * x\n    b = a + 7\n    c = b // 3\n    return c\n";

fn query(project: &Project, extra: &[&str]) -> Value {
    let mut args = vec![
        "query",
        ".",
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
    ];
    args.extend_from_slice(extra);
    project.json(&args)
}

fn family(report: &Value) -> &Value {
    report["families"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| {
            f["locations"]
                .as_array()
                .unwrap()
                .iter()
                .all(|l| l["name"] == "compute")
        })
        .expect("compute family")
}

#[test]
fn query_keys_bind_analyzed_bytes_across_clean_cold_warm_and_leaf_edit() {
    let project = Project::new();
    project.write("a.py", SOURCE);
    project.write("b.py", SOURCE);
    let clean = query(&project, &[]);
    assert_eq!(clean["schema_version"], 10);
    let before = family(&clean);
    assert_eq!(before["review_key"].as_str().unwrap().len(), 64);
    for location in before["locations"].as_array().unwrap() {
        let source = std::fs::read(project.0.join(location["file"].as_str().unwrap())).unwrap();
        let region = &location["region"];
        let start = region["start_byte"].as_u64().unwrap() as usize;
        let end = region["end_byte"].as_u64().unwrap() as usize;
        assert_eq!(
            region["content_digest"],
            nose_il::ContentDigest::sha256(&source[start..end]).hex()
        );
    }
    for _ in 0..2 {
        assert_same_analysis(&clean, &query(&project, &["--cache-dir", "cache"]));
    }
    project.write("a.py", &format!("# inserted α comment\r\n\n{SOURCE}"));
    let after = query(&project, &["--cache-dir", "cache"]);
    assert_same_analysis(&after, &query(&project, &[]));
    assert_eq!(family(&after)["review_key"], before["review_key"]);
    assert_ne!(family(&after)["id"], before["id"]);
    std::fs::rename(project.0.join("a.py"), project.0.join("moved.py")).unwrap();
    assert_eq!(
        family(&query(&project, &["--cache-dir", "cache"]))["review_key"],
        before["review_key"]
    );
    project.write("copy.py", SOURCE);
    assert_ne!(
        family(&query(&project, &[]))["review_key"],
        before["review_key"]
    );
}

#[test]
fn portable_snapshots_include_singletons_and_compare_without_workspace_sources() {
    let project = Project::new();
    project.write("single.py", SOURCE);
    let before = project.run(&["regions", "snapshot", "."]);
    let snapshot: Value = serde_json::from_slice(&before).unwrap();
    assert!(!snapshot["regions"].as_array().unwrap().is_empty());
    assert!(snapshot["regions"]
        .as_array()
        .unwrap()
        .iter()
        .all(|r| !Path::new(r["file"].as_str().unwrap()).is_absolute()));
    project.write("single.py", &format!("# shifted\n{SOURCE}"));
    let after = project.run(&["regions", "snapshot", "."]);
    std::fs::write(project.0.join("before.json"), before).unwrap();
    std::fs::write(project.0.join("after.json"), after).unwrap();
    std::fs::remove_file(project.0.join("single.py")).unwrap();
    let compared = project.json(&["regions", "compare", "before.json", "after.json"]);
    assert_eq!(compared["complete"], true);
    assert!(compared["correspondences"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["kind"] == "content-match" && r["unchanged_evidence"] == true));
    let capped = project.json(&[
        "regions",
        "compare",
        "before.json",
        "after.json",
        "--max-candidates",
        "0",
    ]);
    assert_eq!(capped["complete"], false);
    assert!(capped["correspondences"]
        .as_array()
        .unwrap()
        .iter()
        .all(|r| r["unchanged_evidence"] == false));
}

#[test]
fn syntax_regions_preserve_original_bytes_and_embedded_offsets() {
    let project = Project::new();
    let source = String::from("<template><div>α</div></template>\r\n<script>\nfunction compute(x) {\n const a = x * x;\n const b = a + 7;\n const c = b / 3;\n return c;\n}\n</script>\n");
    project.write("a.vue", &source);
    project.write("b.vue", &source);
    let mut args = vec![
        "query",
        ".",
        "all",
        "top=0",
        "--mode",
        "syntax",
        "--min-size",
        "1",
        "--min-lines",
        "1",
        "--format",
        "json",
    ];
    let report = project.json(&args);
    args.extend_from_slice(&["--cache-dir", "cache"]);
    for _ in 0..2 {
        assert_same_analysis(&report, &project.json(&args));
    }
    let mut checked = 0;
    for family in report["families"].as_array().unwrap() {
        for location in family["locations"].as_array().unwrap() {
            let region = &location["region"];
            if region.is_null() {
                continue;
            }
            let start = region["start_byte"].as_u64().unwrap() as usize;
            let end = region["end_byte"].as_u64().unwrap() as usize;
            assert_eq!(
                region["source_digest"],
                nose_il::ContentDigest::sha256(source.as_bytes()).hex()
            );
            assert_eq!(
                region["content_digest"],
                nose_il::ContentDigest::sha256(&source.as_bytes()[start..end]).hex()
            );
            checked += 1;
        }
    }
    assert!(checked > 0);
}

fn mode_query(project: &Project, mode: &str, cache: bool) -> Value {
    let mut args = vec![
        "query",
        ".",
        "all",
        "top=0",
        "--mode",
        mode,
        "--min-size",
        "8",
        "--min-lines",
        "3",
        "--format",
        "json",
    ];
    if cache {
        args.extend(["--cache-dir", "cache"]);
    }
    project.json(&args)
}

fn review_keys(report: &Value) -> Vec<String> {
    let mut keys: Vec<_> = report["families"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            f["review_key"]
                .as_str()
                .expect("normal detection has review identity")
                .to_owned()
        })
        .collect();
    assert!(!keys.is_empty());
    keys.sort();
    keys
}

#[test]
fn abstraction_review_survives_moves_and_representative_reversal() {
    let project = Project::new();
    let source = "def sum_values(xs):\n    total = 0\n    for x in xs:\n        total = total + x\n    return total\n";
    project.write("a.py", source);
    project.write("b.py", &source.replace("= 0", "= 0.0"));
    let before = mode_query(&project, "abstraction", false);
    let keys = review_keys(&before);
    for _ in 0..2 {
        assert_same_analysis(&before, &mode_query(&project, "abstraction", true));
    }
    project.write("a.py", &format!("# shifted α\r\n{source}"));
    std::fs::rename(project.0.join("a.py"), project.0.join("z.py")).unwrap();
    let moved = mode_query(&project, "abstraction", false);
    assert_eq!(review_keys(&moved), keys);
    assert_same_analysis(&moved, &mode_query(&project, "abstraction", true));
    project.write("z.py", &source.replace("= 0", "= 1"));
    assert_ne!(
        review_keys(&mode_query(&project, "abstraction", false)),
        keys
    );
}

#[test]
fn bounded_windows_keep_original_bytes_on_cached_movement_and_reject_edits() {
    let project = Project::new();
    let source = r#"int set_option(const char *name, const char *value) {
  if (!strcmp(name, "progress")) {
    if (!strcmp(value, "true")) options.progress = 1;
    else if (!strcmp(value, "false")) options.progress = 0;
    else return -1;
    return 0;
  }
  if (!strcmp(name, "deepen-relative")) {
    if (!strcmp(value, "true")) options.deepen_relative = 1;
    else if (!strcmp(value, "false")) options.deepen_relative = 0;
    else return -1;
    return 0;
  }
  return 1;
}
"#;
    project.write("options.c", source);
    let before = mode_query(&project, "near", false);
    assert!(before["families"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["witness"] == "bounded-window"));
    let keys = review_keys(&before);
    for _ in 0..2 {
        assert_same_analysis(&before, &mode_query(&project, "near", true));
    }
    let shifted = format!("// α header\r\n{source}");
    project.write("options.c", &shifted);
    let moved = mode_query(&project, "near", true);
    assert_same_analysis(&moved, &mode_query(&project, "near", false));
    assert_eq!(review_keys(&moved), keys);
    for family in moved["families"].as_array().unwrap() {
        for loc in family["locations"].as_array().unwrap() {
            let region = &loc["region"];
            let start = region["start_byte"].as_u64().unwrap() as usize;
            let end = region["end_byte"].as_u64().unwrap() as usize;
            assert_eq!(
                region["content_digest"],
                nose_il::ContentDigest::sha256(&shifted.as_bytes()[start..end]).hex()
            );
        }
    }
    std::fs::rename(project.0.join("options.c"), project.0.join("moved.c")).unwrap();
    assert_eq!(review_keys(&mode_query(&project, "near", true)), keys);
    project.write("moved.c", &source.replace("return -1", "return -2"));
    assert_ne!(review_keys(&mode_query(&project, "near", false)), keys);
}

#[test]
fn syntax_module_containers_do_not_bind_unmatched_file_headers() {
    let project = Project::new();
    project.write("a.py", SOURCE);
    project.write("b.py", SOURCE);
    let before = mode_query(&project, "syntax", false);
    let keys = review_keys(&before);
    assert_same_analysis(&before, &mode_query(&project, "syntax", true));
    project.write("a.py", &format!("# unrelated α header\r\n{SOURCE}"));
    let shifted = mode_query(&project, "syntax", false);
    assert_eq!(review_keys(&shifted), keys);
    for _ in 0..2 {
        assert_same_analysis(&shifted, &mode_query(&project, "syntax", true));
    }
    std::fs::rename(project.0.join("a.py"), project.0.join("moved.py")).unwrap();
    assert_eq!(review_keys(&mode_query(&project, "syntax", true)), keys);
}
