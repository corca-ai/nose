func swiftMissing(_ value: Int?, _ other: Int?) -> Bool {
    return value == nil
}

func swiftPresent(_ value: Int?, _ other: Int?) -> Bool {
    return value != nil
}

func swiftWrongValue(_ value: Int?, _ other: Int?) -> Bool {
    return other == nil
}

func swiftRebound(_ original: Int?, _ other: Int?) -> Bool {
    var value = original
    value = other
    return value == nil
}
