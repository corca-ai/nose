func flatMapExact(_ groups: [[Bool]]) -> [Bool] {
    groups.flatMap { (group: [Bool]) in group.map { value in value } }
}

func flatMapIdentity(_ groups: [[Bool]]) -> [Bool] {
    groups.flatMap { (group: [Bool]) in group }
}

func flatMapZeroDepth(_ groups: [[Bool]]) -> [[Bool]] {
    groups.map { (group: [Bool]) in group.map { value in value } }
}

func flatMapDerivedOuter(_ groups: [[Bool]]) -> [Bool] {
    groups.map { group in group }.flatMap { (group: [Bool]) in
        group.map { value in value }
    }
}

func flatMapDerivedInner(_ groups: [[Bool]]) -> [Bool] {
    groups.flatMap { (group: [Bool]) in
        group.filter { value in value }.map { value in value }
    }
}

func flatMapScalarResult(_ values: [Bool]) -> [Bool] {
    values.flatMap { value in value }
}

func flatMapObserve(_ value: Bool) {}

func flatMapEffectful(_ groups: [[Bool]]) -> [Bool] {
    groups.flatMap { (group: [Bool]) in
        group.map { value in
            flatMapObserve(value)
            return value
        }
    }
}

func flatMapCrossExact(_ xs: [Bool], _ ys: [Bool]) -> [(Bool, Bool)] {
    xs.flatMap { x in ys.map { y in (x, y) } }
}

func flatMapReordered(_ xs: [Bool], _ ys: [Bool]) -> [(Bool, Bool)] {
    ys.flatMap { y in xs.map { x in (x, y) } }
}

func flatMapChangedValue(_ xs: [Bool], _ ys: [Bool]) -> [(Bool, Bool)] {
    xs.flatMap { x in ys.map { y in (y, x) } }
}

func flatMapWrongSource(
    _ xs: [Bool],
    _ ys: [Bool],
    _ other: [Bool]
) -> [(Bool, Bool)] {
    xs.flatMap { x in other.map { y in (x, y) } }
}

func flatMapRecursiveDepth(_ groups: [[[Bool]]]) -> [Bool] {
    groups.flatMap { (rows: [[Bool]]) in
        rows.flatMap { (row: [Bool]) in row.map { value in value } }
    }
}
