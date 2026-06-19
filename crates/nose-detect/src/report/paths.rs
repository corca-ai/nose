use crate::Loc;
use std::path::Path;

/// The directory ("module") a file lives in — the design-level grouping key.
pub(super) fn module_of(file: &str) -> &str {
    Path::new(file)
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or("")
}

pub(super) fn span_lines(l: &Loc) -> u32 {
    l.span_lines
}

/// Fraction of `b`'s lines that lie inside `a` (both in the same file). Used to
/// collapse a site that is contained in — or near-identical to — a larger one.
pub(super) fn overlap_frac(a: &Loc, b: &Loc) -> f64 {
    let start = a.start_line.max(b.start_line);
    let end = a.end_line.min(b.end_line);
    if end < start {
        return 0.0;
    }
    (end - start + 1) as f64 / span_lines(b).max(1) as f64
}

/// Is this site test code, by the usual path / unit-name conventions? The markers
/// are well-known ecosystem conventions (pytest `test_*`, RSpec `spec/`, Go
/// `_test.go`); production code that adopts a test-naming convention against its
/// ecosystem (a prod validator named `test_data_loader.py`, an OpenAPI `spec/`
/// directory) WILL be tagged test — the conventions win (coevo S3-C3). Scope is
/// display context plus the opt-in `--scope` filter, never a worthiness input.
/// Public so presentation layers can scope-guard per-location advice (e.g. never
/// recommend calling a test helper from production copies).
/// Whether a file PATH is test code, by the well-known conventions (a `/test(s)/` or
/// `/spec/` or `/__tests__/` directory, a `_test.go` / `conftest.py` / `.test.` /
/// `.spec.` file, or a `test_`-prefixed stem). Path-only — the [`is_test_loc`] superset
/// also consults the unit name and the inline-test-module flag.
pub fn is_test_path(file: &str) -> bool {
    let p = file.to_ascii_lowercase();
    p.contains("/test/")
        || p.contains("/tests/")
        || p.contains("/__tests__/")
        || p.contains("/spec/")
        || p.starts_with("test/")
        || p.starts_with("tests/")
        || p.ends_with("_test.go")
        || p.ends_with("conftest.py")
        || ["_test.", ".test.", ".spec.", "_spec."]
            .iter()
            .any(|m| p.contains(m))
        || file_stem(&p).starts_with("test_")
}

pub fn is_test_loc(l: &Loc) -> bool {
    let name_test = l
        .name
        .as_deref()
        .is_some_and(|n| n.starts_with("Test") || n.starts_with("test_"));
    is_test_path(&l.file) || name_test || l.in_test_module
}

fn file_stem(path: &str) -> &str {
    let file = path.rsplit('/').next().unwrap_or(path);
    file.split('.').next().unwrap_or(file)
}

/// Is this site vendored / generated / third-party code — not the maintainer's to
/// dedupe? Conservative, well-known markers only. On the labelset, families all of
/// whose sites match this were 0/12 worthy.
pub(super) fn is_generated_loc(l: &Loc) -> bool {
    let p = l.file.to_ascii_lowercase();
    [
        "vendor/",
        "third_party/",
        "third-party/",
        "/deps/",
        "node_modules/",
        "/dist/",
        "/build/",
        ".min.",
        ".pb.",
        "_pb2",
        ".g.dart",
        ".d.ts", // TS ambient declarations: not extractable refactor targets (often generated)
        "generated/",
        "/gen/",
        ".generated.",
    ]
    .iter()
    .any(|m| p.contains(m))
}

/// Below this mean value-graph size, an all-`Class` family is a field-only type
/// definition (a record/enum/DTO), not shared behavior — see the dogfood audit.
const TYPEDEF_SEM: f64 = 12.0;

/// Refactor-worthiness discount in `(0, 1]`, applied after `refactor_value`.
/// Discounts families a reviewer reliably dismisses, without dropping them:
///   - **value-poor type definitions** — `Class` families matching only on field
///     shape, no behavior to extract;
///   - **vendored / generated code** — not the maintainer's to dedupe (0/12 worthy
///     on the labelset).
///
/// Note: test-code duplication is *not* discounted — it's a real smell, ranked like
/// any other; `scope` stays a context tag, not a penalty.
///
/// Disable with `NOSE_NO_REFACTOR_DISCOUNT=1` (used for A/B measurement).
pub(super) fn refactor_discount(all_class: bool, mean_sem: f64, all_generated: bool) -> f64 {
    if std::env::var_os("NOSE_NO_REFACTOR_DISCOUNT").is_some() {
        return 1.0;
    }
    let mut q = 1.0;
    if all_class && mean_sem < TYPEDEF_SEM {
        q *= 0.25;
    }
    if all_generated {
        q *= 0.1;
    }
    q
}
