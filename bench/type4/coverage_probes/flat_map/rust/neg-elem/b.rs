pub fn f(xss: &[Vec<i64>]) -> Vec<i64> { xss.iter().flat_map(|xs| xs.iter().map(|y| y + 1)).collect() }
