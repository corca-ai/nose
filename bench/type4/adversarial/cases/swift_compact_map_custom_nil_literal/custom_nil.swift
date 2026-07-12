struct CompactMapNilish: ExpressibleByNilLiteral {
    let tag: Int

    init(nilLiteral: ()) {
        tag = 99
    }
}

func compactMapCustomNilLiteral(
    _ xs: [Bool],
    _ value: CompactMapNilish
) -> [CompactMapNilish] {
    return xs.compactMap { flag in flag ? value : nil }
}
