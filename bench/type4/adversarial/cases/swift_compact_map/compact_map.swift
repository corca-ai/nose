func compactMapExact(_ xs: [Bool]) -> [Bool] {
    return xs.compactMap { value in value ? value : nil }
}

func compactMapChangedValue(_ xs: [Bool], _ other: Bool) -> [Bool] {
    return xs.compactMap { value in value ? other : nil }
}

func compactMapChangedDrop(_ xs: [Bool], _ other: Bool) -> [Bool] {
    return xs.compactMap { value in other ? value : nil }
}

func compactMapWrongSource(_ xs: [Bool], _ other: [Bool]) -> [Bool] {
    return other.compactMap { value in value ? value : nil }
}

func compactMapWrongOptionalChannel(_ xs: [Bool]) -> [Bool?] {
    return xs.map { value in value ? value : nil }
}

func compactMapObserve(_ value: Bool) {}

func compactMapEffectful(_ xs: [Bool]) -> [Bool] {
    return xs.compactMap { value in
        compactMapObserve(value)
        return value ? value : nil
    }
}

func compactMapOptionalEmittedValue(_ xs: [Bool], _ maybe: Bool?) -> [Bool] {
    return xs.compactMap { value in value ? maybe : nil }
}

func compactMapDerivedSource(_ xs: [Bool], _ flag: Bool) -> [Bool] {
    return xs.map { _ in flag }.compactMap { value in value ? value : nil }
}
