def square_plus_one(value: int) -> int:
    return value * value + 1


def axis_case(x: int, y: int) -> int:
    total = x + y
    return square_plus_one(total)
