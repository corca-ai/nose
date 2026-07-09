struct Box: ExpressibleByNilLiteral {
    init(nilLiteral: ()) {}
}

func ==(lhs: Box, rhs: Box) -> Bool {
    return false
}

func swiftCustomNil(_ value: Box, _ other: Box) -> Bool {
    return value == nil
}

struct OptionalBox {}

func ==(lhs: OptionalBox?, rhs: OptionalBox?) -> Bool {
    return false
}

func swiftOptionalOverloadedNil(_ value: OptionalBox?, _ other: OptionalBox?) -> Bool {
    return value == nil
}
