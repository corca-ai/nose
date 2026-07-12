typealias NilProtocol = ExpressibleByNilLiteral

extension Bool: @retroactive NilProtocol {
    public init(nilLiteral: ()) {
        self = true
    }
}
