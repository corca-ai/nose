use super::*;

#[test]
fn query_base_uses_configured_semantic_packs_and_portable_cache() {
    let dir = make_project("query_base_semantic_pack_config");
    fs::write(
        dir.join("pack.json"),
        include_str!("../../../../../docs/examples/semantic-packs/v0/language-pack.json"),
    )
    .unwrap();
    fs::write(
        dir.join("nose.toml"),
        "[query]\nsemantic-packs = [\"pack.json\"]\n",
    )
    .unwrap();
    init_git_repo(&dir);
    let changed = dir.join("a/f.py");
    let source = fs::read_to_string(&changed).unwrap();
    fs::write(&changed, source.replace("return total", "return total + 1")).unwrap();

    let cache = dir.join("cache");
    let cache_arg = cache.to_str().unwrap();
    let first = nose_query_in(
        &dir,
        &[
            "base=main",
            "--min-size",
            "8",
            "--format",
            "json",
            "--cache-dir",
            cache_arg,
        ],
    );
    assert!(
        first.status.success(),
        "base= should use configured semantic packs: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&first.stdout).unwrap();
    assert!(
        json["semantic_packs"]
            .as_array()
            .is_some_and(|packs| !packs.is_empty()),
        "base= JSON should report the packs used: {json}"
    );

    let second = Command::new(bin())
        .current_dir(&dir)
        .env("NOSE_CACHE_STATS", "1")
        .args([
            "query",
            ".",
            "base=main",
            "--min-size",
            "8",
            "--format",
            "json",
            "--cache-dir",
            cache_arg,
        ])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&second.stderr);
    assert!(second.status.success(), "cached base= failed: {stderr}");
    assert!(
        stderr.lines().any(|line| {
            line.contains("[cache]") && line.contains("misses=0") && !line.contains("hits=0")
        }),
        "base worktrees should reuse path-independent unit artifacts: {stderr}"
    );

    let _ = fs::remove_dir_all(&dir);
}
