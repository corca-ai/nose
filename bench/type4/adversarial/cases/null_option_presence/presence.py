def py_missing(value, other):
    return value is None


def py_present(value, other):
    return value is not None


def py_wrong_value(value, other):
    return other is None
