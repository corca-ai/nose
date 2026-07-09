struct DefaultBox {}

func ??(lhs: DefaultBox, rhs: Int) -> Int {
    return rhs
}

func swiftCustomCoalesce(_ value: DefaultBox, _ fallback: Int, _ other: DefaultBox) -> Int {
    return value ?? fallback
}

struct OptionalDefaultBox {}

func ??(lhs: OptionalDefaultBox?, rhs: Int) -> Int {
    return rhs + 1
}

func swiftOptionalCustomCoalesce(
    _ value: OptionalDefaultBox?,
    _ fallback: Int,
    _ other: OptionalDefaultBox?
) -> Int {
    return value ?? fallback
}

struct DefaultSource {
    var fallback: Int {
        return 1
    }
}

func swiftComputedPropertyFallback(_ value: Int?, _ source: DefaultSource, _ other: Int?) -> Int {
    return value ?? source.fallback
}
