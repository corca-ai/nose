use super::*;

#[test]
fn same_size_and_mtime_never_override_content_identity() {
    let first = portable_il::source_digest(Lang::Python, b"return x + 1\n");
    let second = portable_il::source_digest(Lang::Python, b"return x - 1\n");
    assert_ne!(first, second);
}
