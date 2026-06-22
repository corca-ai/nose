use super::*;

#[test]
fn external_pack_enabled_by_default_is_rejected() {
    let dir = unique_dir("trust");
    let path = dir.join("pack.json");
    fs::write(
        &path,
        manifest("com.example.pack").replace(
            r#""trust": "external-opt-in",
    "enabled_by_default": false"#,
            r#""trust": "external-opt-in",
    "enabled_by_default": true"#,
        ),
    )
    .unwrap();
    let err = load_local_manifest(&path).expect_err("must reject implicit external default");
    assert!(err
        .to_string()
        .contains("must be external-opt-in and disabled by default"));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn local_manifest_claiming_builtin_trust_is_rejected() {
    let dir = unique_dir("builtin_trust");
    let path = dir.join("pack.json");
    fs::write(
        &path,
        manifest("com.example.pack").replace(
            r#""trust": "external-opt-in""#,
            r#""trust": "builtin-default""#,
        ),
    )
    .unwrap();
    let err = load_local_manifest(&path).expect_err("local manifest must not claim builtin trust");
    assert!(err
        .to_string()
        .contains("must be external-opt-in and disabled by default"));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn local_manifest_legacy_first_party_trust_aliases_are_rejected_after_parse() {
    for legacy_trust in ["default-first-party", "first-party-optional"] {
        let dir = unique_dir(&format!("legacy_{}_trust", legacy_trust.replace('-', "_")));
        let path = dir.join("pack.json");
        fs::write(
            &path,
            manifest("com.example.pack").replace(
                r#""trust": "external-opt-in""#,
                &format!(r#""trust": "{legacy_trust}""#),
            ),
        )
        .unwrap();
        let err =
            load_local_manifest(&path).expect_err("legacy alias must be reserved for builtin");
        assert!(err
            .to_string()
            .contains("must be external-opt-in and disabled by default"));
        let _ = fs::remove_dir_all(dir);
    }
}

#[test]
fn package_entries_must_match_manifest_shape() {
    let dir = unique_dir("package");
    let path = dir.join("pack.json");
    fs::write(
        &path,
        manifest("com.example.pack").replace(
            r#"  "supported_languages": [{ "id": "python" }],
"#,
            r#"  "supported_languages": [{ "id": "python" }],
  "packages": [{ "ecosystem": "pypi", "name": "example" }],
"#,
        ),
    )
    .unwrap();
    let err = load_local_manifest(&path).expect_err("package versions are required");
    assert!(err.to_string().contains("missing field `versions`"));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn compatibility_nose_must_be_version_requirement_like() {
    let dir = unique_dir("compatibility");
    let path = dir.join("pack.json");
    fs::write(
        &path,
        manifest("com.example.pack").replace(
            r#""compatibility": { "nose": ">=0.15.0 <0.16.0" }"#,
            r#""compatibility": { "nose": "current stable" }"#,
        ),
    )
    .unwrap();
    let err = load_local_manifest(&path).expect_err("version range should be comparable");
    assert!(err.to_string().contains("unsupported version requirement"));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn compatibility_nose_accepts_semver_operator_spacing() {
    for (tag, supported_range) in [
        ("operator_spacing", ">= 0.15.0, < 0.16.0"),
        ("v_prefix", ">= v0.15.0 < v0.16.0"),
    ] {
        let dir = unique_dir(&format!("compatibility_{tag}"));
        let path = dir.join("pack.json");
        fs::write(
            &path,
            manifest("com.example.pack").replace(
                r#""compatibility": { "nose": ">=0.15.0 <0.16.0" }"#,
                &format!(r#""compatibility": {{ "nose": "{supported_range}" }}"#),
            ),
        )
        .unwrap();
        load_local_manifest(&path).expect("valid semver requirement should load");
        let _ = fs::remove_dir_all(dir);
    }
}

#[test]
fn compatibility_nose_must_include_current_binary_version() {
    for (tag, unsupported_range) in [
        ("too_old", ">=0.1.0 <0.2.0"),
        ("too_new", ">=999.0.0 <1000.0.0"),
    ] {
        let dir = unique_dir(&format!("compatibility_{tag}"));
        let path = dir.join("pack.json");
        fs::write(
            &path,
            manifest("com.example.pack").replace(
                r#""compatibility": { "nose": ">=0.15.0 <0.16.0" }"#,
                &format!(r#""compatibility": {{ "nose": "{unsupported_range}" }}"#),
            ),
        )
        .unwrap();
        let err = load_local_manifest(&path)
            .expect_err("version range must include the installed binary");
        assert!(
            err.to_string()
                .contains("does not include this nose binary version"),
            "{err}"
        );
        let _ = fs::remove_dir_all(dir);
    }
}

#[test]
fn unsupported_api_versions_are_rejected_before_loading() {
    for (tag, api_version) in [
        ("too_old", "nose.semantic-pack.pre-v0"),
        ("too_new", "nose.semantic-pack.v1"),
    ] {
        let dir = unique_dir(&format!("api_version_{tag}"));
        let path = dir.join("pack.json");
        fs::write(
            &path,
            manifest("com.example.pack").replace(
                r#""api_version": "nose.semantic-pack.v0""#,
                &format!(r#""api_version": "{api_version}""#),
            ),
        )
        .unwrap();
        let err = load_local_manifest(&path).expect_err("unsupported API versions must fail");
        assert!(err.to_string().contains("`api_version` must be"), "{err}");
        let _ = fs::remove_dir_all(dir);
    }
}

#[test]
fn exact_capable_contracts_must_reference_positive_and_hard_negative_fixtures() {
    let dir = unique_dir("contract_fixture_refs");
    let path = dir.join("pack.json");
    fs::write(
        &path,
        manifest("com.example.pack").replace(
            r#""conformance_refs": ["positive", "negative"]"#,
            r#""conformance_refs": ["positive"]"#,
        ),
    )
    .unwrap();
    let err = load_local_manifest(&path)
        .expect_err("exact-capable contracts need both fixture polarities");
    assert!(
        err.to_string()
            .contains("must reference at least one positive and one hard-negative"),
        "{err}"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn value_law_semantics_must_be_an_object_even_when_not_exact_capable() {
    let dir = unique_dir("value_law_semantics_shape");
    let path = dir.join("pack.json");
    fs::write(
        &path,
        manifest("com.example.pack").replace(
            r#""value_laws": []"#,
            r#""value_laws": [{
      "id": "python.example.near-law",
      "requires": [],
      "semantics": "not an object",
      "channel": "near-only",
      "proof_status": "missing",
      "conformance_refs": []
    }]"#,
        ),
    )
    .unwrap();
    let err = load_local_manifest(&path).expect_err("value law semantics must match schema");
    assert!(
        err.to_string().contains("semantics must be an object"),
        "{err}"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn exact_capable_contracts_must_have_required_evidence_requirements() {
    let dir = unique_dir("required_evidence_requirement");
    let path = dir.join("pack.json");
    fs::write(
        &path,
        manifest("com.example.pack").replace(r#""required": true"#, r#""required": false"#),
    )
    .unwrap();
    let err =
        load_local_manifest(&path).expect_err("optional-only requirements must not open exact");
    assert!(
        err.to_string()
            .contains("must declare at least one required evidence requirement"),
        "{err}"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn evidence_kind_must_match_schema_shape() {
    let dir = unique_dir("evidence_kind_shape");
    let path = dir.join("pack.json");
    fs::write(
        &path,
        manifest("com.example.pack").replace(
            r#""kind": "LibraryApi.Contract""#,
            r#""kind": "LibraryApi.""#,
        ),
    )
    .unwrap();
    let err = load_local_manifest(&path).expect_err("empty evidence-kind suffix is invalid");
    assert!(
        err.to_string().contains("unknown kind `LibraryApi.`"),
        "{err}"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn conformance_fixtures_must_use_manifest_relative_paths() {
    let dir = unique_dir("absolute_fixture_path");
    let outside = unique_dir("absolute_fixture_path_outside");
    let absolute_fixture = outside.join("positive.py");
    fs::write(&absolute_fixture, "print('external fixture')\n").unwrap();
    let path = dir.join("pack.json");
    fs::write(
        &path,
        manifest("com.example.pack")
            .replace("fixtures/positive.py", absolute_fixture.to_str().unwrap()),
    )
    .unwrap();

    let report = check_semantic_pack_conformance(&[path]).expect("manifest is structurally valid");

    assert!(!report.passed());
    let issues = report.manifests[0]
        .fixtures
        .iter()
        .flat_map(|fixture| fixture.issues.iter().map(|issue| issue.as_str()))
        .collect::<Vec<_>>();
    assert_eq!(issues, vec!["absolute-path", "missing-file"]);
    let _ = fs::remove_dir_all(dir);
    let _ = fs::remove_dir_all(outside);
}

#[test]
fn evidence_producer_anchors_must_be_known_anchor_names() {
    let dir = unique_dir("anchor");
    let path = dir.join("pack.json");
    fs::write(
        &path,
        manifest("com.example.pack")
            .replace(r#""anchors": ["node"]"#, r#""anchors": ["raw-selector"]"#),
    )
    .unwrap();
    let err = load_local_manifest(&path).expect_err("unknown anchors must not load");
    assert!(err.to_string().contains("unknown variant"));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn duplicate_pack_ids_fail_closed() {
    let dir = unique_dir("dupe");
    let one = dir.join("one.json");
    let two = dir.join("two.json");
    fs::write(&one, manifest("com.example.pack")).unwrap();
    fs::write(&two, manifest("com.example.pack")).unwrap();
    let err = SemanticPackSet::new_local(&[one, two]).expect_err("duplicate id");
    assert!(err.to_string().contains("duplicate semantic pack id"));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn local_manifest_cannot_claim_compiled_builtin_pack_id() {
    let dir = unique_dir("compiled_builtin_id");
    let path = dir.join("pack.json");
    fs::write(&path, manifest(PYTHON_STDLIB_TYPE_DOMAIN_PACK_ID)).unwrap();
    let err = SemanticPackSet::new_local(&[path]).expect_err("compiled id is reserved");
    assert!(err.to_string().contains("duplicate semantic pack id"));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn local_manifest_cannot_claim_builtin_language_pack_id() {
    let dir = unique_dir("builtin_language_id");
    let path = dir.join("pack.json");
    fs::write(&path, manifest(PYTHON_LANGUAGE_PACK_ID)).unwrap();
    let err = SemanticPackSet::new_local(&[path]).expect_err("builtin language id is reserved");
    assert!(err.to_string().contains("duplicate semantic pack id"));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn conformance_check_cannot_claim_compiled_builtin_pack_id() {
    let dir = unique_dir("compiled_builtin_conformance");
    let path = dir.join("pack.json");
    fs::write(&path, manifest(PYTHON_STDLIB_TYPE_DOMAIN_PACK_ID)).unwrap();
    let err = check_semantic_pack_conformance(&[path]).expect_err("compiled id is reserved");
    assert!(err.to_string().contains("duplicate semantic pack id"));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn conformance_check_cannot_claim_builtin_language_pack_id() {
    let dir = unique_dir("builtin_language_conformance");
    let path = dir.join("pack.json");
    fs::write(&path, manifest(JS_TS_LANGUAGE_PACK_ID)).unwrap();
    let err =
        check_semantic_pack_conformance(&[path]).expect_err("builtin language id is reserved");
    assert!(err.to_string().contains("duplicate semantic pack id"));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn directory_discovery_sorts_json_manifests() {
    let dir = unique_dir("dir");
    fs::write(dir.join("b.json"), manifest("com.example.b")).unwrap();
    fs::write(dir.join("a.json"), manifest("com.example.a")).unwrap();
    let paths = discover_manifest_paths(std::slice::from_ref(&dir)).expect("discover");
    let names = paths
        .iter()
        .map(|path| path.file_name().unwrap().to_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(names, vec!["a.json", "b.json"]);
    let _ = fs::remove_dir_all(dir);
}
