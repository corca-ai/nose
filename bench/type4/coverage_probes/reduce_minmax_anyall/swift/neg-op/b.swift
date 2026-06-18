func f(_ xs: [Int]) -> Int {
    var p = 1
    for x in xs {
        p *= x
    }
    return p
}
