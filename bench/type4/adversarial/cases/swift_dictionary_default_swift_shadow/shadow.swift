enum Swift {
    struct Dictionary<Key: Hashable, Value> {
        subscript(key: Key, default fallback: Int) -> Int where Value == Int {
            return fallback + 6
        }
    }
}

func dictionaryDefaultShadowedSwift(
    _ lookup: Swift.Dictionary<String, Int>, _ key: String, _ fallback: Int
) -> Int {
    return lookup[key, default: fallback]
}
