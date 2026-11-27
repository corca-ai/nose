use super::*;

fn command(p: &Project, name: &str, cached: bool, extra: &[&str]) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_nose"));
    cmd.current_dir(&p.0)
        .env("RAYON_NUM_THREADS", "2")
        .env("NOSE_CACHE_STATS", "1")
        .args([
            "query",
            ".",
            "--mode",
            "syntax,semantic,near",
            "--min-size",
            "1",
            "--min-lines",
            "1",
            "--format",
            "json",
            "--save-analysis",
            name,
        ]);
    if cached {
        cmd.args(["--cache-dir", ".cache"]);
    }
    cmd.args(extra);
    cmd
}

fn capture(p: &Project, name: &str, cached: bool, extra: &[&str]) -> (Vec<u8>, Output) {
    let out = command(p, name, cached, extra).output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    (std::fs::read(p.0.join(name)).unwrap(), out)
}

fn hit(out: &Output, expected: bool) {
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains(&format!("[capture-cache] hit={expected}")),
        "{stderr}"
    );
}

fn artifacts(root: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(root)
        .unwrap()
        .flat_map(|entry| {
            let path = entry.unwrap().path();
            if path.is_dir() {
                artifacts(&path)
            } else {
                vec![path]
            }
        })
        .collect()
}

#[test]
fn unchanged_capture_reuses_complete_bytes_and_preserves_new_file_requirement() {
    let p = Project::new();
    let (cold, out) = capture(&p, "cold.json", true, &[]);
    hit(&out, false);
    let (warm, out) = capture(&p, "warm.json", true, &[]);
    hit(&out, true);
    assert_eq!(cold, warm);
    assert_eq!(warm, capture(&p, "clean.json", false, &[]).0);
    let out = command(&p, "warm.json", true, &[]).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("already exists"));
    assert_eq!(warm, std::fs::read(p.0.join("warm.json")).unwrap());
}

#[test]
fn edits_additions_deletions_and_moves_recalculate_complete_captures() {
    let p = Project::new();
    capture(&p, "initial.json", true, &[]);
    for (step, action) in ["edit", "add", "delete", "move"].into_iter().enumerate() {
        match action {
            "edit" => p.write("a.py", &SOURCE.replace("+ 7", "+ 23")),
            "add" => p.write("c.py", SOURCE),
            "delete" => std::fs::remove_file(p.0.join("b.py")).unwrap(),
            "move" => std::fs::rename(p.0.join("c.py"), p.0.join("moved.py")).unwrap(),
            _ => unreachable!(),
        }
        let (changed, out) = capture(&p, &format!("changed-{step}.json"), true, &[]);
        hit(&out, false);
        assert_eq!(
            changed,
            capture(&p, &format!("clean-{step}.json"), false, &[]).0
        );
        let (reused, out) = capture(&p, &format!("reused-{step}.json"), true, &[]);
        hit(&out, true);
        assert_eq!(changed, reused);
    }
}

#[test]
fn settings_scoring_and_candidate_budget_are_checked_before_reuse() {
    let p = Project::new();
    capture(&p, "initial.json", true, &[]);
    p.write("nose.toml", "[query]\nmin-value = 1000000\n");
    let (filtered, out) = capture(&p, "filtered.json", true, &[]);
    hit(&out, false);
    let snapshot: Value = serde_json::from_slice(&filtered).unwrap();
    assert!(snapshot["families"].as_array().unwrap().is_empty());
    assert_eq!(filtered, capture(&p, "clean-filtered.json", false, &[]).0);
    let out = command(&p, "scoring.json", true, &[])
        .env("NOSE_CAND_VJ", "0.95")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    hit(&out, false);
    let out = command(&p, "scoring-again.json", true, &[])
        .env("NOSE_CAND_VJ", "0.95")
        .output()
        .unwrap();
    assert!(out.status.success());
    hit(&out, true);
    std::fs::remove_file(p.0.join("nose.toml")).unwrap();
    p.write("c.py", SOURCE);
    capture(&p, "more.json", true, &[]);
    let out = command(&p, "capped.json", true, &["--max-candidate-pairs", "1"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(!p.0.join("capped.json").exists());
}

#[test]
fn corrupt_capture_artifacts_are_rebuilt_and_reused() {
    let p = Project::new();
    let (cold, _) = capture(&p, "cold.json", true, &[]);
    let entries = artifacts(&p.0.join(".cache/cas-v2/analysis-capture"));
    assert_eq!(entries.len(), 1);
    std::fs::write(&entries[0], b"corrupt artifact").unwrap();
    let (rebuilt, out) = capture(&p, "rebuilt.json", true, &[]);
    hit(&out, false);
    assert_eq!(cold, rebuilt);
    let (warm, out) = capture(&p, "warm.json", true, &[]);
    hit(&out, true);
    assert_eq!(cold, warm);
}

#[test]
fn suppression_and_exclusion_policies_invalidate_capture_results() {
    let p = Project::new();
    p.write("a.py", &format!("# nose-ignore\n{SOURCE}"));
    capture(&p, "respect.json", true, &[]);
    let (included, out) = capture(&p, "include.json", true, &["--include-inline-suppressed"]);
    hit(&out, false);
    assert_eq!(
        included,
        capture(
            &p,
            "clean-include.json",
            false,
            &["--include-inline-suppressed"]
        )
        .0
    );
    let (excluded, out) = capture(&p, "exclude.json", true, &["--exclude", "a.py"]);
    hit(&out, false);
    assert_eq!(
        excluded,
        capture(&p, "clean-exclude.json", false, &["--exclude", "a.py"]).0
    );
}

#[test]
fn c_includes_outside_the_inventory_keep_the_existing_analysis_path() {
    let p = Project::new();
    p.write("types.inc", "typedef unsigned char byte_t;\n");
    p.write("pack.c", "#include \"types.inc\"\nunsigned short load(const byte_t *p) {\n    return p[0] | (p[1] << 8);\n}\n");
    let (cold, out) = capture(&p, "cold.json", true, &[]);
    hit(&out, false);
    let (warm, out) = capture(&p, "warm.json", true, &[]);
    hit(&out, false);
    assert_eq!(cold, warm);
    assert_eq!(warm, capture(&p, "clean.json", false, &[]).0);
}

#[cfg(unix)]
#[test]
fn unreadable_sources_do_not_reuse_a_previous_capture() {
    use std::os::unix::fs::PermissionsExt;
    let p = Project::new();
    capture(&p, "initial.json", true, &[]);
    let path = p.0.join("a.py");
    let original = std::fs::metadata(&path).unwrap().permissions();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o0)).unwrap();
    if std::fs::read(&path).is_ok() {
        std::fs::set_permissions(&path, original).unwrap();
        return;
    }
    let out = command(&p, "unreadable.json", true, &[]).output().unwrap();
    std::fs::set_permissions(&path, original).unwrap();
    assert!(!out.status.success());
    hit(&out, false);
    assert!(!p.0.join("unreadable.json").exists());
}
