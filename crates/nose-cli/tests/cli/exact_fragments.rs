use super::*;

type FragmentScan = (PathBuf, String, Vec<serde_json::Value>);

/// Write `fixtures` into a unique temp dir and run a semantic JSON scan,
/// returning the project dir, the raw scan output, and the parsed families.
fn scan_fragment_fixtures(tag: &str, fixtures: &[(&str, &str)]) -> FragmentScan {
    scan_fragment_fixtures_with(tag, fixtures, &[])
}

/// Like [`scan_fragment_fixtures`], but raises the size gates so only exact
/// fragments can report.
fn scan_fragment_only_fixtures(tag: &str, fixtures: &[(&str, &str)]) -> FragmentScan {
    scan_fragment_fixtures_with(tag, fixtures, &["--min-lines", "100", "--min-size", "100"])
}

fn scan_fragment_fixtures_with(
    tag: &str,
    fixtures: &[(&str, &str)],
    size_args: &[&str],
) -> FragmentScan {
    let dir = std::env::temp_dir().join(format!("{tag}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    for (name, src) in fixtures {
        fs::write(dir.join(name), src).unwrap();
    }
    let mut args = vec!["scan", dir.to_str().unwrap(), "--mode", "semantic"];
    args.extend_from_slice(size_args);
    args.extend_from_slice(&["--format", "json", "--top", "0"]);
    let out = run(&args);
    let families = scan_families(&scan_json(&out)).to_vec();
    (dir, out, families)
}

fn family_locations(family: &serde_json::Value) -> &[serde_json::Value] {
    family["locations"].as_array().expect("locations")
}

fn location_files(family: &serde_json::Value) -> Vec<&str> {
    family_locations(family)
        .iter()
        .filter_map(|loc| loc["file"].as_str())
        .collect()
}

fn family_all_blocks(family: &serde_json::Value) -> bool {
    family_locations(family)
        .iter()
        .all(|loc| loc["kind"] == "Block")
}

/// Locations of `family` whose file ends with `left` or `right`.
fn pair_locations<'a>(
    family: &'a serde_json::Value,
    left: &str,
    right: &str,
) -> Vec<&'a serde_json::Value> {
    family_locations(family)
        .iter()
        .filter(|loc| {
            loc["file"].as_str().unwrap_or("").ends_with(left)
                || loc["file"].as_str().unwrap_or("").ends_with(right)
        })
        .collect()
}

/// First family whose locations include both a `left` and a `right` file.
fn find_pair_family<'a>(
    families: &'a [serde_json::Value],
    left: &str,
    right: &str,
) -> Option<&'a serde_json::Value> {
    families.iter().find(|family| {
        let files = location_files(family);
        files.iter().any(|file| file.ends_with(left))
            && files.iter().any(|file| file.ends_with(right))
    })
}

fn has_pair_family(families: &[serde_json::Value], left: &str, right: &str) -> bool {
    find_pair_family(families, left, right).is_some()
}

/// First all-Block family that pairs `left` with `right` and excludes `negative`.
fn find_block_pair_family<'a>(
    families: &'a [serde_json::Value],
    left: &str,
    right: &str,
    negative: &str,
) -> Option<&'a serde_json::Value> {
    families.iter().find(|family| {
        let files = location_files(family);
        files.iter().any(|file| file.ends_with(left))
            && files.iter().any(|file| file.ends_with(right))
            && family_all_blocks(family)
            && files.iter().all(|file| !file.ends_with(negative))
    })
}

/// Like [`find_block_pair_family`], but `left`/`right` must report the
/// `start_line..end_line` span.
fn find_block_pair_family_at<'a>(
    families: &'a [serde_json::Value],
    left: &str,
    right: &str,
    negative: &str,
    start_line: u64,
    end_line: u64,
) -> Option<&'a serde_json::Value> {
    families.iter().find(|family| {
        let span_files: Vec<&str> = family_locations(family)
            .iter()
            .filter(|loc| loc["start_line"] == start_line && loc["end_line"] == end_line)
            .filter_map(|loc| loc["file"].as_str())
            .collect();
        span_files.iter().any(|file| file.ends_with(left))
            && span_files.iter().any(|file| file.ends_with(right))
            && family_all_blocks(family)
            && location_files(family)
                .iter()
                .all(|file| !file.ends_with(negative))
    })
}

/// Like [`find_block_pair_family`], but some location must span multiple lines.
fn find_multiline_block_pair_family<'a>(
    families: &'a [serde_json::Value],
    left: &str,
    right: &str,
    negative: &str,
) -> Option<&'a serde_json::Value> {
    families.iter().find(|family| {
        let files = location_files(family);
        files.iter().any(|file| file.ends_with(left))
            && files.iter().any(|file| file.ends_with(right))
            && family_all_blocks(family)
            && family_locations(family).iter().any(|loc| {
                loc["end_line"].as_u64().unwrap_or(0) > loc["start_line"].as_u64().unwrap_or(0) + 1
            })
            && files.iter().all(|file| !file.ends_with(negative))
    })
}

