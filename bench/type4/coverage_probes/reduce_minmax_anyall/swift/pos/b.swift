func f(_ xs: [Int]) -> Int {
    var s = 0
    var i = 0
    while i < xs.count {
        s += xs[i]
        i += 1
    }
    return s
}
