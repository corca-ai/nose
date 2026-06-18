func axisCase(_ groups: [[Int]]) -> [[Int]] {
    return groups.map { (xs: [Int]) in xs.map { y in y } }
}
