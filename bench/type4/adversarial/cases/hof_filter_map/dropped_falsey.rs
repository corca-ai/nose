fn build_case(xs: &[i32]) -> Vec<i32> {
    xs.iter()
        .copied()
        .filter_map(|x| if x > 0 { Some(0) } else { None })
        .filter(|x| *x != 0)
        .collect()
}
