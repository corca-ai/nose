use super::*;

// Broad fixture matrix for ordered loop conditional-effect branch boundaries. The size is
// intentional until the fixture setup has a clearer table-builder abstraction.
#[allow(clippy::too_many_lines)]
#[test]
fn semantic_query_reports_exact_safe_ordered_loop_conditional_effect_branch_fragments() {
    let fixtures = [
        (
            "loop_cond_append_a.ts",
            "function loopCondAppendLeft(enabled: boolean, xs: number[], y: number, out: number[]): void {\n  if (enabled) {\n    for (const x of xs) {\n      out.push(x + 1);\n    }\n    if (y > 0) {\n      out.push(y * y);\n    }\n  }\n  audit(enabled);\n}\n",
        ),
        (
            "loop_cond_append_b.ts",
            "function loopCondAppendRight(flag: boolean, ys: number[], b: number, dst: number[]): void {\n  if (flag) {\n    for (const a of ys) {\n      dst.push(a + 1);\n    }\n    if (b > 0) {\n      dst.push(b * b);\n    }\n  }\n  trace(flag);\n}\n",
        ),
        (
            "loop_cond_append_wrong_order.ts",
            "function loopCondAppendWrongOrder(flag: boolean, ys: number[], b: number, dst: number[]): void {\n  if (flag) {\n    if (b > 0) {\n      dst.push(b * b);\n    }\n    for (const a of ys) {\n      dst.push(a + 1);\n    }\n  }\n  trace(flag);\n}\n",
        ),
        (
            "loop_cond_append_wrong_guard.ts",
            "function loopCondAppendWrongGuard(flag: boolean, ys: number[], b: number, dst: number[]): void {\n  if (flag) {\n    for (const a of ys) {\n      dst.push(a + 1);\n    }\n    if (b >= 0) {\n      dst.push(b * b);\n    }\n  }\n  trace(flag);\n}\n",
        ),
        (
            "loop_cond_append_wrong_receiver.ts",
            "function loopCondAppendWrongReceiver(flag: boolean, ys: number[], b: number, dst: number[], other: number[]): void {\n  if (flag) {\n    for (const a of ys) {\n      dst.push(a + 1);\n    }\n    if (b > 0) {\n      other.push(b * b);\n    }\n  }\n  trace(flag);\n}\n",
        ),
        (
            "loop_cond_append_mutated.ts",
            "function loopCondAppendMutated(flag: boolean, ys: number[], b: number, dst: number[]): void {\n  dst.push(0);\n  if (flag) {\n    for (const a of ys) {\n      dst.push(a + 1);\n    }\n    if (b > 0) {\n      dst.push(b * b);\n    }\n  }\n  trace(flag);\n}\n",
        ),
        (
            "loop_cond_append_third.ts",
            "function loopCondAppendThird(flag: boolean, ys: number[], b: number, c: number, dst: number[]): void {\n  if (flag) {\n    for (const a of ys) {\n      dst.push(a + 1);\n    }\n    if (b > 0) {\n      dst.push(b * b);\n    }\n    dst.push(c + 3);\n  }\n  trace(flag);\n}\n",
        ),
        (
            "loop_cond_append_a.py",
            "def loop_cond_append_left(flag: bool, xs: list[int], y: int, out: list[int]):\n    if flag:\n        if y > 0:\n            out.append(y * y)\n        for x in xs:\n            value = x + 1\n            out.append(value)\n    audit(flag)\n",
        ),
        (
            "loop_cond_append_b.py",
            "def loop_cond_append_right(enabled: bool, ys: list[int], b: int, dst: list[int]):\n    if enabled:\n        if b > 0:\n            dst.append(b * b)\n        for a in ys:\n            item = 1 + a\n            dst.append(item)\n    trace(enabled)\n",
        ),
        (
            "loop_cond_append_wrong_order.py",
            "def loop_cond_append_wrong_order(flag: bool, ys: list[int], b: int, dst: list[int]):\n    if flag:\n        for a in ys:\n            item = 1 + a\n            dst.append(item)\n        if b > 0:\n            dst.append(b * b)\n    trace(flag)\n",
        ),
        (
            "loop_cond_append_wrong_temp.py",
            "def loop_cond_append_wrong_temp(flag: bool, ys: list[int], b: int, dst: list[int]):\n    if flag:\n        if b > 0:\n            dst.append(b * b)\n        for a in ys:\n            item = 2 + a\n            dst.append(item)\n    trace(flag)\n",
        ),
        (
            "loop_cond_index_a.go",
            "package p\nfunc loopCondIndexLeft(flag bool, xs []int, y int, out []int) {\n  if flag {\n    for i, x := range xs {\n      out[i] = x * x\n    }\n    if y > 0 {\n      out[0] = y + 1\n    }\n  }\n  audit(out)\n}\n",
        ),
        (
            "loop_cond_index_b.go",
            "package p\nfunc loopCondIndexRight(enabled bool, ys []int, b int, dst []int) {\n  if enabled {\n    for j, a := range ys {\n      dst[j] = a * a\n    }\n    if b > 0 {\n      dst[0] = 1 + b\n    }\n  }\n  trace(dst)\n}\n",
        ),
        (
            "loop_cond_index_wrong_index.go",
            "package p\nfunc loopCondIndexWrongIndex(flag bool, ys []int, b int, dst []int) {\n  if flag {\n    for j, a := range ys {\n      dst[j] = a * a\n    }\n    if b > 0 {\n      dst[1] = 1 + b\n    }\n  }\n  trace(dst)\n}\n",
        ),
        (
            "loop_cond_index_wrong_receiver.go",
            "package p\nfunc loopCondIndexWrongReceiver(flag bool, ys []int, b int, dst []int, other []int) {\n  if flag {\n    for j, a := range ys {\n      dst[j] = a * a\n    }\n    if b > 0 {\n      other[0] = 1 + b\n    }\n  }\n  trace(dst)\n}\n",
        ),
    ];
    let (out, families) = query_fragment_only_fixture_families(
        "nose_ordered_loop_conditional_effect_branch_fragments",
        &fixtures,
    );

    assert_branch_pair_cases(
        &families,
        &out,
        "ordered loop conditional-effect branch",
        &[
            branch_pair(
                "loop_cond_append_a.ts",
                "loop_cond_append_b.ts",
                "loop_cond_append_wrong_order.ts",
                2,
                9,
            ),
            branch_pair(
                "loop_cond_append_a.py",
                "loop_cond_append_b.py",
                "loop_cond_append_wrong_temp.py",
                2,
                7,
            ),
            branch_pair(
                "loop_cond_index_a.go",
                "loop_cond_index_b.go",
                "loop_cond_index_wrong_index.go",
                3,
                10,
            ),
        ],
        &[
            branch_non_pair(
                "loop_cond_append_a.ts",
                "loop_cond_append_wrong_order.ts",
                (2, 9),
                (2, 9),
            ),
            branch_non_pair(
                "loop_cond_append_a.ts",
                "loop_cond_append_wrong_guard.ts",
                (2, 9),
                (2, 9),
            ),
            branch_non_pair(
                "loop_cond_append_a.ts",
                "loop_cond_append_wrong_receiver.ts",
                (2, 9),
                (2, 9),
            ),
            branch_non_pair(
                "loop_cond_append_a.ts",
                "loop_cond_append_mutated.ts",
                (2, 9),
                (3, 10),
            ),
            branch_non_pair(
                "loop_cond_append_a.ts",
                "loop_cond_append_third.ts",
                (2, 9),
                (2, 10),
            ),
            branch_non_pair(
                "loop_cond_append_a.py",
                "loop_cond_append_wrong_order.py",
                (2, 7),
                (2, 7),
            ),
            branch_non_pair(
                "loop_cond_append_a.py",
                "loop_cond_append_wrong_temp.py",
                (2, 7),
                (2, 7),
            ),
            branch_non_pair(
                "loop_cond_index_a.go",
                "loop_cond_index_wrong_index.go",
                (3, 10),
                (3, 10),
            ),
            branch_non_pair(
                "loop_cond_index_a.go",
                "loop_cond_index_wrong_receiver.go",
                (3, 10),
                (3, 10),
            ),
        ],
    );
}

