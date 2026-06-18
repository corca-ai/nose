func axisCase(_ value: Double) -> Double {
    return value < 0.0 ? 0.0 : (value > 10.0 ? 10.0 : value)
}
