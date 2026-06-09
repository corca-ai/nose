fn swapped(x: i64, lo: i64, hi: i64) -> i64 {
    if hi < lo {
        panic!();
    }
    std::cmp::min(std::cmp::max(x, hi), lo)
}

fn direct(x: i64, lo: i64, hi: i64) -> i64 {
    if hi < lo {
        panic!();
    }
    x.clamp(lo, hi)
}
