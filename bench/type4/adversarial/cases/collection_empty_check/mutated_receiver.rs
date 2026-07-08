fn rust_mutated_empty(mut items: Vec<i32>, other: Vec<i32>) -> bool {
    items.push(1);
    items.is_empty()
}
