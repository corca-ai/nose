func swiftPureCallbackMap(_ xs: [Int]) -> [Int] {
    xs.map { value in value }
}

func swiftPrefixPlusCallbackMap(_ xs: [Int]) -> [Int] {
    xs.map { value in +value }
}

func swiftForceUnwrapCallbackMap(_ xs: [Int?]) -> [Int] {
    xs.map { value in value! }
}

prefix operator +++

prefix func +++ (value: Int) -> Int {
    value
}

func swiftCustomPrefixCallbackMap(_ xs: [Int]) -> [Int] {
    xs.map { value in +++value }
}

func swiftInterpolationCallbackMap(_ xs: [Int]) -> [String] {
    xs.map { value in "\(value)" }
}

struct NoisyInteger: ExpressibleByIntegerLiteral {
    init(integerLiteral value: Int) {
        print(value)
    }
}

func swiftContextualLiteralCallbackMap(_ xs: [Int]) -> [NoisyInteger] {
    xs.map { _ in 1 }
}

enum CallbackCustomTypes {
    struct Float {
        static prefix func - (value: Float) -> Float {
            print("effect")
            return value
        }
    }
}

func swiftCustomNominalFloatCallbackMap(
    _ xs: [CallbackCustomTypes.Float]
) -> [CallbackCustomTypes.Float] {
    xs.map { (value: CallbackCustomTypes.Float) in -value }
}

func observeCapture() -> Int {
    print("capture")
    return 0
}

func swiftCaptureInitializerCallbackMap(_ xs: [Int]) -> [Int] {
    xs.map { [snapshot = observeCapture()] value in value }
}

func swiftForcedCastCallbackMap(_ xs: [Any]) -> [Int] {
    xs.map { value in value as! Int }
}
