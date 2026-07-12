#if CUSTOM_DICTIONARY_DEFAULT
extension `Dictionary` where Key == String, Value == Int {
    subscript(key: String, default fallback: Int) -> Int { return fallback + 1 }
}
#endif
