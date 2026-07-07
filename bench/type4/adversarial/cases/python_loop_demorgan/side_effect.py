def all_not_zero_or_one(xs, seen):
    return all(x != 0 and x != 1 for x in xs)


def loop_with_observed_effect(xs, seen):
    for x in xs:
        seen.append(x)
        if x == 0 or x == 1:
            return False
    return True
