pub fn rust_unwrap_or(
    value: Option<i32>,
    fallback: i32,
    other: Option<i32>,
    other_default: i32,
) -> i32 {
    value.unwrap_or(fallback)
}

pub fn rust_match_default(
    value: Option<i32>,
    fallback: i32,
    other: Option<i32>,
    other_default: i32,
) -> i32 {
    match value {
        Some(inner) => inner,
        None => fallback,
    }
}

pub fn rust_wrong_value(
    value: Option<i32>,
    fallback: i32,
    other: Option<i32>,
    other_default: i32,
) -> i32 {
    other.unwrap_or(fallback)
}

pub fn rust_wrong_fallback(
    value: Option<i32>,
    fallback: i32,
    other: Option<i32>,
    other_default: i32,
) -> i32 {
    value.unwrap_or(other_default)
}
