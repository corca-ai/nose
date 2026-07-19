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
        "<project><modelVersion>4.0.0</modelVersion><dependencies><dependency>\
         <groupId>com.google.guava</groupId><artifactId>guava</artifactId>\
         <version>33.0.0-jre</version></dependency></dependencies></project>\n",
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
    assert_eq!(created_json["influence"], "near-only");
    assert_eq!(created_json["totals"]["packs"], 1);
    assert_eq!(created_json["totals"]["selected_rows"], 3);
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
fn locked_query_reports_near_authorization_without_evidence_occurrences() {
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
    assert_eq!(pack["influence"], "near-only");
    assert_eq!(pack["near_influence"]["admitted_occurrences"], 0);
    assert_eq!(pack["near_influence"]["influential_occurrences"], 0);
    assert_eq!(pack["lock"]["status"], "valid");
    assert_eq!(pack["lock"]["api_version"], "nose.semantic-pack-lock.v1");
    assert_eq!(
        pack["lock"]["allowed_channels"],
        serde_json::json!(["near"])
    );
    assert_eq!(pack["lock"]["selected_rows"].as_array().unwrap().len(), 3);
    assert_eq!(pack["lock"]["dependencies"][0]["path"], "pom.xml");
}

#[test]
fn locked_near_row_changes_only_the_supported_near_family_with_provenance() {
    let (dir, _, _, _) = prepare_locked_project("semantic_pack_near_influence");
    for child in ["a", "b", "c", "tests"] {
        let _ = fs::remove_dir_all(dir.join(child));
    }
    write_files(
        &dir,
        &[
            (
                "External.java",
                "import com.google.common.collect.ImmutableList;\n\
                 class External {\n\
                   Object collect(Object first, Object second) {\n\
                     Object values = ImmutableList.of(first, second);\n\
                     int size = first.hashCode() + second.hashCode();\n\
                     if (size > 0) { return values; }\n\
                     return ImmutableList.of(second, first);\n\
                   }\n\
                 }\n",
            ),
            (
                "Builtin.java",
                "import java.util.List;\n\
                 class Builtin {\n\
                   Object gather(Object first, Object second) {\n\
                     Object values = List.of(first, second);\n\
                     int size = first.hashCode() + second.hashCode();\n\
                     if (size > 0) { return values; }\n\
                     return List.of(first);\n\
                   }\n\
                 }\n",
            ),
        ],
    );
    assert!(create_lock(&dir, "human").status.success());
    let query = |locked: bool, mode: &str, cache: bool| {
        let mut command = Command::new(bin());
        command.args([
            "query",
            ".",
            "all",
            "top=0",
            "--mode",
            mode,
            "--min-lines",
            "3",
            "--min-size",
            "12",
            "--format",
            "json",
        ]);
        if locked {
            command.args(["--semantic-pack-lock", "nose.semantic-pack-lock.json"]);
        }
        if cache {
            command.args(["--cache-dir", ".nose-cache"]);
        }
        let output = command.current_dir(&dir).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap()
    };

    let without = query(false, "near", false);
    let with = query(true, "near", false);
    let influenced = with["families"]
        .as_array()
        .unwrap()
        .iter()
        .find(|family| family.get("semantic_pack_near").is_some())
        .unwrap_or_else(|| panic!("locked query should expose influenced family: {with}"));
    assert!(
        without["families"]
            .as_array()
            .unwrap()
            .iter()
            .all(|family| family["id"] != influenced["id"]),
        "removing the lock must remove the protocol-supported family"
    );
    let provenance = &influenced["semantic_pack_near"][0];
    assert_eq!(provenance["lane"], "near");
    assert_eq!(provenance["trust"], "external-opt-in");
    assert_eq!(provenance["dependency"]["matched_version"], "33.0.0");
    assert!(provenance["row_digest"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));
    assert!(influenced["locations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|location| location.get("semantic_pack_near").is_some()));
    let pack = semantic_pack_by_id(&with, "com.example.java-guava-typed-factories");
    assert_eq!(pack["near_influence"]["admitted_occurrences"], 2);
    assert_eq!(pack["near_influence"]["influential_occurrences"], 2);

    let repeated = query(true, "near", false);
    assert_eq!(with, repeated, "locked near output must be deterministic");
    let cached = query(true, "near", true);
    assert_eq!(with["families"], cached["families"]);
    let semantic_without = query(false, "semantic", false);
    let semantic_with = query(true, "semantic", false);
    assert_eq!(semantic_without["families"], semantic_with["families"]);
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
