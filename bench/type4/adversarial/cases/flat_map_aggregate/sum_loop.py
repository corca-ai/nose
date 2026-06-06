def sum_loop(xs, ys):
    total = 0
    for x in xs:
        for y in ys:
            total = total + x + y
    return total
