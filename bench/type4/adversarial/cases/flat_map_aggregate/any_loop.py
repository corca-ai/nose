def any_loop(xs, ys):
    for x in xs:
        for y in ys:
            if x + y > 0:
                return True
    return False
