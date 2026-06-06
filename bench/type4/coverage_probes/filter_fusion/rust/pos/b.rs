pub fn f(xs: &[i64]) -> Vec<i64> { xs.iter().copied().filter(|x| *x > 0).filter(|x| *x < 10).collect() }
