use super::*;

fn exact_manifest() -> String {
    include_str!("../../../../../../docs/examples/semantic-packs/v1/vavr-list.json")
        .replace(
            "org.corca.reference.java-vavr-list",
            "com.example.fast-list",
        )
        .replace("io.vavr:vavr", "com.example:fast-list")
        .replace(">=0.9.0 <0.10.0", ">=1.2.0 <2.0.0")
        .replace("java.vavr.list.of-five-exact", "java.fast-list.of-five")
        .replace("io.vavr.collection", "com.example.collect")
        .replace("List", "FastList")
        .replace("vavr-list-hard-negatives", "wrong-member-negative")
        .replace("vavr-list-of-five-positive", "factory-positive")
        .replace("vavr-list-fixtures/hard-negatives", "fixtures/wrong-member")
        .replace("vavr-list-fixtures/positive", "fixtures/positive")
        .replace("vavr-list-pom.xml", "pom.xml")
}

fn prepare_exact_project(tag: &str) -> PathBuf {
    let dir = make_project(tag);
    for child in ["a", "b", "c", "tests"] {
        let _ = fs::remove_dir_all(dir.join(child));
    }
    fs::create_dir_all(dir.join("packs/fixtures/positive")).unwrap();
    fs::create_dir_all(dir.join("packs/fixtures/wrong-member")).unwrap();
    fs::write(dir.join("packs/fast-list.json"), exact_manifest()).unwrap();
    fs::write(
        dir.join("packs/pom.xml"),
        "<project><modelVersion>4.0.0</modelVersion><dependencies><dependency>\
         <groupId>com.example</groupId><artifactId>fast-list</artifactId>\
         <version>1.2.3</version></dependency></dependencies></project>\n",
    )
    .unwrap();
    fs::write(
        dir.join("packs/fixtures/positive/Fixture.java"),
        "import com.example.collect.FastList;\n\
         import java.util.List;\n\
         class Fixture {\n\
           static boolean externalFactory(int value) {\n\
             return FastList.of(1, 2, 3, 4, 5).contains(value);\n\
           }\n\
           static boolean builtinFactory(int value) {\n\
             return List.of(1, 2, 3, 4, 5).contains(value);\n\
           }\n\
         }\n",
    )
    .unwrap();
    fs::write(
        dir.join("packs/fixtures/wrong-member/Fixture.java"),
        "import com.example.collect.FastList;\n\
         import java.util.List;\n\
         class Fixture {\n\
           static boolean externalFactory(int value) {\n\
             return FastList.copyOf(1, 2, 3, 4, 5).contains(value);\n\
           }\n\
           static boolean builtinFactory(int value) {\n\
             return List.of(1, 2, 3, 4, 5).contains(value);\n\
           }\n\
         }\n",
    )
    .unwrap();
    dir
}

fn check_with_receipt(dir: &Path) -> std::process::Output {
    Command::new(bin())
        .args([
            "semantic-pack",
            "check",
            "packs/fast-list.json",
            "--receipt-out",
            "packs/receipt.json",
            "--format",
            "json",
        ])
        .current_dir(dir)
        .output()
        .unwrap()
}

fn create_exact_lock(dir: &Path, with_receipt: bool) -> std::process::Output {
    let mut command = Command::new(bin());
    command.args([
        "semantic-pack",
        "lock",
        "packs/fast-list.json",
        "--dependency",
        "packs/pom.xml",
        "--channel",
        "external-exact",
        "--output",
        "nose.semantic-pack-lock.json",
        "--format",
        "json",
    ]);
    if with_receipt {
        command.args(["--exact-receipt", "packs/receipt.json"]);
    }
    command.current_dir(dir).output().unwrap()
}

