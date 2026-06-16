def aggregate(items, seed):
    acc = seed
    kept = []
    for it in items:
        v = transform(it)
        g = grade(v)
        if g is not None:
            acc = acc + g
            kept.append(v)
    return acc, kept
