struct Dictionary<Key: Hashable, Value> {
    subscript(key: Key, default fallback: Value) -> Value { return fallback }
}
