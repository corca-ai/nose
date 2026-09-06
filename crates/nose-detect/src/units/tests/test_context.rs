use nose_il::{FileId, Interner, Lang};

#[test]
fn rust_test_attributes_scope_functions_and_nested_regions() {
    let source = r##"
#[test]
fn checks_result() {
    let value = 15;
    if value > 10 { assert_eq!(value * 2 + 3, 33); }
}
#[cfg(test)]
mod fixtures {
    fn helper(value: i32) -> i32 { if value > 10 { value * 2 + 3 } else { value - 2 } }
}
#[cfg(not(test))]
fn production(value: i32) -> i32 { if value > 10 { value * 2 + 3 } else { value - 2 } }
fn literal() -> &'static str { "#[test]" }
"##;
    let interner = Interner::new();
    let raw = nose_frontend::lower_source(
        FileId(0),
        "src/main_tests/cases.rs",
        source.as_bytes(),
        Lang::Rust,
        &interner,
    )
    .unwrap();
    let opts = crate::DetectOptions {
        min_lines: 1,
        min_tokens: 1,
        ..Default::default()
    };
    let units = crate::units_of_file(&raw, &interner, &opts);
    let test_units: Vec<_> = units
        .iter()
        .filter(|u| u.start_line >= 3 && u.end_line <= 9)
        .collect();
    assert!(
        test_units.len() >= 2,
        "test functions and fragments must be admitted"
    );
    assert!(
        test_units.iter().all(|u| u.in_test_module),
        "every nested test region inherits context"
    );
    assert!(units
        .iter()
        .filter(|u| u.start_line >= 11)
        .all(|u| !u.in_test_module));
    assert!(units
        .iter()
        .any(|u| u.name.as_deref() == Some("production")));
}
