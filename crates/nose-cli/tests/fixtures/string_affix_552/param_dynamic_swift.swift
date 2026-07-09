func dynamicParamPrefix(_ subject: String, _ prefix: String) -> Bool {
    let normalized = prefix.trimmingCharacters(in: .whitespaces)
    return subject.hasPrefix(normalized)
}
