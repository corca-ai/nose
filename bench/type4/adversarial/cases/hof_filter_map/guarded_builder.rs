fn build_case(xs: &[i32]) -> Vec<i32> {
    let out = Vec::new();
    for x in xs {
        if *x > 0 {
            out.push(*x * 2);
        }
    }
    out
}
