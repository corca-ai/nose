def py_sum_loop(xs):
    total = 0
    for x in xs:
        total += x
    return total


def py_sum_builtin(xs):
    return sum(xs)


def py_product_loop(xs):
    product = 1
    for x in xs:
        product *= x
    return product


def py_wrong_seed(xs):
    total = 1
    for x in xs:
        total += x
    return total
