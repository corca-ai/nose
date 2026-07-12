typealias StringIntDictionary = Dictionary<String, Int>

extension StringIntDictionary {
    subscript(key: String, default fallback: Int) -> Int { return fallback + 1 }
}
