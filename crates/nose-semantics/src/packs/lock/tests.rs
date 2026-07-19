use super::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_DIR: AtomicUsize = AtomicUsize::new(0);

fn project(tag: &str) -> PathBuf {
    let id = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "nose_semantic_pack_lock_{tag}_{}_{}",
        std::process::id(),
        id
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

fn v1_manifest() -> String {
    include_str!("../../../../../docs/examples/semantic-packs/v1/guava-immutable-collections.json")
        .to_string()
}

fn write_project(root: &Path, manifest: &str) -> (PathBuf, PathBuf, PathBuf) {
    let pack_dir = root.join("packs");
    fs::create_dir_all(&pack_dir).unwrap();
    let manifest_path = pack_dir.join("guava.json");
    fs::write(&manifest_path, manifest).unwrap();
    let dependency_path = root.join("pom.xml");
    fs::write(
        &dependency_path,
        "<project><dependency>com.google.guava:guava:33.0.0</dependency></project>\n",
    )
    .unwrap();
    let lock_path = root.join("nose.semantic-pack-lock.json");
    (manifest_path, dependency_path, lock_path)
}

fn options(dependency: PathBuf) -> SemanticPackLockOptions {
    SemanticPackLockOptions {
        allowed_channels: vec![SemanticPackV1Channel::Near],
        selected_rows: Vec::new(),
        dependency_paths: vec![dependency],
        exact_receipt: None,
    }
}

#[test]
fn creates_and_validates_a_content_pinned_relative_lock() {
    let root = project("valid");
    let (manifest, dependency, lock_path) = write_project(&root, &v1_manifest());
    let created = create_project_lock(&lock_path, &[manifest], options(dependency))
        .expect("valid lock should be created");

    assert!(lock_path.is_file());
    assert_eq!(created.authorizations().len(), 1);
    assert_eq!(created.authorizations()[0].selected_rows().len(), 3);
    assert!(created.summary().decision_digest().starts_with("sha256:"));
    let json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&lock_path).unwrap()).unwrap();
    assert_eq!(json["packs"][0]["manifest"], "packs/guava.json");
    assert_eq!(json["dependencies"][0]["path"], "pom.xml");
    assert!(!fs::read_to_string(&lock_path)
        .unwrap()
        .contains(&root.display().to_string()));

    let validated = validate_project_lock(&lock_path).expect("created lock should validate");
    assert_eq!(
        validated.summary().decision_digest(),
        created.summary().decision_digest()
    );
    let set = validated.into_semantic_packs();
    let authorization = set
        .external_v1_authorization("com.example.java-guava-typed-factories")
        .unwrap();
    assert!(authorization.allows("java.guava.immutable-list.of", SemanticPackV1Channel::Near));
    assert_eq!(
        set.project_lock().unwrap().api_version(),
        SEMANTIC_PACK_LOCK_API_VERSION_V1
    );
}

#[test]
fn relocation_and_document_reordering_preserve_the_decision_digest() {
    let first = project("relocate_first");
    let second = project("relocate_second");
    let alternate = v1_manifest()
        .replace(
            "com.example.java-guava-typed-factories",
            "com.example.java-guava-alternate-factories",
        )
        .replace(
            "java.guava.immutable-list.of",
            "java.guava.alternate-list.of",
        )
        .replace("java.guava.immutable-map.of", "java.guava.alternate-map.of")
        .replace("ImmutableList", "AlternateList")
        .replace("ImmutableMap", "AlternateMap");
    let (first_manifest, first_dependency, first_lock) = write_project(&first, &v1_manifest());
    let second_manifest_in_first = first.join("packs/alternate.json");
    fs::write(&second_manifest_in_first, alternate).unwrap();
    let created = create_project_lock(
        &first_lock,
        &[first_manifest, second_manifest_in_first],
        options(first_dependency),
    )
    .unwrap();

    fs::create_dir_all(second.join("packs")).unwrap();
    for name in ["guava.json", "alternate.json"] {
        fs::copy(
            first.join("packs").join(name),
            second.join("packs").join(name),
        )
        .unwrap();
    }
    fs::copy(first.join("pom.xml"), second.join("pom.xml")).unwrap();
    let second_lock = second.join("nose.semantic-pack-lock.json");
    fs::copy(&first_lock, &second_lock).unwrap();
    let relocated = validate_project_lock(&second_lock).unwrap();
    assert_eq!(
        relocated.summary().decision_digest(),
        created.summary().decision_digest()
    );

    let mut value: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&second_lock).unwrap()).unwrap();
    value["packs"].as_array_mut().unwrap().reverse();
    for pack in value["packs"].as_array_mut().unwrap() {
        pack["allowed_channels"].as_array_mut().unwrap().reverse();
        pack["selected_rows"].as_array_mut().unwrap().reverse();
    }
    fs::write(&second_lock, serde_json::to_string_pretty(&value).unwrap()).unwrap();
    let reordered = validate_project_lock(&second_lock).unwrap();
    assert_eq!(
        reordered.summary().decision_digest(),
        created.summary().decision_digest()
    );
}

