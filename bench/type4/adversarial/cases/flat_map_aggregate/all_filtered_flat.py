def all_filtered_flat(xs, ys):
    return all(x + y > 0 for x in xs if x > 0 for y in ys if y < 10)
