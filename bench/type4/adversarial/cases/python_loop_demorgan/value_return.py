def boolean_demorgan_predicate(x):
    return x != 0 and x != 1


def value_returning_operand(x):
    return x != 0 and marker(x)


def marker(x):
    return x
