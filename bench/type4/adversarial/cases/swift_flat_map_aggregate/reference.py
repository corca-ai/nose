def swiftFlatMapAllReference(groups, minimum):
    return all(value >= minimum for group in groups for value in group)


def swiftFilteredFlatMapAllReference(groups, outer_guard, inner_guard, minimum):
    return all(
        value >= minimum
        for group in groups
        if outer_guard
        for value in group
        if inner_guard
    )


def swiftFlatMapSourceReference(groups, other, minimum):
    return all(value >= minimum for group in groups for value in group)
