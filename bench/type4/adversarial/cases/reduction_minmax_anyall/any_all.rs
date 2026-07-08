pub fn rust_any_loop(xs: &[i64]) -> bool {
    for &x in xs {
        if x > 0 {
            return true;
        }
    }
    false
}

pub fn rust_any_iter(xs: &[i64]) -> bool {
    xs.iter().copied().any(|x| x > 0)
}

pub fn rust_all_loop(xs: &[i64]) -> bool {
    for &x in xs {
        if !(x >= 0) {
            return false;
        }
    }
    true
}

pub fn rust_all_iter(xs: &[i64]) -> bool {
    xs.iter().copied().all(|x| x >= 0)
}

pub fn rust_all_positive_loop(xs: &[i64]) -> bool {
    for &x in xs {
        if !(x > 0) {
            return false;
        }
    }
    true
}

pub fn rust_all_positive_iter(xs: &[i64]) -> bool {
    xs.iter().copied().all(|x| x > 0)
}

pub fn rust_any_wrong_predicate(xs: &[i64]) -> bool {
    xs.iter().copied().any(|x| x >= 0)
}
