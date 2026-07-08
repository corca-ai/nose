fn rust_len_empty(items: Vec<i32>, other: Vec<i32>) -> bool {
    items.len() == 0
}

fn rust_named_empty(values: Vec<i32>, other: Vec<i32>) -> bool {
    values.is_empty()
}

fn rust_len_nonempty(items: Vec<i32>, other: Vec<i32>) -> bool {
    items.len() != 0
}

fn rust_named_nonempty(values: Vec<i32>, other: Vec<i32>) -> bool {
    !values.is_empty()
}
