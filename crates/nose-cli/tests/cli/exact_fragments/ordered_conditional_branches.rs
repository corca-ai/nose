use super::*;

// Broad fixture matrix for ordered conditional-effect branch boundaries. The size is
// intentional until the fixture setup has a clearer table-builder abstraction.
#[allow(clippy::too_many_lines)]
#[test]
fn semantic_query_reports_exact_safe_ordered_conditional_effect_branch_fragments() {
    let fixtures = [
        (
            "cond_pair_append_a.ts",
            "function condPairAppendLeft(enabled: boolean, x: number, y: number, out: number[]): void {\n  if (enabled) {\n    if (x > 0) {\n      out.push(x + 1);\n    }\n    if (y > 0) {\n      out.push(y * y);\n    }\n  }\n  audit(enabled);\n}\n",
        ),
        (
            "cond_pair_append_b.ts",
            "function condPairAppendRight(flag: boolean, a: number, b: number, dst: number[]): void {\n  if (flag) {\n    if (0 < a) {\n      dst.push(1 + a);\n    }\n    if (b > 0) {\n      dst.push(b * b);\n    }\n  }\n  trace(flag);\n}\n",
        ),
        (
            "cond_pair_append_wrong_order.ts",
            "function condPairAppendWrongOrder(flag: boolean, a: number, b: number, dst: number[]): void {\n  if (flag) {\n    if (b > 0) {\n      dst.push(b * b);\n    }\n    if (0 < a) {\n      dst.push(1 + a);\n    }\n  }\n  trace(flag);\n}\n",
        ),
        (
            "cond_pair_append_wrong_receiver.ts",
            "function condPairAppendWrongReceiver(flag: boolean, a: number, b: number, dst: number[], other: number[]): void {\n  if (flag) {\n    if (0 < a) {\n      dst.push(1 + a);\n    }\n    if (b > 0) {\n      other.push(b * b);\n    }\n  }\n  trace(flag);\n}\n",
        ),
        (
            "cond_pair_append_mutated.ts",
            "function condPairAppendMutated(flag: boolean, a: number, b: number, dst: number[]): void {\n  dst.push(0);\n  if (flag) {\n    if (0 < a) {\n      dst.push(1 + a);\n    }\n    if (b > 0) {\n      dst.push(b * b);\n    }\n  }\n  trace(flag);\n}\n",
        ),
        (
            "cond_pair_append_third.ts",
            "function condPairAppendThird(flag: boolean, a: number, b: number, c: number, dst: number[]): void {\n  if (flag) {\n    if (0 < a) {\n      dst.push(1 + a);\n    }\n    if (b > 0) {\n      dst.push(b * b);\n    }\n    if (c > 0) {\n      dst.push(c + 3);\n    }\n  }\n  trace(flag);\n}\n",
        ),
        (
            "cond_pair_append_a.py",
            "def cond_pair_append_left(flag: bool, x: int, y: int, out: list[int]):\n    if flag:\n        if x > 0:\n            out.append(x + 1)\n        if y > 0:\n            out.append(y * y)\n    audit(flag)\n",
        ),
        (
            "cond_pair_append_b.py",
            "def cond_pair_append_right(enabled: bool, a: int, b: int, dst: list[int]):\n    if enabled:\n        if 0 < a:\n            dst.append(1 + a)\n        if b > 0:\n            dst.append(b * b)\n    trace(enabled)\n",
        ),
        (
            "cond_pair_append_wrong_guard.py",
            "def cond_pair_append_wrong_guard(flag: bool, a: int, b: int, dst: list[int]):\n    if flag:\n        if 0 < a:\n            dst.append(1 + a)\n        if b >= 0:\n            dst.append(b * b)\n    trace(flag)\n",
        ),
        (
            "cond_pair_append_wrong_order.py",
            "def cond_pair_append_wrong_order(flag: bool, a: int, b: int, dst: list[int]):\n    if flag:\n        if b > 0:\n            dst.append(b * b)\n        if 0 < a:\n            dst.append(1 + a)\n    trace(flag)\n",
        ),
        (
            "cond_pair_index_a.go",
            "package p\nfunc condPairIndexLeft(flag bool, x int, y int, out []int) {\n  if flag {\n    if x > 0 {\n      out[0] = x + 1\n    }\n    if y > 0 {\n      out[1] = y * y\n    }\n  }\n  audit(out)\n}\n",
        ),
        (
            "cond_pair_index_b.go",
            "package p\nfunc condPairIndexRight(enabled bool, a int, b int, dst []int) {\n  if enabled {\n    if 0 < a {\n      dst[0] = 1 + a\n    }\n    if b > 0 {\n      dst[1] = b * b\n    }\n  }\n  trace(dst)\n}\n",
        ),
        (
            "cond_pair_index_wrong_index.go",
            "package p\nfunc condPairIndexWrongIndex(flag bool, a int, b int, dst []int) {\n  if flag {\n    if 0 < a {\n      dst[0] = 1 + a\n    }\n    if b > 0 {\n      dst[2] = b * b\n    }\n  }\n  trace(dst)\n}\n",
        ),
        (
            "cond_pair_index_wrong_receiver.go",
            "package p\nfunc condPairIndexWrongReceiver(flag bool, a int, b int, dst []int, other []int) {\n  if flag {\n    if 0 < a {\n      dst[0] = 1 + a\n    }\n    if b > 0 {\n      other[1] = b * b\n    }\n  }\n  trace(dst)\n}\n",
        ),
    ];
    let (out, families) = query_fragment_only_fixture_families(
        "nose_ordered_conditional_effect_branch_fragments",
        &fixtures,
    );

    assert_branch_pair_cases(
        &families,
        &out,
        "ordered conditional-effect branch",
        &[
            branch_pair(
                "cond_pair_append_a.ts",
                "cond_pair_append_b.ts",
                "cond_pair_append_wrong_order.ts",
                2,
                9,
            ),
            branch_pair(
                "cond_pair_append_a.py",
                "cond_pair_append_b.py",
                "cond_pair_append_wrong_guard.py",
                2,
                6,
            ),
            branch_pair(
                "cond_pair_index_a.go",
                "cond_pair_index_b.go",
                "cond_pair_index_wrong_index.go",
                3,
                10,
            ),
        ],
        &[
            branch_non_pair(
                "cond_pair_append_a.ts",
                "cond_pair_append_wrong_order.ts",
                (2, 9),
                (2, 9),
            ),
            branch_non_pair(
                "cond_pair_append_a.ts",
                "cond_pair_append_wrong_receiver.ts",
                (2, 9),
                (2, 9),
            ),
            branch_non_pair(
                "cond_pair_append_a.ts",
                "cond_pair_append_mutated.ts",
                (2, 9),
                (3, 10),
            ),
            branch_non_pair(
                "cond_pair_append_a.ts",
                "cond_pair_append_third.ts",
                (2, 9),
                (2, 12),
            ),
            branch_non_pair(
                "cond_pair_append_a.py",
                "cond_pair_append_wrong_guard.py",
                (2, 6),
                (2, 6),
            ),
            branch_non_pair(
                "cond_pair_append_a.py",
                "cond_pair_append_wrong_order.py",
                (2, 6),
                (2, 6),
            ),
            branch_non_pair(
                "cond_pair_index_a.go",
                "cond_pair_index_wrong_index.go",
                (3, 10),
                (3, 10),
            ),
            branch_non_pair(
                "cond_pair_index_a.go",
                "cond_pair_index_wrong_receiver.go",
                (3, 10),
                (3, 10),
            ),
        ],
    );
}

