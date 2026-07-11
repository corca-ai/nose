use super::load;
use nose_detect::{LineSpan, Loc, LocInit, RefactorFamily};

fn loc(file: &str, lang: &str) -> Loc {
    Loc::new(LocInit {
        file: file.into(),
        source_span: LineSpan::new(1, 8),
        lang: lang.into(),
        kind: nose_il::UnitKind::Function,
        origin: Default::default(),
        name: Some("f".into()),
        sem: 24,
        span_tokens: 24,
    })
}

fn family(langs: &[&str]) -> RefactorFamily {
    family_with_locations(
        &langs
            .iter()
            .enumerate()
            .map(|(index, lang)| (format!("{lang}/{index}.txt"), *lang))
            .collect::<Vec<_>>(),
    )
}

fn family_with_locations(locations: &[(String, &str)]) -> RefactorFamily {
    RefactorFamily {
        value: 1.0,
        members: locations.len(),
        files: locations.len(),
        modules: 1,
        languages: locations.len(),
        mean_score: 1.0,
        mean_lines: 8,
        dup_lines: 8,
        shared_lines: 8,
        params: 0,
        shared_weight: 8.0,
        locations: locations
            .iter()
            .map(|(file, lang)| loc(file, lang))
            .collect(),
        accepted_coverage: Vec::new(),
        mean_sem: 24.0,
        scope: "prod",
        discount: 1.0,
        abstraction_witness: None,
        witness: None,
        varying_spots: Vec::new(),
        semantic_laws: Vec::new(),
    }
}

fn ignore_set(tag: &str, body: &str) -> super::IgnoreSet {
    let thread_name = std::thread::current()
        .name()
        .unwrap_or("test")
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect::<String>();
    let path = std::env::temp_dir().join(format!(
        "nose_ignore_match_{tag}_{}_{}.json",
        std::process::id(),
        thread_name
    ));
    std::fs::write(&path, body).unwrap();
    let set = load(&path).expect("ignore file should load");
    let _ = std::fs::remove_file(path);
    set
}

fn load_error(tag: &str, body: &str) -> String {
    let thread_name = std::thread::current()
        .name()
        .unwrap_or("test")
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect::<String>();
    let path = std::env::temp_dir().join(format!(
        "nose_ignore_match_error_{tag}_{}_{}.json",
        std::process::id(),
        thread_name
    ));
    std::fs::write(&path, body).unwrap();
    let error = match load(&path) {
        Ok(_) => panic!("ignore file should fail to load"),
        Err(error) => error,
    };
    let _ = std::fs::remove_file(path);
    format!("{error:#}")
}

#[test]
fn language_ignore_requires_every_family_member_to_match() {
    let set = ignore_set(
        "partial_language",
        "{\"ignores\":[{\"languages\":[\"python\"],\"reason\":\"accepted-python\"}]}\n",
    );
    assert!(
        set.match_family(&family(&["python", "rust"])).is_none(),
        "a language selector covering only one member must not hide the family"
    );
}

#[test]
fn language_ignore_matches_when_all_members_match_case_insensitively() {
    let set = ignore_set(
        "all_language",
        "{\"ignores\":[{\"languages\":[\"Python\"],\"reason\":\"accepted-python\"}]}\n",
    );
    let matched = set
        .match_family(&family(&["python", "python"]))
        .expect("all-python family should be ignored");
    assert_eq!(matched.matched_languages, vec!["python"]);
}

#[test]
fn path_ignore_requires_every_family_member_to_match() {
    let set = ignore_set(
        "partial_path",
        "{\"ignores\":[{\"paths\":[\"vendor/**\"],\"reason\":\"vendored\"}]}\n",
    );
    let family = family_with_locations(&[
        ("vendor/generated_a.py".to_string(), "python"),
        ("src/owned_b.py".to_string(), "python"),
    ]);

    assert!(
        set.match_family(&family).is_none(),
        "a path selector covering only one member must not hide the family"
    );
}

#[test]
fn path_ignore_matches_all_members_and_reports_sorted_paths() {
    let set = ignore_set(
        "all_path",
        "{\"ignores\":[{\"paths\":[\"vendor/**\"],\"reason\":\"vendored\"}]}\n",
    );
    let family = family_with_locations(&[
        ("vendor/generated_b.py".to_string(), "python"),
        ("vendor/generated_a.py".to_string(), "python"),
    ]);

    let matched = set
        .match_family(&family)
        .expect("all vendor paths should match");
    assert_eq!(
        matched.matched_paths,
        vec!["vendor/generated_a.py", "vendor/generated_b.py"]
    );
}

