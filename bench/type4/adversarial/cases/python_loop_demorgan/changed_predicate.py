def all_not_zero_or_one(xs):
    return all(x != 0 and x != 1 for x in xs)


def all_changed_predicate(xs):
    return all(x != 0 or x != 1 for x in xs)
