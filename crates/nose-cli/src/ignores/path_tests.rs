use super::{Entry, RawEntry};
use std::path::Path;

fn entry(base: &Path, pattern: &str) -> Entry {
    Entry::from_raw(
        0,
        RawEntry {
            family_id: None,
            paths: vec![pattern.into()],
            languages: Vec::new(),
            reason: "generated".into(),
            note: None,
            owner: None,
            expires_at: None,
        },
        base,
    )
    .unwrap()
}

fn matches(entry: &Entry, file: &Path) -> bool {
    let family = super::match_tests::family_with_locations(&[(
        file.to_string_lossy().into_owned(),
        "python",
    )]);
    entry.match_family(&family).is_some()
}

#[test]
fn absolute_ignore_base_matches_cwd_relative_locations() {
    let cwd = std::env::current_dir().unwrap();
    let rule = entry(&cwd.join("project"), "vendor/**");
    let relative = Path::new("project/vendor/generated.py");
    assert!(matches(&rule, relative));
    assert!(matches(&rule, &cwd.join(relative)));
}

#[test]
fn cwd_relative_patterns_work_with_an_ignore_file_in_a_subdirectory() {
    let cwd = std::env::current_dir().unwrap();
    let rule = entry(&cwd.join("suppressions"), "vendor/**");
    assert!(matches(&rule, Path::new("vendor/generated.py")));
    assert!(matches(&rule, &cwd.join("vendor/generated.py")));
}

#[test]
fn directory_patterns_cover_nested_files() {
    let rule = entry(Path::new("."), "vendor/");
    assert!(matches(&rule, Path::new("vendor/generated.py")));
    assert!(matches(&rule, Path::new("vendor/nested/generated.py")));
    assert!(!matches(&rule, Path::new("src/vendor")));
    assert!(!matches(&rule, Path::new("src/owned.py")));
    assert!(matches(&rule, Path::new("pkg/vendor/generated.py")));

    let anchored = entry(Path::new("."), "/vendor/");
    assert!(matches(&anchored, Path::new("vendor/nested/generated.py")));
    assert!(!matches(&anchored, Path::new("pkg/vendor/generated.py")));
}

#[test]
fn parent_components_do_not_expand_suppression_scope() {
    let rule = entry(Path::new("."), "vendor/**");
    assert!(!matches(&rule, Path::new("vendor/../src/owned.py")));
    assert!(matches(&rule, Path::new("src/../vendor/generated.py")));
}

#[test]
fn directory_patterns_still_require_every_family_member() {
    let rule = entry(Path::new("."), "vendor/");
    let family = super::match_tests::family_with_locations(&[
        ("vendor/nested/generated.py".into(), "python"),
        ("src/owned.py".into(), "python"),
    ]);
    assert!(rule.match_family(&family).is_none());
}

#[test]
fn broad_patterns_retain_absolute_path_matching() {
    let cwd = std::env::current_dir().unwrap();
    let rule = entry(&cwd.join("suppressions"), "**/*.py");
    assert!(matches(&rule, &std::env::temp_dir().join("outside.py")));
}
