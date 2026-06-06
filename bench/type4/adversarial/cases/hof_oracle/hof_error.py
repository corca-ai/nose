def map_error(xs):
    return [1 / 0 for x in xs]


def map_error_loop(xs):
    out = []
    for x in xs:
        out.append(1 / 0)
    return out


def empty_flat_map(xs):
    return [1 / 0 for x in [] for y in xs]
