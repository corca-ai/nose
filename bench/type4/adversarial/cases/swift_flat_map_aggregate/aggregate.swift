func swiftFlatMapAllLoop(_ groups: [[Int]], _ minimum: Int) -> Bool {
    for group in groups {
        for value in group {
            if !(value >= minimum) {
                return false
            }
        }
    }
    return true
}

func swiftFlatMapAll(_ groups: [[Int]], _ minimum: Int) -> Bool {
    groups.flatMap { (group: [Int]) in group.map { value in value } }
        .allSatisfy { value in value >= minimum }
}

func swiftFilteredFlatMapAllLoop(
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

func swiftFilteredFlatMapAll(
    _ groups: [[Int]],
    _ outerGuard: Bool,
    _ innerGuard: Bool,
    _ minimum: Int
) -> Bool {
    groups.filter { group in outerGuard }.flatMap { (group: [Int]) in
        group.filter { value in innerGuard }.map { value in value }
    }.allSatisfy { value in value >= minimum }
}

func swiftWrongOuterGuard(
    _ groups: [[Int]],
    _ outerGuard: Bool,
    _ innerGuard: Bool,
    _ minimum: Int
) -> Bool {
    groups.filter { group in !outerGuard }.flatMap { (group: [Int]) in
        group.filter { value in innerGuard }.map { value in value }
    }.allSatisfy { value in value >= minimum }
}

func swiftWrongInnerGuard(
    _ groups: [[Int]],
    _ outerGuard: Bool,
    _ innerGuard: Bool,
    _ minimum: Int
) -> Bool {
    groups.filter { group in outerGuard }.flatMap { (group: [Int]) in
        group.filter { value in !innerGuard }.map { value in value }
    }.allSatisfy { value in value >= minimum }
}

func swiftWrongTerminal(
    _ groups: [[Int]],
    _ outerGuard: Bool,
    _ innerGuard: Bool,
    _ minimum: Int
) -> Bool {
    groups.filter { group in outerGuard }.flatMap { (group: [Int]) in
        group.filter { value in innerGuard }.map { value in value }
    }.allSatisfy { value in value > minimum }
}

func swiftFlatMapSource(
    _ groups: [[Int]],
    _ other: [[Int]],
    _ minimum: Int
) -> Bool {
    groups.flatMap { (group: [Int]) in group.map { value in value } }
        .allSatisfy { value in value >= minimum }
}

func swiftWrongSource(
    _ groups: [[Int]],
    _ other: [[Int]],
    _ minimum: Int
) -> Bool {
    other.flatMap { (group: [Int]) in group.map { value in value } }
        .allSatisfy { value in value >= minimum }
}

func swiftIgnoredInnerSourceYs(
    _ xs: [Int],
    _ ys: [Int],
    _ other: [Int],
    _ minimum: Int
) -> Bool {
    xs.flatMap { (x: Int) in ys.map { y in x } }
        .allSatisfy { value in value >= minimum }
}

func swiftIgnoredInnerSourceOther(
    _ xs: [Int],
    _ ys: [Int],
    _ other: [Int],
    _ minimum: Int
) -> Bool {
    xs.flatMap { (x: Int) in other.map { y in x } }
        .allSatisfy { value in value >= minimum }
}

func swiftIgnoredTerminalGroups(
    _ groups: [[Int]],
    _ other: [[Int]]
) -> Bool {
    groups.flatMap { (group: [Int]) in group.map { value in value } }
        .allSatisfy { value in false }
}

func swiftIgnoredTerminalOther(
    _ groups: [[Int]],
    _ other: [[Int]]
) -> Bool {
    other.flatMap { (group: [Int]) in group.map { value in value } }
        .allSatisfy { value in false }
}

func swiftWrongDepth(_ groups: [[Int]], _ minimum: Int) -> Bool {
    groups.map { (group: [Int]) in group.count }
        .allSatisfy { count in count >= minimum }
}

func swiftAggregateObserve(_ value: Int) {}

func swiftEffectfulCallback(_ groups: [[Int]], _ minimum: Int) -> Bool {
    groups.flatMap { (group: [Int]) in
        group.map { value in
            swiftAggregateObserve(value)
            return value
        }
    }.allSatisfy { value in value >= minimum }
}
