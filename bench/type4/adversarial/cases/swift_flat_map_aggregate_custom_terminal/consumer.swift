func swiftFlatMapAllLoopReference(_ groups: [[Int]], _ minimum: Int) -> Bool {
    for group in groups {
        for value in group {
            if !(value >= minimum) {
                return false
            }
        }
    }
    return true
}

func swiftCustomTerminal(_ groups: [[Int]], _ minimum: Int) -> Bool {
    groups.flatMap { (group: [Int]) in group.map { value in value } }
        .allSatisfy { value in value >= minimum }
}
