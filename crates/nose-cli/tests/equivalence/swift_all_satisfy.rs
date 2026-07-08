use super::*;

fn assert_swift_named_eq(src: &str, left: &str, right: &str, message: &str) {
    let i = Interner::new();
    assert_eq!(
        value_fp_named(&i, src, Lang::Swift, left),
        value_fp_named(&i, src, Lang::Swift, right),
        "{message}"
    );
}

fn assert_swift_named_ne(src: &str, left: &str, right: &str, message: &str) {
    let i = Interner::new();
    assert_ne!(
        value_fp_named(&i, src, Lang::Swift, left),
        value_fp_named(&i, src, Lang::Swift, right),
        "{message}"
    );
}

#[test]
fn swift_all_satisfy_converges_with_counterexample_loop() {
    let positive = r#"
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
"#;

    assert_swift_named_eq(
        positive,
        "swiftAllLoop",
        "swiftAllSatisfy",
        "Swift allSatisfy should converge with the same-source counterexample loop",
    );
    assert_swift_named_eq(
        positive,
        "swiftAllEmptyLoop",
        "swiftAllEmptySatisfy",
        "Swift allSatisfy should preserve universal vacuous truth",
    );
}

#[test]
fn swift_all_satisfy_keeps_predicate_source_and_empty_truth_boundaries() {
    let changed_predicate = r#"
func swiftAllLoop(_ xs: [Int], _ min: Int) -> Bool {
    for x in xs {
        if !(x >= min) {
            return false
        }
    }
    return true
}

func swiftAllChangedPredicate(_ xs: [Int], _ min: Int) -> Bool {
    return xs.allSatisfy { x in x > min }
}
"#;
    assert_swift_named_ne(
        changed_predicate,
        "swiftAllLoop",
        "swiftAllChangedPredicate",
        "changing the Swift allSatisfy predicate must stay distinct",
    );

    let different_source = r#"
func swiftAllLoop(_ xs: [Int], _ ys: [Int], _ min: Int) -> Bool {
    for x in xs {
        if !(x >= min) {
            return false
        }
    }
    return true
}

func swiftAllDifferentSource(_ xs: [Int], _ ys: [Int], _ min: Int) -> Bool {
    return ys.allSatisfy { y in y >= min }
}
"#;
    assert_swift_named_ne(
        different_source,
        "swiftAllLoop",
        "swiftAllDifferentSource",
        "Swift allSatisfy and loop admission requires the same source identity",
    );

    let wrong_empty_truth = r#"
func swiftAllEmptySatisfy() -> Bool {
    let xs: [Int] = []
    return xs.allSatisfy { x in x >= 0 }
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
"#;
    assert_swift_named_ne(
        wrong_empty_truth,
        "swiftAllEmptySatisfy",
        "swiftAllWrongEmptyTruth",
        "changing Swift allSatisfy's empty-input truth must stay distinct",
    );
}

#[test]
fn swift_all_satisfy_keeps_effect_and_lazy_boundaries() {
    let callback_effect = r#"
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
"#;
    assert_swift_named_ne(
        callback_effect,
        "swiftAllPure",
        "swiftAllCallbackEffect",
        "Swift allSatisfy callbacks with observed effects stay outside the admitted perimeter",
    );
    assert_swift_named_ne(
        callback_effect,
        "swiftAllPure",
        "swiftAllLoopEffect",
        "Swift loop-side effects stay outside allSatisfy admission",
    );

    let lazy_boundary = r#"
func swiftAllLoop(_ xs: [Int]) -> Bool {
    for x in xs {
        if !(x >= 0) {
            return false
        }
    }
    return true
}

func swiftAllLazy(_ xs: [Int]) -> Bool {
    return xs.lazy.allSatisfy { x in x >= 0 }
}
"#;
    assert_swift_named_ne(
        lazy_boundary,
        "swiftAllLoop",
        "swiftAllLazy",
        "Swift lazy allSatisfy stays closed until lazy demand semantics are modeled",
    );
}

#[test]
fn swift_all_satisfy_keeps_custom_overload_callback_shape_boundary() {
    let custom_overload = r#"
extension Array {
    func allSatisfy(_ predicate: (Element, Int) -> Bool) -> Bool {
        return false
    }
}

func swiftAllLoop(_ xs: [Int]) -> Bool {
    for x in xs {
        if !(x >= 0) {
            return false
        }
    }
    return true
}

func swiftAllCustomOverload(_ xs: [Int]) -> Bool {
    return xs.allSatisfy { x, i in x >= 0 }
}
"#;
    assert_swift_named_ne(
        custom_overload,
        "swiftAllLoop",
        "swiftAllCustomOverload",
        "Swift allSatisfy admission must not treat a two-argument custom overload as the stdlib Sequence quantifier",
    );
}
