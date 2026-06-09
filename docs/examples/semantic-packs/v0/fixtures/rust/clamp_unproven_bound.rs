fn unproven(x: i64, lo: i64, hi: i64) -> i64 {
    std::cmp::min(std::cmp::max(x, lo), hi)
}

fn direct(x: i64, lo: i64, hi: i64) -> i64 {
    x.clamp(lo, hi)
}
