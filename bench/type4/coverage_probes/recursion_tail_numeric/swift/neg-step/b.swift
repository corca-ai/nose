func sumDown(_ n: Int, _ acc: Int) -> Int {
    var i = n
    var total = acc
    while i != 0 {
        total = total + i
        i = i - 1
    }
    return total
}
