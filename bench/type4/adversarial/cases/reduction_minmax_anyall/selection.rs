pub fn rust_max_loop(xs: &[i32]) -> i32 {
    let mut best = 0;
    for &x in xs {
        if x > best {
            best = x;
        }
    }
    best
}

pub fn rust_max_fold(xs: &[i32]) -> i32 {
    xs.iter()
        .copied()
        .fold(0, |best, x| if x > best { x } else { best })
}

pub fn rust_min_loop(xs: &[i32]) -> i32 {
    let mut best = 0;
    for &x in xs {
        if x < best {
            best = x;
        }
    }
    best
}

pub fn rust_min_fold(xs: &[i32]) -> i32 {
    xs.iter()
        .copied()
        .fold(0, |best, x| if x < best { x } else { best })
}

pub fn rust_unseeded_max_default(xs: &[i32]) -> i32 {
    xs.iter().copied().max().unwrap_or(0)
}
