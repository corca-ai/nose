pub fn axis_case(xs: &[i64]) -> i64 {
    let mut total = 0;
    for i in 1..xs.len() {
        total += xs[i];
    }
    total
}
