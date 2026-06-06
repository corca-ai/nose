pub fn f(xs: &[i64]) -> Vec<i64> { xs.iter().copied().filter(|x| *x > 0 && *x < 10).collect() }
