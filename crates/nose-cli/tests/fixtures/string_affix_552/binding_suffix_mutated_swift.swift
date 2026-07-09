func mutatedBindingSuffix(_ subject: String) -> Bool {
    var suffix = "pre"
    suffix = "other"
    return subject.hasSuffix(suffix)
}
