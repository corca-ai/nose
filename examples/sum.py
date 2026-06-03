def sum_while(items):
    total = 0
    i = 0
    while i < len(items):
        total += items[i]
        i = i + 1
    return total


def sum_for(numbers):
    acc = 0
    for n in numbers:
        acc = acc + n
    return acc


class Calc:
    def add(self, a, b):
        if a > b:
            return a + b
        else:
            return b + a
