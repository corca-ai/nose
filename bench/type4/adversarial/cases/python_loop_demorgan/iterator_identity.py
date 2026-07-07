def all_not_zero_or_one(xs, ys):
    return all(x != 0 and x != 1 for x in xs)


def loop_different_iterable(xs, ys):
    for x in ys:
        if x == 0 or x == 1:
            return False
    return True
