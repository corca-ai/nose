func axisCase(_ lookup: Dictionary<String, Int>, _ key: String, _ fallback: Int, _ other: String) -> Int {
    return lookup[key, default: fallback]
}
