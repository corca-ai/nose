func axisCase(_ xs: [Int]) -> [Int] {
    return xs.filter { x in (x > 0) || (x < 10) }
}
