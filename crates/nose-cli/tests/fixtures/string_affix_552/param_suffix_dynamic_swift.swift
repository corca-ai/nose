func dynamicParamSuffix(_ subject: String, _ suffix: String) -> Bool {
    let normalized = suffix.trimmingCharacters(in: .whitespaces)
    return subject.hasSuffix(normalized)
}
