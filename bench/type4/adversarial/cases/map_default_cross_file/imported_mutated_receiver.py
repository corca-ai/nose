from tables import LOOKUP

LOOKUP.clear()


def lookup(key, other):
    return LOOKUP.get(key, 0)
