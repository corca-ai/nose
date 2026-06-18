func squarePlusOne(_ value: Int) -> Int {
    return value * value + 1
}

func axisCase(_ x: Int, _ y: Int) -> Int {
    let total = x + y
    return squarePlusOne(total)
}
