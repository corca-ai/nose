func mutatedBindingPrefix(_ subject: String) -> Bool {
    var prefix = "pre"
    prefix = "other"
    return subject.hasPrefix(prefix)
}
