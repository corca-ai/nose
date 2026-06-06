fn build_case(xs: &[i32]) -> Vec<i32> {
    xs.iter()
        .copied()
        .filter_map(|x| Some(x).and_then(|value| {
            if value > 0 {
                Some(value * 2)
            } else {
                None
            }
        }))
        .collect()
}
