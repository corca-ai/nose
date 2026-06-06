def clamp_minmax_guarded(x: int, lo: int, hi: int):
    if hi < lo:
        raise 0
    return min(max(x, lo), hi)


def clamp_ternary_guarded(x: int, lo: int, hi: int):
    if hi < lo:
        raise 0
    return lo if x < lo else (hi if x > hi else x)


def clamp_upper_first_guarded(x: int, lo: int, hi: int):
    if hi < lo:
        raise 0
    return hi if x > hi else (lo if x < lo else x)


def clamp_ternary_unproven(x: int, lo: int, hi: int):
    return lo if x < lo else (hi if x > hi else x)


def clamp_ternary_swapped_bounds(x: int, lo: int, hi: int):
    if hi < lo:
        raise 0
    return hi if x < hi else (lo if x > lo else x)


def clamp_ternary_float(x: float, lo: float, hi: float):
    if hi < lo:
        raise 0
    return lo if x < lo else (hi if x > hi else x)


def clamp_literal_minmax(x: int):
    return min(max(x, 0), 10)
