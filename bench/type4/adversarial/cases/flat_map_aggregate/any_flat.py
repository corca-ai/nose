def any_flat(xs, ys):
    return any(x + y > 0 for x in xs for y in ys)
