use super::*;

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn query(mode: &str, locked: bool) -> serde_json::Value {
    let mut command = Command::new(bin());
    command.args([
        "query",
        "bench/semantic_pack/reference/vavr-study-control",
        "all",
        "top=0",
        "--mode",
        mode,
        "--min-size",
        "1",
        "--format",
        "json",
    ]);
    if locked {
        command.args([
            "--semantic-pack-lock",
            "docs/examples/vavr-list-project-lock-v1.json",
        ]);
    }
    let output = command.current_dir(workspace()).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn shipped_vavr_reference_pack_changes_only_explicitly_locked_controls() {
    let status = Command::new(bin())
        .args([
            "semantic-pack",
            "status",
            "docs/examples/vavr-list-project-lock-v1.json",
            "--format",
            "json",
        ])
        .current_dir(workspace())
        .output()
        .unwrap();
    assert!(
        status.status.success(),
        "{}",
        String::from_utf8_lossy(&status.stderr)
    );

    let semantic_without = query("semantic", false);
    let semantic_with = query("semantic", true);
    assert!(semantic_without["families"].as_array().unwrap().is_empty());
    let exact_family = semantic_with["families"]
        .as_array()
        .unwrap()
        .iter()
        .find(|family| family.get("semantic_pack_external_exact").is_some())
        .expect("locked reference pack should produce one attributed exact family");
    assert_eq!(
        exact_family["semantic_pack_external_exact"][0]["row_id"],
        "java.vavr.list.of-five-exact"
    );

    let near_without = query("near", false);
    let near_with = query("near", true);
    let influenced = near_with["families"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|family| family.get("semantic_pack_near").is_some())
        .count();
    assert!(influenced >= 1);
    assert!(
        near_with["families"].as_array().unwrap().len()
            > near_without["families"].as_array().unwrap().len()
    );

    let pack = semantic_pack_by_id(&near_with, "org.corca.reference.java-vavr-list");
    assert_eq!(pack["enabled_by_default"], false);
    assert_eq!(pack["near_influence"]["influential_occurrences"], 2);
    assert_eq!(
        pack["external_exact_influence"]["influential_occurrences"],
        1
    );
}
