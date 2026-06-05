def f(items):
    acc = 0
    for v in items:
        if v > 0:
            acc = acc + v * v
    return acc
