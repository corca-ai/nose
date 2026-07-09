struct SwiftEmptyBox {
    var isEmpty: Bool {
        return false
    }
}

func swiftCustomEmpty(_ value: SwiftEmptyBox, _ other: SwiftEmptyBox) -> Bool {
    return value.isEmpty
}