// Broad fixture matrix for ordered conditional mixed-effect branch boundaries. The size is
// intentional until the fixture setup has a clearer table-builder abstraction.
#[allow(clippy::too_many_lines)]
#[test]
fn semantic_query_reports_exact_safe_ordered_conditional_mixed_effect_branch_fragments() {
    let fixtures = [
        (
            "cond_mixed_append_a.ts",
            "function condMixedAppendLeft(enabled: boolean, x: number, y: number, out: number[]): void {\n  if (enabled) {\n    if (x > 0) {\n      out.push(x + 1);\n    }\n    out.push(y * y);\n  }\n  audit(enabled);\n}\n",
        ),
        (
            "cond_mixed_append_b.ts",
            "function condMixedAppendRight(flag: boolean, a: number, b: number, dst: number[]): void {\n  if (flag) {\n    if (0 < a) {\n      dst.push(1 + a);\n    }\n    dst.push(b * b);\n  }\n  trace(flag);\n}\n",
        ),
        (
            "cond_mixed_append_wrong_order.ts",
            "function condMixedAppendWrongOrder(flag: boolean, a: number, b: number, dst: number[]): void {\n  if (flag) {\n    dst.push(b * b);\n    if (0 < a) {\n      dst.push(1 + a);\n    }\n  }\n  trace(flag);\n}\n",
        ),
        (
            "cond_mixed_append_wrong_guard.ts",
            "function condMixedAppendWrongGuard(flag: boolean, a: number, b: number, dst: number[]): void {\n  if (flag) {\n    if (0 <= a) {\n      dst.push(1 + a);\n    }\n    dst.push(b * b);\n  }\n  trace(flag);\n}\n",
        ),
        (
            "cond_mixed_append_wrong_receiver.ts",
            "function condMixedAppendWrongReceiver(flag: boolean, a: number, b: number, dst: number[], other: number[]): void {\n  if (flag) {\n    if (0 < a) {\n      dst.push(1 + a);\n    }\n    other.push(b * b);\n  }\n  trace(flag);\n}\n",
        ),
        (
            "cond_mixed_append_mutated.ts",
            "function condMixedAppendMutated(flag: boolean, a: number, b: number, dst: number[]): void {\n  dst.push(0);\n  if (flag) {\n    if (0 < a) {\n      dst.push(1 + a);\n    }\n    dst.push(b * b);\n  }\n  trace(flag);\n}\n",
        ),
        (
            "cond_mixed_append_third.ts",
            "function condMixedAppendThird(flag: boolean, a: number, b: number, c: number, dst: number[]): void {\n  if (flag) {\n    if (0 < a) {\n      dst.push(1 + a);\n    }\n    dst.push(b * b);\n    dst.push(c + 3);\n  }\n  trace(flag);\n}\n",
        ),
        (
            "cond_mixed_append_a.py",
            "def cond_mixed_append_left(flag: bool, x: int, y: int, out: list[int]):\n    if flag:\n        out.append(y * y)\n        if x > 0:\n            out.append(x + 1)\n    audit(flag)\n",
        ),
        (
            "cond_mixed_append_b.py",
            "def cond_mixed_append_right(enabled: bool, a: int, b: int, dst: list[int]):\n    if enabled:\n        dst.append(b * b)\n        if 0 < a:\n            dst.append(1 + a)\n    trace(enabled)\n",
        ),
        (
            "cond_mixed_append_wrong_order.py",
            "def cond_mixed_append_wrong_order(flag: bool, a: int, b: int, dst: list[int]):\n    if flag:\n        if 0 < a:\n            dst.append(1 + a)\n        dst.append(b * b)\n    trace(flag)\n",
        ),
        (
            "cond_mixed_append_wrong_guard.py",
            "def cond_mixed_append_wrong_guard(flag: bool, a: int, b: int, dst: list[int]):\n    if flag:\n        dst.append(b * b)\n        if 0 <= a:\n            dst.append(1 + a)\n    trace(flag)\n",
        ),
        (
            "cond_mixed_index_a.go",
            "package p\nfunc condMixedIndexLeft(flag bool, x int, y int, out []int) {\n  if flag {\n    if x > 0 {\n      out[0] = x + 1\n    }\n    out[1] = y * y\n  }\n  audit(out)\n}\n",
        ),
        (
            "cond_mixed_index_b.go",
            "package p\nfunc condMixedIndexRight(enabled bool, a int, b int, dst []int) {\n  if enabled {\n    if 0 < a {\n      dst[0] = 1 + a\n    }\n    dst[1] = b * b\n  }\n  trace(dst)\n}\n",
        ),
        (
            "cond_mixed_index_wrong_index.go",
            "package p\nfunc condMixedIndexWrongIndex(flag bool, a int, b int, dst []int) {\n  if flag {\n    if 0 < a {\n      dst[0] = 1 + a\n    }\n    dst[2] = b * b\n  }\n  trace(dst)\n}\n",
        ),
        (
            "cond_mixed_index_wrong_receiver.go",
            "package p\nfunc condMixedIndexWrongReceiver(flag bool, a int, b int, dst []int, other []int) {\n  if flag {\n    if 0 < a {\n      dst[0] = 1 + a\n    }\n    other[1] = b * b\n  }\n  trace(dst)\n}\n",
        ),
    ];
    let (out, families) = query_fragment_only_fixture_families(
        "nose_ordered_conditional_mixed_effect_branch_fragments",
        &fixtures,
    );

    assert_branch_pair_cases(
        &families,
        &out,
        "ordered conditional mixed-effect branch",
        &[
            branch_pair(
                "cond_mixed_append_a.ts",
                "cond_mixed_append_b.ts",
                "cond_mixed_append_wrong_order.ts",
                2,
                7,
            ),
            branch_pair(
                "cond_mixed_append_a.py",
                "cond_mixed_append_b.py",
                "cond_mixed_append_wrong_guard.py",
                2,
                5,
            ),
            branch_pair(
                "cond_mixed_index_a.go",
                "cond_mixed_index_b.go",
                "cond_mixed_index_wrong_index.go",
                3,
                8,
            ),
        ],
        &[
            branch_non_pair(
                "cond_mixed_append_a.ts",
                "cond_mixed_append_wrong_order.ts",
                (2, 7),
                (2, 7),
            ),
            branch_non_pair(
                "cond_mixed_append_a.ts",
                "cond_mixed_append_wrong_guard.ts",
                (2, 7),
                (2, 7),
            ),
            branch_non_pair(
                "cond_mixed_append_a.ts",
                "cond_mixed_append_wrong_receiver.ts",
                (2, 7),
                (2, 7),
            ),
            branch_non_pair(
                "cond_mixed_append_a.ts",
                "cond_mixed_append_mutated.ts",
                (2, 7),
                (3, 8),
            ),
            branch_non_pair(
                "cond_mixed_append_a.ts",
                "cond_mixed_append_third.ts",
                (2, 7),
                (2, 8),
            ),
            branch_non_pair(
                "cond_mixed_append_a.py",
                "cond_mixed_append_wrong_order.py",
                (2, 5),
                (2, 5),
            ),
            branch_non_pair(
                "cond_mixed_append_a.py",
                "cond_mixed_append_wrong_guard.py",
                (2, 5),
                (2, 5),
            ),
            branch_non_pair(
                "cond_mixed_index_a.go",
                "cond_mixed_index_wrong_index.go",
                (3, 8),
                (3, 8),
            ),
            branch_non_pair(
                "cond_mixed_index_a.go",
                "cond_mixed_index_wrong_receiver.go",
                (3, 8),
                (3, 8),
            ),
        ],
    );
}
