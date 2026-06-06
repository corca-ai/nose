def sum_filtered_inner_changed(xs, ys):
    return sum(x + y for x in xs if x > 0 for y in ys if False)
