func dictionaryDefaultExact(
    _ lookup: Dictionary<String, Int>,
    _ key: String,
    _ fallback: Int,
    _ otherLookup: Dictionary<String, Int>,
    _ otherKey: String,
    _ otherDefault: Int
) -> Int {
    return lookup[key, default: fallback]
}

func dictionaryDefaultBracket(
    _ lookup: [String: Int],
    _ key: String,
    _ fallback: Int,
    _ otherLookup: [String: Int],
    _ otherKey: String,
    _ otherDefault: Int
) -> Int {
    return lookup[key, default: fallback]
}

func dictionaryDefaultQualified(
    _ lookup: Swift.Dictionary<String, Int>,
    _ key: String,
    _ fallback: Int,
    _ otherLookup: Swift.Dictionary<String, Int>,
    _ otherKey: String,
    _ otherDefault: Int
) -> Int {
    return lookup[key, default: fallback]
}

func dictionaryDefaultWrongKey(
    _ lookup: Dictionary<String, Int>,
    _ key: String,
    _ fallback: Int,
    _ otherLookup: Dictionary<String, Int>,
    _ otherKey: String,
    _ otherDefault: Int
) -> Int {
    return lookup[otherKey, default: fallback]
}

func dictionaryDefaultWrongFallback(
    _ lookup: Dictionary<String, Int>,
    _ key: String,
    _ fallback: Int,
    _ otherLookup: Dictionary<String, Int>,
    _ otherKey: String,
    _ otherDefault: Int
) -> Int {
    return lookup[key, default: otherDefault]
}

func dictionaryDefaultWrongReceiver(
    _ lookup: Dictionary<String, Int>,
    _ key: String,
    _ fallback: Int,
    _ otherLookup: Dictionary<String, Int>,
    _ otherKey: String,
    _ otherDefault: Int
) -> Int {
    return otherLookup[key, default: fallback]
}

func dictionaryDefaultObserve() -> Int { return 7 }

func dictionaryDefaultEffectfulFallback(
    _ lookup: Dictionary<String, Int>,
    _ key: String,
    _ fallback: Int,
    _ otherLookup: Dictionary<String, Int>,
    _ otherKey: String,
    _ otherDefault: Int
) -> Int {
    return lookup[key, default: dictionaryDefaultObserve()]
}

func dictionaryDefaultHoistedFallback(
    _ lookup: Dictionary<String, Int>,
    _ key: String,
    _ fallback: Int,
    _ otherLookup: Dictionary<String, Int>,
    _ otherKey: String,
    _ otherDefault: Int
) -> Int {
    let observed = dictionaryDefaultObserve()
    return lookup[key, default: observed]
}

@propertyWrapper
struct DictionaryDefaultShifted {
    private var value: Int
    var wrappedValue: Int { value + 1 }
    init(wrappedValue: Int) { self.value = wrappedValue }
}

func dictionaryDefaultWrappedFallback(
    _ lookup: Dictionary<String, Int>,
    _ key: String,
    @DictionaryDefaultShifted _ fallback: Int,
    _ otherLookup: Dictionary<String, Int>,
    _ otherKey: String,
    _ otherDefault: Int
) -> Int {
    return lookup[key, default: fallback]
}

func dictionaryDefaultMutatedReceiver(
    _ lookup: inout Dictionary<String, Int>,
    _ key: String,
    _ fallback: Int,
    _ otherLookup: Dictionary<String, Int>,
    _ otherKey: String,
    _ otherDefault: Int
) -> Int {
    lookup[key] = otherDefault
    return lookup[key, default: fallback]
}

func dictionaryDefaultNullish(
    _ lookup: Dictionary<String, Int>,
    _ key: String,
    _ fallback: Int,
    _ otherLookup: Dictionary<String, Int>,
    _ otherKey: String,
    _ otherDefault: Int
) -> Int {
    return lookup[key] ?? fallback
}
