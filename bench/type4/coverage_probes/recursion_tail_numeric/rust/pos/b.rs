pub fn fac(mut n: i64) -> i64 { let mut acc = 1; while n != 0 { acc = acc * n; n = n - 1; } acc }
