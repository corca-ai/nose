use super::*;

// Broad fixture matrix for exact branch temp-consumption fragments. The size is
// intentional until the fixture setup has a clearer table-builder abstraction.
#[allow(clippy::too_many_lines)]
#[test]
fn semantic_query_reports_exact_safe_branch_temp_consumption_fragments_under_opaque_functions() {
    let fixtures = [
        (
            "temp_return_a.py",
            "def temp_return_left(xs):\n    if xs[0] > 0:\n        result = xs[0] * xs[0] + xs[1]\n        return result\n    audit(xs)\n",
        ),
        (
            "temp_return_b.py",
            "def temp_return_right(ys):\n    if 0 < ys[0]:\n        return ys[1] + ys[0] * ys[0]\n    trace(ys)\n",
        ),
        (
            "temp_return_neg.py",
            "def temp_return_wrong(zs):\n    if zs[0] > 0:\n        result = zs[0] * zs[0] - zs[1]\n        return result\n    audit(zs)\n",
        ),
        (
            "temp_return_self_dependent.py",
            "def temp_return_self_dependent(xs):\n    result = xs[0]\n    if xs[0] > 0:\n        result = result + xs[1]\n        return result\n    audit(xs)\n",
        ),
        (
            "temp_return_window_gap.py",
            "def temp_return_window_gap(xs):\n    if xs[0] > 0:\n        result = xs[0] * xs[0] + xs[1]\n        observe(result)\n        return result\n    audit(xs)\n",
        ),
        (
            "temp_throw_a.js",
            "function tempThrowLeft(xs) {\n  if (xs[0] + xs[1] > 10) {\n    const result = xs[0] + xs[1];\n    throw result;\n  }\n  audit(xs);\n}\n",
        ),
        (
            "temp_throw_b.js",
            "function tempThrowRight(ys) {\n  if (10 < ys[0] + ys[1]) {\n    throw ys[0] + ys[1];\n  }\n  trace(ys);\n}\n",
        ),
        (
            "temp_throw_neg.js",
            "function tempThrowWrong(zs) {\n  if (zs[0] + zs[1] > 10) {\n    const result = zs[0] - zs[1];\n    throw result;\n  }\n  audit(zs);\n}\n",
        ),
        (
            "temp_effect_a.js",
            "function tempEffectLeft(xs, out) {\n  if (xs[0] > 0) {\n    const result = xs[0] * xs[0] + xs[1];\n    out.push(result);\n  }\n  audit(xs);\n}\n",
        ),
        (
            "temp_effect_b.js",
            "function tempEffectRight(ys, dst) {\n  if (0 < ys[0]) {\n    dst.push(ys[0] * ys[0] + ys[1]);\n  }\n  trace(ys);\n}\n",
        ),
        (
            "temp_effect_neg.js",
            "function tempEffectWrong(zs, out) {\n  if (zs[0] > 0) {\n    const result = zs[0] * zs[0] - zs[1];\n    out.push(result);\n  }\n  audit(zs);\n}\n",
        ),
        (
            "temp_chain_return_a.py",
            "def temp_chain_return_left(xs):\n    if xs[0] > 0:\n        shifted = xs[0] + 1\n        result = shifted * shifted + xs[1]\n        return result\n    audit(xs)\n",
        ),
        (
            "temp_chain_return_b.py",
            "def temp_chain_return_right(ys):\n    if 0 < ys[0]:\n        return ys[1] + (1 + ys[0]) * (1 + ys[0])\n    trace(ys)\n",
        ),
        (
            "temp_chain_return_neg.py",
            "def temp_chain_return_wrong(zs):\n    if zs[0] > 0:\n        shifted = zs[0] + 2\n        result = shifted * shifted + zs[1]\n        return result\n    audit(zs)\n",
        ),
        (
            "temp_chain_throw_a.js",
            "function tempChainThrowLeft(xs) {\n  if (xs[0] > 0) {\n    const shifted = xs[0] + 1;\n    const result = shifted * shifted + xs[1];\n    throw result;\n  }\n  audit(xs);\n}\n",
        ),
        (
            "temp_chain_throw_b.js",
            "function tempChainThrowRight(ys) {\n  if (0 < ys[0]) {\n    throw (ys[0] + 1) * (ys[0] + 1) + ys[1];\n  }\n  trace(ys);\n}\n",
        ),
        (
            "temp_chain_throw_neg.js",
            "function tempChainThrowWrong(zs) {\n  if (zs[0] > 0) {\n    const shifted = zs[0] + 1;\n    const result = shifted + shifted + zs[1];\n    throw result;\n  }\n  audit(zs);\n}\n",
        ),
        (
            "temp_chain_effect_a.js",
            "function tempChainEffectLeft(xs, out) {\n  if (xs[0] > 0) {\n    const shifted = xs[0] + 1;\n    const result = shifted * shifted + xs[1];\n    out.push(result);\n  }\n  audit(xs);\n}\n",
        ),
        (
            "temp_chain_effect_b.js",
            "function tempChainEffectRight(ys, dst) {\n  if (0 < ys[0]) {\n    dst.push((ys[0] + 1) * (ys[0] + 1) + ys[1]);\n  }\n  trace(ys);\n}\n",
        ),
        (
            "temp_chain_effect_neg.js",
            "function tempChainEffectWrong(zs, out) {\n  if (zs[0] > 0) {\n    const shifted = zs[0] + 1;\n    const result = shifted * shifted - zs[1];\n    out.push(result);\n  }\n  audit(zs);\n}\n",
        ),
        (
            "temp_chain_unconsumed_first.js",
            "function tempChainUnconsumedFirst(xs, out) {\n  if (xs[0] > 0) {\n    const shifted = xs[0] + 1;\n    const result = xs[0] * xs[0] + xs[1];\n    out.push(result);\n  }\n  audit(xs);\n}\n",
        ),
        (
            "temp_chain_effect_uses_prior.js",
            "function tempChainEffectUsesPrior(xs, out) {\n  if (xs[0] > 0) {\n    const shifted = xs[0] + 1;\n    const result = shifted * shifted + xs[1];\n    out.push(result + shifted);\n  }\n  audit(xs);\n}\n",
        ),
        (
            "temp_index_value_a.go",
            "package p\nfunc tempIndexValueLeft(xs []int, out []int, ok bool) {\n  if ok {\n    value := xs[0] + 1\n    out[0] = value * value\n  }\n  audit(xs)\n}\n",
        ),
        (
            "temp_index_value_b.go",
            "package p\nfunc tempIndexValueRight(ys []int, dst []int, flag bool) {\n  if flag {\n    dst[0] = (1 + ys[0]) * (1 + ys[0])\n  }\n  trace(ys)\n}\n",
        ),
        (
            "temp_index_value_neg.go",
            "package p\nfunc tempIndexValueWrong(zs []int, out []int, ok bool) {\n  if ok {\n    value := zs[0] + 2\n    out[0] = value * value\n  }\n  audit(zs)\n}\n",
        ),
        (
            "temp_index_key_a.go",
            "package p\nfunc tempIndexKeyLeft(xs []int, out []int, ok bool) {\n  if ok {\n    slot := xs[0] + 1\n    out[slot] = xs[1] * 2\n  }\n  audit(xs)\n}\n",
        ),
        (
            "temp_index_key_b.go",
            "package p\nfunc tempIndexKeyRight(ys []int, dst []int, flag bool) {\n  if flag {\n    dst[1 + ys[0]] = 2 * ys[1]\n  }\n  trace(ys)\n}\n",
        ),
        (
            "temp_index_key_neg.go",
            "package p\nfunc tempIndexKeyWrong(zs []int, out []int, ok bool) {\n  if ok {\n    slot := zs[0] + 2\n    out[slot] = zs[1] * 2\n  }\n  audit(zs)\n}\n",
        ),
        (
            "temp_index_chain_a.go",
            "package p\nfunc tempIndexChainLeft(xs []int, out []int, ok bool) {\n  if ok {\n    shifted := xs[0] + 1\n    slot := shifted * shifted\n    out[slot] = xs[1]\n  }\n  audit(xs)\n}\n",
        ),
        (
            "temp_index_chain_b.go",
            "package p\nfunc tempIndexChainRight(ys []int, dst []int, flag bool) {\n  if flag {\n    dst[(1 + ys[0]) * (1 + ys[0])] = ys[1]\n  }\n  trace(ys)\n}\n",
        ),
        (
            "temp_index_chain_neg.go",
            "package p\nfunc tempIndexChainWrong(zs []int, out []int, ok bool) {\n  if ok {\n    shifted := zs[0] + 1\n    slot := shifted + shifted\n    out[slot] = zs[1]\n  }\n  audit(zs)\n}\n",
        ),
        (
            "temp_index_receiver_uses_temp.go",
            "package p\nfunc tempIndexReceiverUsesTemp(xs []int, tables [][]int, ok bool) {\n  if ok {\n    shifted := xs[0] + 1\n    tables[shifted][0] = xs[1]\n  }\n  audit(xs)\n}\n",
        ),
        (
            "temp_index_chain_unconsumed_first.go",
            "package p\nfunc tempIndexChainUnconsumedFirst(xs []int, out []int, ok bool) {\n  if ok {\n    shifted := xs[0] + 1\n    slot := xs[0] * xs[0]\n    out[slot] = xs[1]\n  }\n  audit(xs)\n}\n",
        ),
        (
            "temp_index_chain_uses_prior.go",
            "package p\nfunc tempIndexChainUsesPrior(xs []int, out []int, ok bool) {\n  if ok {\n    shifted := xs[0] + 1\n    slot := shifted * shifted\n    out[slot + shifted] = xs[1]\n  }\n  audit(xs)\n}\n",
        ),
    ];
    let (dir, out, families) =
        query_fragment_only_fixtures("nose_exact_branch_temp_fragments", &fixtures);

    let assert_temp_family = |left: &str, right: &str, negative: &str| {
        let family = families
            .iter()
            .find(|family| {
                let locations = family["locations"].as_array().expect("locations");
                let files: Vec<&str> = locations
                    .iter()
                    .filter_map(|loc| loc["file"].as_str())
                    .collect();
                files.iter().any(|file| file.ends_with(left))
                    && files.iter().any(|file| file.ends_with(right))
                    && locations.iter().all(|loc| loc["kind"] == "Block")
                    && files.iter().all(|file| !file.ends_with(negative))
            })
            .unwrap_or_else(|| {
                panic!("missing exact branch temp-consumption family {left}/{right}: {out}")
            });
        assert!(
            family["locations"]
                .as_array()
                .expect("locations")
                .iter()
                .all(|loc| loc["kind"] == "Block"),
            "branch temp-consumption fragments should report as Block units: {family:?}"
        );
    };

    let assert_no_pair = |left: &str, right: &str| {
        let has_pair = families.iter().any(|family| {
            let files: Vec<&str> = family["locations"]
                .as_array()
                .expect("locations")
                .iter()
                .filter_map(|loc| loc["file"].as_str())
                .collect();
            files.iter().any(|file| file.ends_with(left))
                && files.iter().any(|file| file.ends_with(right))
        });
        assert!(
            !has_pair,
            "self-dependent or non-adjacent temp consumption must stay outside exact fragments: {left}/{right}: {out}"
        );
    };

    assert_temp_family("temp_return_a.py", "temp_return_b.py", "temp_return_neg.py");
    assert_temp_family("temp_throw_a.js", "temp_throw_b.js", "temp_throw_neg.js");
    assert_temp_family("temp_effect_a.js", "temp_effect_b.js", "temp_effect_neg.js");
    assert_temp_family(
        "temp_chain_return_a.py",
        "temp_chain_return_b.py",
        "temp_chain_return_neg.py",
    );
    assert_temp_family(
        "temp_chain_throw_a.js",
        "temp_chain_throw_b.js",
        "temp_chain_throw_neg.js",
    );
    assert_temp_family(
        "temp_chain_effect_a.js",
        "temp_chain_effect_b.js",
        "temp_chain_effect_neg.js",
    );
    assert_temp_family(
        "temp_index_value_a.go",
        "temp_index_value_b.go",
        "temp_index_value_neg.go",
    );
    assert_temp_family(
        "temp_index_key_a.go",
        "temp_index_key_b.go",
        "temp_index_key_neg.go",
    );
    assert_temp_family(
        "temp_index_chain_a.go",
        "temp_index_chain_b.go",
        "temp_index_chain_neg.go",
    );
    assert_no_pair("temp_return_self_dependent.py", "temp_return_b.py");
    assert_no_pair("temp_return_window_gap.py", "temp_return_b.py");
    assert_no_pair("temp_chain_unconsumed_first.js", "temp_chain_effect_b.js");
    assert_no_pair("temp_chain_effect_uses_prior.js", "temp_chain_effect_b.js");
    assert_no_pair("temp_index_receiver_uses_temp.go", "temp_index_key_b.go");
    assert_no_pair(
        "temp_index_chain_unconsumed_first.go",
        "temp_index_chain_b.go",
    );
    assert_no_pair("temp_index_chain_uses_prior.go", "temp_index_chain_b.go");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn semantic_query_reports_exact_safe_nested_conditional_effect_fragments_under_opaque_functions() {
    let fixtures = [
        (
            "nested_push_a.ts",
            "function nestedPushLeft(x: number, y: number, z: number, out: number[]) {\n  if (x > 0 && y > 0) {\n    out.push(x + y);\n  } else {\n    if (z > 0) {\n      out.push(z * z);\n    }\n  }\n  audit(x);\n}\n",
        ),
        (
            "nested_push_b.ts",
            "function nestedPushRight(a: number, b: number, c: number, dst: number[]) {\n  if (b > 0 && a > 0) {\n    dst.push(a + b);\n  } else {\n    if (0 < c) {\n      dst.push(c * c);\n    }\n  }\n  trace(a);\n}\n",
        ),
        (
            "nested_push_mutated.ts",
            "function nestedPushMutated(z: number, w: number, q: number, out: number[]) {\n  out.push(0);\n  if (z > 0 && w > 0) {\n    out.push(z + w);\n  } else {\n    if (q > 0) {\n      out.push(q * q);\n    }\n  }\n  audit(z);\n}\n",
        ),
        (
            "nested_push_sum_a.ts",
            "function nestedPushSumLeft(xs: number[], out: number[]) {\n  if (xs[0] + xs[1] > 10) {\n    out.push(xs[0] + xs[1]);\n  } else {\n    if (xs[2] > 0) {\n      out.push(xs[2] * xs[2]);\n    }\n  }\n  audit(xs);\n}\n",
        ),
        (
            "nested_push_sum_b.ts",
            "function nestedPushSumRight(ys: number[], dst: number[]) {\n  if (10 < ys[0] + ys[1]) {\n    dst.push(ys[0] + ys[1]);\n  } else {\n    if (0 < ys[2]) {\n      dst.push(ys[2] * ys[2]);\n    }\n  }\n  trace(ys);\n}\n",
        ),
        (
            "nested_push_sum_neg.ts",
            "function nestedPushSumWrong(zs: number[], out: number[]) {\n  if (zs[0] + zs[1] > 10) {\n    out.push(zs[0] - zs[1]);\n  } else {\n    if (zs[2] > 0) {\n      out.push(zs[2] * zs[2]);\n    }\n  }\n  audit(zs);\n}\n",
        ),
        (
            "nested_push_product_a.ts",
            "function nestedPushProductLeft(xs: number[], out: number[]) {\n  if ((xs[0] + 1) > 10) {\n    out.push((xs[0] + 1) * 2);\n  } else {\n    if (xs[1] + xs[2] > 0) {\n      out.push(xs[1] + xs[2]);\n    }\n  }\n  audit(xs);\n}\n",
        ),
        (
            "nested_push_product_b.ts",
            "function nestedPushProductRight(ys: number[], dst: number[]) {\n  if (10 < (ys[0] + 1)) {\n    dst.push(2 * (ys[0] + 1));\n  } else {\n    if (ys[1] + ys[2] > 0) {\n      dst.push(ys[1] + ys[2]);\n    }\n  }\n  trace(ys);\n}\n",
        ),
        (
            "nested_push_product_neg.ts",
            "function nestedPushProductWrong(zs: number[], out: number[]) {\n  if ((zs[0] + 2) > 10) {\n    out.push((zs[0] + 2) * 2);\n  } else {\n    if (zs[1] + zs[2] > 0) {\n      out.push(zs[1] + zs[2]);\n    }\n  }\n  audit(zs);\n}\n",
        ),
    ];
    let (dir, out, families) =
        query_fragment_only_fixtures("nose_exact_nested_fragments", &fixtures);

    let assert_guard_family = |left: &str, right: &str, negative: &str| {
        let family = find_block_pair_family_at(&families, left, right, negative, 2, 8)
            .unwrap_or_else(|| {
                panic!("missing exact nested conditional effect family {left}/{right}: {out}")
            });
        assert!(
            pair_locations(family, left, right)
                .iter()
                .all(|loc| loc["start_line"] == 2 && loc["end_line"] == 8),
            "nested conditional effect fragments should report the full nested if: {family:?}"
        );
    };

    assert_guard_family(
        "nested_push_a.ts",
        "nested_push_b.ts",
        "nested_push_mutated.ts",
    );
    assert_guard_family(
        "nested_push_sum_a.ts",
        "nested_push_sum_b.ts",
        "nested_push_sum_neg.ts",
    );
    assert_guard_family(
        "nested_push_product_a.ts",
        "nested_push_product_b.ts",
        "nested_push_product_neg.ts",
    );
    let _ = fs::remove_dir_all(&dir);
}
