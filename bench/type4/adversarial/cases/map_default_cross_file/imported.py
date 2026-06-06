from tables import LOOKUP


def lookup(key, other):
    return LOOKUP.get(key, 0)
