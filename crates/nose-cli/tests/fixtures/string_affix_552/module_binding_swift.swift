let SWIFT_PREFIX = "pre"

func moduleBindingPrefix(_ subject: String) -> Bool {
    return subject.hasPrefix(SWIFT_PREFIX)
}
