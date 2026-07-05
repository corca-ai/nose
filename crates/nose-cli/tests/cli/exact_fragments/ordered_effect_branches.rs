use super::*;

// Broad fixture matrix for ordered foreach-effect branch boundaries. The size is
// intentional until the fixture setup has a clearer table-builder abstraction.
#[allow(clippy::too_many_lines)]
#[test]
fn semantic_query_reports_exact_safe_ordered_foreach_effect_branch_fragments() {
    let fixtures = [
        (
            "cond_two_loops_a.ts",
            "function condTwoLoopsLeft(enabled: boolean, xs: number[], ys: number[], out: number[]): void {\n  if (enabled) {\n    for (const x of xs) {\n      out.push(x + 1);\n    }\n    for (const y of ys) {\n      out.push(y * y);\n    }\n  }\n  audit(enabled);\n}\n",
        ),
        (
            "cond_two_loops_b.ts",
            "function condTwoLoopsRight(flag: boolean, as: number[], bs: number[], dst: number[]): void {\n  if (flag) {\n    for (const a of as) {\n      dst.push(a + 1);\n    }\n    for (const b of bs) {\n      dst.push(b * b);\n    }\n  }\n  trace(flag);\n}\n",
        ),
        (
            "cond_two_loops_wrong_order.ts",
            "function condTwoLoopsWrongOrder(flag: boolean, xs: number[], ys: number[], out: number[]): void {\n  if (flag) {\n    for (const y of ys) {\n      out.push(y * y);\n    }\n    for (const x of xs) {\n      out.push(x + 1);\n    }\n  }\n  audit(flag);\n}\n",
        ),
        (
            "cond_two_loops_wrong_receiver.ts",
            "function condTwoLoopsWrongReceiver(flag: boolean, xs: number[], ys: number[], out: number[]): void {\n  if (flag) {\n    for (const x of xs) {\n      out.push(x + 1);\n    }\n    for (const y of ys) {\n      ys.push(y * y);\n    }\n  }\n  audit(flag);\n}\n",
        ),
        (
            "cond_two_loops_mutated.ts",
            "function condTwoLoopsMutated(flag: boolean, xs: number[], ys: number[], out: number[]): void {\n  out.push(0);\n  if (flag) {\n    for (const x of xs) {\n      out.push(x + 1);\n    }\n    for (const y of ys) {\n      out.push(y * y);\n    }\n  }\n  audit(flag);\n}\n",
        ),
        (
            "cond_three_loops.ts",
            "function condThreeLoops(flag: boolean, xs: number[], ys: number[], zs: number[], out: number[]): void {\n  if (flag) {\n    for (const x of xs) {\n      out.push(x + 1);\n    }\n    for (const y of ys) {\n      out.push(y * y);\n    }\n    for (const z of zs) {\n      out.push(z + 3);\n    }\n  }\n  audit(flag);\n}\n",
        ),
        (
            "cond_two_temp_loops_a.py",
            "def cond_two_temp_loops_left(flag: bool, xs: list[int], ys: list[int], out: list[int]):\n    if flag:\n        for x in xs:\n            value = x + 1\n            out.append(value * value)\n        for y in ys:\n            out.append(y + 2)\n    audit(flag)\n",
        ),
        (
            "cond_two_temp_loops_b.py",
            "def cond_two_temp_loops_right(enabled: bool, as_: list[int], bs: list[int], dst: list[int]):\n    if enabled:\n        for a in as_:\n            item = 1 + a\n            dst.append(item * item)\n        for b in bs:\n            dst.append(2 + b)\n    trace(enabled)\n",
        ),
        (
            "cond_two_temp_loops_wrong.py",
            "def cond_two_temp_loops_wrong(flag: bool, xs: list[int], ys: list[int], out: list[int]):\n    if flag:\n        for x in xs:\n            value = x + 2\n            out.append(value * value)\n        for y in ys:\n            out.append(y + 2)\n    audit(flag)\n",
        ),
        (
            "cond_two_index_loops_a.go",
            "package p\nfunc condTwoIndexLoopsLeft(flag bool, xs []int, ys []int, out []int) {\n  if flag {\n    for i, x := range xs {\n      out[i] = x * x\n    }\n    for j, y := range ys {\n      out[j+1] = y + 1\n    }\n  }\n  audit(out)\n}\n",
        ),
        (
            "cond_two_index_loops_b.go",
            "package p\nfunc condTwoIndexLoopsRight(enabled bool, as []int, bs []int, dst []int) {\n  if enabled {\n    for k, a := range as {\n      dst[k] = a * a\n    }\n    for m, b := range bs {\n      dst[1+m] = 1 + b\n    }\n  }\n  trace(dst)\n}\n",
        ),
        (
            "cond_two_index_loops_wrong_index.go",
            "package p\nfunc condTwoIndexLoopsWrongIndex(flag bool, xs []int, ys []int, out []int) {\n  if flag {\n    for i, x := range xs {\n      out[i] = x * x\n    }\n    for j, y := range ys {\n      out[j+2] = y + 1\n    }\n  }\n  audit(out)\n}\n",
        ),
        (
            "cond_two_index_loops_wrong_receiver.go",
            "package p\nfunc condTwoIndexLoopsWrongReceiver(flag bool, xs []int, ys []int, out []int) {\n  if flag {\n    for i, x := range xs {\n      out[i] = x * x\n    }\n    for j, y := range ys {\n      ys[j+1] = y + 1\n    }\n  }\n  audit(out)\n}\n",
        ),
    ];
    let (dir, out, families) =
        query_fragment_only_fixtures("nose_ordered_foreach_effect_branch_fragments", &fixtures);

    let assert_branch_pair =
        |left: &str, right: &str, negative: &str, start_line: u64, end_line: u64| {
            let family =
                block_branch_pair_family(&families, left, right, negative, start_line, end_line)
                    .unwrap_or_else(|| {
                        panic!("missing ordered foreach-effect branch family {left}/{right}: {out}")
                    });
            assert!(
                family["locations"]
                    .as_array()
                    .expect("locations")
                    .iter()
                    .all(|loc| loc["kind"] == "Block"),
                "ordered foreach-effect branch fragments should report as Block units: {family:?}"
            );
        };

    let assert_no_branch_pair = |left: &str,
                                 right: &str,
                                 left_start: u64,
                                 left_end: u64,
                                 right_start: u64,
                                 right_end: u64| {
        assert!(
            !families_pair_locations(
                &families,
                (left, left_start, left_end),
                (right, right_start, right_end),
            ),
            "ordered foreach-effect branch boundary must not merge {left}/{right}: {out}"
        );
    };

    assert_branch_pair(
        "cond_two_loops_a.ts",
        "cond_two_loops_b.ts",
        "cond_two_loops_wrong_order.ts",
        2,
        9,
    );
    assert_branch_pair(
        "cond_two_temp_loops_a.py",
        "cond_two_temp_loops_b.py",
        "cond_two_temp_loops_wrong.py",
        2,
        7,
    );
    assert_branch_pair(
        "cond_two_index_loops_a.go",
        "cond_two_index_loops_b.go",
        "cond_two_index_loops_wrong_index.go",
        3,
        10,
    );
    assert_no_branch_pair(
        "cond_two_loops_a.ts",
        "cond_two_loops_wrong_order.ts",
        2,
        9,
        2,
        9,
    );
    assert_no_branch_pair(
        "cond_two_loops_a.ts",
        "cond_two_loops_wrong_receiver.ts",
        2,
        9,
        2,
        9,
    );
    assert_no_branch_pair(
        "cond_two_loops_a.ts",
        "cond_two_loops_mutated.ts",
        2,
        9,
        3,
        10,
    );
    assert_no_branch_pair("cond_two_loops_a.ts", "cond_three_loops.ts", 2, 9, 2, 12);
    assert_no_branch_pair(
        "cond_two_temp_loops_a.py",
        "cond_two_temp_loops_wrong.py",
        2,
        7,
        2,
        7,
    );
    assert_no_branch_pair(
        "cond_two_index_loops_a.go",
        "cond_two_index_loops_wrong_index.go",
        3,
        10,
        3,
        10,
    );
    assert_no_branch_pair(
        "cond_two_index_loops_a.go",
        "cond_two_index_loops_wrong_receiver.go",
        3,
        10,
        3,
        10,
    );

    let _ = fs::remove_dir_all(&dir);
}

