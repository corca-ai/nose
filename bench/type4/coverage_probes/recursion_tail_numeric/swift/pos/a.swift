func sumDown(_ n: Int, _ acc: Int) -> Int {
    if n == 0 { return acc }
    return sumDown(n - 1, acc + n)
}
