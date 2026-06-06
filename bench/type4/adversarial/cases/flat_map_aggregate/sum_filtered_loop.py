def sum_filtered_loop(xs, ys):
    total = 0
    for x in xs:
        if x > 0:
            for y in ys:
                if y < 10:
                    total = total + x + y
    return total
