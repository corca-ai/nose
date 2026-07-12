def flatMapReference(groups):
    return [value for group in groups for value in group]


def flatMapCrossReference(xs, ys):
    return [(x, y) for x in xs for y in ys]


def flatMapOneLevelRowsReference(groups):
    return [row for rows in groups for row in rows]