#[test]
fn feature_extraction_keeps_dense_small_functions_and_exact_fragments_but_not_small_control_blocks()
{
    let dir = std::env::temp_dir().join(format!("nose_dense_gate_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("a.py"),
        "def dense(xs):\n    return sum(x for x in xs if x > 0)\n\n\
def blocky(xs):\n    total = 0\n    if xs:\n        total = total + xs[0]\n    return total\n",
    )
    .unwrap();

    let out = run(&[
        "features",
        dir.to_str().unwrap(),
        "--min-lines",
        "20",
        "--min-tokens",
        "60",
    ]);
    let json: serde_json::Value = serde_json::from_str(&out).expect("features JSON");
    let units = json["units"].as_array().expect("features units array");
    assert!(
        units
            .iter()
            .any(|unit| unit["kind"] == "Function" && unit["name"] == "dense"),
        "behaviorally dense functions keep the semantic size-gate escape: {out}"
    );
    let block_units: Vec<&serde_json::Value> = units
        .iter()
        .filter(|unit| unit["kind"] == "Block")
        .collect();
    assert!(
        block_units
            .iter()
            .all(|unit| unit["start_line"] == 2 && unit["end_line"] == 2),
        "small control-flow blocks should stay behind the syntactic gate; exact return fragments may pass: {out}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn semantic_scan_reports_exact_safe_return_fragments_under_opaque_functions() {
    let dir = std::env::temp_dir().join(format!(
        "nose_exact_return_fragments_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    let fixtures = [
        (
            "arith_a.py",
            "def arith_left(xs):\n    return (xs[0] + 1) * 2\n    audit(xs)\n",
        ),
        (
            "arith_b.py",
            "def arith_right(ys):\n    return 2 * (ys[0] + 1)\n    trace(ys)\n",
        ),
        (
            "arith_neg.py",
            "def arith_wrong(zs):\n    return (zs[0] + 2) * 2\n    audit(zs)\n",
        ),
        (
            "squares_a.py",
            "def squares_left(xs):\n    return xs[0] * xs[0] + xs[1] * xs[1]\n    audit(xs)\n",
        ),
        (
            "squares_b.py",
            "def squares_right(ys):\n    return ys[1] * ys[1] + ys[0] * ys[0]\n    trace(ys)\n",
        ),
        (
            "squares_neg.py",
            "def squares_wrong(zs):\n    return zs[0] * zs[0] - zs[1] * zs[1]\n    audit(zs)\n",
        ),
        (
            "product_a.py",
            "def product_left(xs):\n    return (xs[0] + xs[1]) * (xs[2] + 4)\n    audit(xs)\n",
        ),
        (
            "product_b.py",
            "def product_right(ys):\n    return (4 + ys[2]) * (ys[1] + ys[0])\n    trace(ys)\n",
        ),
        (
            "product_neg.py",
            "def product_wrong(zs):\n    return (zs[0] + zs[1]) * (zs[2] + 5)\n    audit(zs)\n",
        ),
    ];
    for (name, src) in fixtures {
        fs::write(dir.join(name), src).unwrap();
    }

    let out = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let json = scan_json(&out);
    let families = scan_families(&json);

    let assert_fragment_family = |left: &str, right: &str, negative: &str| {
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
            .unwrap_or_else(|| {
                panic!("missing exact return fragment family {left}/{right}: {out}")
            });
        let locations = family["locations"].as_array().expect("locations");
        assert!(
            locations.iter().all(|loc| loc["kind"] == "Block"),
            "return fragments should report as Block units: {family:?}"
        );
        assert!(
            locations
                .iter()
                .all(|loc| !loc["file"].as_str().unwrap_or("").ends_with(negative)),
            "hard negative must not merge into {left}/{right}: {family:?}"
        );
    };

    assert_fragment_family("arith_a.py", "arith_b.py", "arith_neg.py");
    assert_fragment_family("squares_a.py", "squares_b.py", "squares_neg.py");
    assert_fragment_family("product_a.py", "product_b.py", "product_neg.py");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn semantic_scan_reports_exact_safe_conditional_return_fragments_under_opaque_functions() {
    let dir =
        std::env::temp_dir().join(format!("nose_exact_guard_fragments_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    let fixtures = [
        (
            "square_guard_a.py",
            "def square_guard_left(xs):\n    if xs[0] > 0:\n        return xs[0] * xs[0]\n    audit(xs)\n",
        ),
        (
            "square_guard_b.py",
            "def square_guard_right(ys):\n    if 0 < ys[0]:\n        return ys[0] * ys[0]\n    trace(ys)\n",
        ),
        (
            "square_guard_neg.py",
            "def square_guard_wrong(zs):\n    if zs[0] > 1:\n        return zs[0] * zs[0]\n    audit(zs)\n",
        ),
        (
            "sum_guard_a.py",
            "def sum_guard_left(xs):\n    if xs[0] + xs[1] > 10:\n        return xs[0] + xs[1]\n    audit(xs)\n",
        ),
        (
            "sum_guard_b.py",
            "def sum_guard_right(ys):\n    if 10 < ys[1] + ys[0]:\n        return ys[1] + ys[0]\n    trace(ys)\n",
        ),
        (
            "sum_guard_neg.py",
            "def sum_guard_wrong(zs):\n    if zs[0] + zs[1] > 10:\n        return zs[0] - zs[1]\n    audit(zs)\n",
        ),
        (
            "both_guard_a.py",
            "def both_guard_left(xs):\n    if xs[0] > 0 and xs[1] > 0:\n        return xs[0] + xs[1]\n    audit(xs)\n",
        ),
        (
            "both_guard_b.py",
            "def both_guard_right(ys):\n    if ys[1] > 0 and ys[0] > 0:\n        return ys[1] + ys[0]\n    trace(ys)\n",
        ),
        (
            "both_guard_mutated.py",
            "def both_guard_mutated(zs):\n    zs.append(1)\n    if zs[0] > 0 and zs[1] > 0:\n        return zs[0] + zs[1]\n    audit(zs)\n",
        ),
    ];
    for (name, src) in fixtures {
        fs::write(dir.join(name), src).unwrap();
    }

    let out = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let json = scan_json(&out);
    let families = scan_families(&json);

    let assert_guard_family = |left: &str, right: &str, negative: &str| {
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
            .unwrap_or_else(|| {
                panic!("missing exact conditional return fragment family {left}/{right}: {out}")
            });
        let locations = family["locations"].as_array().expect("locations");
        assert!(
            locations.iter().all(|loc| loc["kind"] == "Block"),
            "conditional return fragments should report as Block units: {family:?}"
        );
        assert!(
            locations
                .iter()
                .all(|loc| !loc["file"].as_str().unwrap_or("").ends_with(negative)),
            "hard negative must not merge into {left}/{right}: {family:?}"
        );
    };

    assert_guard_family(
        "square_guard_a.py",
        "square_guard_b.py",
        "square_guard_neg.py",
    );
    assert_guard_family("sum_guard_a.py", "sum_guard_b.py", "sum_guard_neg.py");
    assert_guard_family(
        "both_guard_a.py",
        "both_guard_b.py",
        "both_guard_mutated.py",
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn semantic_scan_reports_exact_safe_conditional_throw_fragments_under_opaque_functions() {
    let fixtures = [
        (
            "square_throw_guard_a.js",
            "function squareThrowLeft(xs) {\n  if (xs[0] > 0) {\n    throw xs[0] * xs[0];\n  }\n  audit(xs);\n}\n",
        ),
        (
            "square_throw_guard_b.js",
            "function squareThrowRight(ys) {\n  if (0 < ys[0]) {\n    throw ys[0] * ys[0];\n  }\n  trace(ys);\n}\n",
        ),
        (
            "square_throw_guard_neg.js",
            "function squareThrowWrong(zs) {\n  if (zs[0] > 1) {\n    throw zs[0] * zs[0];\n  }\n  audit(zs);\n}\n",
        ),
        (
            "sum_throw_guard_a.js",
            "function sumThrowLeft(xs) {\n  if (xs[0] + xs[1] > 10) {\n    throw xs[0] + xs[1];\n  }\n  audit(xs);\n}\n",
        ),
        (
            "sum_throw_guard_b.js",
            "function sumThrowRight(ys) {\n  if (10 < ys[1] + ys[0]) {\n    throw ys[1] + ys[0];\n  }\n  trace(ys);\n}\n",
        ),
        (
            "sum_throw_guard_neg.js",
            "function sumThrowWrong(zs) {\n  if (zs[0] + zs[1] > 10) {\n    throw zs[0] - zs[1];\n  }\n  audit(zs);\n}\n",
        ),
        (
            "both_throw_guard_a.js",
            "function bothThrowLeft(xs) {\n  if (xs[0] > 0 && xs[1] > 0) {\n    throw xs[0] + xs[1];\n  }\n  audit(xs);\n}\n",
        ),
        (
            "both_throw_guard_b.js",
            "function bothThrowRight(ys) {\n  if (ys[1] > 0 && ys[0] > 0) {\n    throw ys[1] + ys[0];\n  }\n  trace(ys);\n}\n",
        ),
        (
            "both_throw_guard_mutated.js",
            "function bothThrowMutated(zs) {\n  zs.push(1);\n  if (zs[0] > 0 && zs[1] > 0) {\n    throw zs[0] + zs[1];\n  }\n  audit(zs);\n}\n",
        ),
    ];
    let (dir, out, families) =
        scan_fragment_fixtures("nose_exact_throw_guard_fragments", &fixtures);

    let assert_guard_family = |left: &str, right: &str, negative: &str| {
        let family = find_pair_family(&families, left, right).unwrap_or_else(|| {
            panic!("missing exact conditional throw fragment family {left}/{right}: {out}")
        });
        assert!(
            family_all_blocks(family),
            "conditional throw fragments should report as Block units: {family:?}"
        );
        assert!(
            location_files(family)
                .iter()
                .all(|file| !file.ends_with(negative)),
            "hard negative must not merge into {left}/{right}: {family:?}"
        );
    };

    assert_guard_family(
        "square_throw_guard_a.js",
        "square_throw_guard_b.js",
        "square_throw_guard_neg.js",
    );
    assert_guard_family(
        "sum_throw_guard_a.js",
        "sum_throw_guard_b.js",
        "sum_throw_guard_neg.js",
    );
    assert_guard_family(
        "both_throw_guard_a.js",
        "both_throw_guard_b.js",
        "both_throw_guard_mutated.js",
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn semantic_scan_reports_exact_safe_empty_branch_conditional_exit_fragments_under_opaque_functions()
{
    let fixtures = [
        (
            "empty_else_return_a.js",
            "function emptyElseReturnLeft(xs) {\n  if (xs[0] > 0) {\n    return xs[0] * xs[0];\n  } else {\n  }\n  audit(xs);\n}\n",
        ),
        (
            "empty_else_return_b.js",
            "function emptyElseReturnRight(ys) {\n  if (0 < ys[0]) {\n    return ys[0] * ys[0];\n  } else {\n  }\n  trace(ys);\n}\n",
        ),
        (
            "empty_else_return_neg.js",
            "function emptyElseReturnWrong(zs) {\n  if (zs[0] > 1) {\n    return zs[0] * zs[0];\n  } else {\n  }\n  audit(zs);\n}\n",
        ),
        (
            "empty_else_throw_a.js",
            "function emptyElseThrowLeft(xs) {\n  if (xs[0] + xs[1] > 10) {\n    throw xs[0] + xs[1];\n  } else {\n  }\n  audit(xs);\n}\n",
        ),
        (
            "empty_else_throw_b.js",
            "function emptyElseThrowRight(ys) {\n  if (10 < ys[1] + ys[0]) {\n    throw ys[1] + ys[0];\n  } else {\n  }\n  trace(ys);\n}\n",
        ),
        (
            "empty_else_throw_neg.js",
            "function emptyElseThrowWrong(zs) {\n  if (zs[0] + zs[1] > 10) {\n    throw zs[0] - zs[1];\n  } else {\n  }\n  audit(zs);\n}\n",
        ),
        (
            "empty_then_throw_a.js",
            "function emptyThenThrowLeft(xs) {\n  if (xs[0] > 0 && xs[1] > 0) {\n  } else {\n    throw xs[0] + xs[1];\n  }\n  audit(xs);\n}\n",
        ),
        (
            "empty_then_throw_b.js",
            "function emptyThenThrowRight(ys) {\n  if (ys[1] > 0 && ys[0] > 0) {\n  } else {\n    throw ys[1] + ys[0];\n  }\n  trace(ys);\n}\n",
        ),
        (
            "empty_then_throw_mutated.js",
            "function emptyThenThrowMutated(zs) {\n  zs.push(1);\n  if (zs[0] > 0 && zs[1] > 0) {\n  } else {\n    throw zs[0] + zs[1];\n  }\n  audit(zs);\n}\n",
        ),
    ];
    let (dir, out, families) =
        scan_fragment_fixtures("nose_exact_empty_branch_fragments", &fixtures);

    let assert_guard_family = |left: &str, right: &str, negative: &str| {
        let family = find_pair_family(&families, left, right).unwrap_or_else(|| {
            panic!("missing exact empty-branch fragment family {left}/{right}: {out}")
        });
        assert!(
            family_all_blocks(family),
            "empty-branch conditional fragments should report as Block units: {family:?}"
        );
        assert!(
            location_files(family)
                .iter()
                .all(|file| !file.ends_with(negative)),
            "hard negative must not merge into {left}/{right}: {family:?}"
        );
    };

    assert_guard_family(
        "empty_else_return_a.js",
        "empty_else_return_b.js",
        "empty_else_return_neg.js",
    );
    assert_guard_family(
        "empty_else_throw_a.js",
        "empty_else_throw_b.js",
        "empty_else_throw_neg.js",
    );
    assert_guard_family(
        "empty_then_throw_a.js",
        "empty_then_throw_b.js",
        "empty_then_throw_mutated.js",
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn semantic_scan_reports_exact_safe_conditional_bare_return_fragments_under_opaque_functions() {
    let dir = std::env::temp_dir().join(format!(
        "nose_exact_bare_return_fragments_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    let fixtures = [
        (
            "bare_square_a.js",
            "function bareSquareLeft(xs) {\n  if (xs[0] > 0) {\n    return;\n  }\n  audit(xs);\n}\n",
        ),
        (
            "bare_square_b.js",
            "function bareSquareRight(ys) {\n  if (0 < ys[0]) {\n    return;\n  }\n  trace(ys);\n}\n",
        ),
        (
            "bare_square_neg.js",
            "function bareSquareWrong(zs) {\n  if (zs[0] > 1) {\n    return;\n  }\n  audit(zs);\n}\n",
        ),
        (
            "bare_sum_a.js",
            "function bareSumLeft(xs) {\n  if (xs[0] + xs[1] > 10) {\n    return;\n  }\n  audit(xs);\n}\n",
        ),
        (
            "bare_sum_b.js",
            "function bareSumRight(ys) {\n  if (10 < ys[1] + ys[0]) {\n    return;\n  }\n  trace(ys);\n}\n",
        ),
        (
            "bare_sum_mutated.js",
            "function bareSumMutated(zs) {\n  zs.push(1);\n  if (zs[0] + zs[1] > 10) {\n    return;\n  }\n  audit(zs);\n}\n",
        ),
        (
            "bare_else_a.js",
            "function bareElseLeft(xs) {\n  if (xs[0] > 0 && xs[1] > 0) {\n  } else {\n    return;\n  }\n  audit(xs);\n}\n",
        ),
        (
            "bare_else_b.js",
            "function bareElseRight(ys) {\n  if (ys[1] > 0 && ys[0] > 0) {\n  } else {\n    return;\n  }\n  trace(ys);\n}\n",
        ),
        (
            "bare_else_neg.js",
            "function bareElseWrong(zs) {\n  if (zs[0] > 0 && zs[1] > 1) {\n  } else {\n    return;\n  }\n  audit(zs);\n}\n",
        ),
    ];
    for (name, src) in fixtures {
        fs::write(dir.join(name), src).unwrap();
    }

    let out = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let json = scan_json(&out);
    let families = scan_families(&json);

    let assert_guard_family = |left: &str, right: &str, negative: &str| {
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
            .unwrap_or_else(|| {
                panic!("missing exact conditional bare-return family {left}/{right}: {out}")
            });
        let locations = family["locations"].as_array().expect("locations");
        assert!(
            locations.iter().all(|loc| loc["kind"] == "Block"),
            "conditional bare-return fragments should report as Block units: {family:?}"
        );
        assert!(
            locations
                .iter()
                .all(|loc| !loc["file"].as_str().unwrap_or("").ends_with(negative)),
            "hard negative must not merge into {left}/{right}: {family:?}"
        );
    };

    assert_guard_family("bare_square_a.js", "bare_square_b.js", "bare_square_neg.js");
    assert_guard_family("bare_sum_a.js", "bare_sum_b.js", "bare_sum_mutated.js");
    assert_guard_family("bare_else_a.js", "bare_else_b.js", "bare_else_neg.js");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn semantic_scan_reports_exact_safe_conditional_expr_effect_fragments_under_opaque_functions() {
    let dir = std::env::temp_dir().join(format!(
        "nose_exact_expr_effect_fragments_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    let fixtures = [
        (
            "push_square_a.js",
            "function pushSquareLeft(xs, out) {\n  if (xs[0] > 0) {\n    out.push(xs[0] * xs[0]);\n  }\n  audit(xs);\n}\n",
        ),
        (
            "push_square_b.js",
            "function pushSquareRight(ys, dst) {\n  if (0 < ys[0]) {\n    dst.push(ys[0] * ys[0]);\n  }\n  trace(ys);\n}\n",
        ),
        (
            "push_square_neg.js",
            "function pushSquareWrong(zs, out) {\n  if (zs[0] > 1) {\n    out.push(zs[0] * zs[0]);\n  }\n  audit(zs);\n}\n",
        ),
        (
            "push_sum_a.js",
            "function pushSumLeft(xs, out) {\n  if (xs[0] + xs[1] > 10) {\n    out.push(xs[0] + xs[1]);\n  }\n  audit(xs);\n}\n",
        ),
        (
            "push_sum_b.js",
            "function pushSumRight(ys, dst) {\n  if (10 < ys[1] + ys[0]) {\n    dst.push(ys[1] + ys[0]);\n  }\n  trace(ys);\n}\n",
        ),
        (
            "push_sum_neg.js",
            "function pushSumWrong(zs, out) {\n  if (zs[0] + zs[1] > 10) {\n    out.push(zs[0] - zs[1]);\n  }\n  audit(zs);\n}\n",
        ),
        (
            "push_else_a.js",
            "function pushElseLeft(xs, out) {\n  if (xs[0] > 0 && xs[1] > 0) {\n  } else {\n    out.push(xs[0] + xs[1]);\n  }\n  audit(xs);\n}\n",
        ),
        (
            "push_else_b.js",
            "function pushElseRight(ys, dst) {\n  if (ys[1] > 0 && ys[0] > 0) {\n  } else {\n    dst.push(ys[1] + ys[0]);\n  }\n  trace(ys);\n}\n",
        ),
        (
            "push_else_mutated.js",
            "function pushElseMutated(zs, out) {\n  out.push(0);\n  if (zs[0] > 0 && zs[1] > 0) {\n  } else {\n    out.push(zs[0] + zs[1]);\n  }\n  audit(zs);\n}\n",
        ),
    ];
    for (name, src) in fixtures {
        fs::write(dir.join(name), src).unwrap();
    }

    let out = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let json = scan_json(&out);
    let families = scan_families(&json);

    let assert_guard_family = |left: &str, right: &str, negative: &str| {
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
            .unwrap_or_else(|| {
                panic!("missing exact conditional expression-effect family {left}/{right}: {out}")
            });
        let locations = family["locations"].as_array().expect("locations");
        assert!(
            locations.iter().all(|loc| loc["kind"] == "Block"),
            "conditional expression-effect fragments should report as Block units: {family:?}"
        );
        assert!(
            locations
                .iter()
                .all(|loc| !loc["file"].as_str().unwrap_or("").ends_with(negative)),
            "hard negative must not merge into {left}/{right}: {family:?}"
        );
    };

    assert_guard_family("push_square_a.js", "push_square_b.js", "push_square_neg.js");
    assert_guard_family("push_sum_a.js", "push_sum_b.js", "push_sum_neg.js");
    assert_guard_family("push_else_a.js", "push_else_b.js", "push_else_mutated.js");
    let _ = fs::remove_dir_all(&dir);
}

// Broad fixture matrix for exact branch temp-consumption fragments. The size is
// intentional until the fixture setup has a clearer table-builder abstraction.
#[allow(clippy::too_many_lines)]
#[test]
fn semantic_scan_reports_exact_safe_branch_temp_consumption_fragments_under_opaque_functions() {
    let dir = std::env::temp_dir().join(format!(
        "nose_exact_branch_temp_fragments_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

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
            "function tempThrowRight(ys) {\n  if (10 < ys[1] + ys[0]) {\n    throw ys[1] + ys[0];\n  }\n  trace(ys);\n}\n",
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
            "function tempEffectRight(ys, dst) {\n  if (0 < ys[0]) {\n    dst.push(ys[1] + ys[0] * ys[0]);\n  }\n  trace(ys);\n}\n",
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
            "function tempChainThrowRight(ys) {\n  if (0 < ys[0]) {\n    throw ys[1] + (1 + ys[0]) * (1 + ys[0]);\n  }\n  trace(ys);\n}\n",
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
            "function tempChainEffectRight(ys, dst) {\n  if (0 < ys[0]) {\n    dst.push(ys[1] + (1 + ys[0]) * (1 + ys[0]));\n  }\n  trace(ys);\n}\n",
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
    for (name, src) in fixtures {
        fs::write(dir.join(name), src).unwrap();
    }

    let out = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "100",
        "--min-size",
        "100",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let json = scan_json(&out);
    let families = scan_families(&json);

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
fn semantic_scan_reports_exact_safe_nested_conditional_effect_fragments_under_opaque_functions() {
    let fixtures = [
        (
            "nested_push_a.js",
            "function nestedPushLeft(xs, out) {\n  if (xs[0] > 0 && xs[1] > 0) {\n    out.push(xs[0] + xs[1]);\n  } else {\n    if (xs[2] > 0) {\n      out.push(xs[2] * xs[2]);\n    }\n  }\n  audit(xs);\n}\n",
        ),
        (
            "nested_push_b.js",
            "function nestedPushRight(ys, dst) {\n  if (ys[1] > 0 && ys[0] > 0) {\n    dst.push(ys[1] + ys[0]);\n  } else {\n    if (0 < ys[2]) {\n      dst.push(ys[2] * ys[2]);\n    }\n  }\n  trace(ys);\n}\n",
        ),
        (
            "nested_push_mutated.js",
            "function nestedPushMutated(zs, out) {\n  out.push(0);\n  if (zs[0] > 0 && zs[1] > 0) {\n    out.push(zs[0] + zs[1]);\n  } else {\n    if (zs[2] > 0) {\n      out.push(zs[2] * zs[2]);\n    }\n  }\n  audit(zs);\n}\n",
        ),
        (
            "nested_push_sum_a.js",
            "function nestedPushSumLeft(xs, out) {\n  if (xs[0] + xs[1] > 10) {\n    out.push(xs[0] + xs[1]);\n  } else {\n    if (xs[2] > 0) {\n      out.push(xs[2] * xs[2]);\n    }\n  }\n  audit(xs);\n}\n",
        ),
        (
            "nested_push_sum_b.js",
            "function nestedPushSumRight(ys, dst) {\n  if (10 < ys[1] + ys[0]) {\n    dst.push(ys[1] + ys[0]);\n  } else {\n    if (0 < ys[2]) {\n      dst.push(ys[2] * ys[2]);\n    }\n  }\n  trace(ys);\n}\n",
        ),
        (
            "nested_push_sum_neg.js",
            "function nestedPushSumWrong(zs, out) {\n  if (zs[0] + zs[1] > 10) {\n    out.push(zs[0] - zs[1]);\n  } else {\n    if (zs[2] > 0) {\n      out.push(zs[2] * zs[2]);\n    }\n  }\n  audit(zs);\n}\n",
        ),
        (
            "nested_push_product_a.js",
            "function nestedPushProductLeft(xs, out) {\n  if ((xs[0] + 1) > 10) {\n    out.push((xs[0] + 1) * 2);\n  } else {\n    if (xs[1] + xs[2] > 0) {\n      out.push(xs[1] + xs[2]);\n    }\n  }\n  audit(xs);\n}\n",
        ),
        (
            "nested_push_product_b.js",
            "function nestedPushProductRight(ys, dst) {\n  if (10 < (1 + ys[0])) {\n    dst.push(2 * (1 + ys[0]));\n  } else {\n    if (ys[2] + ys[1] > 0) {\n      dst.push(ys[2] + ys[1]);\n    }\n  }\n  trace(ys);\n}\n",
        ),
        (
            "nested_push_product_neg.js",
            "function nestedPushProductWrong(zs, out) {\n  if ((zs[0] + 2) > 10) {\n    out.push((zs[0] + 2) * 2);\n  } else {\n    if (zs[1] + zs[2] > 0) {\n      out.push(zs[1] + zs[2]);\n    }\n  }\n  audit(zs);\n}\n",
        ),
    ];
    let (dir, out, families) =
        scan_fragment_only_fixtures("nose_exact_nested_fragments", &fixtures);

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
        "nested_push_a.js",
        "nested_push_b.js",
        "nested_push_mutated.js",
    );
    assert_guard_family(
        "nested_push_sum_a.js",
        "nested_push_sum_b.js",
        "nested_push_sum_neg.js",
    );
    assert_guard_family(
        "nested_push_product_a.js",
        "nested_push_product_b.js",
        "nested_push_product_neg.js",
    );
    let _ = fs::remove_dir_all(&dir);
}

// Broad fixture matrix for exact foreach append-effect fragments. The size is
// intentional until the fixture setup has a clearer table-builder abstraction.
#[allow(clippy::too_many_lines)]
#[test]
fn semantic_scan_reports_exact_safe_foreach_append_effect_fragments_under_opaque_functions() {
    let dir = std::env::temp_dir().join(format!(
        "nose_exact_loop_effect_fragments_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

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
            "function loopPushProductRight(ys: number[], dst: number[]): void {\n  for (const y of ys) {\n    dst.push(2 * (1 + y));\n  }\n  trace(ys);\n}\n",
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
            "function loopPushGuardRight(ys: number[], dst: number[]): void {\n  for (const y of ys) {\n    if (0 < y) dst.push(1 + y);\n  }\n  trace(ys);\n}\n",
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
            "function loopTempChainPushRight(ys: number[], dst: number[]): void {\n  for (const y of ys) {\n    const offset = 1 + y;\n    const result = offset * offset;\n    dst.push(result);\n  }\n  trace(ys);\n}\n",
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
    for (name, src) in fixtures {
        fs::write(dir.join(name), src).unwrap();
    }

    let out = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "100",
        "--min-size",
        "100",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let json = scan_json(&out);
    let families = scan_families(&json);

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

// Broad fixture matrix for exact Go foreach index-assignment fragments. The size is
// intentional until the fixture setup has a clearer table-builder abstraction.
#[allow(clippy::too_many_lines)]
#[test]
fn semantic_scan_reports_exact_safe_foreach_index_assignment_fragments_for_go() {
    let dir = std::env::temp_dir().join(format!(
        "nose_exact_foreach_index_assign_fragments_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

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
    for (name, src) in fixtures {
        fs::write(dir.join(name), src).unwrap();
    }

    let out = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "100",
        "--min-size",
        "100",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let json = scan_json(&out);
    let families = scan_families(&json);

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
fn semantic_scan_reports_exact_safe_conditional_foreach_append_effect_fragments_under_opaque_functions(
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
            "function condLoopElseRight(flag: boolean, ys: number[], dst: number[]): void {\n  if (flag) {\n  } else {\n    for (const y of ys) {\n      dst.push(1 + y);\n    }\n  }\n  trace(flag);\n}\n",
        ),
        (
            "cond_loop_else_wrong_receiver.ts",
            "function condLoopElseWrongReceiver(flag: boolean, ys: number[], dst: number[]): void {\n  if (flag) {\n  } else {\n    for (const y of ys) {\n      ys.push(1 + y);\n    }\n  }\n  trace(flag);\n}\n",
        ),
        (
            "cond_loop_guard_a.ts",
            "function condLoopGuardLeft(enabled: boolean, xs: number[], out: number[]): void {\n  if (enabled) {\n    for (const x of xs) {\n      if (x > 0) out.push(x + 1);\n    }\n  }\n  audit(enabled);\n}\n",
        ),
        (
            "cond_loop_guard_b.ts",
            "function condLoopGuardRight(flag: boolean, ys: number[], dst: number[]): void {\n  if (flag) {\n    for (const y of ys) {\n      if (0 < y) dst.push(1 + y);\n    }\n  }\n  trace(flag);\n}\n",
        ),
        (
            "cond_loop_guard_wrong_value.ts",
            "function condLoopGuardWrongValue(flag: boolean, ys: number[], dst: number[]): void {\n  if (flag) {\n    for (const y of ys) {\n      if (0 < y) dst.push(2 + y);\n    }\n  }\n  trace(flag);\n}\n",
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
        scan_fragment_only_fixtures("nose_exact_conditional_loop_effect_fragments", &fixtures);

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

// Broad fixture matrix for ordered foreach-effect branch boundaries. The size is
// intentional until the fixture setup has a clearer table-builder abstraction.
#[allow(clippy::too_many_lines)]
#[test]
fn semantic_scan_reports_exact_safe_ordered_foreach_effect_branch_fragments() {
    let dir = std::env::temp_dir().join(format!(
        "nose_ordered_foreach_effect_branch_fragments_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    let fixtures = [
        (
            "cond_two_loops_a.ts",
            "function condTwoLoopsLeft(enabled: boolean, xs: number[], ys: number[], out: number[]): void {\n  if (enabled) {\n    for (const x of xs) {\n      out.push(x + 1);\n    }\n    for (const y of ys) {\n      out.push(y * y);\n    }\n  }\n  audit(enabled);\n}\n",
        ),
        (
            "cond_two_loops_b.ts",
            "function condTwoLoopsRight(flag: boolean, as: number[], bs: number[], dst: number[]): void {\n  if (flag) {\n    for (const a of as) {\n      dst.push(1 + a);\n    }\n    for (const b of bs) {\n      dst.push(b * b);\n    }\n  }\n  trace(flag);\n}\n",
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
    for (name, src) in fixtures {
        fs::write(dir.join(name), src).unwrap();
    }

    let out = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "100",
        "--min-size",
        "100",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let json = scan_json(&out);
    let families = scan_families(&json);

    let assert_branch_pair =
        |left: &str, right: &str, negative: &str, start_line: u64, end_line: u64| {
            let family =
                block_branch_pair_family(families, left, right, negative, start_line, end_line)
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
                families,
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
fn semantic_scan_reports_exact_safe_ordered_mixed_effect_branch_fragments() {
    let dir = std::env::temp_dir().join(format!(
        "nose_ordered_mixed_effect_branch_fragments_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    let fixtures = [
        (
            "mixed_loop_append_a.ts",
            "function mixedLoopAppendLeft(enabled: boolean, xs: number[], out: number[], seed: number): void {\n  if (enabled) {\n    for (const x of xs) {\n      out.push(x + 1);\n    }\n    out.push(seed * seed);\n  }\n  audit(enabled);\n}\n",
        ),
        (
            "mixed_loop_append_b.ts",
            "function mixedLoopAppendRight(flag: boolean, ys: number[], dst: number[], base: number): void {\n  if (flag) {\n    for (const y of ys) {\n      dst.push(1 + y);\n    }\n    dst.push(base * base);\n  }\n  trace(flag);\n}\n",
        ),
        (
            "mixed_loop_append_wrong_order.ts",
            "function mixedLoopAppendWrongOrder(flag: boolean, ys: number[], dst: number[], base: number): void {\n  if (flag) {\n    dst.push(base * base);\n    for (const y of ys) {\n      dst.push(1 + y);\n    }\n  }\n  trace(flag);\n}\n",
        ),
        (
            "mixed_loop_append_wrong_receiver.ts",
            "function mixedLoopAppendWrongReceiver(flag: boolean, ys: number[], dst: number[], base: number): void {\n  if (flag) {\n    for (const y of ys) {\n      dst.push(1 + y);\n    }\n    ys.push(base * base);\n  }\n  trace(flag);\n}\n",
        ),
        (
            "mixed_loop_append_mutated.ts",
            "function mixedLoopAppendMutated(flag: boolean, ys: number[], dst: number[], base: number): void {\n  dst.push(0);\n  if (flag) {\n    for (const y of ys) {\n      dst.push(1 + y);\n    }\n    dst.push(base * base);\n  }\n  trace(flag);\n}\n",
        ),
        (
            "mixed_loop_append_third_effect.ts",
            "function mixedLoopAppendThirdEffect(flag: boolean, ys: number[], dst: number[], base: number): void {\n  if (flag) {\n    for (const y of ys) {\n      dst.push(1 + y);\n    }\n    dst.push(base * base);\n    dst.push(base + 1);\n  }\n  trace(flag);\n}\n",
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
    for (name, src) in fixtures {
        fs::write(dir.join(name), src).unwrap();
    }

    let out = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "100",
        "--min-size",
        "100",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let json = scan_json(&out);
    let families = scan_families(&json);

    let assert_branch_pair =
        |left: &str, right: &str, negative: &str, start_line: u64, end_line: u64| {
            let family =
                block_branch_pair_family(families, left, right, negative, start_line, end_line)
                    .unwrap_or_else(|| {
                        panic!("missing ordered mixed-effect branch family {left}/{right}: {out}")
                    });
            assert!(
                family["locations"]
                    .as_array()
                    .expect("locations")
                    .iter()
                    .all(|loc| loc["kind"] == "Block"),
                "ordered mixed-effect branch fragments should report as Block units: {family:?}"
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
                families,
                (left, left_start, left_end),
                (right, right_start, right_end),
            ),
            "ordered mixed-effect branch boundary must not merge {left}/{right}: {out}"
        );
    };

    assert_branch_pair(
        "mixed_loop_append_a.ts",
        "mixed_loop_append_b.ts",
        "mixed_loop_append_wrong_order.ts",
        2,
        7,
    );
    assert_branch_pair(
        "mixed_append_loop_a.py",
        "mixed_append_loop_b.py",
        "mixed_append_loop_wrong_temp.py",
        2,
        6,
    );
    assert_branch_pair(
        "mixed_index_loop_a.go",
        "mixed_index_loop_b.go",
        "mixed_index_loop_wrong_index.go",
        3,
        8,
    );

    assert_no_branch_pair(
        "mixed_loop_append_a.ts",
        "mixed_loop_append_wrong_order.ts",
        2,
        7,
        2,
        7,
    );
    assert_no_branch_pair(
        "mixed_loop_append_a.ts",
        "mixed_loop_append_wrong_receiver.ts",
        2,
        7,
        2,
        7,
    );
    assert_no_branch_pair(
        "mixed_loop_append_a.ts",
        "mixed_loop_append_mutated.ts",
        2,
        7,
        3,
        8,
    );
    assert_no_branch_pair(
        "mixed_loop_append_a.ts",
        "mixed_loop_append_third_effect.ts",
        2,
        7,
        2,
        8,
    );
    assert_no_branch_pair(
        "mixed_append_loop_a.py",
        "mixed_append_loop_wrong_temp.py",
        2,
        6,
        2,
        6,
    );
    assert_no_branch_pair(
        "mixed_append_loop_a.py",
        "mixed_append_loop_wrong_order.py",
        2,
        6,
        2,
        6,
    );
    assert_no_branch_pair(
        "mixed_index_loop_a.go",
        "mixed_index_loop_wrong_index.go",
        3,
        8,
        3,
        8,
    );
    assert_no_branch_pair(
        "mixed_index_loop_a.go",
        "mixed_index_loop_wrong_receiver.go",
        3,
        8,
        3,
        8,
    );

    let _ = fs::remove_dir_all(&dir);
}

// Broad fixture matrix for ordered conditional-effect branch boundaries. The size is
// intentional until the fixture setup has a clearer table-builder abstraction.
#[allow(clippy::too_many_lines)]
#[test]
fn semantic_scan_reports_exact_safe_ordered_conditional_effect_branch_fragments() {
    let dir = std::env::temp_dir().join(format!(
        "nose_ordered_conditional_effect_branch_fragments_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

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
    for (name, src) in fixtures {
        fs::write(dir.join(name), src).unwrap();
    }

    let out = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "100",
        "--min-size",
        "100",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let json = scan_json(&out);
    let families = scan_families(&json);

    let assert_branch_pair =
        |left: &str, right: &str, negative: &str, start_line: u64, end_line: u64| {
            let family = block_branch_pair_family(
                families, left, right, negative, start_line, end_line,
            )
            .unwrap_or_else(|| {
                panic!("missing ordered conditional-effect branch family {left}/{right}: {out}")
            });
            assert!(
            family["locations"]
                .as_array()
                .expect("locations")
                .iter()
                .all(|loc| loc["kind"] == "Block"),
            "ordered conditional-effect branch fragments should report as Block units: {family:?}"
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
                families,
                (left, left_start, left_end),
                (right, right_start, right_end),
            ),
            "ordered conditional-effect branch boundary must not merge {left}/{right}: {out}"
        );
    };

    assert_branch_pair(
        "cond_pair_append_a.ts",
        "cond_pair_append_b.ts",
        "cond_pair_append_wrong_order.ts",
        2,
        9,
    );
    assert_branch_pair(
        "cond_pair_append_a.py",
        "cond_pair_append_b.py",
        "cond_pair_append_wrong_guard.py",
        2,
        6,
    );
    assert_branch_pair(
        "cond_pair_index_a.go",
        "cond_pair_index_b.go",
        "cond_pair_index_wrong_index.go",
        3,
        10,
    );

    assert_no_branch_pair(
        "cond_pair_append_a.ts",
        "cond_pair_append_wrong_order.ts",
        2,
        9,
        2,
        9,
    );
    assert_no_branch_pair(
        "cond_pair_append_a.ts",
        "cond_pair_append_wrong_receiver.ts",
        2,
        9,
        2,
        9,
    );
    assert_no_branch_pair(
        "cond_pair_append_a.ts",
        "cond_pair_append_mutated.ts",
        2,
        9,
        3,
        10,
    );
    assert_no_branch_pair(
        "cond_pair_append_a.ts",
        "cond_pair_append_third.ts",
        2,
        9,
        2,
        12,
    );
    assert_no_branch_pair(
        "cond_pair_append_a.py",
        "cond_pair_append_wrong_guard.py",
        2,
        6,
        2,
        6,
    );
    assert_no_branch_pair(
        "cond_pair_append_a.py",
        "cond_pair_append_wrong_order.py",
        2,
        6,
        2,
        6,
    );
    assert_no_branch_pair(
        "cond_pair_index_a.go",
        "cond_pair_index_wrong_index.go",
        3,
        10,
        3,
        10,
    );
    assert_no_branch_pair(
        "cond_pair_index_a.go",
        "cond_pair_index_wrong_receiver.go",
        3,
        10,
        3,
        10,
    );

    let _ = fs::remove_dir_all(&dir);
}

// Broad fixture matrix for ordered conditional mixed-effect branch boundaries. The size is
// intentional until the fixture setup has a clearer table-builder abstraction.
#[allow(clippy::too_many_lines)]
#[test]
fn semantic_scan_reports_exact_safe_ordered_conditional_mixed_effect_branch_fragments() {
    let dir = std::env::temp_dir().join(format!(
        "nose_ordered_conditional_mixed_effect_branch_fragments_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

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
    for (name, src) in fixtures {
        fs::write(dir.join(name), src).unwrap();
    }

    let out = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "100",
        "--min-size",
        "100",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let json = scan_json(&out);
    let families = scan_families(&json);

    let assert_branch_pair = |left: &str,
                              right: &str,
                              negative: &str,
                              start_line: u64,
                              end_line: u64| {
        let family = block_branch_pair_family(
            families, left, right, negative, start_line, end_line,
        )
        .unwrap_or_else(|| {
            panic!("missing ordered conditional mixed-effect branch family {left}/{right}: {out}")
        });
        assert!(
                family["locations"]
                    .as_array()
                    .expect("locations")
                    .iter()
                    .all(|loc| loc["kind"] == "Block"),
                "ordered conditional mixed-effect branch fragments should report as Block units: {family:?}"
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
                families,
                (left, left_start, left_end),
                (right, right_start, right_end),
            ),
            "ordered conditional mixed-effect branch boundary must not merge {left}/{right}: {out}"
        );
    };

    assert_branch_pair(
        "cond_mixed_append_a.ts",
        "cond_mixed_append_b.ts",
        "cond_mixed_append_wrong_order.ts",
        2,
        7,
    );
    assert_branch_pair(
        "cond_mixed_append_a.py",
        "cond_mixed_append_b.py",
        "cond_mixed_append_wrong_guard.py",
        2,
        5,
    );
    assert_branch_pair(
        "cond_mixed_index_a.go",
        "cond_mixed_index_b.go",
        "cond_mixed_index_wrong_index.go",
        3,
        8,
    );

    assert_no_branch_pair(
        "cond_mixed_append_a.ts",
        "cond_mixed_append_wrong_order.ts",
        2,
        7,
        2,
        7,
    );
    assert_no_branch_pair(
        "cond_mixed_append_a.ts",
        "cond_mixed_append_wrong_guard.ts",
        2,
        7,
        2,
        7,
    );
    assert_no_branch_pair(
        "cond_mixed_append_a.ts",
        "cond_mixed_append_wrong_receiver.ts",
        2,
        7,
        2,
        7,
    );
    assert_no_branch_pair(
        "cond_mixed_append_a.ts",
        "cond_mixed_append_mutated.ts",
        2,
        7,
        3,
        8,
    );
    assert_no_branch_pair(
        "cond_mixed_append_a.ts",
        "cond_mixed_append_third.ts",
        2,
        7,
        2,
        8,
    );
    assert_no_branch_pair(
        "cond_mixed_append_a.py",
        "cond_mixed_append_wrong_order.py",
        2,
        5,
        2,
        5,
    );
    assert_no_branch_pair(
        "cond_mixed_append_a.py",
        "cond_mixed_append_wrong_guard.py",
        2,
        5,
        2,
        5,
    );
    assert_no_branch_pair(
        "cond_mixed_index_a.go",
        "cond_mixed_index_wrong_index.go",
        3,
        8,
        3,
        8,
    );
    assert_no_branch_pair(
        "cond_mixed_index_a.go",
        "cond_mixed_index_wrong_receiver.go",
        3,
        8,
        3,
        8,
    );

    let _ = fs::remove_dir_all(&dir);
}

// Broad fixture matrix for ordered loop conditional-effect branch boundaries. The size is
// intentional until the fixture setup has a clearer table-builder abstraction.
#[allow(clippy::too_many_lines)]
#[test]
fn semantic_scan_reports_exact_safe_ordered_loop_conditional_effect_branch_fragments() {
    let dir = std::env::temp_dir().join(format!(
        "nose_ordered_loop_conditional_effect_branch_fragments_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    let fixtures = [
        (
            "loop_cond_append_a.ts",
            "function loopCondAppendLeft(enabled: boolean, xs: number[], y: number, out: number[]): void {\n  if (enabled) {\n    for (const x of xs) {\n      out.push(x + 1);\n    }\n    if (y > 0) {\n      out.push(y * y);\n    }\n  }\n  audit(enabled);\n}\n",
        ),
        (
            "loop_cond_append_b.ts",
            "function loopCondAppendRight(flag: boolean, ys: number[], b: number, dst: number[]): void {\n  if (flag) {\n    for (const a of ys) {\n      dst.push(1 + a);\n    }\n    if (b > 0) {\n      dst.push(b * b);\n    }\n  }\n  trace(flag);\n}\n",
        ),
        (
            "loop_cond_append_wrong_order.ts",
            "function loopCondAppendWrongOrder(flag: boolean, ys: number[], b: number, dst: number[]): void {\n  if (flag) {\n    if (b > 0) {\n      dst.push(b * b);\n    }\n    for (const a of ys) {\n      dst.push(1 + a);\n    }\n  }\n  trace(flag);\n}\n",
        ),
        (
            "loop_cond_append_wrong_guard.ts",
            "function loopCondAppendWrongGuard(flag: boolean, ys: number[], b: number, dst: number[]): void {\n  if (flag) {\n    for (const a of ys) {\n      dst.push(1 + a);\n    }\n    if (b >= 0) {\n      dst.push(b * b);\n    }\n  }\n  trace(flag);\n}\n",
        ),
        (
            "loop_cond_append_wrong_receiver.ts",
            "function loopCondAppendWrongReceiver(flag: boolean, ys: number[], b: number, dst: number[], other: number[]): void {\n  if (flag) {\n    for (const a of ys) {\n      dst.push(1 + a);\n    }\n    if (b > 0) {\n      other.push(b * b);\n    }\n  }\n  trace(flag);\n}\n",
        ),
        (
            "loop_cond_append_mutated.ts",
            "function loopCondAppendMutated(flag: boolean, ys: number[], b: number, dst: number[]): void {\n  dst.push(0);\n  if (flag) {\n    for (const a of ys) {\n      dst.push(1 + a);\n    }\n    if (b > 0) {\n      dst.push(b * b);\n    }\n  }\n  trace(flag);\n}\n",
        ),
        (
            "loop_cond_append_third.ts",
            "function loopCondAppendThird(flag: boolean, ys: number[], b: number, c: number, dst: number[]): void {\n  if (flag) {\n    for (const a of ys) {\n      dst.push(1 + a);\n    }\n    if (b > 0) {\n      dst.push(b * b);\n    }\n    dst.push(c + 3);\n  }\n  trace(flag);\n}\n",
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
    for (name, src) in fixtures {
        fs::write(dir.join(name), src).unwrap();
    }

    let out = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "100",
        "--min-size",
        "100",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let json = scan_json(&out);
    let families = scan_families(&json);

    let assert_branch_pair = |left: &str,
                              right: &str,
                              negative: &str,
                              start_line: u64,
                              end_line: u64| {
        let family = block_branch_pair_family(
            families, left, right, negative, start_line, end_line,
        )
        .unwrap_or_else(|| {
            panic!("missing ordered loop conditional-effect branch family {left}/{right}: {out}")
        });
        assert!(
                family["locations"]
                    .as_array()
                    .expect("locations")
                    .iter()
                    .all(|loc| loc["kind"] == "Block"),
                "ordered loop conditional-effect branch fragments should report as Block units: {family:?}"
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
                families,
                (left, left_start, left_end),
                (right, right_start, right_end),
            ),
            "ordered loop conditional-effect branch boundary must not merge {left}/{right}: {out}"
        );
    };

    assert_branch_pair(
        "loop_cond_append_a.ts",
        "loop_cond_append_b.ts",
        "loop_cond_append_wrong_order.ts",
        2,
        9,
    );
    assert_branch_pair(
        "loop_cond_append_a.py",
        "loop_cond_append_b.py",
        "loop_cond_append_wrong_temp.py",
        2,
        7,
    );
    assert_branch_pair(
        "loop_cond_index_a.go",
        "loop_cond_index_b.go",
        "loop_cond_index_wrong_index.go",
        3,
        10,
    );

    assert_no_branch_pair(
        "loop_cond_append_a.ts",
        "loop_cond_append_wrong_order.ts",
        2,
        9,
        2,
        9,
    );
    assert_no_branch_pair(
        "loop_cond_append_a.ts",
        "loop_cond_append_wrong_guard.ts",
        2,
        9,
        2,
        9,
    );
    assert_no_branch_pair(
        "loop_cond_append_a.ts",
        "loop_cond_append_wrong_receiver.ts",
        2,
        9,
        2,
        9,
    );
    assert_no_branch_pair(
        "loop_cond_append_a.ts",
        "loop_cond_append_mutated.ts",
        2,
        9,
        3,
        10,
    );
    assert_no_branch_pair(
        "loop_cond_append_a.ts",
        "loop_cond_append_third.ts",
        2,
        9,
        2,
        10,
    );
    assert_no_branch_pair(
        "loop_cond_append_a.py",
        "loop_cond_append_wrong_order.py",
        2,
        7,
        2,
        7,
    );
    assert_no_branch_pair(
        "loop_cond_append_a.py",
        "loop_cond_append_wrong_temp.py",
        2,
        7,
        2,
        7,
    );
    assert_no_branch_pair(
        "loop_cond_index_a.go",
        "loop_cond_index_wrong_index.go",
        3,
        10,
        3,
        10,
    );
    assert_no_branch_pair(
        "loop_cond_index_a.go",
        "loop_cond_index_wrong_receiver.go",
        3,
        10,
        3,
        10,
    );

    let _ = fs::remove_dir_all(&dir);
}

// Broad fixture matrix for ordered loop conditional mixed-effect branch boundaries. The size is
// intentional until the fixture setup has a clearer table-builder abstraction.
#[allow(clippy::too_many_lines)]
#[test]
fn semantic_scan_reports_exact_safe_ordered_loop_conditional_mixed_effect_branch_fragments() {
    let dir = std::env::temp_dir().join(format!(
        "nose_ordered_loop_conditional_mixed_effect_branch_fragments_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    let fixtures = [
        (
            "loop_cond_mixed_append_a.ts",
            "function loopCondMixedLeft(enabled: boolean, xs: number[], y: number, z: number, out: number[]): void {\n  if (enabled) {\n    for (const x of xs) {\n      out.push(x + 1);\n    }\n    if (y > 0) {\n      out.push(y * y);\n    }\n    out.push(z + 3);\n  }\n  audit(enabled);\n}\n",
        ),
        (
            "loop_cond_mixed_append_b.ts",
            "function loopCondMixedRight(flag: boolean, ys: number[], b: number, c: number, dst: number[]): void {\n  if (flag) {\n    for (const a of ys) {\n      dst.push(1 + a);\n    }\n    if (b > 0) {\n      dst.push(b * b);\n    }\n    dst.push(3 + c);\n  }\n  trace(flag);\n}\n",
        ),
        (
            "loop_cond_mixed_append_wrong_order.ts",
            "function loopCondMixedWrongOrder(flag: boolean, ys: number[], b: number, c: number, dst: number[]): void {\n  if (flag) {\n    if (b > 0) {\n      dst.push(b * b);\n    }\n    for (const a of ys) {\n      dst.push(1 + a);\n    }\n    dst.push(3 + c);\n  }\n  trace(flag);\n}\n",
        ),
        (
            "loop_cond_mixed_append_wrong_guard.ts",
            "function loopCondMixedWrongGuard(flag: boolean, ys: number[], b: number, c: number, dst: number[]): void {\n  if (flag) {\n    for (const a of ys) {\n      dst.push(1 + a);\n    }\n    if (b >= 0) {\n      dst.push(b * b);\n    }\n    dst.push(3 + c);\n  }\n  trace(flag);\n}\n",
        ),
        (
            "loop_cond_mixed_append_wrong_receiver.ts",
            "function loopCondMixedWrongReceiver(flag: boolean, ys: number[], b: number, c: number, dst: number[], other: number[]): void {\n  if (flag) {\n    for (const a of ys) {\n      dst.push(1 + a);\n    }\n    if (b > 0) {\n      dst.push(b * b);\n    }\n    other.push(3 + c);\n  }\n  trace(flag);\n}\n",
        ),
        (
            "loop_cond_mixed_append_mutated.ts",
            "function loopCondMixedMutated(flag: boolean, ys: number[], b: number, c: number, dst: number[]): void {\n  dst.push(0);\n  if (flag) {\n    for (const a of ys) {\n      dst.push(1 + a);\n    }\n    if (b > 0) {\n      dst.push(b * b);\n    }\n    dst.push(3 + c);\n  }\n  trace(flag);\n}\n",
        ),
        (
            "loop_cond_mixed_append_fourth.ts",
            "function loopCondMixedFourth(flag: boolean, ys: number[], b: number, c: number, d: number, dst: number[]): void {\n  if (flag) {\n    for (const a of ys) {\n      dst.push(1 + a);\n    }\n    if (b > 0) {\n      dst.push(b * b);\n    }\n    dst.push(3 + c);\n    dst.push(d + 4);\n  }\n  trace(flag);\n}\n",
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
    for (name, src) in fixtures {
        fs::write(dir.join(name), src).unwrap();
    }

    let out = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "100",
        "--min-size",
        "100",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let json = scan_json(&out);
    let families = scan_families(&json);

    let assert_branch_pair = |left: &str,
                              right: &str,
                              negative: &str,
                              start_line: u64,
                              end_line: u64| {
        let family =
            block_branch_pair_family(families, left, right, negative, start_line, end_line)
                .unwrap_or_else(|| {
                    panic!(
                "missing ordered loop conditional mixed-effect branch family {left}/{right}: {out}"
            )
                });
        assert!(
                family["locations"]
                    .as_array()
                    .expect("locations")
                    .iter()
                    .all(|loc| loc["kind"] == "Block"),
                "ordered loop conditional mixed-effect branch fragments should report as Block units: {family:?}"
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
                families,
                (left, left_start, left_end),
                (right, right_start, right_end),
            ),
            "ordered loop conditional mixed-effect branch boundary must not merge {left}/{right}: {out}"
        );
    };

    assert_branch_pair(
        "loop_cond_mixed_append_a.ts",
        "loop_cond_mixed_append_b.ts",
        "loop_cond_mixed_append_wrong_order.ts",
        2,
        10,
    );
    assert_branch_pair(
        "loop_cond_mixed_append_a.py",
        "loop_cond_mixed_append_b.py",
        "loop_cond_mixed_append_wrong_temp.py",
        2,
        8,
    );
    assert_branch_pair(
        "loop_cond_mixed_index_a.go",
        "loop_cond_mixed_index_b.go",
        "loop_cond_mixed_index_wrong_index.go",
        3,
        11,
    );

    assert_no_branch_pair(
        "loop_cond_mixed_append_a.ts",
        "loop_cond_mixed_append_wrong_order.ts",
        2,
        10,
        2,
        10,
    );
    assert_no_branch_pair(
        "loop_cond_mixed_append_a.ts",
        "loop_cond_mixed_append_wrong_guard.ts",
        2,
        10,
        2,
        10,
    );
    assert_no_branch_pair(
        "loop_cond_mixed_append_a.ts",
        "loop_cond_mixed_append_wrong_receiver.ts",
        2,
        10,
        2,
        10,
    );
    assert_no_branch_pair(
        "loop_cond_mixed_append_a.ts",
        "loop_cond_mixed_append_mutated.ts",
        2,
        10,
        3,
        11,
    );
    assert_no_branch_pair(
        "loop_cond_mixed_append_a.ts",
        "loop_cond_mixed_append_fourth.ts",
        2,
        10,
        2,
        11,
    );
    assert_no_branch_pair(
        "loop_cond_mixed_append_a.py",
        "loop_cond_mixed_append_wrong_order.py",
        2,
        8,
        2,
        8,
    );
    assert_no_branch_pair(
        "loop_cond_mixed_append_a.py",
        "loop_cond_mixed_append_wrong_temp.py",
        2,
        8,
        2,
        8,
    );
    assert_no_branch_pair(
        "loop_cond_mixed_index_a.go",
        "loop_cond_mixed_index_wrong_index.go",
        3,
        11,
        3,
        11,
    );
    assert_no_branch_pair(
        "loop_cond_mixed_index_a.go",
        "loop_cond_mixed_index_wrong_receiver.go",
        3,
        11,
        3,
        11,
    );

    let _ = fs::remove_dir_all(&dir);
}

// Broad fixture matrix for ordered append-effect branch boundaries. The size is
// intentional until the fixture setup has a clearer table-builder abstraction.
#[allow(clippy::too_many_lines)]
#[test]
fn semantic_scan_reports_exact_safe_ordered_append_effect_branch_fragments() {
    let dir = std::env::temp_dir().join(format!(
        "nose_append_effect_order_boundary_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

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
    for (name, src) in fixtures {
        fs::write(dir.join(name), src).unwrap();
    }

    let out = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "100",
        "--min-size",
        "100",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let json = scan_json(&out);
    let families = scan_families(&json);

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
fn semantic_scan_reports_exact_safe_three_append_effect_branch_fragments() {
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
        scan_fragment_only_fixtures("nose_three_append_effect_boundary", &fixtures);

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

#[test]
fn semantic_scan_reports_exact_safe_ordered_index_assignment_branch_fragments_for_go() {
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
        scan_fragment_only_fixtures("nose_index_effect_order_boundary", &fixtures);

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
fn semantic_scan_reports_exact_safe_three_index_assignment_branch_fragments_for_go() {
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
        scan_fragment_only_fixtures("nose_three_index_effect_boundary", &fixtures);

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
fn semantic_scan_reports_exact_safe_index_assignment_fragments_for_non_overloaded_languages() {
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
        scan_fragment_only_fixtures("nose_exact_index_assign_fragments", &fixtures);

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
fn semantic_scan_reports_exact_safe_java_this_field_assignment_fragments() {
    let fixtures = [
        (
            "FieldSelfSquareA.java",
            "class FieldSelfSquareA {\n  int value;\n  void f(int v) {\n    this.value = (v + 1) * (v + 1);\n    audit(this);\n  }\n}\n",
        ),
        (
            "FieldSelfSquareB.java",
            "class FieldSelfSquareB {\n  int value;\n  void f(int w) {\n    this.value = (1 + w) * (1 + w);\n    trace(this);\n  }\n}\n",
        ),
        (
            "FieldSelfSquareWrongValue.java",
            "class FieldSelfSquareWrongValue {\n  int value;\n  void f(int x) {\n    this.value = (x + 2) * (x + 2);\n    audit(this);\n  }\n}\n",
        ),
        (
            "FieldSelfConditionalA.java",
            "class FieldSelfConditionalA {\n  int total;\n  int other;\n  void f(boolean enabled, int a, int b) {\n    if (enabled) {\n      this.total = a + b;\n    }\n    audit(this);\n  }\n}\n",
        ),
        (
            "FieldSelfConditionalB.java",
            "class FieldSelfConditionalB {\n  int total;\n  int other;\n  void f(boolean ready, int c, int d) {\n    if (ready) {\n      this.total = d + c;\n    }\n    trace(this);\n  }\n}\n",
        ),
        (
            "FieldSelfConditionalWrongField.java",
            "class FieldSelfConditionalWrongField {\n  int total;\n  int other;\n  void f(boolean ready, int c, int d) {\n    if (ready) {\n      this.other = d + c;\n    }\n    audit(this);\n  }\n}\n",
        ),
        (
            "FieldSelfNestedA.java",
            "class FieldSelfNestedA {\n  int score;\n  void f(boolean enabled, int a, int b) {\n    if (enabled) {\n      if (a > 0) {\n        this.score = (a + b) * 2;\n      }\n    }\n    audit(this);\n  }\n}\n",
        ),
        (
            "FieldSelfNestedB.java",
            "class FieldSelfNestedB {\n  int score;\n  void f(boolean ready, int c, int d) {\n    if (ready) {\n      if (0 < c) {\n        this.score = 2 * (d + c);\n      }\n    }\n    trace(this);\n  }\n}\n",
        ),
        (
            "FieldSelfNestedWrongReceiver.java",
            "class FieldSelfNestedWrongReceiverBox { int score; }\nclass FieldSelfNestedWrongReceiver {\n  int score;\n  void f(FieldSelfNestedWrongReceiverBox other, boolean ready, int c, int d) {\n    if (ready) {\n      if (0 < c) {\n        other.score = 2 * (d + c);\n      }\n    }\n    audit(this);\n  }\n}\n",
        ),
        (
            "js_this_field_a.js",
            "function jsThisFieldLeft(v) {\n  this.value = (v + 1) * (v + 1);\n  audit(this);\n}\n",
        ),
        (
            "js_this_field_b.js",
            "function jsThisFieldRight(w) {\n  this.value = (1 + w) * (1 + w);\n  trace(this);\n}\n",
        ),
        (
            "py_self_field_a.py",
            "class PyFieldLeft:\n    def f(self, v):\n        self.value = (v + 1) * (v + 1)\n        audit(self)\n",
        ),
        (
            "py_self_field_b.py",
            "class PyFieldRight:\n    def f(self, w):\n        self.value = (1 + w) * (1 + w)\n        trace(self)\n",
        ),
    ];
    let (dir, out, families) =
        scan_fragment_only_fixtures("nose_exact_this_field_assign_fragments", &fixtures);

    let assert_fragment_family = |left: &str, right: &str, negative: &str| {
        let family =
            find_block_pair_family(&families, left, right, negative).unwrap_or_else(|| {
                panic!("missing exact this-field fragment family {left}/{right}: {out}")
            });
        assert!(
            pair_locations(family, left, right)
                .iter()
                .all(|loc| loc["end_line"].as_u64().unwrap_or(0)
                    <= loc["start_line"].as_u64().unwrap_or(0) + 5),
            "this-field fragments should stay tightly scoped: {family:?}"
        );
    };

    let assert_no_pair = |left: &str, right: &str| {
        assert!(
            !has_pair_family(&families, left, right),
            "dynamic or wrong-receiver field assignment must stay outside exact fragments: {left}/{right}: {out}"
        );
    };

    assert_fragment_family(
        "FieldSelfSquareA.java",
        "FieldSelfSquareB.java",
        "FieldSelfSquareWrongValue.java",
    );
    assert_fragment_family(
        "FieldSelfConditionalA.java",
        "FieldSelfConditionalB.java",
        "FieldSelfConditionalWrongField.java",
    );
    assert_fragment_family(
        "FieldSelfNestedA.java",
        "FieldSelfNestedB.java",
        "FieldSelfNestedWrongReceiver.java",
    );
    assert_no_pair("FieldSelfNestedA.java", "FieldSelfNestedWrongReceiver.java");
    assert_no_pair("js_this_field_a.js", "js_this_field_b.js");
    assert_no_pair("py_self_field_a.py", "py_self_field_b.py");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn semantic_scan_reports_exact_safe_ordered_java_this_field_branch_fragments() {
    let fixtures = [
        (
            "FieldBranchOrderedA.java",
            "class FieldBranchOrderedA {\n  int value;\n  int limit;\n  void f(boolean enabled, int a, int b) {\n    if (enabled) {\n      this.value = (a + 1) * (a + 1);\n      this.limit = b - a;\n    }\n    audit(this);\n  }\n}\n",
        ),
        (
            "FieldBranchOrderedB.java",
            "class FieldBranchOrderedB {\n  int value;\n  int limit;\n  void f(boolean ready, int c, int d) {\n    if (ready) {\n      this.value = (1 + c) * (1 + c);\n      this.limit = d - c;\n    }\n    trace(this);\n  }\n}\n",
        ),
        (
            "FieldBranchOrderedWrongReceiver.java",
            "class FieldBranchOrderedWrongReceiverBox { int value; }\nclass FieldBranchOrderedWrongReceiver {\n  int value;\n  int limit;\n  void f(FieldBranchOrderedWrongReceiverBox other, boolean ready, int c, int d) {\n    if (ready) {\n      other.value = (1 + c) * (1 + c);\n      this.limit = d - c;\n    }\n    audit(this);\n  }\n}\n",
        ),
        (
            "FieldBranchTripleA.java",
            "class FieldBranchTripleA {\n  int value;\n  int limit;\n  int score;\n  void f(boolean enabled, int a, int b) {\n    if (enabled) {\n      this.value = a + b;\n      this.limit = (a + b) * 2;\n      this.score = b - a;\n    }\n    audit(this);\n  }\n}\n",
        ),
        (
            "FieldBranchTripleB.java",
            "class FieldBranchTripleB {\n  int value;\n  int limit;\n  int score;\n  void f(boolean ready, int c, int d) {\n    if (ready) {\n      this.value = d + c;\n      this.limit = 2 * (d + c);\n      this.score = d - c;\n    }\n    trace(this);\n  }\n}\n",
        ),
        (
            "FieldBranchTripleWrongReceiver.java",
            "class FieldBranchTripleWrongReceiverBox { int limit; }\nclass FieldBranchTripleWrongReceiver {\n  int value;\n  int limit;\n  int score;\n  void f(FieldBranchTripleWrongReceiverBox other, boolean ready, int c, int d) {\n    if (ready) {\n      this.value = d + c;\n      other.limit = 2 * (d + c);\n      this.score = d - c;\n    }\n    audit(this);\n  }\n}\n",
        ),
    ];
    let (dir, out, families) =
        scan_fragment_only_fixtures("nose_ordered_this_field_branch_fragments", &fixtures);

    let assert_branch_family = |left: &str, right: &str, negative: &str| {
        let family = families
            .iter()
            .find(|family| {
                let files = location_files(family);
                files.iter().any(|file| file.ends_with(left))
                    && files.iter().any(|file| file.ends_with(right))
                    && files.iter().all(|file| !file.ends_with(negative))
                    && family_locations(family)
                        .iter()
                        .all(|loc| loc["fragment_kind"] == "conditional-guard")
            })
            .unwrap_or_else(|| {
                panic!("missing ordered self-field branch fragment family {left}/{right}: {out}")
            });
        assert!(
            pair_locations(family, left, right)
                .iter()
                .all(|loc| loc["end_line"].as_u64().unwrap_or(0)
                    <= loc["start_line"].as_u64().unwrap_or(0) + 5),
            "ordered self-field branch fragments should stay tightly scoped: {family:?}"
        );
    };

    let assert_no_conditional_guard_location = |negative: &str| {
        let has_conditional_guard = families.iter().any(|family| {
            family_locations(family).iter().any(|loc| {
                loc["file"]
                    .as_str()
                    .is_some_and(|file| file.ends_with(negative))
                    && loc["fragment_kind"] == "conditional-guard"
            })
        });
        assert!(
            !has_conditional_guard,
            "wrong receiver must not produce an ordered self-field conditional guard: {negative}: {out}"
        );
    };

    assert_branch_family(
        "FieldBranchOrderedA.java",
        "FieldBranchOrderedB.java",
        "FieldBranchOrderedWrongReceiver.java",
    );
    assert_branch_family(
        "FieldBranchTripleA.java",
        "FieldBranchTripleB.java",
        "FieldBranchTripleWrongReceiver.java",
    );
    assert_no_conditional_guard_location("FieldBranchOrderedWrongReceiver.java");
    assert_no_conditional_guard_location("FieldBranchTripleWrongReceiver.java");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn semantic_scan_reports_exact_safe_java_this_field_assignment_body_fragments() {
    let fixtures = [
        (
            "FieldBodyDirectA.java",
            "class FieldBodyDirectA {\n  int value;\n  int limit;\n  void f(int v, int n) {\n    this.value = (v + 1) * (v + 1);\n    this.limit = n + 3;\n  }\n}\n",
        ),
        (
            "FieldBodyDirectB.java",
            "class FieldBodyDirectB {\n  int value;\n  int limit;\n  void f(int w, int m) {\n    this.value = (1 + w) * (1 + w);\n    this.limit = 3 + m;\n  }\n}\n",
        ),
        (
            "FieldBodyDirectWrongValue.java",
            "class FieldBodyDirectWrongValue {\n  int value;\n  int limit;\n  void f(int x, int m) {\n    this.value = (x + 1) * (x + 1);\n    this.limit = 4 + m;\n  }\n}\n",
        ),
        (
            "FieldBodyConditionalA.java",
            "class FieldBodyConditionalA {\n  int total;\n  int score;\n  void f(boolean enabled, int a, int b) {\n    this.total = a + b;\n    if (enabled) {\n      this.score = (a + b) * 2;\n    }\n  }\n}\n",
        ),
        (
            "FieldBodyConditionalB.java",
            "class FieldBodyConditionalB {\n  int total;\n  int score;\n  void f(boolean ready, int c, int d) {\n    this.total = d + c;\n    if (ready) {\n      this.score = 2 * (d + c);\n    }\n  }\n}\n",
        ),
        (
            "FieldBodyConditionalWrongField.java",
            "class FieldBodyConditionalWrongField {\n  int total;\n  int score;\n  int other;\n  void f(boolean ready, int c, int d) {\n    this.total = d + c;\n    if (ready) {\n      this.other = 2 * (d + c);\n    }\n  }\n}\n",
        ),
        (
            "FieldBodyNestedA.java",
            "class FieldBodyNestedA {\n  int base;\n  int score;\n  void f(boolean enabled, int a, int b) {\n    this.base = a + b;\n    if (enabled) {\n      if (a > 0) {\n        this.score = (a + b) * (a + b);\n      }\n    }\n  }\n}\n",
        ),
        (
            "FieldBodyNestedB.java",
            "class FieldBodyNestedB {\n  int base;\n  int score;\n  void f(boolean ready, int c, int d) {\n    this.base = d + c;\n    if (ready) {\n      if (0 < c) {\n        this.score = (d + c) * (d + c);\n      }\n    }\n  }\n}\n",
        ),
        (
            "FieldBodyNestedWrongReceiver.java",
            "class FieldBodyNestedWrongReceiverBox { int score; }\nclass FieldBodyNestedWrongReceiver {\n  int base;\n  int score;\n  void f(FieldBodyNestedWrongReceiverBox other, boolean ready, int c, int d) {\n    this.base = d + c;\n    if (ready) {\n      if (0 < c) {\n        other.score = (d + c) * (d + c);\n      }\n    }\n  }\n}\n",
        ),
    ];
    let (dir, out, families) =
        scan_fragment_only_fixtures("nose_exact_this_field_body_fragments", &fixtures);

    let assert_body_family = |left: &str, right: &str, negative: &str| {
        let family = find_multiline_block_pair_family(&families, left, right, negative)
            .unwrap_or_else(|| {
                panic!("missing exact this-field body fragment family {left}/{right}: {out}")
            });
        assert!(
            pair_locations(family, left, right)
                .iter()
                .all(|loc| loc["end_line"].as_u64().unwrap_or(0)
                    <= loc["start_line"].as_u64().unwrap_or(0) + 7),
            "this-field body fragments should stay tightly scoped: {family:?}"
        );
    };

    let assert_no_pair = |left: &str, right: &str| {
        assert!(
            !has_pair_family(&families, left, right),
            "wrong-receiver field body must stay outside exact fragments: {left}/{right}: {out}"
        );
    };

    assert_body_family(
        "FieldBodyDirectA.java",
        "FieldBodyDirectB.java",
        "FieldBodyDirectWrongValue.java",
    );
    assert_body_family(
        "FieldBodyConditionalA.java",
        "FieldBodyConditionalB.java",
        "FieldBodyConditionalWrongField.java",
    );
    assert_body_family(
        "FieldBodyNestedA.java",
        "FieldBodyNestedB.java",
        "FieldBodyNestedWrongReceiver.java",
    );
    assert_no_pair("FieldBodyNestedA.java", "FieldBodyNestedWrongReceiver.java");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn semantic_scan_reports_exact_safe_java_this_field_return_this_body_fragments() {
    let fixtures = [
        (
            "FluentBodyDirectA.java",
            "class FluentBodyDirectA {\n  int value;\n  int limit;\n  FluentBodyDirectA f(int v, int n) {\n    this.value = (v + 1) * (v + 1);\n    this.limit = n + 3;\n    return this;\n  }\n}\n",
        ),
        (
            "FluentBodyDirectB.java",
            "class FluentBodyDirectB {\n  int value;\n  int limit;\n  FluentBodyDirectB f(int w, int m) {\n    this.value = (1 + w) * (1 + w);\n    this.limit = 3 + m;\n    return this;\n  }\n}\n",
        ),
        (
            "FluentBodyDirectWrongReturn.java",
            "class FluentBodyDirectWrongReturn {\n  int value;\n  int limit;\n  FluentBodyDirectWrongReturn f(FluentBodyDirectWrongReturn other, int w, int m) {\n    this.value = (1 + w) * (1 + w);\n    this.limit = 3 + m;\n    return other;\n  }\n}\n",
        ),
        (
            "FluentBodyConditionalA.java",
            "class FluentBodyConditionalA {\n  int total;\n  int score;\n  FluentBodyConditionalA f(boolean enabled, int a, int b) {\n    this.total = a + b;\n    if (enabled) {\n      this.score = (a + b) * 2;\n    }\n    return this;\n  }\n}\n",
        ),
        (
            "FluentBodyConditionalB.java",
            "class FluentBodyConditionalB {\n  int total;\n  int score;\n  FluentBodyConditionalB f(boolean ready, int c, int d) {\n    this.total = d + c;\n    if (ready) {\n      this.score = 2 * (d + c);\n    }\n    return this;\n  }\n}\n",
        ),
        (
            "FluentBodyConditionalWrongField.java",
            "class FluentBodyConditionalWrongField {\n  int total;\n  int score;\n  int other;\n  FluentBodyConditionalWrongField f(boolean ready, int c, int d) {\n    this.total = d + c;\n    if (ready) {\n      this.other = 2 * (d + c);\n    }\n    return this;\n  }\n}\n",
        ),
        (
            "FluentBodyNestedA.java",
            "class FluentBodyNestedA {\n  int base;\n  int score;\n  FluentBodyNestedA f(boolean enabled, int a, int b) {\n    this.base = a + b;\n    if (enabled) {\n      if (a > 0) {\n        this.score = (a + b) * (a + b);\n      }\n    }\n    return this;\n  }\n}\n",
        ),
        (
            "FluentBodyNestedB.java",
            "class FluentBodyNestedB {\n  int base;\n  int score;\n  FluentBodyNestedB f(boolean ready, int c, int d) {\n    this.base = d + c;\n    if (ready) {\n      if (0 < c) {\n        this.score = (d + c) * (d + c);\n      }\n    }\n    return this;\n  }\n}\n",
        ),
        (
            "FluentBodyNestedWrongValue.java",
            "class FluentBodyNestedWrongValue {\n  int base;\n  int score;\n  FluentBodyNestedWrongValue f(boolean ready, int c, int d) {\n    this.base = d + c;\n    if (ready) {\n      if (0 < c) {\n        this.score = (d + c) + (d + c);\n      }\n    }\n    return this;\n  }\n}\n",
        ),
    ];
    let (dir, out, families) = scan_fragment_only_fixtures(
        "nose_exact_this_field_return_this_body_fragments",
        &fixtures,
    );

    let assert_body_family = |left: &str, right: &str, negative: &str| {
        let family = find_multiline_block_pair_family(&families, left, right, negative)
            .unwrap_or_else(|| {
                panic!(
                    "missing exact this-field return-this body fragment family {left}/{right}: {out}"
                )
            });
        assert!(
            pair_locations(family, left, right)
                .iter()
                .all(|loc| loc["end_line"].as_u64().unwrap_or(0)
                    <= loc["start_line"].as_u64().unwrap_or(0) + 9),
            "this-field return-this body fragments should stay tightly scoped: {family:?}"
        );
    };

    let assert_no_pair = |left: &str, right: &str| {
        assert!(
            !has_pair_family(&families, left, right),
            "wrong-return field body must stay outside exact fragments: {left}/{right}: {out}"
        );
    };

    assert_body_family(
        "FluentBodyDirectA.java",
        "FluentBodyDirectB.java",
        "FluentBodyDirectWrongReturn.java",
    );
    assert_body_family(
        "FluentBodyConditionalA.java",
        "FluentBodyConditionalB.java",
        "FluentBodyConditionalWrongField.java",
    );
    assert_body_family(
        "FluentBodyNestedA.java",
        "FluentBodyNestedB.java",
        "FluentBodyNestedWrongValue.java",
    );
    assert_no_pair("FluentBodyDirectA.java", "FluentBodyDirectWrongReturn.java");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn semantic_scan_reports_exact_safe_throw_fragments_under_opaque_functions() {
    let dir =
        std::env::temp_dir().join(format!("nose_exact_throw_fragments_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

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
            "function throwSquaresRight(ys) {\n  throw ys[1] * ys[1] + ys[0] * ys[0];\n  trace(ys);\n}\n",
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
            "function throwProductRight(ys) {\n  throw (4 + ys[2]) * (ys[1] + ys[0]);\n  trace(ys);\n}\n",
        ),
        (
            "throw_product_mutated.js",
            "function throwProductMutated(zs) {\n  zs.push(1);\n  throw (zs[0] + zs[1]) * (zs[2] + 4);\n  audit(zs);\n}\n",
        ),
    ];
    for (name, src) in fixtures {
        fs::write(dir.join(name), src).unwrap();
    }

    let out = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let json = scan_json(&out);
    let families = scan_families(&json);

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
