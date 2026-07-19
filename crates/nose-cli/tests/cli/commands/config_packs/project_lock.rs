use super::*;

fn prepare_locked_project(tag: &str) -> (PathBuf, PathBuf, PathBuf, PathBuf) {
    let dir = make_project(tag);
    let packs = dir.join("packs");
    fs::create_dir_all(&packs).unwrap();
    let manifest = packs.join("guava.json");
    fs::write(&manifest, semantic_pack_manifest_v1()).unwrap();
    let dependency = dir.join("pom.xml");
    fs::write(
        &dependency,
        "<project><dependency>com.google.guava:guava:33.0.0</dependency></project>\n",
    )
    .unwrap();
    let lock = dir.join("nose.semantic-pack-lock.json");
    (dir, manifest, dependency, lock)
}

fn create_lock(dir: &Path, format: &str) -> std::process::Output {
    Command::new(bin())
        .args([
            "semantic-pack",
            "lock",
            "packs/guava.json",
            "--dependency",
            "pom.xml",
            "--output",
            "nose.semantic-pack-lock.json",
            "--format",
            format,
        ])
        .current_dir(dir)
        .output()
        .expect("create semantic-pack project lock")
}

#[test]
fn lock_and_status_commands_report_the_same_valid_local_decision() {
    let (dir, _, _, lock) = prepare_locked_project("semantic_pack_lock_commands");
    let created = create_lock(&dir, "json");
    assert!(
        created.status.success(),
        "lock creation failed: {}",
        String::from_utf8_lossy(&created.stderr)
    );
    assert!(lock.is_file());
    let created_json: serde_json::Value = serde_json::from_slice(&created.stdout).unwrap();
    assert_eq!(created_json["schema_version"], 1);
    assert_eq!(created_json["status"], "ok");
    assert_eq!(
        created_json["lock_api_version"],
        "nose.semantic-pack-lock.v1"
    );
    assert_eq!(created_json["influence"], "metadata-only");
    assert_eq!(created_json["totals"]["packs"], 1);
    assert_eq!(created_json["totals"]["selected_rows"], 2);
    assert_eq!(created_json["totals"]["conflicts"], 0);
    assert_eq!(created_json["dependencies"][0]["path"], "pom.xml");

    let status = Command::new(bin())
        .args([
            "semantic-pack",
            "status",
            "nose.semantic-pack-lock.json",
            "--format",
            "json",
        ])
        .current_dir(&dir)
        .output()
        .expect("validate semantic-pack project lock");
    assert!(status.status.success());
    let status_json: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(
        status_json["decision_digest"],
        created_json["decision_digest"]
    );
}

#[test]
fn locked_query_reports_authorization_but_keeps_families_unchanged() {
    let (dir, _, _, _) = prepare_locked_project("semantic_pack_locked_query");
    assert!(create_lock(&dir, "human").status.success());
    let without = Command::new(bin())
        .args(["query", ".", "all", "top=0", "--format", "json"])
        .current_dir(&dir)
        .output()
        .unwrap();
    let with = Command::new(bin())
        .args([
            "query",
            ".",
            "all",
            "top=0",
            "--format",
            "json",
            "--semantic-pack-lock",
            "nose.semantic-pack-lock.json",
        ])
        .current_dir(&dir)
        .output()
        .unwrap();
    assert!(
        with.status.success(),
        "{}",
        String::from_utf8_lossy(&with.stderr)
    );
    let without: serde_json::Value = serde_json::from_slice(&without.stdout).unwrap();
    let with: serde_json::Value = serde_json::from_slice(&with.stdout).unwrap();
    assert_eq!(with["families"], without["families"]);
    let pack = semantic_pack_by_id(&with, "com.example.java-guava-typed-factories");
    assert_eq!(pack["influence"], "metadata-only");
    assert_eq!(pack["lock"]["status"], "valid");
    assert_eq!(pack["lock"]["api_version"], "nose.semantic-pack-lock.v1");
    assert_eq!(
        pack["lock"]["allowed_channels"],
        serde_json::json!(["near"])
    );
    assert_eq!(pack["lock"]["selected_rows"].as_array().unwrap().len(), 2);
    assert_eq!(pack["lock"]["dependencies"][0]["path"], "pom.xml");
}

#[test]
fn config_relative_lock_is_validated_before_analysis() {
    let (dir, _, dependency, _) = prepare_locked_project("semantic_pack_lock_config");
    assert!(create_lock(&dir, "human").status.success());
    fs::write(
        dir.join("nose.toml"),
        "[query]\nsemantic-pack-lock = \"nose.semantic-pack-lock.json\"\n",
    )
    .unwrap();
    let valid = Command::new(bin())
        .args(["query", ".", "--format", "json"])
        .current_dir(&dir)
        .output()
        .unwrap();
    assert!(
        valid.status.success(),
        "{}",
        String::from_utf8_lossy(&valid.stderr)
    );

    fs::write(&dependency, "dependency changed after lock creation\n").unwrap();
    let stale = Command::new(bin())
        .args(["query", ".", "--format", "json"])
        .current_dir(&dir)
        .output()
        .unwrap();
    assert!(!stale.status.success());
    let stderr = String::from_utf8_lossy(&stale.stderr);
    assert!(stderr.contains("locked file `pom.xml` changed"), "{stderr}");
    assert!(
        stale.stdout.is_empty(),
        "analysis output must not start for a stale lock"
    );
}

#[test]
fn missing_or_mixed_lock_inputs_fail_closed() {
    let (dir, _, _, _) = prepare_locked_project("semantic_pack_lock_failures");
    let missing = Command::new(bin())
        .args([
            "query",
            ".",
            "--format",
            "json",
            "--semantic-pack-lock",
            "missing.lock.json",
        ])
        .current_dir(&dir)
        .output()
        .unwrap();
    assert!(!missing.status.success());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("reading semantic-pack lock"));

    assert!(create_lock(&dir, "human").status.success());
    let mixed = Command::new(bin())
        .args([
            "query",
            ".",
            "--semantic-pack",
            "packs/guava.json",
            "--semantic-pack-lock",
            "nose.semantic-pack-lock.json",
        ])
        .current_dir(&dir)
        .output()
        .unwrap();
    assert!(!mixed.status.success());
    assert!(
        String::from_utf8_lossy(&mixed.stderr).contains("mutually exclusive"),
        "{}",
        String::from_utf8_lossy(&mixed.stderr)
    );
}
