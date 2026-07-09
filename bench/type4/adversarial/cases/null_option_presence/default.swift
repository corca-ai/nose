func swiftCoalesce(_ value: Int?, _ fallback: Int, _ other: Int?, _ otherDefault: Int) -> Int {
    return value ?? fallback
}

func swiftWrongFallback(_ value: Int?, _ fallback: Int, _ other: Int?, _ otherDefault: Int) -> Int {
    return value ?? otherDefault
}

func swiftWrongValue(_ value: Int?, _ fallback: Int, _ other: Int?, _ otherDefault: Int) -> Int {
    return other ?? fallback
}

func expensive() -> Int {
    return 1
}

func swiftEffectfulFallback(_ value: Int?, _ fallback: Int, _ other: Int?) -> Int {
    return value ?? expensive()
}
