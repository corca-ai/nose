def clamp_minmax_guarded(x: int, lo: int, hi: int):
    if hi < lo:
        raise 0
    return min(max(x, lo), hi)


def clamp_maxmin_guarded(x: int, lo: int, hi: int):
    if hi < lo:
        raise 0
    return max(min(x, hi), lo)


def clamp_minmax_unproven(x: int, lo: int, hi: int):
    return min(max(x, lo), hi)


def clamp_maxmin_unproven(x: int, lo: int, hi: int):
    return max(min(x, hi), lo)


def clamp_swapped_bounds(x: int, lo: int, hi: int):
    if hi < lo:
        raise 0
    return min(max(x, hi), lo)


def clamp_float_domain(x: float, lo: float, hi: float):
    if hi < lo:
        raise 0
    return min(max(x, lo), hi)


def clamp_float_domain_maxmin(x: float, lo: float, hi: float):
    if hi < lo:
        raise 0
    return max(min(x, hi), lo)
