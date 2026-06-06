def any_filtered_flat(xs, ys):
    return any(x + y > 0 for x in xs if x > 0 for y in ys if y < 10)