#[test]
fn changed_dependency_or_semantic_content_invalidates_before_use() {
    let root = project("stale");
    let (manifest, dependency, lock_path) = write_project(&root, &v1_manifest());
    create_project_lock(
        &lock_path,
        std::slice::from_ref(&manifest),
        options(dependency.clone()),
    )
    .unwrap();

    fs::write(&dependency, "changed dependency graph\n").unwrap();
    let dependency_error = validate_project_lock(&lock_path).unwrap_err().to_string();
    assert!(dependency_error.contains("locked file `pom.xml` changed"));

    fs::write(
        &dependency,
        "<project><dependency>com.google.guava:guava:33.0.0</dependency></project>\n",
    )
    .unwrap();
    fs::write(
        &manifest,
        v1_manifest().replace("\"max\": 12", "\"max\": 11"),
    )
    .unwrap();
    let manifest_error = validate_project_lock(&lock_path).unwrap_err().to_string();
    assert!(manifest_error.contains("semantic content digest is stale"));
}

#[test]
fn path_escape_and_v0_lock_attempts_fail_closed() {
    let root = project("escape");
    let (manifest, dependency, lock_path) = write_project(&root, &v1_manifest());
    create_project_lock(&lock_path, &[manifest], options(dependency.clone())).unwrap();
    let mut value: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&lock_path).unwrap()).unwrap();
    value["packs"][0]["manifest"] = serde_json::json!("../outside.json");
    fs::write(&lock_path, serde_json::to_string_pretty(&value).unwrap()).unwrap();
    let error = validate_project_lock(&lock_path).unwrap_err().to_string();
    assert!(error.contains("project-relative non-escaping path"));

    let v0 = root.join("packs/v0.json");
    fs::write(
        &v0,
        include_str!("../../../../../docs/examples/semantic-packs/v0/library-pack.json"),
    )
    .unwrap();
    let error = create_project_lock(&root.join("v0.lock.json"), &[v0], options(dependency))
        .unwrap_err()
        .to_string();
    assert!(error.contains("v0 remains metadata-only"));
}

#[test]
fn overlapping_rows_conflict_independent_of_order_and_narrowing_resolves_it() {
    let root = project("conflict");
    let (first, dependency, lock_path) = write_project(&root, &v1_manifest());
    let second = root.join("packs/second.json");
    let second_manifest = v1_manifest()
        .replace(
            "com.example.java-guava-typed-factories",
            "com.example.java-guava-conflicting-factories",
        )
        .replace(
            "java.guava.immutable-list.of",
            "java.guava.conflict-list.of",
        )
        .replace("java.guava.immutable-map.of", "java.guava.conflict-map.of");
    fs::write(&second, second_manifest).unwrap();
    let first_error = create_project_lock(
        &lock_path,
        &[first.clone(), second.clone()],
        options(dependency.clone()),
    )
    .unwrap_err()
    .to_string();
    let reverse_error = create_project_lock(
        &lock_path,
        &[second.clone(), first.clone()],
        options(dependency.clone()),
    )
    .unwrap_err()
    .to_string();
    assert_eq!(first_error, reverse_error);
    assert!(first_error.contains("overlap a semantic coordinate"));

    let mut narrowed = options(dependency);
    narrowed.selected_rows = vec![
        "com.example.java-guava-typed-factories/java.guava.immutable-list.of".to_string(),
        "com.example.java-guava-conflicting-factories/java.guava.conflict-map.of".to_string(),
    ];
    let valid = create_project_lock(&lock_path, &[first, second], narrowed)
        .expect("non-overlapping selected rows should restore a valid decision");
    assert_eq!(
        valid
            .authorizations()
            .iter()
            .map(|authorization| authorization.selected_rows().len())
            .sum::<usize>(),
        2
    );
}
