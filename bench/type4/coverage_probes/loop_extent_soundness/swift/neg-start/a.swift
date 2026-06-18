func axisCase(_ xs: [Int]) -> Int {
    var total = 0
    var i = 0
    while i < xs.count {
        total += xs[i]
        i += 1
    }
    return total
}
