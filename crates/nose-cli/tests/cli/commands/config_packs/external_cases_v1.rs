use super::*;

#[test]
fn typed_v1_pack_compiles_and_reports_digest_without_changing_families() {
    let dir = make_project("semantic_pack_v1_report");
    let pack = dir.join("pack.json");
    fs::write(&pack, semantic_pack_manifest_v1()).unwrap();

    let without_pack = query_json(&run(&[
        "query",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--format",
        "json",
    ]));
    let with_pack = query_json(&run(&[
        "query",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--semantic-pack",
        pack.to_str().unwrap(),
        "--format",
        "json",
    ]));

    assert_eq!(
        query_families(&with_pack),
        query_families(&without_pack),
        "compiled v1 packs must remain metadata-only before lock and evidence work"
    );
    let reported = semantic_pack_by_id(&with_pack, "com.example.java-guava-typed-factories");
    assert_eq!(reported["api_version"], "nose.semantic-pack.v1");
    assert_eq!(reported["source"], "local-manifest");
    assert_eq!(reported["influence"], "metadata-only");
    assert_eq!(reported["counts"]["contracts"], 3);
    let digest = reported["semantic_digest"].as_str().unwrap();
    assert!(digest.starts_with("sha256:"));
    assert_eq!(digest.len(), 71);

    let check = Command::new(bin())
        .args([
            "semantic-pack",
            "check",
            pack.to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .expect("v1 semantic-pack check");
    assert!(check.status.success());
    let check_json: serde_json::Value = serde_json::from_slice(&check.stdout).unwrap();
    assert_eq!(check_json["schema_version"], 3);
    assert_eq!(check_json["influence_preflight"]["status"], "unavailable");
    assert_eq!(
        check_json["manifests"][0]["api_version"],
        "nose.semantic-pack.v1"
    );
    assert_eq!(check_json["manifests"][0]["semantic_digest"], digest);

    let _ = fs::remove_dir_all(&dir);
}
