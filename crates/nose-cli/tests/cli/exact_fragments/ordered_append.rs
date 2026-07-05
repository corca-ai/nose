use super::*;

// Broad fixture matrix for ordered append-effect branch boundaries. The size is
// intentional until the fixture setup has a clearer table-builder abstraction.
#[allow(clippy::too_many_lines)]
#[test]
fn semantic_query_reports_exact_safe_ordered_append_effect_branch_fragments() {
    let fixtures = [
        (
            "append_pair_a.ts",
            "function appendPairLeft(flag: boolean, out: number[], x: number): void {\n  if (flag) {\n    out.push(x + 1);\n    out.push(x + 2);\n  }\n  audit(/opaque/);\n}\n",
        ),
        (
            "append_pair_b.ts",
            "function appendPairRight(enabled: boolean, dst: number[], y: number): void {\n  if (enabled) {\n    dst.push(1 + y);\n    dst.push(2 + y);\n  }\n  trace(/opaque/);\n}\n",
        ),
        (
            "append_pair_wrong_order.ts",
            "function appendPairWrongOrder(flag: boolean, out: number[], x: number): void {\n  if (flag) {\n    out.push(x + 2);\n    out.push(x + 1);\n  }\n  audit(/opaque/);\n}\n",
        ),
        (
            "append_pair_wrong_receiver.ts",
            "function appendPairWrongReceiver(flag: boolean, out: number[], other: number[], x: number): void {\n  if (flag) {\n    out.push(x + 1);\n    other.push(x + 2);\n  }\n  audit(/opaque/);\n}\n",
        ),
        (
            "append_pair_mutated.ts",
            "function appendPairMutated(flag: boolean, out: number[], x: number): void {\n  out.push(0);\n  if (flag) {\n    out.push(x + 1);\n    out.push(x + 2);\n  }\n  audit(/opaque/);\n}\n",
        ),
        (
            "append_temp_pair_a.ts",
            "function appendTempPairLeft(flag: boolean, out: number[], x: number): void {\n  if (flag) {\n    const first = x + 1;\n    out.push(first);\n    out.push(x + 2);\n  }\n  audit(/opaque/);\n}\n",
        ),
        (
            "append_temp_pair_b.ts",
            "function appendTempPairRight(enabled: boolean, dst: number[], y: number): void {\n  if (enabled) {\n    dst.push(1 + y);\n    dst.push(2 + y);\n  }\n  trace(/opaque/);\n}\n",
        ),
        (
            "append_temp_pair_wrong.ts",
            "function appendTempPairWrong(flag: boolean, out: number[], x: number): void {\n  if (flag) {\n    const first = x + 3;\n    out.push(first);\n    out.push(x + 2);\n  }\n  audit(/opaque/);\n}\n",
        ),
        (
            "append_chain_pair_a.ts",
            "function appendChainPairLeft(flag: boolean, out: number[], x: number): void {\n  if (flag) {\n    const base = x + 1;\n    const first = base * base;\n    out.push(first);\n    out.push(x + 2);\n  }\n  audit(/opaque/);\n}\n",
        ),
        (
            "append_chain_pair_b.ts",
            "function appendChainPairRight(enabled: boolean, dst: number[], y: number): void {\n  if (enabled) {\n    dst.push((1 + y) * (1 + y));\n    dst.push(2 + y);\n  }\n  trace(/opaque/);\n}\n",
        ),
        (
            "append_chain_pair_wrong.ts",
            "function appendChainPairWrong(flag: boolean, out: number[], x: number): void {\n  if (flag) {\n    const base = x + 1;\n    const first = base + base;\n    out.push(first);\n    out.push(x + 2);\n  }\n  audit(/opaque/);\n}\n",
        ),
        (
            "append_chain_pair_uses_prior.ts",
            "function appendChainPairUsesPrior(flag: boolean, out: number[], x: number): void {\n  if (flag) {\n    const base = x + 1;\n    const first = base * base;\n    out.push(first + base);\n    out.push(x + 2);\n  }\n  audit(/opaque/);\n}\n",
        ),
        (
            "append_chain_pair_forward_ref.ts",
            "function appendChainPairForwardRef(flag: boolean, out: number[], x: number): void {\n  if (flag) {\n    const base = first + 1;\n    const first = x * x;\n    out.push(first);\n    out.push(x + 2);\n  }\n  audit(/opaque/);\n}\n",
        ),
        (
            "append_cond_before.ts",
            "function appendCondBefore(flag: boolean, out: number[], x: number): void {\n  if (flag) {\n    out.push(x + 1);\n  }\n  out.push(x + 2);\n}\n",
        ),
        (
            "append_cond_after.ts",
            "function appendCondAfter(flag: boolean, out: number[], x: number): void {\n  out.push(x + 2);\n  if (flag) {\n    out.push(x + 1);\n  }\n}\n",
        ),
    ];
    let (dir, out, families) =
        query_fragment_only_fixtures("nose_append_effect_order_boundary", &fixtures);

    let assert_block_pair = |left: &str, right: &str, negative: &str| {
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
                    && files.iter().all(|file| !file.ends_with(negative))
                    && locations.iter().all(|loc| loc["kind"] == "Block")
            })
            .unwrap_or_else(|| {
                panic!("missing ordered append-effect branch family {left}/{right}: {out}")
            });
        assert!(
            family["locations"]
                .as_array()
                .expect("locations")
                .iter()
                .all(|loc| loc["kind"] == "Block"),
            "ordered append-effect branch fragments should report as Block units: {family:?}"
        );
    };

    let assert_no_merge = |left: &str, right: &str, kind: Option<&str>| {
        let merged = families.iter().any(|family| {
            let files: Vec<&str> = family["locations"]
                .as_array()
                .expect("locations")
                .iter()
                .filter(|loc| kind.is_none_or(|kind| loc["kind"].as_str() == Some(kind)))
                .filter_map(|loc| loc["file"].as_str())
                .collect();
            files.iter().any(|file| file.ends_with(left))
                && files.iter().any(|file| file.ends_with(right))
        });
        assert!(
            !merged,
            "semantic mode must not merge ordered append effects when the order changes ({left}/{right}): {out}"
        );
    };

    assert_block_pair(
        "append_pair_a.ts",
        "append_pair_b.ts",
        "append_pair_wrong_order.ts",
    );
    assert_block_pair(
        "append_temp_pair_a.ts",
        "append_temp_pair_b.ts",
        "append_temp_pair_wrong.ts",
    );
    assert_block_pair(
        "append_chain_pair_a.ts",
        "append_chain_pair_b.ts",
        "append_chain_pair_wrong.ts",
    );
    assert_no_merge("append_pair_a.ts", "append_pair_wrong_order.ts", None);
    assert_no_merge("append_pair_a.ts", "append_pair_wrong_receiver.ts", None);
    assert_no_merge("append_pair_a.ts", "append_pair_mutated.ts", None);
    assert_no_merge(
        "append_chain_pair_a.ts",
        "append_chain_pair_uses_prior.ts",
        None,
    );
    assert_no_merge(
        "append_chain_pair_a.ts",
        "append_chain_pair_forward_ref.ts",
        None,
    );
    assert_no_merge(
        "append_cond_before.ts",
        "append_cond_after.ts",
        Some("Function"),
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn semantic_query_reports_exact_safe_three_append_effect_branch_fragments() {
    let fixtures = [
        (
            "append_three_a.ts",
            "function appendThreeLeft(flag: boolean, out: number[], x: number): void {\n  if (flag) {\n    out.push(x + 1);\n    out.push(x + 2);\n    out.push(x + 3);\n  }\n  audit(/opaque/);\n}\n",
        ),
        (
            "append_three_b.ts",
            "function appendThreeRight(enabled: boolean, dst: number[], y: number): void {\n  if (enabled) {\n    dst.push(1 + y);\n    dst.push(2 + y);\n    dst.push(3 + y);\n  }\n  trace(/opaque/);\n}\n",
        ),
        (
            "append_three_wrong_order.ts",
            "function appendThreeWrongOrder(flag: boolean, out: number[], x: number): void {\n  if (flag) {\n    out.push(x + 1);\n    out.push(x + 3);\n    out.push(x + 2);\n  }\n  audit(/opaque/);\n}\n",
        ),
        (
            "append_three_wrong_receiver.ts",
            "function appendThreeWrongReceiver(flag: boolean, out: number[], other: number[], x: number): void {\n  if (flag) {\n    out.push(x + 1);\n    out.push(x + 2);\n    other.push(x + 3);\n  }\n  audit(/opaque/);\n}\n",
        ),
        (
            "append_three_mutated.ts",
            "function appendThreeMutated(flag: boolean, out: number[], x: number): void {\n  out.push(0);\n  if (flag) {\n    out.push(x + 1);\n    out.push(x + 2);\n    out.push(x + 3);\n  }\n  audit(/opaque/);\n}\n",
        ),
        (
            "append_three_temp_a.ts",
            "function appendThreeTempLeft(flag: boolean, out: number[], x: number): void {\n  if (flag) {\n    const first = x + 1;\n    out.push(first * first);\n    out.push(x + 2);\n    out.push(x + 3);\n  }\n  audit(/opaque/);\n}\n",
        ),
        (
            "append_three_temp_b.ts",
            "function appendThreeTempRight(enabled: boolean, dst: number[], y: number): void {\n  if (enabled) {\n    dst.push((1 + y) * (1 + y));\n    dst.push(2 + y);\n    dst.push(3 + y);\n  }\n  trace(/opaque/);\n}\n",
        ),
        (
            "append_three_temp_wrong.ts",
            "function appendThreeTempWrong(flag: boolean, out: number[], x: number): void {\n  if (flag) {\n    const first = x + 4;\n    out.push(first * first);\n    out.push(x + 2);\n    out.push(x + 3);\n  }\n  audit(/opaque/);\n}\n",
        ),
        (
            "append_three_chain_a.ts",
            "function appendThreeChainLeft(flag: boolean, out: number[], x: number): void {\n  if (flag) {\n    const base = x + 1;\n    const first = base * base;\n    out.push(first);\n    out.push(x + 2);\n    out.push(x + 3);\n  }\n  audit(/opaque/);\n}\n",
        ),
        (
            "append_three_chain_b.ts",
            "function appendThreeChainRight(enabled: boolean, dst: number[], y: number): void {\n  if (enabled) {\n    dst.push((1 + y) * (1 + y));\n    dst.push(2 + y);\n    dst.push(3 + y);\n  }\n  trace(/opaque/);\n}\n",
        ),
        (
            "append_three_chain_wrong.ts",
            "function appendThreeChainWrong(flag: boolean, out: number[], x: number): void {\n  if (flag) {\n    const base = x + 1;\n    const first = base + base;\n    out.push(first);\n    out.push(x + 2);\n    out.push(x + 3);\n  }\n  audit(/opaque/);\n}\n",
        ),
        (
            "append_three_chain_uses_prior.ts",
            "function appendThreeChainUsesPrior(flag: boolean, out: number[], x: number): void {\n  if (flag) {\n    const base = x + 1;\n    const first = base * base;\n    out.push(first + base);\n    out.push(x + 2);\n    out.push(x + 3);\n  }\n  audit(/opaque/);\n}\n",
        ),
        (
            "append_four_a.ts",
            "function appendFourLeft(flag: boolean, out: number[], x: number): void {\n  if (flag) {\n    out.push(x + 1);\n    out.push(x + 2);\n    out.push(x + 3);\n    out.push(x + 4);\n  }\n  audit(/opaque/);\n}\n",
        ),
    ];
    let (dir, out, families) =
        query_fragment_only_fixtures("nose_three_append_effect_boundary", &fixtures);

    for (left, right, negative) in [
        (
            "append_three_a.ts",
            "append_three_b.ts",
            "append_three_wrong_order.ts",
        ),
        (
            "append_three_temp_a.ts",
            "append_three_temp_b.ts",
            "append_three_temp_wrong.ts",
        ),
        (
            "append_three_chain_a.ts",
            "append_three_chain_b.ts",
            "append_three_chain_wrong.ts",
        ),
    ] {
        let family =
            find_block_pair_family(&families, left, right, negative).unwrap_or_else(|| {
                panic!("missing three-append-effect branch family {left}/{right}: {out}")
            });
        assert!(
            family_all_blocks(family),
            "three append-effect branch fragments should report as Block units: {family:?}"
        );
    }

    let assert_no_merge = |left: &str, right: &str| {
        assert!(
            !has_pair_family(&families, left, right),
            "semantic mode must not merge three append effects across the boundary ({left}/{right}): {out}"
        );
    };

    assert_no_merge("append_three_a.ts", "append_three_wrong_order.ts");
    assert_no_merge("append_three_a.ts", "append_three_wrong_receiver.ts");
    assert_no_merge("append_three_a.ts", "append_three_mutated.ts");
    assert_no_merge(
        "append_three_chain_a.ts",
        "append_three_chain_uses_prior.ts",
    );
    assert_no_merge("append_three_a.ts", "append_four_a.ts");

    let _ = fs::remove_dir_all(&dir);
}
