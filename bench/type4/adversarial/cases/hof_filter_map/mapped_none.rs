fn build_case(xs: &[i32]) -> Vec<Option<i32>> {
    xs.iter()
        .copied()
        .map(|x| if x > 0 { Some(x * 2) } else { None })
        .collect()
}
