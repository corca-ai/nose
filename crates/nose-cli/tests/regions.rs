//! Public source identity and explicit snapshot interoperability.
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
        assert_eq!(clean, query(&project, &["--cache-dir", "cache"]));
    }
    project.write("a.py", &format!("# inserted α comment\r\n\n{SOURCE}"));
    let after = query(&project, &["--cache-dir", "cache"]);
    assert_eq!(after, query(&project, &[]));
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
        assert_eq!(report, project.json(&args));
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
