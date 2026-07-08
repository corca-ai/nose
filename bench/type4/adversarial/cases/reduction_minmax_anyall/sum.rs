pub fn rust_sum_loop(xs: &[i64]) -> i64 {
    let mut total = 0;
    for x in xs {
        total += x;
    }
    total
}

pub fn rust_sum_iter(xs: &[i64]) -> i64 {
    xs.iter().sum()
}

pub fn rust_product_iter(xs: &[i64]) -> i64 {
    xs.iter().product()
}