// Broad fixture matrix for ordered loop conditional mixed-effect branch boundaries. The size is
// intentional until the fixture setup has a clearer table-builder abstraction.
#[allow(clippy::too_many_lines)]
#[test]
fn semantic_query_reports_exact_safe_ordered_loop_conditional_mixed_effect_branch_fragments() {
    let fixtures = [
        (
            "loop_cond_mixed_append_a.ts",
            "function loopCondMixedLeft(enabled: boolean, xs: number[], y: number, z: number, out: number[]): void {\n  if (enabled) {\n    for (const x of xs) {\n      out.push(x + 1);\n    }\n    if (y > 0) {\n      out.push(y * y);\n    }\n    out.push(z + 3);\n  }\n  audit(enabled);\n}\n",
        ),
        (
            "loop_cond_mixed_append_b.ts",
            "function loopCondMixedRight(flag: boolean, ys: number[], b: number, c: number, dst: number[]): void {\n  if (flag) {\n    for (const a of ys) {\n      dst.push(a + 1);\n    }\n    if (b > 0) {\n      dst.push(b * b);\n    }\n    dst.push(c + 3);\n  }\n  trace(flag);\n}\n",
        ),
        (
            "loop_cond_mixed_append_wrong_order.ts",
            "function loopCondMixedWrongOrder(flag: boolean, ys: number[], b: number, c: number, dst: number[]): void {\n  if (flag) {\n    if (b > 0) {\n      dst.push(b * b);\n    }\n    for (const a of ys) {\n      dst.push(a + 1);\n    }\n    dst.push(c + 3);\n  }\n  trace(flag);\n}\n",
        ),
        (
            "loop_cond_mixed_append_wrong_guard.ts",
            "function loopCondMixedWrongGuard(flag: boolean, ys: number[], b: number, c: number, dst: number[]): void {\n  if (flag) {\n    for (const a of ys) {\n      dst.push(a + 1);\n    }\n    if (b >= 0) {\n      dst.push(b * b);\n    }\n    dst.push(c + 3);\n  }\n  trace(flag);\n}\n",
        ),
        (
            "loop_cond_mixed_append_wrong_receiver.ts",
            "function loopCondMixedWrongReceiver(flag: boolean, ys: number[], b: number, c: number, dst: number[], other: number[]): void {\n  if (flag) {\n    for (const a of ys) {\n      dst.push(a + 1);\n    }\n    if (b > 0) {\n      dst.push(b * b);\n    }\n    other.push(c + 3);\n  }\n  trace(flag);\n}\n",
        ),
        (
            "loop_cond_mixed_append_mutated.ts",
            "function loopCondMixedMutated(flag: boolean, ys: number[], b: number, c: number, dst: number[]): void {\n  dst.push(0);\n  if (flag) {\n    for (const a of ys) {\n      dst.push(a + 1);\n    }\n    if (b > 0) {\n      dst.push(b * b);\n    }\n    dst.push(c + 3);\n  }\n  trace(flag);\n}\n",
        ),
        (
            "loop_cond_mixed_append_fourth.ts",
            "function loopCondMixedFourth(flag: boolean, ys: number[], b: number, c: number, d: number, dst: number[]): void {\n  if (flag) {\n    for (const a of ys) {\n      dst.push(a + 1);\n    }\n    if (b > 0) {\n      dst.push(b * b);\n    }\n    dst.push(c + 3);\n    dst.push(d + 4);\n  }\n  trace(flag);\n}\n",
        ),
        (
            "loop_cond_mixed_append_a.py",
            "def loop_cond_mixed_left(flag: bool, xs: list[int], y: int, z: int, out: list[int]):\n    if flag:\n        if y > 0:\n            out.append(y * y)\n        for x in xs:\n            value = x + 1\n            out.append(value)\n        out.append(z + 3)\n    audit(flag)\n",
        ),
        (
            "loop_cond_mixed_append_b.py",
            "def loop_cond_mixed_right(enabled: bool, ys: list[int], b: int, c: int, dst: list[int]):\n    if enabled:\n        if b > 0:\n            dst.append(b * b)\n        for a in ys:\n            item = 1 + a\n            dst.append(item)\n        dst.append(3 + c)\n    trace(enabled)\n",
        ),
        (
            "loop_cond_mixed_append_wrong_order.py",
            "def loop_cond_mixed_wrong_order(flag: bool, ys: list[int], b: int, c: int, dst: list[int]):\n    if flag:\n        for a in ys:\n            item = 1 + a\n            dst.append(item)\n        if b > 0:\n            dst.append(b * b)\n        dst.append(3 + c)\n    trace(flag)\n",
        ),
        (
            "loop_cond_mixed_append_wrong_temp.py",
            "def loop_cond_mixed_wrong_temp(flag: bool, ys: list[int], b: int, c: int, dst: list[int]):\n    if flag:\n        if b > 0:\n            dst.append(b * b)\n        for a in ys:\n            item = 2 + a\n            dst.append(item)\n        dst.append(3 + c)\n    trace(flag)\n",
        ),
        (
            "loop_cond_mixed_index_a.go",
            "package p\nfunc loopCondMixedIndexLeft(flag bool, xs []int, y int, z int, out []int) {\n  if flag {\n    for i, x := range xs {\n      out[i] = x * x\n    }\n    if y > 0 {\n      out[0] = y + 1\n    }\n    out[1] = z + 3\n  }\n  audit(out)\n}\n",
        ),
        (
            "loop_cond_mixed_index_b.go",
            "package p\nfunc loopCondMixedIndexRight(enabled bool, ys []int, b int, c int, dst []int) {\n  if enabled {\n    for j, a := range ys {\n      dst[j] = a * a\n    }\n    if b > 0 {\n      dst[0] = 1 + b\n    }\n    dst[1] = 3 + c\n  }\n  trace(dst)\n}\n",
        ),
        (
            "loop_cond_mixed_index_wrong_index.go",
            "package p\nfunc loopCondMixedIndexWrongIndex(flag bool, ys []int, b int, c int, dst []int) {\n  if flag {\n    for j, a := range ys {\n      dst[j] = a * a\n    }\n    if b > 0 {\n      dst[0] = 1 + b\n    }\n    dst[2] = 3 + c\n  }\n  trace(dst)\n}\n",
        ),
        (
            "loop_cond_mixed_index_wrong_receiver.go",
            "package p\nfunc loopCondMixedIndexWrongReceiver(flag bool, ys []int, b int, c int, dst []int, other []int) {\n  if flag {\n    for j, a := range ys {\n      dst[j] = a * a\n    }\n    if b > 0 {\n      dst[0] = 1 + b\n    }\n    other[1] = 3 + c\n  }\n  trace(dst)\n}\n",
        ),
    ];
    let (out, families) = query_fragment_only_fixture_families(
        "nose_ordered_loop_conditional_mixed_effect_branch_fragments",
        &fixtures,
    );

    assert_branch_pair_cases(
        &families,
        &out,
        "ordered loop conditional mixed-effect branch",
        &[
            branch_pair(
                "loop_cond_mixed_append_a.ts",
                "loop_cond_mixed_append_b.ts",
                "loop_cond_mixed_append_wrong_order.ts",
                2,
                10,
            ),
            branch_pair(
                "loop_cond_mixed_append_a.py",
                "loop_cond_mixed_append_b.py",
                "loop_cond_mixed_append_wrong_temp.py",
                2,
                8,
            ),
            branch_pair(
                "loop_cond_mixed_index_a.go",
                "loop_cond_mixed_index_b.go",
                "loop_cond_mixed_index_wrong_index.go",
                3,
                11,
            ),
        ],
        &[
            branch_non_pair(
                "loop_cond_mixed_append_a.ts",
                "loop_cond_mixed_append_wrong_order.ts",
                (2, 10),
                (2, 10),
            ),
            branch_non_pair(
                "loop_cond_mixed_append_a.ts",
                "loop_cond_mixed_append_wrong_guard.ts",
                (2, 10),
                (2, 10),
            ),
            branch_non_pair(
                "loop_cond_mixed_append_a.ts",
                "loop_cond_mixed_append_wrong_receiver.ts",
                (2, 10),
                (2, 10),
            ),
            branch_non_pair(
                "loop_cond_mixed_append_a.ts",
                "loop_cond_mixed_append_mutated.ts",
                (2, 10),
                (3, 11),
            ),
            branch_non_pair(
                "loop_cond_mixed_append_a.ts",
                "loop_cond_mixed_append_fourth.ts",
                (2, 10),
                (2, 11),
            ),
            branch_non_pair(
                "loop_cond_mixed_append_a.py",
                "loop_cond_mixed_append_wrong_order.py",
                (2, 8),
                (2, 8),
            ),
            branch_non_pair(
                "loop_cond_mixed_append_a.py",
                "loop_cond_mixed_append_wrong_temp.py",
                (2, 8),
                (2, 8),
            ),
            branch_non_pair(
                "loop_cond_mixed_index_a.go",
                "loop_cond_mixed_index_wrong_index.go",
                (3, 11),
                (3, 11),
            ),
            branch_non_pair(
                "loop_cond_mixed_index_a.go",
                "loop_cond_mixed_index_wrong_receiver.go",
                (3, 11),
                (3, 11),
            ),
        ],
    );
}
