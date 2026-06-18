func axisCase(_ lookup: Dictionary<String, Int>, _ key: String, _ fallback: Int, _ otherDefault: Int) -> Int {
    return lookup[key, default: otherDefault]
}
