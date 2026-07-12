func compactMapCustomOverload(_ xs: [Bool]) -> [Bool] {
    return xs.compactMap { value in value ? value : nil }
}
