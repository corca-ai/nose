def flat_comp(xs, ys):
    return [x + y for x in xs for y in ys]


def nested_loop(xs, ys):
    out = []
    for x in xs:
        for y in ys:
            out.append(x + y)
    return out


def nested_list_comp(xs, ys):
    return [[x + y for y in ys] for x in xs]
