pub fn rust_missing(value: Option<i32>, other: Option<i32>) -> bool {
    value.is_none()
}

pub fn rust_iflet_missing(value: Option<i32>, other: Option<i32>) -> bool {
    if let None = value { true } else { false }
}

pub fn rust_present(value: Option<i32>, other: Option<i32>) -> bool {
    value.is_some()
}

pub fn rust_wrong_value(value: Option<i32>, other: Option<i32>) -> bool {
    other.is_none()
}
