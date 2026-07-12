def javaFlatMapSumReference(xs, ys):
    return sum(x + y for x in xs for y in ys)


def javaFilteredFlatMapSumReference(xs, ys):
    return sum(x + y for x in xs if x > 0 for y in ys if y < 10)


def javaFlatMapSourceReference(xs, ys, other):
    return sum(x + y for x in xs for y in ys)
