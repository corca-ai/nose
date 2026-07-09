struct Values {
    func contains(_ value: String) -> Bool {
        return false
    }
}

func swiftCustomContains(_ value: String, _ other: String) -> Bool {
    let values = Values()
    return values.contains(value)
}
