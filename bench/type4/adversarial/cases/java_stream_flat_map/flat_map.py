def flat_map_py(xs, ys):
    return [x + y for x in xs for y in ys]


def nested_map_py(xs, ys):
    return [[x + y for y in ys] for x in xs]
