fn build_case(xs: &[i32]) -> Vec<i32> {
    xs.iter()
        .copied()
        .filter_map(|x| match x {
            _ if x > 0 => Some(x * 2),
            _ => None,
        })
        .collect()
}
