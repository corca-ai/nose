pub fn f(xs: &[i64]) -> Vec<i64> { xs.iter().map(|x| x + 1).map(|y| y * 2).collect() }
