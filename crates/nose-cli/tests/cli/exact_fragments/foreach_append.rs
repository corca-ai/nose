use super::*;

// Broad fixture matrix for exact foreach append-effect fragments. The size is
// intentional until the fixture setup has a clearer table-builder abstraction.
#[allow(clippy::too_many_lines)]
#[test]
fn semantic_query_reports_exact_safe_foreach_append_effect_fragments_under_opaque_functions() {
    let fixtures = [
        (
            "loop_push_square_a.ts",
            "function loopPushSquareLeft(xs: number[], out: number[]): void {\n  for (const x of xs) {\n    out.push(x * x);\n  }\n  audit(xs);\n}\n",
        ),
        (
            "loop_push_square_b.ts",
            "function loopPushSquareRight(ys: number[], dst: number[]): void {\n  for (const y of ys) {\n    dst.push(y * y);\n  }\n  trace(ys);\n}\n",
        ),
        (
            "loop_push_square_mutated.ts",
            "function loopPushSquareMutated(zs: number[], out: number[]): void {\n  out.push(0);\n  for (const z of zs) {\n    out.push(z * z);\n  }\n  audit(zs);\n}\n",
        ),
        (
            "loop_push_square_wrong_receiver.ts",
            "function loopPushSquareWrongReceiver(xs: number[], out: number[]): void {\n  for (const x of xs) {\n    xs.push(x * x);\n  }\n  audit(xs);\n}\n",
        ),
        (
            "loop_push_product_a.ts",
            "function loopPushProductLeft(xs: number[], out: number[]): void {\n  for (const x of xs) {\n    out.push((x + 1) * 2);\n  }\n  audit(xs);\n}\n",
        ),
        (
            "loop_push_product_b.ts",
            "function loopPushProductRight(ys: number[], dst: number[]): void {\n  for (const y of ys) {\n    dst.push(2 * (y + 1));\n  }\n  trace(ys);\n}\n",
        ),
        (
            "loop_push_product_neg.ts",
            "function loopPushProductWrong(zs: number[], out: number[]): void {\n  for (const z of zs) {\n    out.push((z + 2) * 2);\n  }\n  audit(zs);\n}\n",
        ),
        (
            "loop_push_guard_a.ts",
            "function loopPushGuardLeft(xs: number[], out: number[]): void {\n  for (const x of xs) {\n    if (x > 0) out.push(x + 1);\n  }\n  audit(xs);\n}\n",
        ),
        (
            "loop_push_guard_b.ts",
            "function loopPushGuardRight(ys: number[], dst: number[]): void {\n  for (const y of ys) {\n    if (0 < y) dst.push(y + 1);\n  }\n  trace(ys);\n}\n",
        ),
        (
            "loop_push_guard_neg.ts",
            "function loopPushGuardWrong(zs: number[], out: number[]): void {\n  for (const z of zs) {\n    if (z >= 0) out.push(z + 1);\n  }\n  audit(zs);\n}\n",
        ),
        (
            "loop_temp_push_square_a.ts",
            "function loopTempPushSquareLeft(xs: number[], out: number[]): void {\n  for (const x of xs) {\n    const squared = x * x;\n    out.push(squared);\n  }\n  audit(xs);\n}\n",
        ),
        (
            "loop_temp_push_square_b.ts",
            "function loopTempPushSquareRight(ys: number[], dst: number[]): void {\n  for (const y of ys) {\n    const result = y * y;\n    dst.push(result);\n  }\n  trace(ys);\n}\n",
        ),
        (
            "loop_temp_push_square_wrong.ts",
            "function loopTempPushSquareWrong(zs: number[], out: number[]): void {\n  for (const z of zs) {\n    const squared = z + z;\n    out.push(squared);\n  }\n  audit(zs);\n}\n",
        ),
        (
            "loop_temp_append_py_a.py",
            "def loop_temp_append_left(xs: list[int], out: list[int]):\n    for x in xs:\n        value = x + 1\n        out.append(value)\n    audit(xs)\n",
        ),
        (
            "loop_temp_append_py_b.py",
            "def loop_temp_append_right(ys: list[int], dst: list[int]):\n    for y in ys:\n        item = 1 + y\n        dst.append(item)\n    trace(ys)\n",
        ),
        (
            "loop_temp_append_py_wrong.py",
            "def loop_temp_append_wrong(zs: list[int], out: list[int]):\n    for z in zs:\n        value = z + 2\n        out.append(value)\n    audit(zs)\n",
        ),
        (
            "loop_temp_chain_push_a.ts",
            "function loopTempChainPushLeft(xs: number[], out: number[]): void {\n  for (const x of xs) {\n    const shifted = x + 1;\n    const squared = shifted * shifted;\n    out.push(squared);\n  }\n  audit(xs);\n}\n",
        ),
        (
            "loop_temp_chain_push_b.ts",
            "function loopTempChainPushRight(ys: number[], dst: number[]): void {\n  for (const y of ys) {\n    const offset = y + 1;\n    const result = offset * offset;\n    dst.push(result);\n  }\n  trace(ys);\n}\n",
        ),
        (
            "loop_temp_chain_push_wrong.ts",
            "function loopTempChainPushWrong(zs: number[], out: number[]): void {\n  for (const z of zs) {\n    const shifted = z + 2;\n    const squared = shifted * shifted;\n    out.push(squared);\n  }\n  audit(zs);\n}\n",
        ),
        (
            "loop_temp_chain_append_py_a.py",
            "def loop_temp_chain_append_left(xs: list[int], out: list[int]):\n    for x in xs:\n        shifted = x + 1\n        value = shifted * shifted\n        out.append(value)\n    audit(xs)\n",
        ),
        (
            "loop_temp_chain_append_py_b.py",
            "def loop_temp_chain_append_right(ys: list[int], dst: list[int]):\n    for y in ys:\n        offset = 1 + y\n        item = offset * offset\n        dst.append(item)\n    trace(ys)\n",
        ),
        (
            "loop_temp_chain_append_py_wrong.py",
            "def loop_temp_chain_append_wrong(zs: list[int], out: list[int]):\n    for z in zs:\n        shifted = z + 1\n        value = shifted + shifted\n        out.append(value)\n    audit(zs)\n",
        ),
        (
            "loop_temp_chain_unconsumed.ts",
            "function loopTempChainUnconsumed(xs: number[], out: number[]): void {\n  for (const x of xs) {\n    const shifted = x + 1;\n    const squared = x * x;\n    out.push(squared);\n  }\n  audit(xs);\n}\n",
        ),
        (
            "loop_temp_chain_uses_prior.ts",
            "function loopTempChainUsesPrior(xs: number[], out: number[]): void {\n  for (const x of xs) {\n    const shifted = x + 1;\n    const squared = shifted * shifted;\n    out.push(squared + shifted);\n  }\n  audit(xs);\n}\n",
        ),
        (
            "loop_temp_unused.ts",
            "function loopTempUnused(xs: number[], out: number[]): void {\n  for (const x of xs) {\n    const constant = 1;\n    out.push(constant);\n  }\n  audit(xs);\n}\n",
        ),
        (
            "loop_temp_rebind_iter.ts",
            "function loopTempRebindIter(xs: number[], out: number[]): void {\n  for (let x of xs) {\n    x = x + 1;\n    out.push(x);\n  }\n  audit(xs);\n}\n",
        ),
        (
            "loop_unused_effect.ts",
            "function loopUnusedEffect(xs: number[], out: number[]): void {\n  for (const unused of xs) {\n    out.push(1);\n  }\n  audit(xs);\n}\n",
        ),
        (
            "direct_unused_effect.ts",
            "function directUnusedEffect(out: number[]): void {\n  out.push(1);\n  audit(out);\n}\n",
        ),
        (
            "loop_untyped_push_square.js",
            "function loopUntypedPushSquare(xs, out) {\n  for (const x of xs) {\n    out.push(x * x);\n  }\n  audit(xs);\n}\n",
        ),
    ];
    let (dir, out, families) =
        query_fragment_only_fixtures("nose_exact_loop_effect_fragments", &fixtures);

    let assert_loop_family = |left: &str,
                              right: &str,
                              negative: &str,
                              start_line: u64,
                              end_line: u64| {
        let family = families
            .iter()
            .find(|family| {
                let locations = family["locations"].as_array().expect("locations");
                let loop_files: Vec<&str> = locations
                    .iter()
                    .filter(|loc| loc["start_line"] == start_line && loc["end_line"] == end_line)
                    .filter_map(|loc| loc["file"].as_str())
                    .collect();
                loop_files.iter().any(|file| file.ends_with(left))
                    && loop_files.iter().any(|file| file.ends_with(right))
                    && locations.iter().all(|loc| loc["kind"] == "Block")
                    && locations
                        .iter()
                        .filter_map(|loc| loc["file"].as_str())
                        .all(|file| !file.ends_with(negative))
            })
            .unwrap_or_else(|| {
                panic!("missing exact foreach append-effect fragment family {left}/{right}: {out}")
            });
        let locations = family["locations"].as_array().expect("locations");
        assert!(
            locations
                .iter()
                .filter(|loc| loc["file"].as_str().unwrap_or("").ends_with(left)
                    || loc["file"].as_str().unwrap_or("").ends_with(right))
                .all(|loc| loc["start_line"] == start_line && loc["end_line"] == end_line),
            "foreach append-effect fragments should report the loop span only: {family:?}"
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
            "foreach append effect boundary must not merge {left}/{right}: {out}"
        );
    };

    assert_loop_family(
        "loop_push_square_a.ts",
        "loop_push_square_b.ts",
        "loop_push_square_mutated.ts",
        2,
        4,
    );
    assert_no_pair(
        "loop_push_square_a.ts",
        "loop_push_square_wrong_receiver.ts",
    );
    assert_loop_family(
        "loop_push_product_a.ts",
        "loop_push_product_b.ts",
        "loop_push_product_neg.ts",
        2,
        4,
    );
    assert_loop_family(
        "loop_push_guard_a.ts",
        "loop_push_guard_b.ts",
        "loop_push_guard_neg.ts",
        2,
        4,
    );
    assert_loop_family(
        "loop_temp_push_square_a.ts",
        "loop_temp_push_square_b.ts",
        "loop_temp_push_square_wrong.ts",
        2,
        5,
    );
    assert_loop_family(
        "loop_temp_append_py_a.py",
        "loop_temp_append_py_b.py",
        "loop_temp_append_py_wrong.py",
        2,
        4,
    );
    assert_loop_family(
        "loop_temp_chain_push_a.ts",
        "loop_temp_chain_push_b.ts",
        "loop_temp_chain_push_wrong.ts",
        2,
        6,
    );
    assert_loop_family(
        "loop_temp_chain_append_py_a.py",
        "loop_temp_chain_append_py_b.py",
        "loop_temp_chain_append_py_wrong.py",
        2,
        5,
    );
    assert_no_pair("loop_unused_effect.ts", "direct_unused_effect.ts");
    assert_no_pair("loop_temp_unused.ts", "direct_unused_effect.ts");
    assert_no_pair("loop_temp_append_py_a.py", "loop_temp_rebind_iter.ts");
    assert_no_pair("loop_temp_chain_unconsumed.ts", "loop_temp_chain_push_b.ts");
    assert_no_pair("loop_temp_chain_uses_prior.ts", "loop_temp_chain_push_b.ts");
    assert_no_pair("loop_untyped_push_square.js", "loop_push_square_a.ts");
    let _ = fs::remove_dir_all(&dir);
}
