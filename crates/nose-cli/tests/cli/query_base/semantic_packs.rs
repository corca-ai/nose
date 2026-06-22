use super::*;

#[test]
fn query_base_rejects_configured_semantic_packs() {
    let dir = make_project("query_base_semantic_pack_config");
    fs::write(
        dir.join("nose.toml"),
        "[query]\nsemantic-packs = [\"pack.json\"]\n",
    )
    .unwrap();

    let out = nose_query_in(&dir, &["base=main", "--min-size", "8"]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "base= should reject configured semantic packs"
    );
    assert!(
        stderr.contains("semantic-packs config"),
        "base= names configured semantic packs as unsupported: {stderr}"
    );

    let _ = fs::remove_dir_all(&dir);
}
