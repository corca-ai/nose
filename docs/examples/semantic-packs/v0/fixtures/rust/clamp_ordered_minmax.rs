fn minmax(x: i64) -> i64 {
    std::cmp::min(std::cmp::max(x, 0), 10)
}

fn direct(x: i64) -> i64 {
    x.clamp(0, 10)
}
