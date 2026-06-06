def sum_filtered_outer_changed(xs, ys):
    return sum(x + y for x in xs if False for y in ys if y < 10)
