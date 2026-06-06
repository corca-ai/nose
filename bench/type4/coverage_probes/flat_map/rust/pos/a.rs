pub fn f(xss: &[Vec<i64>]) -> Vec<i64> { let mut out = Vec::new(); for xs in xss { for y in xs { out.push(*y); } } out }
