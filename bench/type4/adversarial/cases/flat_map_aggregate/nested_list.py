def nested_list(xs, ys):
    return sum([x + y for y in ys] for x in xs)
