def any_wrong_predicate(xs, ys):
    return any(x + y < 0 for x in xs for y in ys)
