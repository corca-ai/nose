fn float_minmax(x: f64) -> f64 {
    x.max(0.0).min(10.0)
}

fn float_clamp(x: f64) -> f64 {
    x.clamp(0.0, 10.0)
}
