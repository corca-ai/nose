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

pub fn rust_all_empty_loop() -> bool {
    let xs: [i64; 0] = [];
    for &x in &xs {
        if !(x < 100) {
            return false;
        }
    }
    true
}

pub fn rust_all_empty_iter() -> bool {
    let xs: [i64; 0] = [];
    xs.iter().copied().all(|x| x < 100)
}

pub fn rust_all_wrong_empty_truth() -> bool {
    let xs: [i64; 0] = [];
    for &x in &xs {
        if !(x < 100) {
            return false;
        }
    }
    false
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

pub fn rust_all_iter_two_sources(xs: &[i64], ys: &[i64]) -> bool {
    let _ = ys.len();
    xs.iter().copied().all(|x| x < 100)
}

pub fn rust_all_different_source(xs: &[i64], ys: &[i64]) -> bool {
    ys.iter().copied().all(|y| y < 100)
}

pub fn rust_all_consumed_iterator(xs: &[i64]) -> bool {
    let mut iter = xs.iter().copied();
    let _ = iter.next();
    iter.all(|x| x >= 0)
}

pub fn rust_all_pure_with_log(xs: &[i64], log: &mut Vec<i64>) -> bool {
    let _ = log.len();
    xs.iter().copied().all(|x| x < 100)
}

pub fn rust_all_callback_effect(xs: &[i64], log: &mut Vec<i64>) -> bool {
    xs.iter().copied().all(|x| {
        log.push(x);
        x < 100
    })
}

pub fn rust_all_loop_effect(xs: &[i64], log: &mut Vec<i64>) -> bool {
    for &x in xs {
        if !(x < 100) {
            log.push(x);
            return false;
        }
    }
    true
}

pub fn rust_all_pure_mutable(xs: &mut [i64]) -> bool {
    xs.iter().copied().all(|x| x < 100)
}

pub fn rust_all_mutating_borrow(xs: &mut [i64]) -> bool {
    xs.iter_mut().all(|x| {
        *x += 1;
        *x < 100
    })
}

pub fn rust_any_wrong_predicate(xs: &[i64]) -> bool {
    xs.iter().copied().any(|x| x >= 0)
}
