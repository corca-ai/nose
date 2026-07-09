struct PrefixBox {
    func hasPrefix(_ prefix: String) -> Bool {
        return prefix.count > 0
    }
}

func swiftCustomSameName(_ value: PrefixBox, _ other: PrefixBox) -> Bool {
    return value.hasPrefix("pre")
}