// Broad fixture matrix for ordered mixed-effect branch boundaries. The size is
// intentional until the fixture setup has a clearer table-builder abstraction.
#[allow(clippy::too_many_lines)]
#[test]
fn semantic_query_reports_exact_safe_ordered_mixed_effect_branch_fragments() {
    let fixtures = [
        (
            "mixed_loop_append_a.ts",
            "function mixedLoopAppendLeft(enabled: boolean, xs: number[], out: number[], seed: number): void {\n  if (enabled) {\n    for (const x of xs) {\n      out.push(x + 1);\n    }\n    out.push(seed * seed);\n  }\n  audit(enabled);\n}\n",
        ),
        (
            "mixed_loop_append_b.ts",
            "function mixedLoopAppendRight(flag: boolean, ys: number[], dst: number[], base: number): void {\n  if (flag) {\n    for (const y of ys) {\n      dst.push(y + 1);\n    }\n    dst.push(base * base);\n  }\n  trace(flag);\n}\n",
        ),
        (
            "mixed_loop_append_wrong_order.ts",
            "function mixedLoopAppendWrongOrder(flag: boolean, ys: number[], dst: number[], base: number): void {\n  if (flag) {\n    dst.push(base * base);\n    for (const y of ys) {\n      dst.push(y + 1);\n    }\n  }\n  trace(flag);\n}\n",
        ),
        (
            "mixed_loop_append_wrong_receiver.ts",
            "function mixedLoopAppendWrongReceiver(flag: boolean, ys: number[], dst: number[], base: number): void {\n  if (flag) {\n    for (const y of ys) {\n      dst.push(y + 1);\n    }\n    ys.push(base * base);\n  }\n  trace(flag);\n}\n",
        ),
        (
            "mixed_loop_append_mutated.ts",
            "function mixedLoopAppendMutated(flag: boolean, ys: number[], dst: number[], base: number): void {\n  dst.push(0);\n  if (flag) {\n    for (const y of ys) {\n      dst.push(y + 1);\n    }\n    dst.push(base * base);\n  }\n  trace(flag);\n}\n",
        ),
        (
            "mixed_loop_append_third_effect.ts",
            "function mixedLoopAppendThirdEffect(flag: boolean, ys: number[], dst: number[], base: number): void {\n  if (flag) {\n    for (const y of ys) {\n      dst.push(y + 1);\n    }\n    dst.push(base * base);\n    dst.push(base + 1);\n  }\n  trace(flag);\n}\n",
        ),
        (
            "mixed_append_loop_a.py",
            "def mixed_append_loop_left(flag: bool, xs: list[int], out: list[int], seed: int):\n    if flag:\n        out.append(seed + 1)\n        for x in xs:\n            value = x + 1\n            out.append(value)\n    audit(flag)\n",
        ),
        (
            "mixed_append_loop_b.py",
            "def mixed_append_loop_right(enabled: bool, ys: list[int], dst: list[int], base: int):\n    if enabled:\n        dst.append(1 + base)\n        for y in ys:\n            item = 1 + y\n            dst.append(item)\n    trace(enabled)\n",
        ),
        (
            "mixed_append_loop_wrong_temp.py",
            "def mixed_append_loop_wrong_temp(flag: bool, ys: list[int], dst: list[int], base: int):\n    if flag:\n        dst.append(1 + base)\n        for y in ys:\n            item = 2 + y\n            dst.append(item)\n    trace(flag)\n",
        ),
        (
            "mixed_append_loop_wrong_order.py",
            "def mixed_append_loop_wrong_order(flag: bool, ys: list[int], dst: list[int], base: int):\n    if flag:\n        for y in ys:\n            item = 1 + y\n            dst.append(item)\n        dst.append(1 + base)\n    trace(flag)\n",
        ),
        (
            "mixed_index_loop_a.go",
            "package p\nfunc mixedIndexLoopLeft(flag bool, xs []int, out []int, seed int) {\n  if flag {\n    for i, x := range xs {\n      out[i] = x * x\n    }\n    out[0] = seed + 1\n  }\n  audit(out)\n}\n",
        ),
        (
            "mixed_index_loop_b.go",
            "package p\nfunc mixedIndexLoopRight(enabled bool, ys []int, dst []int, base int) {\n  if enabled {\n    for j, y := range ys {\n      dst[j] = y * y\n    }\n    dst[0] = 1 + base\n  }\n  trace(dst)\n}\n",
        ),
        (
            "mixed_index_loop_wrong_index.go",
            "package p\nfunc mixedIndexLoopWrongIndex(flag bool, ys []int, dst []int, base int) {\n  if flag {\n    for j, y := range ys {\n      dst[j] = y * y\n    }\n    dst[1] = 1 + base\n  }\n  trace(dst)\n}\n",
        ),
        (
            "mixed_index_loop_wrong_receiver.go",
            "package p\nfunc mixedIndexLoopWrongReceiver(flag bool, ys []int, dst []int, base int) {\n  if flag {\n    for j, y := range ys {\n      dst[j] = y * y\n    }\n    ys[0] = 1 + base\n  }\n  trace(dst)\n}\n",
        ),
    ];
    let (out, families) = query_fragment_only_fixture_families(
        "nose_ordered_mixed_effect_branch_fragments",
        &fixtures,
    );

    assert_branch_pair_cases(
        &families,
        &out,
        "ordered mixed-effect branch",
        &[
            branch_pair(
                "mixed_loop_append_a.ts",
                "mixed_loop_append_b.ts",
                "mixed_loop_append_wrong_order.ts",
                2,
                7,
            ),
            branch_pair(
                "mixed_append_loop_a.py",
                "mixed_append_loop_b.py",
                "mixed_append_loop_wrong_temp.py",
                2,
                6,
            ),
            branch_pair(
                "mixed_index_loop_a.go",
                "mixed_index_loop_b.go",
                "mixed_index_loop_wrong_index.go",
                3,
                8,
            ),
        ],
        &[
            branch_non_pair(
                "mixed_loop_append_a.ts",
                "mixed_loop_append_wrong_order.ts",
                (2, 7),
                (2, 7),
            ),
            branch_non_pair(
                "mixed_loop_append_a.ts",
                "mixed_loop_append_wrong_receiver.ts",
                (2, 7),
                (2, 7),
            ),
            branch_non_pair(
                "mixed_loop_append_a.ts",
                "mixed_loop_append_mutated.ts",
                (2, 7),
                (3, 8),
            ),
            branch_non_pair(
                "mixed_loop_append_a.ts",
                "mixed_loop_append_third_effect.ts",
                (2, 7),
                (2, 8),
            ),
            branch_non_pair(
                "mixed_append_loop_a.py",
                "mixed_append_loop_wrong_temp.py",
                (2, 6),
                (2, 6),
            ),
            branch_non_pair(
                "mixed_append_loop_a.py",
                "mixed_append_loop_wrong_order.py",
                (2, 6),
                (2, 6),
            ),
            branch_non_pair(
                "mixed_index_loop_a.go",
                "mixed_index_loop_wrong_index.go",
                (3, 8),
                (3, 8),
            ),
            branch_non_pair(
                "mixed_index_loop_a.go",
                "mixed_index_loop_wrong_receiver.go",
                (3, 8),
                (3, 8),
            ),
        ],
    );
}
