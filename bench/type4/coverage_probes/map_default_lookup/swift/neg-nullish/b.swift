func axisCase(_ lookup: Dictionary<String, Int>, _ key: String, _ fallback: Int) -> Int {
    return lookup[key] ?? fallback
}
