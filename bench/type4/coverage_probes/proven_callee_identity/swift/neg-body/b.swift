func finish(_ input: Int) -> Int {
    let doubled = input * 2
    return doubled + 4
}

func axisCase(_ value: Int) -> Int {
    let shifted = value + 1
    return finish(shifted)
}
