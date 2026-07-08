import functools


def py_max_loop(xs):
    best = 0
    for x in xs:
        if x > best:
            best = x
    return best


def py_min_loop(xs):
    best = 0
    for x in xs:
        if x < best:
            best = x
    return best


def py_min_reduce(xs):
    return functools.reduce(lambda best, x: x if x < best else best, xs, 0)


def py_changed_selection_direction(xs):
    best = 0
    for x in xs:
        if x < best:
            best = x
    return best