#[test]
fn kernel_check_receipt_and_exact_lock_open_only_external_claim_lane() {
    let dir = prepare_exact_project("semantic_pack_external_exact");
    let checked = check_with_receipt(&dir);
    assert!(
        checked.status.success(),
        "{}",
        String::from_utf8_lossy(&checked.stderr)
    );
    let check_json: serde_json::Value = serde_json::from_slice(&checked.stdout).unwrap();
    assert_eq!(check_json["schema_version"], 4);
    assert_eq!(check_json["kernel_conformance"]["status"], "ok");
    assert_eq!(check_json["totals"]["kernel_conformance_fixtures"], 2);
    assert_eq!(
        check_json["totals"]["passed_kernel_conformance_fixtures"],
        2
    );
    let receipt: serde_json::Value =
        serde_json::from_slice(&fs::read(dir.join("packs/receipt.json")).unwrap()).unwrap();
    assert_eq!(
        receipt["api_version"],
        "nose.semantic-pack-conformance-receipt.v1"
    );
    assert_eq!(receipt["passed"], true);

    let missing_receipt = create_exact_lock(&dir, false);
    assert!(!missing_receipt.status.success());
    assert!(String::from_utf8_lossy(&missing_receipt.stderr)
        .contains("has no exact conformance receipt"));

    let locked = create_exact_lock(&dir, true);
    assert!(
        locked.status.success(),
        "{}",
        String::from_utf8_lossy(&locked.stderr)
    );
    let lock_json: serde_json::Value = serde_json::from_slice(&locked.stdout).unwrap();
    assert_eq!(lock_json["influence"], "external-claim-exact");

    fs::create_dir_all(dir.join("application")).unwrap();
    fs::copy(
        dir.join("packs/fixtures/positive/Fixture.java"),
        dir.join("application/Fixture.java"),
    )
    .unwrap();
    let query = |locked: bool| {
        let mut command = Command::new(bin());
        command.args([
            "query",
            "application",
            "all",
            "top=0",
            "--mode",
            "semantic",
            "--min-lines",
            "1",
            "--min-size",
            "1",
            "--format",
            "json",
        ]);
        if locked {
            command.args(["--semantic-pack-lock", "nose.semantic-pack-lock.json"]);
        }
        let output = command.current_dir(&dir).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap()
    };
    let without = query(false);
    assert!(without["families"]
        .as_array()
        .unwrap()
        .iter()
        .all(|family| family.get("semantic_pack_external_exact").is_none()));
    let with = query(true);
    let family = with["families"]
        .as_array()
        .unwrap()
        .iter()
        .find(|family| family.get("semantic_pack_external_exact").is_some())
        .unwrap_or_else(|| panic!("exact lock should expose external-claim family: {with}"));
    assert_eq!(family["witness"], "exact");
    assert_eq!(
        family["semantic_pack_external_exact"][0]["assurance"],
        "external-claim-exact"
    );
    assert_eq!(
        family["semantic_pack_external_exact"][0]["lane"],
        "external-exact"
    );
    let pack = semantic_pack_by_id(&with, "com.example.fast-list");
    assert_eq!(pack["influence"], "external-claim-exact");
    assert_eq!(
        pack["external_exact_influence"]["influential_occurrences"],
        1
    );

    let keys = review_keys_for_pack(&with, "semantic_pack_external_exact");
    let original = fs::read_to_string(dir.join("application/Fixture.java")).unwrap();
    fs::write(
        dir.join("application/Fixture.java"),
        format!("// shifted α\r\n{original}"),
    )
    .unwrap();
    fs::rename(
        dir.join("application/Fixture.java"),
        dir.join("application/Moved.java"),
    )
    .unwrap();
    assert_eq!(
        review_keys_for_pack(&query(true), "semantic_pack_external_exact"),
        keys
    );
    fs::write(
        dir.join("application/Moved.java"),
        original.replace("return FastList", "return  FastList"),
    )
    .unwrap();
    assert_ne!(
        review_keys_for_pack(&query(true), "semantic_pack_external_exact"),
        keys
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn exact_receipt_content_and_fixture_drift_fail_closed() {
    let dir = prepare_exact_project("semantic_pack_external_exact_stale");
    assert!(check_with_receipt(&dir).status.success());
    assert!(create_exact_lock(&dir, true).status.success());

    let receipt_path = dir.join("packs/receipt.json");
    let receipt = fs::read(&receipt_path).unwrap();
    let mut tampered: serde_json::Value = serde_json::from_slice(&receipt).unwrap();
    tampered["kernel_capability"] = serde_json::json!("provider-defined");
    fs::write(&receipt_path, serde_json::to_vec_pretty(&tampered).unwrap()).unwrap();
    let tampered_status = Command::new(bin())
        .args(["semantic-pack", "status", "nose.semantic-pack-lock.json"])
        .current_dir(&dir)
        .output()
        .unwrap();
    assert!(!tampered_status.status.success());
    assert!(String::from_utf8_lossy(&tampered_status.stderr).contains("locked file"));
    fs::write(&receipt_path, receipt).unwrap();

    fs::write(
        dir.join("packs/fixtures/positive/Fixture.java"),
        "class Fixture { static Object changed() { return null; } }\n",
    )
    .unwrap();
    let stale = Command::new(bin())
        .args(["semantic-pack", "status", "nose.semantic-pack-lock.json"])
        .current_dir(&dir)
        .output()
        .unwrap();
    assert!(!stale.status.success());
    assert!(String::from_utf8_lossy(&stale.stderr).contains("content changed"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn source_conformance_resource_limit_fails_closed() {
    let dir = prepare_exact_project("semantic_pack_external_exact_resource_cap");
    fs::write(
        dir.join("packs/fixtures/positive/oversized.java"),
        vec![b' '; nose_semantics::MAX_SEMANTIC_PACK_FIXTURE_BYTES + 1],
    )
    .unwrap();
    let checked = check_with_receipt(&dir);
    assert!(!checked.status.success());
    let json: serde_json::Value = serde_json::from_slice(&checked.stdout).unwrap();
    let positive = json["kernel_conformance"]["receipts"][0]["fixtures"]
        .as_array()
        .unwrap()
        .iter()
        .find(|fixture| fixture["id"] == "factory-positive")
        .unwrap();
    assert_eq!(positive["observed"], "resource-limit");
    assert_eq!(positive["passed"], false);
    assert!(!dir.join("packs/receipt.json").exists());

    fs::remove_file(dir.join("packs/fixtures/positive/oversized.java")).unwrap();
    fs::write(
        dir.join("packs/pom.xml"),
        vec![b' '; nose_semantics::MAX_SEMANTIC_PACK_DEPENDENCY_BYTES + 1],
    )
    .unwrap();
    let dependency_limited = check_with_receipt(&dir);
    assert!(!dependency_limited.status.success());
    let json: serde_json::Value = serde_json::from_slice(&dependency_limited.stdout).unwrap();
    assert!(json["kernel_conformance"]["receipts"][0]["fixtures"]
        .as_array()
        .unwrap()
        .iter()
        .all(|fixture| fixture["observed"] == "resource-limit"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn malformed_hard_negative_is_an_analysis_failure() {
    let dir = prepare_exact_project("semantic_pack_external_exact_malformed");
    fs::write(
        dir.join("packs/fixtures/wrong-member/Fixture.java"),
        "class Fixture { static boolean broken( { return false; }\n",
    )
    .unwrap();
    let checked = check_with_receipt(&dir);
    assert!(!checked.status.success());
    let json: serde_json::Value = serde_json::from_slice(&checked.stdout).unwrap();
    let negative = json["kernel_conformance"]["receipts"][0]["fixtures"]
        .as_array()
        .unwrap()
        .iter()
        .find(|fixture| fixture["id"] == "wrong-member-negative")
        .unwrap();
    assert_eq!(negative["observed"], "analysis-failure");
    assert_eq!(negative["passed"], false);
    assert!(!dir.join("packs/receipt.json").exists());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn positive_requires_new_cross_boundary_exact_evidence() {
    let dir = prepare_exact_project("semantic_pack_external_exact_causal_positive");
    fs::write(
        dir.join("packs/fixtures/positive/Fixture.java"),
        "import com.example.collect.FastList;\n\
         class Fixture {\n\
           static boolean first(int value) {\n\
             return FastList.of(1, 2, 3, 4, 5).contains(value);\n\
           }\n\
           static boolean second(int value) {\n\
             return FastList.of(1, 2, 3, 4, 5).contains(value);\n\
           }\n\
         }\n",
    )
    .unwrap();
    let checked = check_with_receipt(&dir);
    assert!(!checked.status.success());
    let json: serde_json::Value = serde_json::from_slice(&checked.stdout).unwrap();
    let positive = json["kernel_conformance"]["receipts"][0]["fixtures"]
        .as_array()
        .unwrap()
        .iter()
        .find(|fixture| fixture["id"] == "factory-positive")
        .unwrap();
    assert_eq!(positive["observed"], "no-external-exact-match");
    assert_eq!(positive["passed"], false);
    assert!(!dir.join("packs/receipt.json").exists());
    let _ = fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn symlinked_fixture_root_fails_closed() {
    use std::os::unix::fs::symlink;

    let dir = prepare_exact_project("semantic_pack_external_exact_symlink");
    fs::rename(
        dir.join("packs/fixtures/wrong-member"),
        dir.join("packs/fixtures/wrong-member-real"),
    )
    .unwrap();
    symlink("wrong-member-real", dir.join("packs/fixtures/wrong-member")).unwrap();
    let checked = check_with_receipt(&dir);
    assert!(!checked.status.success());
    let json: serde_json::Value = serde_json::from_slice(&checked.stdout).unwrap();
    let negative = json["kernel_conformance"]["receipts"][0]["fixtures"]
        .as_array()
        .unwrap()
        .iter()
        .find(|fixture| fixture["id"] == "wrong-member-negative")
        .unwrap();
    assert_eq!(negative["observed"], "analysis-failure");
    assert_eq!(negative["passed"], false);
    assert!(!dir.join("packs/receipt.json").exists());
    let _ = fs::remove_dir_all(&dir);
}
