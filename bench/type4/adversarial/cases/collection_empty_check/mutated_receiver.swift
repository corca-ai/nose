func swiftMutatedEmpty(_ items: [Int], _ other: [Int]) -> Bool {
    var current = items
    current.append(1)
    return current.isEmpty
}
