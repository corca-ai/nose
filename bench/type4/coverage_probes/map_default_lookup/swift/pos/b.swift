func axisCase(_ table: Dictionary<String, Int>, _ name: String, _ missing: Int) -> Int {
    return table[name, default: missing]
}
