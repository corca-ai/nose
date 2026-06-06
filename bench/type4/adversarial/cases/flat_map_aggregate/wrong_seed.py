def wrong_seed(xs, ys):
    total = 1
    for x in xs:
        for y in ys:
            total = total + x + y
    return total
