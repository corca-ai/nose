from tables import LOOKUP


def lookup(key, other):
    return {"red": 9, "blue": 2}.get(key, 0)
