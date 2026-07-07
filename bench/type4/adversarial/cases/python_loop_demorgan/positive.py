def all_not_zero_or_one(xs):
    return all(x != 0 and x != 1 for x in xs)


def loop_no_zero_or_one(xs):
    for x in xs:
        if x == 0 or x == 1:
            return False
    return True
