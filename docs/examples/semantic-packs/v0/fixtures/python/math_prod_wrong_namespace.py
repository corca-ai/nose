import functools as other


def product(values):
    return other.reduce(lambda acc, value: acc * value, values, 1)
