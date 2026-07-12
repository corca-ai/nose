func swiftFilteredFlatMapAllLoopReference(
    _ groups: [[Int]],
    _ outerGuard: Bool,
    _ innerGuard: Bool,
    _ minimum: Int
) -> Bool {
    for group in groups {
        if outerGuard {
            for value in group {
                if innerGuard && !(value >= minimum) {
                    return false
                }
            }
        }
    }
    return true
}

func swiftCustomFilter(
    _ groups: [[Int]],
    _ outerGuard: Bool,
    _ innerGuard: Bool,
    _ minimum: Int
) -> Bool {
    groups.filter { group in outerGuard }.flatMap { (group: [Int]) in
        group.filter { value in innerGuard }.map { value in value }
    }.allSatisfy { value in value >= minimum }
}
