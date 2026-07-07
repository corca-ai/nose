def all_not_zero_or_one(xs):
    return all(x != 0 and x != 1 for x in xs)


def loop_wrong_empty_truth(xs):
    for x in xs:
        if x == 0 or x == 1:
            return False
    return False
