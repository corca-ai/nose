func swiftMutatedArrayMember(_ value: String, _ other: String) -> Bool {
    var values = ["red", "blue"]
    values.append("green")
    return values.contains(value)
}
