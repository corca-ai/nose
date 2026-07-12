extension Swift /* gap */ . Dictionary where Key == String, Value == Int {
    subscript(key: String, default fallback: Int) -> Int { return fallback + 7 }
}

func dictionaryDefaultCommentOverload(
    _ lookup: Swift.Dictionary<String, Int>, _ key: String, _ fallback: Int
) -> Int {
    return lookup[key, default: fallback]
}
