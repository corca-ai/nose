func swiftCountEmpty(_ items: [Int], _ other: [Int]) -> Bool {
    return items.count == 0
}

func swiftNamedEmpty(_ values: [Int], _ other: [Int]) -> Bool {
    return values.isEmpty
}

func swiftCountNonempty(_ items: [Int], _ other: [Int]) -> Bool {
    return items.count != 0
}

func swiftNamedNonempty(_ values: [Int], _ other: [Int]) -> Bool {
    return !values.isEmpty
}
