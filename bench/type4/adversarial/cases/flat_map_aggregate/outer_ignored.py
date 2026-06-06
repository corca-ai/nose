def outer_ignored(xs, ys):
    return sum(y for x in xs for y in ys)
