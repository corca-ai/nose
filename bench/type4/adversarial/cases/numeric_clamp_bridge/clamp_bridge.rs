fn clamp_literal_method(x: i64) -> i64 {
    x.clamp(0, 10)
}

fn clamp_guarded_method(x: i64, lo: i64, hi: i64) -> i64 {
    if hi < lo {
        panic!();
    }
    x.clamp(lo, hi)
}

fn clamp_guarded_minmax_method(x: i64, lo: i64, hi: i64) -> i64 {
    if hi < lo {
        panic!();
    }
    x.max(lo).min(hi)
}

fn clamp_unproven_method(x: i64, lo: i64, hi: i64) -> i64 {
    x.clamp(lo, hi)
}

struct ClampWrap(i64);

impl ClampWrap {
    fn clamp(&self, _lo: i64, _hi: i64) -> i64 {
        0
    }
}

fn clamp_custom_method(x: ClampWrap) -> i64 {
    x.clamp(0, 10)
}
