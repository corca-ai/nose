func flatMapCustomInnerMap(_ groups: [[Bool]]) -> [Bool] {
    groups.flatMap { (group: [Bool]) in group.map { value in value } }
}
