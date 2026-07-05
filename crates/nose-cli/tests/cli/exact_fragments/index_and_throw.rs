use super::*;

#[test]
fn semantic_query_reports_exact_safe_ordered_index_assignment_branch_fragments_for_go() {
    let fixtures = [
        (
            "index_pair_a.go",
            "package p\n\nfunc indexPairLeft(flag bool, out []int, xs []int) {\n    if flag {\n        out[0] = xs[0] + 1\n        out[1] = xs[0] + 2\n    }\n    audit(\"opaque\")\n}\n",
        ),
        (
            "index_pair_b.go",
            "package p\n\nfunc indexPairRight(enabled bool, dst []int, ys []int) {\n    if enabled {\n        dst[0] = 1 + ys[0]\n        dst[1] = 2 + ys[0]\n    }\n    trace(\"opaque\")\n}\n",
        ),
        (
            "index_pair_wrong_order.go",
            "package p\n\nfunc indexPairWrongOrder(flag bool, out []int, xs []int) {\n    if flag {\n        out[1] = xs[0] + 2\n        out[0] = xs[0] + 1\n    }\n    audit(\"opaque\")\n}\n",
        ),
        (
            "index_pair_wrong_receiver.go",
            "package p\n\nfunc indexPairWrongReceiver(flag bool, out []int, other []int, xs []int) {\n    if flag {\n        out[0] = xs[0] + 1\n        other[1] = xs[0] + 2\n    }\n    audit(\"opaque\")\n}\n",
        ),
        (
            "index_pair_mutated.go",
            "package p\n\nfunc indexPairMutated(flag bool, out []int, xs []int) {\n    out[0] = 0\n    if flag {\n        out[0] = xs[0] + 1\n        out[1] = xs[0] + 2\n    }\n    audit(\"opaque\")\n}\n",
        ),
        (
            "index_temp_pair_a.go",
            "package p\n\nfunc indexTempPairLeft(flag bool, out []int, xs []int) {\n    if flag {\n        value := xs[0] + 1\n        out[0] = value * value\n        out[1] = xs[1] + 2\n    }\n    audit(\"opaque\")\n}\n",
        ),
        (
            "index_temp_pair_b.go",
            "package p\n\nfunc indexTempPairRight(enabled bool, dst []int, ys []int) {\n    if enabled {\n        dst[0] = (1 + ys[0]) * (1 + ys[0])\n        dst[1] = ys[1] + 2\n    }\n    trace(\"opaque\")\n}\n",
        ),
        (
            "index_temp_pair_wrong.go",
            "package p\n\nfunc indexTempPairWrong(flag bool, out []int, xs []int) {\n    if flag {\n        value := xs[0] + 3\n        out[0] = value * value\n        out[1] = xs[1] + 2\n    }\n    audit(\"opaque\")\n}\n",
        ),
        (
            "index_chain_pair_a.go",
            "package p\n\nfunc indexChainPairLeft(flag bool, out []int, xs []int) {\n    if flag {\n        base := xs[0] + 1\n        slot := base * base\n        out[slot] = xs[1] + 3\n        out[1] = xs[0] + 2\n    }\n    audit(\"opaque\")\n}\n",
        ),
        (
            "index_chain_pair_b.go",
            "package p\n\nfunc indexChainPairRight(enabled bool, dst []int, ys []int) {\n    if enabled {\n        dst[(1 + ys[0]) * (1 + ys[0])] = ys[1] + 3\n        dst[1] = ys[0] + 2\n    }\n    trace(\"opaque\")\n}\n",
        ),
        (
            "index_chain_pair_wrong.go",
            "package p\n\nfunc indexChainPairWrong(flag bool, out []int, xs []int) {\n    if flag {\n        base := xs[0] + 1\n        slot := base + base\n        out[slot] = xs[1] + 3\n        out[1] = xs[0] + 2\n    }\n    audit(\"opaque\")\n}\n",
        ),
        (
            "index_chain_pair_uses_prior.go",
            "package p\n\nfunc indexChainPairUsesPrior(flag bool, out []int, xs []int) {\n    if flag {\n        base := xs[0] + 1\n        slot := base * base\n        out[slot + base] = xs[1] + 3\n        out[1] = xs[0] + 2\n    }\n    audit(\"opaque\")\n}\n",
        ),
        (
            "index_pair_dynamic_js.js",
            "function indexPairDynamicJs(flag, out, xs) {\n  if (flag) {\n    out[0] = xs[0] + 1;\n    out[1] = xs[0] + 2;\n  }\n  audit(/opaque/);\n}\n",
        ),
    ];
    let (dir, out, families) =
        query_fragment_only_fixtures("nose_index_effect_order_boundary", &fixtures);

    for (left, right, negative) in [
        (
            "index_pair_a.go",
            "index_pair_b.go",
            "index_pair_wrong_order.go",
        ),
        (
            "index_temp_pair_a.go",
            "index_temp_pair_b.go",
            "index_temp_pair_wrong.go",
        ),
        (
            "index_chain_pair_a.go",
            "index_chain_pair_b.go",
            "index_chain_pair_wrong.go",
        ),
    ] {
        let family =
            find_block_pair_family(&families, left, right, negative).unwrap_or_else(|| {
                panic!("missing ordered index-assignment branch family {left}/{right}: {out}")
            });
        assert!(
            family_all_blocks(family),
            "ordered index-assignment branch fragments should report as Block units: {family:?}"
        );
    }

    let assert_no_merge = |left: &str, right: &str| {
        assert!(
            !has_pair_family(&families, left, right),
            "semantic mode must not merge ordered index-assignment effects across the boundary ({left}/{right}): {out}"
        );
    };

    assert_no_merge("index_pair_a.go", "index_pair_wrong_order.go");
    assert_no_merge("index_pair_a.go", "index_pair_wrong_receiver.go");
    assert_no_merge("index_pair_a.go", "index_pair_mutated.go");
    assert_no_merge("index_chain_pair_a.go", "index_chain_pair_uses_prior.go");
    assert_no_merge("index_pair_a.go", "index_pair_dynamic_js.js");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn semantic_query_reports_exact_safe_three_index_assignment_branch_fragments_for_go() {
    let fixtures = [
        (
            "index_three_a.go",
            "package p\n\nfunc indexThreeLeft(flag bool, out []int, xs []int) {\n    if flag {\n        out[0] = xs[0] + 1\n        out[1] = xs[0] + 2\n        out[2] = xs[0] + 3\n    }\n    audit(\"opaque\")\n}\n",
        ),
        (
            "index_three_b.go",
            "package p\n\nfunc indexThreeRight(enabled bool, dst []int, ys []int) {\n    if enabled {\n        dst[0] = 1 + ys[0]\n        dst[1] = 2 + ys[0]\n        dst[2] = 3 + ys[0]\n    }\n    trace(\"opaque\")\n}\n",
        ),
        (
            "index_three_wrong_order.go",
            "package p\n\nfunc indexThreeWrongOrder(flag bool, out []int, xs []int) {\n    if flag {\n        out[0] = xs[0] + 1\n        out[2] = xs[0] + 3\n        out[1] = xs[0] + 2\n    }\n    audit(\"opaque\")\n}\n",
        ),
        (
            "index_three_wrong_receiver.go",
            "package p\n\nfunc indexThreeWrongReceiver(flag bool, out []int, other []int, xs []int) {\n    if flag {\n        out[0] = xs[0] + 1\n        out[1] = xs[0] + 2\n        other[2] = xs[0] + 3\n    }\n    audit(\"opaque\")\n}\n",
        ),
        (
            "index_three_mutated.go",
            "package p\n\nfunc indexThreeMutated(flag bool, out []int, xs []int) {\n    out[0] = 0\n    if flag {\n        out[0] = xs[0] + 1\n        out[1] = xs[0] + 2\n        out[2] = xs[0] + 3\n    }\n    audit(\"opaque\")\n}\n",
        ),
        (
            "index_three_temp_a.go",
            "package p\n\nfunc indexThreeTempLeft(flag bool, out []int, xs []int) {\n    if flag {\n        value := xs[0] + 1\n        out[0] = value * value\n        out[1] = xs[1] + 2\n        out[2] = xs[0] + 3\n    }\n    audit(\"opaque\")\n}\n",
        ),
        (
            "index_three_temp_b.go",
            "package p\n\nfunc indexThreeTempRight(enabled bool, dst []int, ys []int) {\n    if enabled {\n        dst[0] = (1 + ys[0]) * (1 + ys[0])\n        dst[1] = ys[1] + 2\n        dst[2] = ys[0] + 3\n    }\n    trace(\"opaque\")\n}\n",
        ),
        (
            "index_three_temp_wrong.go",
            "package p\n\nfunc indexThreeTempWrong(flag bool, out []int, xs []int) {\n    if flag {\n        value := xs[0] + 4\n        out[0] = value * value\n        out[1] = xs[1] + 2\n        out[2] = xs[0] + 3\n    }\n    audit(\"opaque\")\n}\n",
        ),
        (
            "index_three_chain_a.go",
            "package p\n\nfunc indexThreeChainLeft(flag bool, out []int, xs []int) {\n    if flag {\n        base := xs[0] + 1\n        slot := base * base\n        out[slot] = xs[1] + 3\n        out[1] = xs[0] + 2\n        out[2] = xs[2] + 4\n    }\n    audit(\"opaque\")\n}\n",
        ),
        (
            "index_three_chain_b.go",
            "package p\n\nfunc indexThreeChainRight(enabled bool, dst []int, ys []int) {\n    if enabled {\n        dst[(1 + ys[0]) * (1 + ys[0])] = ys[1] + 3\n        dst[1] = ys[0] + 2\n        dst[2] = ys[2] + 4\n    }\n    trace(\"opaque\")\n}\n",
        ),
        (
            "index_three_chain_wrong.go",
            "package p\n\nfunc indexThreeChainWrong(flag bool, out []int, xs []int) {\n    if flag {\n        base := xs[0] + 1\n        slot := base + base\n        out[slot] = xs[1] + 3\n        out[1] = xs[0] + 2\n        out[2] = xs[2] + 4\n    }\n    audit(\"opaque\")\n}\n",
        ),
        (
            "index_three_chain_uses_prior.go",
            "package p\n\nfunc indexThreeChainUsesPrior(flag bool, out []int, xs []int) {\n    if flag {\n        base := xs[0] + 1\n        slot := base * base\n        out[slot + base] = xs[1] + 3\n        out[1] = xs[0] + 2\n        out[2] = xs[2] + 4\n    }\n    audit(\"opaque\")\n}\n",
        ),
        (
            "index_four_a.go",
            "package p\n\nfunc indexFourLeft(flag bool, out []int, xs []int) {\n    if flag {\n        out[0] = xs[0] + 1\n        out[1] = xs[0] + 2\n        out[2] = xs[0] + 3\n        out[3] = xs[0] + 4\n    }\n    audit(\"opaque\")\n}\n",
        ),
        (
            "index_three_dynamic_js.js",
            "function indexThreeDynamicJs(flag, out, xs) {\n  if (flag) {\n    out[0] = xs[0] + 1;\n    out[1] = xs[0] + 2;\n    out[2] = xs[0] + 3;\n  }\n  audit(/opaque/);\n}\n",
        ),
    ];
    let (dir, out, families) =
        query_fragment_only_fixtures("nose_three_index_effect_boundary", &fixtures);

    for (left, right, negative) in [
        (
            "index_three_a.go",
            "index_three_b.go",
            "index_three_wrong_order.go",
        ),
        (
            "index_three_temp_a.go",
            "index_three_temp_b.go",
            "index_three_temp_wrong.go",
        ),
        (
            "index_three_chain_a.go",
            "index_three_chain_b.go",
            "index_three_chain_wrong.go",
        ),
    ] {
        let family =
            find_block_pair_family(&families, left, right, negative).unwrap_or_else(|| {
                panic!("missing three-index-assignment branch family {left}/{right}: {out}")
            });
        assert!(
            family_all_blocks(family),
            "three index-assignment branch fragments should report as Block units: {family:?}"
        );
    }

    let assert_no_merge = |left: &str, right: &str| {
        assert!(
            !has_pair_family(&families, left, right),
            "semantic mode must not merge three index-assignment effects across the boundary ({left}/{right}): {out}"
        );
    };

    assert_no_merge("index_three_a.go", "index_three_wrong_order.go");
    assert_no_merge("index_three_a.go", "index_three_wrong_receiver.go");
    assert_no_merge("index_three_a.go", "index_three_mutated.go");
    assert_no_merge("index_three_chain_a.go", "index_three_chain_uses_prior.go");
    assert_no_merge("index_three_a.go", "index_four_a.go");
    assert_no_merge("index_three_a.go", "index_three_dynamic_js.js");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn semantic_query_reports_exact_safe_index_assignment_fragments_for_non_overloaded_languages() {
    let fixtures = [
        (
            "index_square_a.c",
            "void write_square_left(int *xs, int i, int v) {\n  xs[i] = (v + 1) * (v + 1);\n  audit(xs);\n}\n",
        ),
        (
            "index_square_b.c",
            "void write_square_right(int *ys, int j, int w) {\n  ys[j] = (1 + w) * (1 + w);\n  trace(ys);\n}\n",
        ),
        (
            "index_square_neg.c",
            "void write_square_wrong(int *zs, int k, int x) {\n  zs[k] = (x + 2) * (x + 2);\n  audit(zs);\n}\n",
        ),
        (
            "index_sum_a.go",
            "package p\nfunc writeSumLeft(xs []int, i int, a int, b int) {\n  if i >= 0 {\n    xs[i] = a + b\n  }\n  audit(xs)\n}\n",
        ),
        (
            "index_sum_b.go",
            "package p\nfunc writeSumRight(ys []int, j int, c int, d int) {\n  if 0 <= j {\n    ys[j] = d + c\n  }\n  trace(ys)\n}\n",
        ),
        (
            "index_sum_neg.go",
            "package p\nfunc writeSumWrong(zs []int, k int, c int, d int) {\n  if 0 <= k {\n    zs[k + 1] = d + c\n  }\n  audit(zs)\n}\n",
        ),
        (
            "IndexProductA.java",
            "class IndexProductA {\n  void f(int[] xs, int i, int a, int b) {\n    if (i >= 0) {\n      xs[i] = (a + b) * 2;\n    }\n    audit(xs);\n  }\n}\n",
        ),
        (
            "IndexProductB.java",
            "class IndexProductB {\n  void f(int[] ys, int j, int c, int d) {\n    if (0 <= j) {\n      ys[j] = 2 * (d + c);\n    }\n    trace(ys);\n  }\n}\n",
        ),
        (
            "IndexProductNeg.java",
            "class IndexProductNeg {\n  void f(int[] zs, int k, int c, int d) {\n    if (0 <= k) {\n      zs[k] = 3 * (d + c);\n    }\n    audit(zs);\n  }\n}\n",
        ),
        (
            "js_index_assign_a.js",
            "function jsIndexLeft(xs, i, v) {\n  xs[i] = v + 1;\n  audit(xs);\n}\n",
        ),
        (
            "js_index_assign_b.js",
            "function jsIndexRight(ys, j, w) {\n  ys[j] = w + 1;\n  trace(ys);\n}\n",
        ),
        (
            "py_index_assign_a.py",
            "def py_index_left(xs, i, v):\n    xs[i] = v + 1\n    audit(xs)\n",
        ),
        (
            "py_index_assign_b.py",
            "def py_index_right(ys, j, w):\n    ys[j] = w + 1\n    trace(ys)\n",
        ),
    ];
    let (dir, out, families) =
        query_fragment_only_fixtures("nose_exact_index_assign_fragments", &fixtures);

    let assert_assignment_family = |left: &str, right: &str, negative: &str| {
        let family =
            find_block_pair_family(&families, left, right, negative).unwrap_or_else(|| {
                panic!("missing exact index-assignment fragment family {left}/{right}: {out}")
            });
        assert!(
            pair_locations(family, left, right)
                .iter()
                .all(|loc| loc["start_line"] == loc["end_line"]
                    || loc["end_line"].as_u64().unwrap_or(0)
                        <= loc["start_line"].as_u64().unwrap_or(0) + 3),
            "index-assignment fragments should stay tightly scoped: {family:?}"
        );
    };

    let assert_no_pair = |left: &str, right: &str| {
        assert!(
            !has_pair_family(&families, left, right),
            "overloadable index-assignment pair must stay outside exact fragments: {left}/{right}: {out}"
        );
    };

    assert_assignment_family("index_square_a.c", "index_square_b.c", "index_square_neg.c");
    assert_assignment_family("index_sum_a.go", "index_sum_b.go", "index_sum_neg.go");
    assert_assignment_family(
        "IndexProductA.java",
        "IndexProductB.java",
        "IndexProductNeg.java",
    );
    assert_no_pair("js_index_assign_a.js", "js_index_assign_b.js");
    assert_no_pair("py_index_assign_a.py", "py_index_assign_b.py");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn semantic_query_reports_exact_safe_throw_fragments_under_opaque_functions() {
    let fixtures = [
        (
            "throw_arith_a.js",
            "function throwArithLeft(xs) {\n  throw (xs[0] + 1) * 2;\n  audit(xs);\n}\n",
        ),
        (
            "throw_arith_b.js",
            "function throwArithRight(ys) {\n  throw 2 * (ys[0] + 1);\n  trace(ys);\n}\n",
        ),
        (
            "throw_arith_neg.js",
            "function throwArithWrong(zs) {\n  throw (zs[0] + 2) * 2;\n  audit(zs);\n}\n",
        ),
        (
            "throw_squares_a.js",
            "function throwSquaresLeft(xs) {\n  throw xs[0] * xs[0] + xs[1] * xs[1];\n  audit(xs);\n}\n",
        ),
        (
            "throw_squares_b.js",
            "function throwSquaresRight(ys) {\n  throw ys[0] * ys[0] + ys[1] * ys[1];\n  trace(ys);\n}\n",
        ),
        (
            "throw_squares_neg.js",
            "function throwSquaresWrong(zs) {\n  throw zs[0] * zs[0] - zs[1] * zs[1];\n  audit(zs);\n}\n",
        ),
        (
            "throw_product_a.js",
            "function throwProductLeft(xs) {\n  throw (xs[0] + xs[1]) * (xs[2] + 4);\n  audit(xs);\n}\n",
        ),
        (
            "throw_product_b.js",
            "function throwProductRight(ys) {\n  throw (ys[2] + 4) * (ys[0] + ys[1]);\n  trace(ys);\n}\n",
        ),
        (
            "throw_product_mutated.js",
            "function throwProductMutated(zs) {\n  zs.push(1);\n  throw (zs[0] + zs[1]) * (zs[2] + 4);\n  audit(zs);\n}\n",
        ),
    ];
    let (dir, out, families) = query_fragment_fixtures("nose_exact_throw_fragments", &fixtures);

    let assert_throw_family = |left: &str, right: &str, negative: &str| {
        let family = families
            .iter()
            .find(|family| {
                let files: Vec<&str> = family["locations"]
                    .as_array()
                    .expect("locations")
                    .iter()
                    .filter_map(|loc| loc["file"].as_str())
                    .collect();
                files.iter().any(|file| file.ends_with(left))
                    && files.iter().any(|file| file.ends_with(right))
            })
            .unwrap_or_else(|| panic!("missing exact throw fragment family {left}/{right}: {out}"));
        let locations = family["locations"].as_array().expect("locations");
        assert!(
            locations.iter().all(|loc| loc["kind"] == "Block"),
            "throw fragments should report as Block units: {family:?}"
        );
        assert!(
            locations
                .iter()
                .all(|loc| !loc["file"].as_str().unwrap_or("").ends_with(negative)),
            "hard negative must not merge into {left}/{right}: {family:?}"
        );
    };

    assert_throw_family("throw_arith_a.js", "throw_arith_b.js", "throw_arith_neg.js");
    assert_throw_family(
        "throw_squares_a.js",
        "throw_squares_b.js",
        "throw_squares_neg.js",
    );
    assert_throw_family(
        "throw_product_a.js",
        "throw_product_b.js",
        "throw_product_mutated.js",
    );
    let _ = fs::remove_dir_all(&dir);
}
