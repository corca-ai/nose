@dynamicMemberLookup
struct Collection {
    subscript(dynamicMember name: String) -> (((Bool) -> Bool?) -> [Bool]) {
        { _ in [] }
    }
}

func compactMapCustomNominal(_ xs: Collection) -> [Bool] {
    return xs.compactMap { value in value ? value : nil }
}
