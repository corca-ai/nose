fn build_case(xs: &[i32]) -> Vec<Option<i32>> {
    xs.iter()
        .copied()
        .filter_map(|x| if x > 0 { Some(None) } else { None })
        .collect()
}
