func swiftAllLoop(_ xs: [Int], _ min: Int) -> Bool {
    for x in xs {
        if !(x >= min) {
            return false
        }
    }
    return true
}

func swiftAllSatisfy(_ xs: [Int], _ min: Int) -> Bool {
    return xs.allSatisfy { x in x >= min }
}

func swiftAllEmptyLoop() -> Bool {
    let xs: [Int] = []
    for x in xs {
        if !(x >= 0) {
            return false
        }
    }
    return true
}

func swiftAllEmptySatisfy() -> Bool {
    let xs: [Int] = []
    return xs.allSatisfy { x in x >= 0 }
}

func swiftAllChangedPredicate(_ xs: [Int], _ min: Int) -> Bool {
    return xs.allSatisfy { x in x > min }
}

func swiftAllDifferentSource(_ xs: [Int], _ ys: [Int], _ min: Int) -> Bool {
    return ys.allSatisfy { y in y >= min }
}

func swiftAllWrongEmptyTruth() -> Bool {
    let xs: [Int] = []
    for x in xs {
        if !(x >= 0) {
            return false
        }
    }
    return false
}

func swiftAllPure(_ xs: [Int]) -> Bool {
    return xs.allSatisfy { x in x >= 0 }
}

func swiftAllCallbackEffect(_ xs: [Int]) -> Bool {
    return xs.allSatisfy { x in
        record(x)
        return x >= 0
    }
}

func swiftAllLoopEffect(_ xs: [Int]) -> Bool {
    for x in xs {
        if !(x >= 0) {
            record(x)
            return false
        }
    }
    return true
}

func swiftAllLazy(_ xs: [Int]) -> Bool {
    return xs.lazy.allSatisfy { x in x >= 0 }
}

extension Array {
    func allSatisfy(_ predicate: (Element, Int) -> Bool) -> Bool {
        return false
    }
}

func swiftAllCustomOverload(_ xs: [Int]) -> Bool {
    return xs.allSatisfy { x, i in x >= 0 }
}
