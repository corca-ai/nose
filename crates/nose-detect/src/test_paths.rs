use std::borrow::Cow;

/// Whether a file path follows a conventional test-file naming pattern.
///
/// This is shared by report scoping and unit-extraction gates so a path is not
/// ranked as production in one layer and cost-gated as test code in another.
pub fn is_test_path(path: &str) -> bool {
    let p = lowercase_path(path);
    p.contains("/test/")
        || p.contains("/tests/")
        || p.contains("/__tests__/")
        || p.contains("/spec/")
        || p.starts_with("test/")
        || p.starts_with("tests/")
        || p.starts_with("__tests__/")
        || p.starts_with("spec/")
        || p.ends_with("_test.go")
        || p.ends_with("conftest.py")
        || ["_test.", ".test.", ".spec.", "_spec."]
            .iter()
            .any(|m| p.contains(m))
        || matches!(file_stem(&p), "test" | "tests")
        || file_stem(&p).starts_with("test_")
        || p.split('/').any(|part| file_stem(part).ends_with("_tests"))
}

/// Repository paths are overwhelmingly already lowercase. Keep that common path allocation-free
/// while preserving the existing case-insensitive classification for mixed-case paths.
pub(crate) fn lowercase_path(path: &str) -> Cow<'_, str> {
    if path.bytes().any(|byte| byte.is_ascii_uppercase()) {
        Cow::Owned(path.to_ascii_lowercase())
    } else {
        Cow::Borrowed(path)
    }
}

fn file_stem(path: &str) -> &str {
    let file = path.rsplit('/').next().unwrap_or(path);
    file.split('.').next().unwrap_or(file)
}

/// Conventional test callable names, shared by whole-unit and nested-region scope.
pub(crate) fn is_test_name(name: &str) -> bool {
    name.starts_with("Test") || name.starts_with("test_")
}

#[cfg(test)]
mod tests {
    use super::is_test_path;

    #[test]
    fn rust_modular_test_files_are_test_scope() {
        assert!(is_test_path("crates/nose-frontend/src/go/tests.rs"));
        assert!(is_test_path("crates/nose-frontend/src/java/test.rs"));
        assert!(is_test_path("crates/nose-frontend/src/corpus_tests.rs"));
        assert!(is_test_path(
            "crates/nose-cli/src/main_tests/query_family.rs"
        ));
    }

    #[test]
    fn common_ecosystem_test_paths_are_test_scope() {
        assert!(is_test_path("pkg/foo_test.go"));
        assert!(is_test_path("__tests__/widget.ts"));
        assert!(is_test_path("src/__tests__/widget.ts"));
        assert!(is_test_path("spec/models/user.rb"));
        assert!(is_test_path("tests/test_parser.py"));
        assert!(is_test_path("src/parser.spec.ts"));
        assert!(is_test_path("conftest.py"));
    }

    #[test]
    fn ordinary_source_paths_stay_production_scope() {
        assert!(!is_test_path("src/contest.rs"));
        assert!(!is_test_path("src/parser.rs"));
        assert!(!is_test_path("src/integration_helpers.py"));
    }

    #[test]
    fn mixed_case_paths_keep_case_insensitive_test_classification() {
        assert!(is_test_path("Crates/Widget/Tests/Parser.RS"));
        assert!(is_test_path("Sources/Widget_SPEC.RB"));
        assert!(!is_test_path("Sources/Contest.RS"));
    }
}
