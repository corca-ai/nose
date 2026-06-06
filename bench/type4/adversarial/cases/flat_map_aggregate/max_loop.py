def max_loop(xs, ys):
    best = 0
    for x in xs:
        for y in ys:
            v = x + y
            if v > best:
                best = v
    return best