#[test]
fn path_ignore_matches_absolute_locations_relative_to_ignore_file() {
    let set = ignore_set(
        "absolute_path",
        "{\"ignores\":[{\"paths\":[\"vendor/**\"],\"reason\":\"vendored\"}]}\n",
    );
    let base = std::env::temp_dir();
    let family = family_with_locations(&[
        (
            base.join("vendor/generated_a.py").display().to_string(),
            "python",
        ),
        (
            base.join("vendor/generated_b.py").display().to_string(),
            "python",
        ),
    ]);

    let matched = set
        .match_family(&family)
        .expect("absolute paths under the ignore file directory should match");
    assert_eq!(
        matched.matched_paths,
        vec![
            base.join("vendor/generated_a.py").display().to_string(),
            base.join("vendor/generated_b.py").display().to_string(),
        ]
    );
}

#[test]
fn path_and_language_selectors_must_both_cover_every_member() {
    let set = ignore_set(
        "path_language",
        "{\"ignores\":[{\"paths\":[\"vendor/**\"],\"languages\":[\"python\"],\"reason\":\"accepted-python-vendor\"}]}\n",
    );
    let mixed_language = family_with_locations(&[
        ("vendor/generated_a.py".to_string(), "python"),
        ("vendor/generated_b.rs".to_string(), "rust"),
    ]);
    assert!(
        set.match_family(&mixed_language).is_none(),
        "path coverage alone is not enough when a language selector is present"
    );

    let all_python = family_with_locations(&[
        ("vendor/generated_a.py".to_string(), "python"),
        ("vendor/generated_b.py".to_string(), "python"),
    ]);
    assert!(
        set.match_family(&all_python).is_some(),
        "all selectors covering all members should suppress the family"
    );
}

#[test]
fn empty_selector_entries_are_rejected() {
    let error = load_error(
        "empty_selectors",
        "{\"ignores\":[{\"reason\":\"missing selectors\"}]}\n",
    );
    assert!(
        error.contains("must set at least one selector"),
        "error should explain selector requirements: {error}"
    );
}

#[test]
fn invalid_path_and_language_selectors_are_rejected() {
    let negative = load_error(
        "negative_path",
        "{\"ignores\":[{\"paths\":[\"!vendor/**\"],\"reason\":\"bad path\"}]}\n",
    );
    assert!(
        negative.contains("does not support negative pattern"),
        "negative glob error should be clear: {negative}"
    );

    let empty_path = load_error(
        "empty_path",
        "{\"ignores\":[{\"paths\":[\"   \"],\"reason\":\"bad path\"}]}\n",
    );
    assert!(
        empty_path.contains("contains an empty pattern"),
        "empty path error should be clear: {empty_path}"
    );

    let empty_language = load_error(
        "empty_language",
        "{\"ignores\":[{\"languages\":[\"   \"],\"reason\":\"bad language\"}]}\n",
    );
    assert!(
        empty_language.contains("languages contains an empty language"),
        "empty language error should be clear: {empty_language}"
    );
}

#[test]
fn invalid_expiry_dates_are_rejected() {
    let error = load_error(
        "bad_expiry",
        "{\"ignores\":[{\"paths\":[\"vendor/**\"],\"reason\":\"temporary\",\"expires_at\":\"2099-02-31\"}]}\n",
    );
    assert!(
        error.contains("expires_at must be YYYY-MM-DD"),
        "expiry error should name the field: {error}"
    );
}

#[test]
fn expired_entries_are_reported_but_not_matched() {
    let set = ignore_set(
        "expired_path",
        "{\"ignores\":[{\"paths\":[\"vendor/**\"],\"reason\":\"temporary\",\"expires_at\":\"2000-01-01\"}]}\n",
    );
    let family = family_with_locations(&[
        ("vendor/generated_a.py".to_string(), "python"),
        ("vendor/generated_b.py".to_string(), "python"),
    ]);

    assert_eq!(set.expired.len(), 1);
    assert_eq!(set.entries.len(), 0);
    assert!(
        set.match_family(&family).is_none(),
        "expired entries are retained for reporting but not applied"
    );
}
