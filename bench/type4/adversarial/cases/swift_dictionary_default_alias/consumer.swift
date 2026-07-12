func dictionaryDefaultAliasOverload(
    _ lookup: Swift.Dictionary<String, Int>, _ key: String, _ fallback: Int
) -> Int {
    return lookup[key, default: fallback]
}
