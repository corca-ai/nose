func axisCase(_ groups: [[Int]]) -> [Int] {
    return groups.flatMap { (xs: [Int]) in xs.map { y in y } }
}
