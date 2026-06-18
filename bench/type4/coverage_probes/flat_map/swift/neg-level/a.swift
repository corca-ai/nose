func axisCase(_ groups: [[Int]]) -> [Int] {
    var out: [Int] = []
    for xs in groups {
        for y in xs {
            out.append(y)
        }
    }
    return out
}
