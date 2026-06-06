def any_filtered_loop(xs, ys):
    for x in xs:
        if x > 0:
            for y in ys:
                if y < 10 and x + y > 0:
                    return True
    return False
