use super::*;

// Broad fixture matrix for exact Go foreach index-assignment fragments. The size is
// intentional until the fixture setup has a clearer table-builder abstraction.
#[allow(clippy::too_many_lines)]
#[test]
fn semantic_query_reports_exact_safe_foreach_index_assignment_fragments_for_go() {
    let fixtures = [
        (
            "loop_index_square_a.go",
            "package p\nfunc loopIndexSquareLeft(xs []int, out []int) {\n  for i, x := range xs {\n    out[i] = x * x\n  }\n  audit(out)\n}\n",
        ),
        (
            "loop_index_square_b.go",
            "package p\nfunc loopIndexSquareRight(ys []int, dst []int) {\n  for j, y := range ys {\n    dst[j] = y * y\n  }\n  trace(dst)\n}\n",
        ),
        (
            "loop_index_square_wrong_value.go",
            "package p\nfunc loopIndexSquareWrongValue(zs []int, dst []int) {\n  for k, z := range zs {\n    dst[k] = z + z\n  }\n  trace(dst)\n}\n",
        ),
        (
            "loop_index_offset_a.go",
            "package p\nfunc loopIndexOffsetLeft(xs []int, out []int) {\n  for i, x := range xs {\n    out[i + 1] = x + 1\n  }\n  audit(out)\n}\n",
        ),
        (
            "loop_index_offset_b.go",
            "package p\nfunc loopIndexOffsetRight(ys []int, dst []int) {\n  for j, y := range ys {\n    dst[1 + j] = 1 + y\n  }\n  trace(dst)\n}\n",
        ),
        (
            "loop_index_offset_wrong_index.go",
            "package p\nfunc loopIndexOffsetWrongIndex(zs []int, dst []int) {\n  for k, z := range zs {\n    dst[2 + k] = 1 + z\n  }\n  trace(dst)\n}\n",
        ),
        (
            "loop_index_guard_a.go",
            "package p\nfunc loopIndexGuardLeft(xs []int, out []int) {\n  for i, x := range xs {\n    if x > 0 {\n      out[i] = x + 1\n    }\n  }\n  audit(out)\n}\n",
        ),
        (
            "loop_index_guard_b.go",
            "package p\nfunc loopIndexGuardRight(ys []int, dst []int) {\n  for j, y := range ys {\n    if 0 < y {\n      dst[j] = 1 + y\n    }\n  }\n  trace(dst)\n}\n",
        ),
        (
            "loop_index_guard_wrong_guard.go",
            "package p\nfunc loopIndexGuardWrongGuard(zs []int, dst []int) {\n  for k, z := range zs {\n    if z >= 0 {\n      dst[k] = 1 + z\n    }\n  }\n  trace(dst)\n}\n",
        ),
        (
            "loop_index_wrong_receiver.go",
            "package p\nfunc loopIndexWrongReceiver(xs []int, out []int) {\n  for i, x := range xs {\n    xs[i] = x * x\n  }\n  audit(out)\n}\n",
        ),
        (
            "loop_index_mutated.go",
            "package p\nfunc loopIndexMutated(xs []int, out []int) {\n  out[0] = 0\n  for i, x := range xs {\n    out[i] = x * x\n  }\n  audit(out)\n}\n",
        ),
        (
            "loop_index_temp_square_a.go",
            "package p\nfunc loopIndexTempSquareLeft(xs []int, out []int) {\n  for i, x := range xs {\n    squared := x * x\n    out[i] = squared\n  }\n  audit(out)\n}\n",
        ),
        (
            "loop_index_temp_square_b.go",
            "package p\nfunc loopIndexTempSquareRight(ys []int, dst []int) {\n  for j, y := range ys {\n    result := y * y\n    dst[j] = result\n  }\n  trace(dst)\n}\n",
        ),
        (
            "loop_index_temp_square_wrong.go",
            "package p\nfunc loopIndexTempSquareWrong(zs []int, dst []int) {\n  for k, z := range zs {\n    result := z + z\n    dst[k] = result\n  }\n  trace(dst)\n}\n",
        ),
        (
            "loop_index_temp_unconsumed.go",
            "package p\nfunc loopIndexTempUnconsumed(xs []int, out []int) {\n  for i, x := range xs {\n    squared := x * x\n    out[i] = x * x\n  }\n  audit(out)\n}\n",
        ),
        (
            "loop_index_temp_chain_a.go",
            "package p\nfunc loopIndexTempChainLeft(xs []int, out []int) {\n  for i, x := range xs {\n    shifted := x + 1\n    squared := shifted * shifted\n    out[i] = squared\n  }\n  audit(out)\n}\n",
        ),
        (
            "loop_index_temp_chain_b.go",
            "package p\nfunc loopIndexTempChainRight(ys []int, dst []int) {\n  for j, y := range ys {\n    offset := 1 + y\n    result := offset * offset\n    dst[j] = result\n  }\n  trace(dst)\n}\n",
        ),
        (
            "loop_index_temp_chain_wrong.go",
            "package p\nfunc loopIndexTempChainWrong(zs []int, dst []int) {\n  for k, z := range zs {\n    shifted := z + 2\n    result := shifted * shifted\n    dst[k] = result\n  }\n  trace(dst)\n}\n",
        ),
        (
            "loop_index_temp_chain_unconsumed.go",
            "package p\nfunc loopIndexTempChainUnconsumed(xs []int, out []int) {\n  for i, x := range xs {\n    shifted := x + 1\n    squared := x * x\n    out[i] = squared\n  }\n  audit(out)\n}\n",
        ),
        (
            "loop_index_temp_chain_uses_prior.go",
            "package p\nfunc loopIndexTempChainUsesPrior(xs []int, out []int) {\n  for i, x := range xs {\n    shifted := x + 1\n    squared := shifted * shifted\n    out[i] = squared + shifted\n  }\n  audit(out)\n}\n",
        ),
        (
            "loop_index_unused.go",
            "package p\nfunc loopIndexUnused(xs []int, out []int) {\n  for _, x := range xs {\n    out[0] = 1\n  }\n  audit(out)\n}\n",
        ),
        (
            "direct_index_unused.go",
            "package p\nfunc directIndexUnused(out []int) {\n  out[0] = 1\n  audit(out)\n}\n",
        ),
    ];
    let (dir, out, families) =
        query_fragment_only_fixtures("nose_exact_foreach_index_assign_fragments", &fixtures);

    let assert_loop_family =
        |left: &str, right: &str, negative: &str, start_line: u64, end_line: u64| {
            let family = families
                .iter()
                .find(|family| {
                    let locations = family["locations"].as_array().expect("locations");
                    let loop_files: Vec<&str> = locations
                        .iter()
                        .filter(|loc| {
                            loc["start_line"] == start_line && loc["end_line"] == end_line
                        })
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
                    panic!(
                    "missing exact foreach index-assignment fragment family {left}/{right}: {out}"
                )
                });
            assert!(
                family["locations"]
                    .as_array()
                    .expect("locations")
                    .iter()
                    .all(|loc| loc["kind"] == "Block"),
                "foreach index-assignment fragments should report as Block units: {family:?}"
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
            "foreach index assignment boundary must not merge {left}/{right}: {out}"
        );
    };

    assert_loop_family(
        "loop_index_square_a.go",
        "loop_index_square_b.go",
        "loop_index_square_wrong_value.go",
        3,
        5,
    );
    assert_loop_family(
        "loop_index_offset_a.go",
        "loop_index_offset_b.go",
        "loop_index_offset_wrong_index.go",
        3,
        5,
    );
    assert_loop_family(
        "loop_index_guard_a.go",
        "loop_index_guard_b.go",
        "loop_index_guard_wrong_guard.go",
        3,
        7,
    );
    assert_loop_family(
        "loop_index_temp_square_a.go",
        "loop_index_temp_square_b.go",
        "loop_index_temp_square_wrong.go",
        3,
        6,
    );
    assert_loop_family(
        "loop_index_temp_chain_a.go",
        "loop_index_temp_chain_b.go",
        "loop_index_temp_chain_wrong.go",
        3,
        7,
    );
    assert_no_pair("loop_index_square_a.go", "loop_index_wrong_receiver.go");
    assert_no_pair("loop_index_square_a.go", "loop_index_mutated.go");
    assert_no_pair("loop_index_unused.go", "direct_index_unused.go");
    assert_no_pair(
        "loop_index_temp_square_a.go",
        "loop_index_temp_unconsumed.go",
    );
    assert_no_pair(
        "loop_index_temp_chain_a.go",
        "loop_index_temp_chain_unconsumed.go",
    );
    assert_no_pair(
        "loop_index_temp_chain_a.go",
        "loop_index_temp_chain_uses_prior.go",
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn semantic_query_reports_exact_safe_conditional_foreach_append_effect_fragments_under_opaque_functions(
) {
    let fixtures = [
        (
            "cond_loop_square_a.ts",
            "function condLoopSquareLeft(enabled: boolean, xs: number[], out: number[]): void {\n  if (enabled) {\n    for (const x of xs) {\n      out.push(x * x);\n    }\n  }\n  audit(enabled);\n}\n",
        ),
        (
            "cond_loop_square_b.ts",
            "function condLoopSquareRight(flag: boolean, ys: number[], dst: number[]): void {\n  if (flag) {\n    for (const y of ys) {\n      dst.push(y * y);\n    }\n  }\n  trace(flag);\n}\n",
        ),
        (
            "cond_loop_square_wrong_guard.ts",
            "function condLoopSquareWrongGuard(flag: boolean, ys: number[], dst: number[]): void {\n  if (!flag) {\n    for (const y of ys) {\n      dst.push(y * y);\n    }\n  }\n  trace(flag);\n}\n",
        ),
        (
            "cond_loop_else_a.ts",
            "function condLoopElseLeft(skip: boolean, xs: number[], out: number[]): void {\n  if (skip) {\n  } else {\n    for (const x of xs) {\n      out.push(x + 1);\n    }\n  }\n  audit(skip);\n}\n",
        ),
        (
            "cond_loop_else_b.ts",
            "function condLoopElseRight(flag: boolean, ys: number[], dst: number[]): void {\n  if (flag) {\n  } else {\n    for (const y of ys) {\n      dst.push(y + 1);\n    }\n  }\n  trace(flag);\n}\n",
        ),
        (
            "cond_loop_else_wrong_receiver.ts",
            "function condLoopElseWrongReceiver(flag: boolean, ys: number[], dst: number[]): void {\n  if (flag) {\n  } else {\n    for (const y of ys) {\n      ys.push(y + 1);\n    }\n  }\n  trace(flag);\n}\n",
        ),
        (
            "cond_loop_guard_a.ts",
            "function condLoopGuardLeft(enabled: boolean, xs: number[], out: number[]): void {\n  if (enabled) {\n    for (const x of xs) {\n      if (x > 0) out.push(x + 1);\n    }\n  }\n  audit(enabled);\n}\n",
        ),
        (
            "cond_loop_guard_b.ts",
            "function condLoopGuardRight(flag: boolean, ys: number[], dst: number[]): void {\n  if (flag) {\n    for (const y of ys) {\n      if (0 < y) dst.push(y + 1);\n    }\n  }\n  trace(flag);\n}\n",
        ),
        (
            "cond_loop_guard_wrong_value.ts",
            "function condLoopGuardWrongValue(flag: boolean, ys: number[], dst: number[]): void {\n  if (flag) {\n    for (const y of ys) {\n      if (0 < y) dst.push(y + 2);\n    }\n  }\n  trace(flag);\n}\n",
        ),
        (
            "cond_loop_mutated.ts",
            "function condLoopMutated(enabled: boolean, xs: number[], out: number[]): void {\n  out.push(0);\n  if (enabled) {\n    for (const x of xs) {\n      out.push(x * x);\n    }\n  }\n  audit(enabled);\n}\n",
        ),
        (
            "cond_loop_unused.ts",
            "function condLoopUnused(enabled: boolean, xs: number[], out: number[]): void {\n  if (enabled) {\n    for (const unused of xs) {\n      out.push(1);\n    }\n  }\n  audit(enabled);\n}\n",
        ),
        (
            "cond_direct_unused.ts",
            "function condDirectUnused(enabled: boolean, out: number[]): void {\n  if (enabled) {\n    out.push(1);\n  }\n  audit(enabled);\n}\n",
        ),
    ];
    let (dir, out, families) =
        query_fragment_only_fixtures("nose_exact_conditional_loop_effect_fragments", &fixtures);

    let assert_conditional_loop_family = |left: &str,
                                          right: &str,
                                          negative: &str,
                                          start_line: u64,
                                          end_line: u64| {
        let family =
                find_block_pair_family_at(&families, left, right, negative, start_line, end_line)
                    .unwrap_or_else(|| {
                        panic!(
                            "missing exact conditional foreach append-effect fragment family {left}/{right}: {out}"
                        )
                    });
        assert!(
            family_all_blocks(family),
            "conditional foreach append-effect fragments should report as Block units: {family:?}"
        );
    };

    let assert_no_pair = |left: &str, right: &str| {
        assert!(
            !has_pair_family(&families, left, right),
            "conditional foreach append effect boundary must not merge {left}/{right}: {out}"
        );
    };

    assert_conditional_loop_family(
        "cond_loop_square_a.ts",
        "cond_loop_square_b.ts",
        "cond_loop_square_wrong_guard.ts",
        2,
        6,
    );
    assert_no_pair("cond_loop_square_a.ts", "cond_loop_mutated.ts");
    assert_conditional_loop_family(
        "cond_loop_else_a.ts",
        "cond_loop_else_b.ts",
        "cond_loop_else_wrong_receiver.ts",
        2,
        7,
    );
    assert_conditional_loop_family(
        "cond_loop_guard_a.ts",
        "cond_loop_guard_b.ts",
        "cond_loop_guard_wrong_value.ts",
        2,
        6,
    );
    assert_no_pair("cond_loop_unused.ts", "cond_direct_unused.ts");
    let _ = fs::remove_dir_all(&dir);
}
