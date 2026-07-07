def all_with_helper_call(xs, seen):
    return all(is_allowed(x, seen) for x in xs)


def loop_no_zero_or_one(xs, seen):
    for x in xs:
        if x == 0 or x == 1:
            return False
    return True


def is_allowed(x, seen):
    seen.append(x)
    return x != 0 and x != 1
