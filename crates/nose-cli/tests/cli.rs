//! End-to-end CLI tests: run the built `nose` binary against a temp project and
//! check the user-visible behavior (discovery, `scan` report, `--exclude`).

use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_nose")
}

/// Write a small project (a 3-copy clone family + a decoy) into a unique temp dir.
fn make_project(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nose_cli_{tag}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let body = |acc: &str, it: &str| {
        format!("def f(items):\n    {acc} = 0\n    for {it} in items:\n        if {it} > 0:\n            {acc} = {acc} + {it} * {it}\n    return {acc}\n")
    };
    for (sub, src) in [
        ("a", body("total", "x")),
        ("b", body("acc", "v")),
        ("tests", body("s", "n")),
    ] {
        let d = dir.join(sub);
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("f.py"), src).unwrap();
    }
    let d = dir.join("c");
    fs::create_dir_all(&d).unwrap();
    fs::write(
        d.join("decoy.py"),
        "def greet(n):\n    m = 'hi ' + n\n    print(m)\n    print(n)\n    return m\n",
    )
    .unwrap();
    dir
}

fn make_mode_project(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nose_modes_{tag}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    fs::write(
        dir.join("renamed_a.py"),
        "def total(items):\n    total = 0\n    for item in items:\n        if item > 0:\n            total = total + item * item\n    return total\n",
    )
    .unwrap();
    fs::write(
        dir.join("renamed_b.py"),
        "def score(values):\n    acc = 0\n    for value in values:\n        if value > 0:\n            acc = acc + value * value\n    return acc\n",
    )
    .unwrap();

    let copied = "def copied(events):\n    out = []\n    for e in events:\n        if e.kind == 1:\n            out.append(e.payload)\n            record(e.id, e.kind)\n    return out\n";
    fs::write(dir.join("copy_a.py"), copied).unwrap();
    fs::write(dir.join("copy_b.py"), copied).unwrap();

    dir
}

fn run(args: &[&str]) -> String {
    let out = Command::new(bin()).args(args).output().expect("run nose");
    assert!(
        out.status.success(),
        "nose exited non-zero: {:?}",
        out.status
    );
    String::from_utf8(out.stdout).unwrap()
}

fn add_distinct_clone_family(dir: &Path) {
    let d = dir.join("new");
    fs::create_dir_all(&d).unwrap();
    let body = |name: &str, acc: &str, it: &str| {
        format!(
            "def {name}(items):\n    {acc} = 1\n    for {it} in items:\n        if {it} < 10:\n            {acc} = {acc} * ({it} + 3)\n            {acc} = {acc} - {it}\n    return {acc}\n"
        )
    };
    fs::write(d.join("fresh_a.py"), body("fresh_a", "total", "item")).unwrap();
    fs::write(d.join("fresh_b.py"), body("fresh_b", "score", "value")).unwrap();
}

fn add_member_to_existing_family(dir: &Path) {
    let d = dir.join("d");
    fs::create_dir_all(&d).unwrap();
    fs::write(
        d.join("f.py"),
        "def f(items):\n    sum = 0\n    for z in items:\n        if z > 0:\n            sum = sum + z * z\n    return sum\n",
    )
    .unwrap();
}

fn scan_json(out: &str) -> serde_json::Value {
    serde_json::from_str(out).expect("scan should emit valid JSON")
}

fn scan_families(json: &serde_json::Value) -> &[serde_json::Value] {
    json["families"]
        .as_array()
        .expect("scan JSON should contain families array")
}

fn json_array_strings<'a>(value: &'a serde_json::Value, key: &str) -> Vec<&'a str> {
    value[key]
        .as_array()
        .unwrap_or_else(|| panic!("{key} should be an array"))
        .iter()
        .map(|item| {
            item.as_str()
                .unwrap_or_else(|| panic!("{key} entries should be strings"))
        })
        .collect()
}

fn assert_scan_json_v1_contract(json: &serde_json::Value) {
    assert_eq!(json["schema_version"], 1);
    assert!(
        json["tool_version"].as_str().is_some_and(|s| !s.is_empty()),
        "tool_version should be a non-empty string: {json}"
    );
    assert!(
        json["scope"]["files"].is_number(),
        "scope.files should be numeric: {json}"
    );
    assert!(
        json["scope"]["languages"].as_array().is_some(),
        "scope.languages should be an array: {json}"
    );

    let ranking = json["ranking"].as_object().expect("ranking object");
    for key in ["sort", "total_families", "shown_families", "limit"] {
        assert!(ranking.contains_key(key), "ranking.{key} missing: {json}");
    }

    let family = scan_families(json)
        .first()
        .expect("fixture should include a family");
    let family = family.as_object().expect("family object");
    for key in [
        "family_id",
        "value",
        "members",
        "files",
        "modules",
        "languages",
        "mean_score",
        "mean_lines",
        "dup_lines",
        "shared_lines",
        "params",
        "shared_weight",
        "locations",
        "mean_sem",
        "scope",
        "discount",
    ] {
        assert!(family.contains_key(key), "family.{key} missing: {json}");
    }

    let loc = family["locations"]
        .as_array()
        .and_then(|locations| locations.first())
        .and_then(|location| location.as_object())
        .expect("family.locations should contain location objects");
    for key in ["file", "start_line", "end_line", "lang", "kind", "sem"] {
        assert!(loc.contains_key(key), "location.{key} missing: {json}");
    }
}

#[test]
fn checked_in_scan_json_v1_example_matches_contract() {
    let json = scan_json(include_str!("fixtures/scan-json-v1.json"));
    assert_scan_json_v1_contract(&json);
}

#[test]
fn scan_json_report_has_versioned_contract() {
    let dir = make_project("json_contract");
    let out = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--min-size",
        "12",
        "--format",
        "json",
        "--top",
        "1",
    ]);
    let json = scan_json(&out);
    assert_scan_json_v1_contract(&json);
    assert_eq!(json["tool_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(json["scope"]["files"], 4);
    assert_eq!(json["scope"]["languages"][0]["language"], "python");
    assert_eq!(json["ranking"]["sort"], "extractability");
    assert_eq!(json["ranking"]["shown_families"], 1);
    assert_eq!(json["ranking"]["limit"], 1);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_syntax_reports_copy_paste_only() {
    let dir = make_mode_project("syntax");
    let p = dir.to_str().unwrap();
    let out = run(&[
        "scan",
        p,
        "--mode",
        "syntax",
        "--min-size",
        "12",
        "--format",
        "json",
    ]);
    assert!(
        out.contains("copy_a.py"),
        "syntax reports exact copies: {out}"
    );
    assert!(
        out.contains("copy_b.py"),
        "syntax reports exact copies: {out}"
    );
    assert!(
        !out.contains("renamed_a.py") && !out.contains("renamed_b.py"),
        "syntax must not report semantic renamed clones: {out}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_syntax_min_tokens_controls_copy_paste_floor() {
    let dir = make_mode_project("syntax_floor");
    let p = dir.to_str().unwrap();
    let out = run(&[
        "scan",
        p,
        "--mode",
        "syntax",
        "--min-size",
        "80",
        "--format",
        "json",
    ]);
    let json = scan_json(&out);
    assert!(
        scan_families(&json).is_empty(),
        "a high syntax token floor suppresses the short copy-paste run: {out}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_keeps_renamed_exact_clone_candidates() {
    let dir = make_mode_project("semantic_mode");
    let p = dir.to_str().unwrap();
    let out = run(&[
        "scan",
        p,
        "--mode",
        "semantic",
        "--min-size",
        "12",
        "--format",
        "json",
    ]);
    assert!(
        out.contains("renamed_a.py") && out.contains("renamed_b.py"),
        "semantic mode keeps exact value-fingerprint candidates: {out}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_reports_c_u16_byte_pack_only_when_byte_buffer_proven() {
    let dir = std::env::temp_dir().join(format!("nose_c_u16_pack_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("add.c"),
        "typedef unsigned char u8;\nunsigned int add_pack(const u8 *a) {\n  return (a[0] << 8) + a[1];\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("or.c"),
        "unsigned int or_pack(unsigned char *a) {\n  return (a[0] << 8) | a[1];\n}\n",
    )
    .unwrap();
    fs::write(dir.join("bytes.h"), "typedef unsigned char u8;\n").unwrap();
    fs::write(
        dir.join("include_add.c"),
        "#include \"bytes.h\"\nunsigned int include_add_pack(const u8 *a) {\n  return (a[0] << 8) + a[1];\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_order.c"),
        "typedef unsigned char u8;\nunsigned int wrong_order(const u8 *a) {\n  return (a[1] << 8) | a[0];\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("int_pointer.c"),
        "unsigned int int_pointer(const int *a) {\n  return (a[0] << 8) | a[1];\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("missing_include.c"),
        "#include \"missing.h\"\nunsigned int missing_include_pack(const u8 *a) {\n  return (a[0] << 8) | a[1];\n}\n",
    )
    .unwrap();
    fs::write(dir.join("not_bytes.h"), "typedef unsigned short u8;\n").unwrap();
    fs::write(
        dir.join("wrong_include.c"),
        "#include \"not_bytes.h\"\nunsigned int wrong_include_pack(const u8 *a) {\n  return (a[0] << 8) | a[1];\n}\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    let family_files = |family: &serde_json::Value| -> Vec<String> {
        family["locations"]
            .as_array()
            .expect("locations")
            .iter()
            .filter_map(|loc| loc["file"].as_str())
            .map(str::to_string)
            .collect()
    };
    let positive_family = semantic_families
        .iter()
        .find(|family| {
            let files = family_files(family);
            files.iter().any(|file| file.ends_with("add.c"))
                && files.iter().any(|file| file.ends_with("or.c"))
                && files.iter().any(|file| file.ends_with("include_add.c"))
        })
        .unwrap_or_else(|| panic!("missing proven byte-buffer u16 pack family: {semantic}"));
    let positive_text = positive_family.to_string();
    assert!(
        !positive_text.contains("wrong_order.c") && !positive_text.contains("int_pointer.c"),
        "semantic mode must preserve byte order and require a byte-buffer proof: {semantic}"
    );
    assert!(
        !positive_text.contains("missing_include.c") && !positive_text.contains("wrong_include.c"),
        "semantic mode must not guess missing or non-byte include aliases: {semantic}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_reports_c_u32_byte_pack_only_when_unsigned_cast_proven() {
    let dir = std::env::temp_dir().join(format!("nose_c_u32_pack_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("recover.c"),
        "typedef unsigned char u8;\ntypedef unsigned int u32;\nu32 recover_get_u32(const u8 *a) {\n  return (((u32)a[0]) << 24) + (((u32)a[1]) << 16) + (((u32)a[2]) << 8) + ((u32)a[3]);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("dbdata.c"),
        "typedef unsigned char u8;\ntypedef unsigned int u32;\nu32 dbdata_get_u32(unsigned char *a) {\n  return ((u32)a[0] << 24) | ((u32)a[1] << 16) | ((u32)a[2] << 8) | ((u32)a[3]);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("bytes.h"),
        "typedef unsigned char u8;\ntypedef unsigned int u32;\n",
    )
    .unwrap();
    fs::write(
        dir.join("include_u32.c"),
        "#include \"bytes.h\"\nu32 include_get_u32(const u8 *a) {\n  return ((u32)a[0] << 24) + ((u32)a[1] << 16) + ((u32)a[2] << 8) + ((u32)a[3] << 0);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("unsigned_int.c"),
        "unsigned int direct_get_u32(unsigned char *a) {\n  return ((unsigned int)a[0] << 24) + ((unsigned int)a[1] << 16) + ((unsigned int)a[2] << 8) + (unsigned int)a[3];\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("uncasted.c"),
        "typedef unsigned char u8;\ntypedef unsigned int u32;\nu32 uncasted_get_u32(const u8 *a) {\n  return (a[0] << 24) | (a[1] << 16) | (a[2] << 8) | a[3];\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_order.c"),
        "typedef unsigned char u8;\ntypedef unsigned int u32;\nu32 wrong_order(const u8 *a) {\n  return ((u32)a[1] << 24) | ((u32)a[0] << 16) | ((u32)a[2] << 8) | ((u32)a[3]);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_alias.c"),
        "typedef unsigned char u8;\ntypedef signed int u32;\nu32 wrong_alias(const u8 *a) {\n  return ((u32)a[0] << 24) | ((u32)a[1] << 16) | ((u32)a[2] << 8) | ((u32)a[3]);\n}\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    let family_files = |family: &serde_json::Value| -> Vec<String> {
        family["locations"]
            .as_array()
            .expect("locations")
            .iter()
            .filter_map(|loc| loc["file"].as_str())
            .map(str::to_string)
            .collect()
    };
    let positive_family = semantic_families
        .iter()
        .find(|family| {
            let files = family_files(family);
            files.iter().any(|file| file.ends_with("recover.c"))
                && files.iter().any(|file| file.ends_with("dbdata.c"))
                && files.iter().any(|file| file.ends_with("include_u32.c"))
                && files.iter().any(|file| file.ends_with("unsigned_int.c"))
        })
        .unwrap_or_else(|| panic!("missing proven unsigned-cast u32 pack family: {semantic}"));
    let positive_text = positive_family.to_string();
    assert!(
        !positive_text.contains("uncasted.c")
            && !positive_text.contains("wrong_order.c")
            && !positive_text.contains("wrong_alias.c"),
        "semantic mode must require unsigned-cast, lane-order, and alias proofs: {semantic}"
    );

    let _ = fs::remove_dir_all(&dir);
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
    let dir = std::env::temp_dir().join(format!(
        "nose_exact_throw_guard_fragments_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

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
                panic!("missing exact conditional throw fragment family {left}/{right}: {out}")
            });
        let locations = family["locations"].as_array().expect("locations");
        assert!(
            locations.iter().all(|loc| loc["kind"] == "Block"),
            "conditional throw fragments should report as Block units: {family:?}"
        );
        assert!(
            locations
                .iter()
                .all(|loc| !loc["file"].as_str().unwrap_or("").ends_with(negative)),
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
    let dir = std::env::temp_dir().join(format!(
        "nose_exact_empty_branch_fragments_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

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
                panic!("missing exact empty-branch fragment family {left}/{right}: {out}")
            });
        let locations = family["locations"].as_array().expect("locations");
        assert!(
            locations.iter().all(|loc| loc["kind"] == "Block"),
            "empty-branch conditional fragments should report as Block units: {family:?}"
        );
        assert!(
            locations
                .iter()
                .all(|loc| !loc["file"].as_str().unwrap_or("").ends_with(negative)),
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
    let dir = std::env::temp_dir().join(format!(
        "nose_exact_nested_fragments_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

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

    let assert_guard_family = |left: &str, right: &str, negative: &str| {
        let family = families
            .iter()
            .find(|family| {
                let locations = family["locations"].as_array().expect("locations");
                let full_nested_files: Vec<&str> = locations
                    .iter()
                    .filter(|loc| loc["start_line"] == 2 && loc["end_line"] == 8)
                    .filter_map(|loc| loc["file"].as_str())
                    .collect();
                full_nested_files.iter().any(|file| file.ends_with(left))
                    && full_nested_files.iter().any(|file| file.ends_with(right))
                    && locations.iter().all(|loc| loc["kind"] == "Block")
                    && locations
                        .iter()
                        .filter_map(|loc| loc["file"].as_str())
                        .all(|file| !file.ends_with(negative))
            })
            .unwrap_or_else(|| {
                panic!("missing exact nested conditional effect family {left}/{right}: {out}")
            });
        let locations = family["locations"].as_array().expect("locations");
        assert!(
            locations
                .iter()
                .filter(|loc| loc["file"].as_str().unwrap_or("").ends_with(left)
                    || loc["file"].as_str().unwrap_or("").ends_with(right))
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
            "loop_push_square_a.js",
            "function loopPushSquareLeft(xs, out) {\n  for (const x of xs) {\n    out.push(x * x);\n  }\n  audit(xs);\n}\n",
        ),
        (
            "loop_push_square_b.js",
            "function loopPushSquareRight(ys, dst) {\n  for (const y of ys) {\n    dst.push(y * y);\n  }\n  trace(ys);\n}\n",
        ),
        (
            "loop_push_square_mutated.js",
            "function loopPushSquareMutated(zs, out) {\n  out.push(0);\n  for (const z of zs) {\n    out.push(z * z);\n  }\n  audit(zs);\n}\n",
        ),
        (
            "loop_push_square_wrong_receiver.js",
            "function loopPushSquareWrongReceiver(xs, out) {\n  for (const x of xs) {\n    xs.push(x * x);\n  }\n  audit(xs);\n}\n",
        ),
        (
            "loop_push_product_a.js",
            "function loopPushProductLeft(xs, out) {\n  for (const x of xs) {\n    out.push((x + 1) * 2);\n  }\n  audit(xs);\n}\n",
        ),
        (
            "loop_push_product_b.js",
            "function loopPushProductRight(ys, dst) {\n  for (const y of ys) {\n    dst.push(2 * (1 + y));\n  }\n  trace(ys);\n}\n",
        ),
        (
            "loop_push_product_neg.js",
            "function loopPushProductWrong(zs, out) {\n  for (const z of zs) {\n    out.push((z + 2) * 2);\n  }\n  audit(zs);\n}\n",
        ),
        (
            "loop_push_guard_a.js",
            "function loopPushGuardLeft(xs, out) {\n  for (const x of xs) {\n    if (x > 0) out.push(x + 1);\n  }\n  audit(xs);\n}\n",
        ),
        (
            "loop_push_guard_b.js",
            "function loopPushGuardRight(ys, dst) {\n  for (const y of ys) {\n    if (0 < y) dst.push(1 + y);\n  }\n  trace(ys);\n}\n",
        ),
        (
            "loop_push_guard_neg.js",
            "function loopPushGuardWrong(zs, out) {\n  for (const z of zs) {\n    if (z >= 0) out.push(z + 1);\n  }\n  audit(zs);\n}\n",
        ),
        (
            "loop_temp_push_square_a.js",
            "function loopTempPushSquareLeft(xs, out) {\n  for (const x of xs) {\n    const squared = x * x;\n    out.push(squared);\n  }\n  audit(xs);\n}\n",
        ),
        (
            "loop_temp_push_square_b.js",
            "function loopTempPushSquareRight(ys, dst) {\n  for (const y of ys) {\n    const result = y * y;\n    dst.push(result);\n  }\n  trace(ys);\n}\n",
        ),
        (
            "loop_temp_push_square_wrong.js",
            "function loopTempPushSquareWrong(zs, out) {\n  for (const z of zs) {\n    const squared = z + z;\n    out.push(squared);\n  }\n  audit(zs);\n}\n",
        ),
        (
            "loop_temp_append_py_a.py",
            "def loop_temp_append_left(xs, out):\n    for x in xs:\n        value = x + 1\n        out.append(value)\n    audit(xs)\n",
        ),
        (
            "loop_temp_append_py_b.py",
            "def loop_temp_append_right(ys, dst):\n    for y in ys:\n        item = 1 + y\n        dst.append(item)\n    trace(ys)\n",
        ),
        (
            "loop_temp_append_py_wrong.py",
            "def loop_temp_append_wrong(zs, out):\n    for z in zs:\n        value = z + 2\n        out.append(value)\n    audit(zs)\n",
        ),
        (
            "loop_temp_chain_push_a.js",
            "function loopTempChainPushLeft(xs, out) {\n  for (const x of xs) {\n    const shifted = x + 1;\n    const squared = shifted * shifted;\n    out.push(squared);\n  }\n  audit(xs);\n}\n",
        ),
        (
            "loop_temp_chain_push_b.js",
            "function loopTempChainPushRight(ys, dst) {\n  for (const y of ys) {\n    const offset = 1 + y;\n    const result = offset * offset;\n    dst.push(result);\n  }\n  trace(ys);\n}\n",
        ),
        (
            "loop_temp_chain_push_wrong.js",
            "function loopTempChainPushWrong(zs, out) {\n  for (const z of zs) {\n    const shifted = z + 2;\n    const squared = shifted * shifted;\n    out.push(squared);\n  }\n  audit(zs);\n}\n",
        ),
        (
            "loop_temp_chain_append_py_a.py",
            "def loop_temp_chain_append_left(xs, out):\n    for x in xs:\n        shifted = x + 1\n        value = shifted * shifted\n        out.append(value)\n    audit(xs)\n",
        ),
        (
            "loop_temp_chain_append_py_b.py",
            "def loop_temp_chain_append_right(ys, dst):\n    for y in ys:\n        offset = 1 + y\n        item = offset * offset\n        dst.append(item)\n    trace(ys)\n",
        ),
        (
            "loop_temp_chain_append_py_wrong.py",
            "def loop_temp_chain_append_wrong(zs, out):\n    for z in zs:\n        shifted = z + 1\n        value = shifted + shifted\n        out.append(value)\n    audit(zs)\n",
        ),
        (
            "loop_temp_chain_unconsumed.js",
            "function loopTempChainUnconsumed(xs, out) {\n  for (const x of xs) {\n    const shifted = x + 1;\n    const squared = x * x;\n    out.push(squared);\n  }\n  audit(xs);\n}\n",
        ),
        (
            "loop_temp_chain_uses_prior.js",
            "function loopTempChainUsesPrior(xs, out) {\n  for (const x of xs) {\n    const shifted = x + 1;\n    const squared = shifted * shifted;\n    out.push(squared + shifted);\n  }\n  audit(xs);\n}\n",
        ),
        (
            "loop_temp_unused.js",
            "function loopTempUnused(xs, out) {\n  for (const x of xs) {\n    const constant = 1;\n    out.push(constant);\n  }\n  audit(xs);\n}\n",
        ),
        (
            "loop_temp_rebind_iter.js",
            "function loopTempRebindIter(xs, out) {\n  for (let x of xs) {\n    x = x + 1;\n    out.push(x);\n  }\n  audit(xs);\n}\n",
        ),
        (
            "loop_unused_effect.js",
            "function loopUnusedEffect(xs, out) {\n  for (const unused of xs) {\n    out.push(1);\n  }\n  audit(xs);\n}\n",
        ),
        (
            "direct_unused_effect.js",
            "function directUnusedEffect(out) {\n  out.push(1);\n  audit(out);\n}\n",
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
            "foreach append effect that ignores the iteration value must not merge with a single effect: {left}/{right}: {out}"
        );
    };

    assert_loop_family(
        "loop_push_square_a.js",
        "loop_push_square_b.js",
        "loop_push_square_mutated.js",
        2,
        4,
    );
    assert_no_pair(
        "loop_push_square_a.js",
        "loop_push_square_wrong_receiver.js",
    );
    assert_loop_family(
        "loop_push_product_a.js",
        "loop_push_product_b.js",
        "loop_push_product_neg.js",
        2,
        4,
    );
    assert_loop_family(
        "loop_push_guard_a.js",
        "loop_push_guard_b.js",
        "loop_push_guard_neg.js",
        2,
        4,
    );
    assert_loop_family(
        "loop_temp_push_square_a.js",
        "loop_temp_push_square_b.js",
        "loop_temp_push_square_wrong.js",
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
        "loop_temp_chain_push_a.js",
        "loop_temp_chain_push_b.js",
        "loop_temp_chain_push_wrong.js",
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
    assert_no_pair("loop_unused_effect.js", "direct_unused_effect.js");
    assert_no_pair("loop_temp_unused.js", "direct_unused_effect.js");
    assert_no_pair("loop_temp_append_py_a.py", "loop_temp_rebind_iter.js");
    assert_no_pair("loop_temp_chain_unconsumed.js", "loop_temp_chain_push_b.js");
    assert_no_pair("loop_temp_chain_uses_prior.js", "loop_temp_chain_push_b.js");
    let _ = fs::remove_dir_all(&dir);
}

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
    let dir = std::env::temp_dir().join(format!(
        "nose_exact_conditional_loop_effect_fragments_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    let fixtures = [
        (
            "cond_loop_square_a.js",
            "function condLoopSquareLeft(enabled, xs, out) {\n  if (enabled) {\n    for (const x of xs) {\n      out.push(x * x);\n    }\n  }\n  audit(enabled);\n}\n",
        ),
        (
            "cond_loop_square_b.js",
            "function condLoopSquareRight(flag, ys, dst) {\n  if (flag) {\n    for (const y of ys) {\n      dst.push(y * y);\n    }\n  }\n  trace(flag);\n}\n",
        ),
        (
            "cond_loop_square_wrong_guard.js",
            "function condLoopSquareWrongGuard(flag, ys, dst) {\n  if (!flag) {\n    for (const y of ys) {\n      dst.push(y * y);\n    }\n  }\n  trace(flag);\n}\n",
        ),
        (
            "cond_loop_else_a.js",
            "function condLoopElseLeft(skip, xs, out) {\n  if (skip) {\n  } else {\n    for (const x of xs) {\n      out.push(x + 1);\n    }\n  }\n  audit(skip);\n}\n",
        ),
        (
            "cond_loop_else_b.js",
            "function condLoopElseRight(flag, ys, dst) {\n  if (flag) {\n  } else {\n    for (const y of ys) {\n      dst.push(1 + y);\n    }\n  }\n  trace(flag);\n}\n",
        ),
        (
            "cond_loop_else_wrong_receiver.js",
            "function condLoopElseWrongReceiver(flag, ys, dst) {\n  if (flag) {\n  } else {\n    for (const y of ys) {\n      ys.push(1 + y);\n    }\n  }\n  trace(flag);\n}\n",
        ),
        (
            "cond_loop_guard_a.js",
            "function condLoopGuardLeft(enabled, xs, out) {\n  if (enabled) {\n    for (const x of xs) {\n      if (x > 0) out.push(x + 1);\n    }\n  }\n  audit(enabled);\n}\n",
        ),
        (
            "cond_loop_guard_b.js",
            "function condLoopGuardRight(flag, ys, dst) {\n  if (flag) {\n    for (const y of ys) {\n      if (0 < y) dst.push(1 + y);\n    }\n  }\n  trace(flag);\n}\n",
        ),
        (
            "cond_loop_guard_wrong_value.js",
            "function condLoopGuardWrongValue(flag, ys, dst) {\n  if (flag) {\n    for (const y of ys) {\n      if (0 < y) dst.push(2 + y);\n    }\n  }\n  trace(flag);\n}\n",
        ),
        (
            "cond_loop_mutated.js",
            "function condLoopMutated(enabled, xs, out) {\n  out.push(0);\n  if (enabled) {\n    for (const x of xs) {\n      out.push(x * x);\n    }\n  }\n  audit(enabled);\n}\n",
        ),
        (
            "cond_loop_unused.js",
            "function condLoopUnused(enabled, xs, out) {\n  if (enabled) {\n    for (const unused of xs) {\n      out.push(1);\n    }\n  }\n  audit(enabled);\n}\n",
        ),
        (
            "cond_direct_unused.js",
            "function condDirectUnused(enabled, out) {\n  if (enabled) {\n    out.push(1);\n  }\n  audit(enabled);\n}\n",
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

    let assert_conditional_loop_family = |left: &str,
                                          right: &str,
                                          negative: &str,
                                          start_line: u64,
                                          end_line: u64| {
        let family = families
                .iter()
                .find(|family| {
                    let locations = family["locations"].as_array().expect("locations");
                    let fragment_files: Vec<&str> = locations
                        .iter()
                        .filter(|loc| {
                            loc["start_line"] == start_line && loc["end_line"] == end_line
                        })
                        .filter_map(|loc| loc["file"].as_str())
                        .collect();
                    fragment_files.iter().any(|file| file.ends_with(left))
                        && fragment_files.iter().any(|file| file.ends_with(right))
                        && locations.iter().all(|loc| loc["kind"] == "Block")
                        && locations
                            .iter()
                            .filter_map(|loc| loc["file"].as_str())
                            .all(|file| !file.ends_with(negative))
                })
                .unwrap_or_else(|| {
                    panic!(
                        "missing exact conditional foreach append-effect fragment family {left}/{right}: {out}"
                    )
                });
        assert!(
            family["locations"]
                .as_array()
                .expect("locations")
                .iter()
                .all(|loc| loc["kind"] == "Block"),
            "conditional foreach append-effect fragments should report as Block units: {family:?}"
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
            "conditional foreach append effect boundary must not merge {left}/{right}: {out}"
        );
    };

    assert_conditional_loop_family(
        "cond_loop_square_a.js",
        "cond_loop_square_b.js",
        "cond_loop_square_wrong_guard.js",
        2,
        6,
    );
    assert_no_pair("cond_loop_square_a.js", "cond_loop_mutated.js");
    assert_conditional_loop_family(
        "cond_loop_else_a.js",
        "cond_loop_else_b.js",
        "cond_loop_else_wrong_receiver.js",
        2,
        7,
    );
    assert_conditional_loop_family(
        "cond_loop_guard_a.js",
        "cond_loop_guard_b.js",
        "cond_loop_guard_wrong_value.js",
        2,
        6,
    );
    assert_no_pair("cond_loop_unused.js", "cond_direct_unused.js");
    let _ = fs::remove_dir_all(&dir);
}

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
            "cond_two_loops_a.js",
            "function condTwoLoopsLeft(enabled, xs, ys, out) {\n  if (enabled) {\n    for (const x of xs) {\n      out.push(x + 1);\n    }\n    for (const y of ys) {\n      out.push(y * y);\n    }\n  }\n  audit(enabled);\n}\n",
        ),
        (
            "cond_two_loops_b.js",
            "function condTwoLoopsRight(flag, as, bs, dst) {\n  if (flag) {\n    for (const a of as) {\n      dst.push(1 + a);\n    }\n    for (const b of bs) {\n      dst.push(b * b);\n    }\n  }\n  trace(flag);\n}\n",
        ),
        (
            "cond_two_loops_wrong_order.js",
            "function condTwoLoopsWrongOrder(flag, xs, ys, out) {\n  if (flag) {\n    for (const y of ys) {\n      out.push(y * y);\n    }\n    for (const x of xs) {\n      out.push(x + 1);\n    }\n  }\n  audit(flag);\n}\n",
        ),
        (
            "cond_two_loops_wrong_receiver.js",
            "function condTwoLoopsWrongReceiver(flag, xs, ys, out) {\n  if (flag) {\n    for (const x of xs) {\n      out.push(x + 1);\n    }\n    for (const y of ys) {\n      ys.push(y * y);\n    }\n  }\n  audit(flag);\n}\n",
        ),
        (
            "cond_two_loops_mutated.js",
            "function condTwoLoopsMutated(flag, xs, ys, out) {\n  out.push(0);\n  if (flag) {\n    for (const x of xs) {\n      out.push(x + 1);\n    }\n    for (const y of ys) {\n      out.push(y * y);\n    }\n  }\n  audit(flag);\n}\n",
        ),
        (
            "cond_three_loops.js",
            "function condThreeLoops(flag, xs, ys, zs, out) {\n  if (flag) {\n    for (const x of xs) {\n      out.push(x + 1);\n    }\n    for (const y of ys) {\n      out.push(y * y);\n    }\n    for (const z of zs) {\n      out.push(z + 3);\n    }\n  }\n  audit(flag);\n}\n",
        ),
        (
            "cond_two_temp_loops_a.py",
            "def cond_two_temp_loops_left(flag, xs, ys, out):\n    if flag:\n        for x in xs:\n            value = x + 1\n            out.append(value * value)\n        for y in ys:\n            out.append(y + 2)\n    audit(flag)\n",
        ),
        (
            "cond_two_temp_loops_b.py",
            "def cond_two_temp_loops_right(enabled, as_, bs, dst):\n    if enabled:\n        for a in as_:\n            item = 1 + a\n            dst.append(item * item)\n        for b in bs:\n            dst.append(2 + b)\n    trace(enabled)\n",
        ),
        (
            "cond_two_temp_loops_wrong.py",
            "def cond_two_temp_loops_wrong(flag, xs, ys, out):\n    if flag:\n        for x in xs:\n            value = x + 2\n            out.append(value * value)\n        for y in ys:\n            out.append(y + 2)\n    audit(flag)\n",
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
            let family = families
                .iter()
                .find(|family| {
                    let locations = family["locations"].as_array().expect("locations");
                    let branch_files: Vec<&str> = locations
                        .iter()
                        .filter(|loc| {
                            loc["start_line"] == start_line && loc["end_line"] == end_line
                        })
                        .filter_map(|loc| loc["file"].as_str())
                        .collect();
                    branch_files.iter().any(|file| file.ends_with(left))
                        && branch_files.iter().any(|file| file.ends_with(right))
                        && locations.iter().all(|loc| loc["kind"] == "Block")
                        && locations
                            .iter()
                            .filter_map(|loc| loc["file"].as_str())
                            .all(|file| !file.ends_with(negative))
                })
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
        let has_branch_pair = families.iter().any(|family| {
            let locations = family["locations"].as_array().expect("locations");
            let has_left = locations.iter().any(|loc| {
                loc["file"]
                    .as_str()
                    .is_some_and(|file| file.ends_with(left))
                    && loc["start_line"] == left_start
                    && loc["end_line"] == left_end
            });
            let has_right = locations.iter().any(|loc| {
                loc["file"]
                    .as_str()
                    .is_some_and(|file| file.ends_with(right))
                    && loc["start_line"] == right_start
                    && loc["end_line"] == right_end
            });
            has_left && has_right
        });
        assert!(
            !has_branch_pair,
            "ordered foreach-effect branch boundary must not merge {left}/{right}: {out}"
        );
    };

    assert_branch_pair(
        "cond_two_loops_a.js",
        "cond_two_loops_b.js",
        "cond_two_loops_wrong_order.js",
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
        "cond_two_loops_a.js",
        "cond_two_loops_wrong_order.js",
        2,
        9,
        2,
        9,
    );
    assert_no_branch_pair(
        "cond_two_loops_a.js",
        "cond_two_loops_wrong_receiver.js",
        2,
        9,
        2,
        9,
    );
    assert_no_branch_pair(
        "cond_two_loops_a.js",
        "cond_two_loops_mutated.js",
        2,
        9,
        3,
        10,
    );
    assert_no_branch_pair("cond_two_loops_a.js", "cond_three_loops.js", 2, 9, 2, 12);
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
            "mixed_loop_append_a.js",
            "function mixedLoopAppendLeft(enabled, xs, out, seed) {\n  if (enabled) {\n    for (const x of xs) {\n      out.push(x + 1);\n    }\n    out.push(seed * seed);\n  }\n  audit(enabled);\n}\n",
        ),
        (
            "mixed_loop_append_b.js",
            "function mixedLoopAppendRight(flag, ys, dst, base) {\n  if (flag) {\n    for (const y of ys) {\n      dst.push(1 + y);\n    }\n    dst.push(base * base);\n  }\n  trace(flag);\n}\n",
        ),
        (
            "mixed_loop_append_wrong_order.js",
            "function mixedLoopAppendWrongOrder(flag, ys, dst, base) {\n  if (flag) {\n    dst.push(base * base);\n    for (const y of ys) {\n      dst.push(1 + y);\n    }\n  }\n  trace(flag);\n}\n",
        ),
        (
            "mixed_loop_append_wrong_receiver.js",
            "function mixedLoopAppendWrongReceiver(flag, ys, dst, base) {\n  if (flag) {\n    for (const y of ys) {\n      dst.push(1 + y);\n    }\n    ys.push(base * base);\n  }\n  trace(flag);\n}\n",
        ),
        (
            "mixed_loop_append_mutated.js",
            "function mixedLoopAppendMutated(flag, ys, dst, base) {\n  dst.push(0);\n  if (flag) {\n    for (const y of ys) {\n      dst.push(1 + y);\n    }\n    dst.push(base * base);\n  }\n  trace(flag);\n}\n",
        ),
        (
            "mixed_loop_append_third_effect.js",
            "function mixedLoopAppendThirdEffect(flag, ys, dst, base) {\n  if (flag) {\n    for (const y of ys) {\n      dst.push(1 + y);\n    }\n    dst.push(base * base);\n    dst.push(base + 1);\n  }\n  trace(flag);\n}\n",
        ),
        (
            "mixed_append_loop_a.py",
            "def mixed_append_loop_left(flag, xs, out, seed):\n    if flag:\n        out.append(seed + 1)\n        for x in xs:\n            value = x + 1\n            out.append(value)\n    audit(flag)\n",
        ),
        (
            "mixed_append_loop_b.py",
            "def mixed_append_loop_right(enabled, ys, dst, base):\n    if enabled:\n        dst.append(1 + base)\n        for y in ys:\n            item = 1 + y\n            dst.append(item)\n    trace(enabled)\n",
        ),
        (
            "mixed_append_loop_wrong_temp.py",
            "def mixed_append_loop_wrong_temp(flag, ys, dst, base):\n    if flag:\n        dst.append(1 + base)\n        for y in ys:\n            item = 2 + y\n            dst.append(item)\n    trace(flag)\n",
        ),
        (
            "mixed_append_loop_wrong_order.py",
            "def mixed_append_loop_wrong_order(flag, ys, dst, base):\n    if flag:\n        for y in ys:\n            item = 1 + y\n            dst.append(item)\n        dst.append(1 + base)\n    trace(flag)\n",
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
            let family = families
                .iter()
                .find(|family| {
                    let locations = family["locations"].as_array().expect("locations");
                    let branch_files: Vec<&str> = locations
                        .iter()
                        .filter(|loc| {
                            loc["start_line"] == start_line && loc["end_line"] == end_line
                        })
                        .filter_map(|loc| loc["file"].as_str())
                        .collect();
                    branch_files.iter().any(|file| file.ends_with(left))
                        && branch_files.iter().any(|file| file.ends_with(right))
                        && locations.iter().all(|loc| loc["kind"] == "Block")
                        && locations
                            .iter()
                            .filter_map(|loc| loc["file"].as_str())
                            .all(|file| !file.ends_with(negative))
                })
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
        let has_branch_pair = families.iter().any(|family| {
            let locations = family["locations"].as_array().expect("locations");
            let has_left = locations.iter().any(|loc| {
                loc["file"]
                    .as_str()
                    .is_some_and(|file| file.ends_with(left))
                    && loc["start_line"] == left_start
                    && loc["end_line"] == left_end
            });
            let has_right = locations.iter().any(|loc| {
                loc["file"]
                    .as_str()
                    .is_some_and(|file| file.ends_with(right))
                    && loc["start_line"] == right_start
                    && loc["end_line"] == right_end
            });
            has_left && has_right
        });
        assert!(
            !has_branch_pair,
            "ordered mixed-effect branch boundary must not merge {left}/{right}: {out}"
        );
    };

    assert_branch_pair(
        "mixed_loop_append_a.js",
        "mixed_loop_append_b.js",
        "mixed_loop_append_wrong_order.js",
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
        "mixed_loop_append_a.js",
        "mixed_loop_append_wrong_order.js",
        2,
        7,
        2,
        7,
    );
    assert_no_branch_pair(
        "mixed_loop_append_a.js",
        "mixed_loop_append_wrong_receiver.js",
        2,
        7,
        2,
        7,
    );
    assert_no_branch_pair(
        "mixed_loop_append_a.js",
        "mixed_loop_append_mutated.js",
        2,
        7,
        3,
        8,
    );
    assert_no_branch_pair(
        "mixed_loop_append_a.js",
        "mixed_loop_append_third_effect.js",
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
            "cond_pair_append_a.js",
            "function condPairAppendLeft(enabled, x, y, out) {\n  if (enabled) {\n    if (x > 0) {\n      out.push(x + 1);\n    }\n    if (y > 0) {\n      out.push(y * y);\n    }\n  }\n  audit(enabled);\n}\n",
        ),
        (
            "cond_pair_append_b.js",
            "function condPairAppendRight(flag, a, b, dst) {\n  if (flag) {\n    if (0 < a) {\n      dst.push(1 + a);\n    }\n    if (b > 0) {\n      dst.push(b * b);\n    }\n  }\n  trace(flag);\n}\n",
        ),
        (
            "cond_pair_append_wrong_order.js",
            "function condPairAppendWrongOrder(flag, a, b, dst) {\n  if (flag) {\n    if (b > 0) {\n      dst.push(b * b);\n    }\n    if (0 < a) {\n      dst.push(1 + a);\n    }\n  }\n  trace(flag);\n}\n",
        ),
        (
            "cond_pair_append_wrong_receiver.js",
            "function condPairAppendWrongReceiver(flag, a, b, dst, other) {\n  if (flag) {\n    if (0 < a) {\n      dst.push(1 + a);\n    }\n    if (b > 0) {\n      other.push(b * b);\n    }\n  }\n  trace(flag);\n}\n",
        ),
        (
            "cond_pair_append_mutated.js",
            "function condPairAppendMutated(flag, a, b, dst) {\n  dst.push(0);\n  if (flag) {\n    if (0 < a) {\n      dst.push(1 + a);\n    }\n    if (b > 0) {\n      dst.push(b * b);\n    }\n  }\n  trace(flag);\n}\n",
        ),
        (
            "cond_pair_append_third.js",
            "function condPairAppendThird(flag, a, b, c, dst) {\n  if (flag) {\n    if (0 < a) {\n      dst.push(1 + a);\n    }\n    if (b > 0) {\n      dst.push(b * b);\n    }\n    if (c > 0) {\n      dst.push(c + 3);\n    }\n  }\n  trace(flag);\n}\n",
        ),
        (
            "cond_pair_append_a.py",
            "def cond_pair_append_left(flag, x, y, out):\n    if flag:\n        if x > 0:\n            out.append(x + 1)\n        if y > 0:\n            out.append(y * y)\n    audit(flag)\n",
        ),
        (
            "cond_pair_append_b.py",
            "def cond_pair_append_right(enabled, a, b, dst):\n    if enabled:\n        if 0 < a:\n            dst.append(1 + a)\n        if b > 0:\n            dst.append(b * b)\n    trace(enabled)\n",
        ),
        (
            "cond_pair_append_wrong_guard.py",
            "def cond_pair_append_wrong_guard(flag, a, b, dst):\n    if flag:\n        if 0 < a:\n            dst.append(1 + a)\n        if b >= 0:\n            dst.append(b * b)\n    trace(flag)\n",
        ),
        (
            "cond_pair_append_wrong_order.py",
            "def cond_pair_append_wrong_order(flag, a, b, dst):\n    if flag:\n        if b > 0:\n            dst.append(b * b)\n        if 0 < a:\n            dst.append(1 + a)\n    trace(flag)\n",
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
            let family = families
                .iter()
                .find(|family| {
                    let locations = family["locations"].as_array().expect("locations");
                    let branch_files: Vec<&str> = locations
                        .iter()
                        .filter(|loc| {
                            loc["start_line"] == start_line && loc["end_line"] == end_line
                        })
                        .filter_map(|loc| loc["file"].as_str())
                        .collect();
                    branch_files.iter().any(|file| file.ends_with(left))
                        && branch_files.iter().any(|file| file.ends_with(right))
                        && locations.iter().all(|loc| loc["kind"] == "Block")
                        && locations
                            .iter()
                            .filter_map(|loc| loc["file"].as_str())
                            .all(|file| !file.ends_with(negative))
                })
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
        let has_branch_pair = families.iter().any(|family| {
            let locations = family["locations"].as_array().expect("locations");
            let has_left = locations.iter().any(|loc| {
                loc["file"]
                    .as_str()
                    .is_some_and(|file| file.ends_with(left))
                    && loc["start_line"] == left_start
                    && loc["end_line"] == left_end
            });
            let has_right = locations.iter().any(|loc| {
                loc["file"]
                    .as_str()
                    .is_some_and(|file| file.ends_with(right))
                    && loc["start_line"] == right_start
                    && loc["end_line"] == right_end
            });
            has_left && has_right
        });
        assert!(
            !has_branch_pair,
            "ordered conditional-effect branch boundary must not merge {left}/{right}: {out}"
        );
    };

    assert_branch_pair(
        "cond_pair_append_a.js",
        "cond_pair_append_b.js",
        "cond_pair_append_wrong_order.js",
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
        "cond_pair_append_a.js",
        "cond_pair_append_wrong_order.js",
        2,
        9,
        2,
        9,
    );
    assert_no_branch_pair(
        "cond_pair_append_a.js",
        "cond_pair_append_wrong_receiver.js",
        2,
        9,
        2,
        9,
    );
    assert_no_branch_pair(
        "cond_pair_append_a.js",
        "cond_pair_append_mutated.js",
        2,
        9,
        3,
        10,
    );
    assert_no_branch_pair(
        "cond_pair_append_a.js",
        "cond_pair_append_third.js",
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
            "cond_mixed_append_a.js",
            "function condMixedAppendLeft(enabled, x, y, out) {\n  if (enabled) {\n    if (x > 0) {\n      out.push(x + 1);\n    }\n    out.push(y * y);\n  }\n  audit(enabled);\n}\n",
        ),
        (
            "cond_mixed_append_b.js",
            "function condMixedAppendRight(flag, a, b, dst) {\n  if (flag) {\n    if (0 < a) {\n      dst.push(1 + a);\n    }\n    dst.push(b * b);\n  }\n  trace(flag);\n}\n",
        ),
        (
            "cond_mixed_append_wrong_order.js",
            "function condMixedAppendWrongOrder(flag, a, b, dst) {\n  if (flag) {\n    dst.push(b * b);\n    if (0 < a) {\n      dst.push(1 + a);\n    }\n  }\n  trace(flag);\n}\n",
        ),
        (
            "cond_mixed_append_wrong_guard.js",
            "function condMixedAppendWrongGuard(flag, a, b, dst) {\n  if (flag) {\n    if (0 <= a) {\n      dst.push(1 + a);\n    }\n    dst.push(b * b);\n  }\n  trace(flag);\n}\n",
        ),
        (
            "cond_mixed_append_wrong_receiver.js",
            "function condMixedAppendWrongReceiver(flag, a, b, dst, other) {\n  if (flag) {\n    if (0 < a) {\n      dst.push(1 + a);\n    }\n    other.push(b * b);\n  }\n  trace(flag);\n}\n",
        ),
        (
            "cond_mixed_append_mutated.js",
            "function condMixedAppendMutated(flag, a, b, dst) {\n  dst.push(0);\n  if (flag) {\n    if (0 < a) {\n      dst.push(1 + a);\n    }\n    dst.push(b * b);\n  }\n  trace(flag);\n}\n",
        ),
        (
            "cond_mixed_append_third.js",
            "function condMixedAppendThird(flag, a, b, c, dst) {\n  if (flag) {\n    if (0 < a) {\n      dst.push(1 + a);\n    }\n    dst.push(b * b);\n    dst.push(c + 3);\n  }\n  trace(flag);\n}\n",
        ),
        (
            "cond_mixed_append_a.py",
            "def cond_mixed_append_left(flag, x, y, out):\n    if flag:\n        out.append(y * y)\n        if x > 0:\n            out.append(x + 1)\n    audit(flag)\n",
        ),
        (
            "cond_mixed_append_b.py",
            "def cond_mixed_append_right(enabled, a, b, dst):\n    if enabled:\n        dst.append(b * b)\n        if 0 < a:\n            dst.append(1 + a)\n    trace(enabled)\n",
        ),
        (
            "cond_mixed_append_wrong_order.py",
            "def cond_mixed_append_wrong_order(flag, a, b, dst):\n    if flag:\n        if 0 < a:\n            dst.append(1 + a)\n        dst.append(b * b)\n    trace(flag)\n",
        ),
        (
            "cond_mixed_append_wrong_guard.py",
            "def cond_mixed_append_wrong_guard(flag, a, b, dst):\n    if flag:\n        dst.append(b * b)\n        if 0 <= a:\n            dst.append(1 + a)\n    trace(flag)\n",
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
        let family = families
            .iter()
            .find(|family| {
                let locations = family["locations"].as_array().expect("locations");
                let branch_files: Vec<&str> = locations
                    .iter()
                    .filter(|loc| loc["start_line"] == start_line && loc["end_line"] == end_line)
                    .filter_map(|loc| loc["file"].as_str())
                    .collect();
                branch_files.iter().any(|file| file.ends_with(left))
                    && branch_files.iter().any(|file| file.ends_with(right))
                    && locations.iter().all(|loc| loc["kind"] == "Block")
                    && locations
                        .iter()
                        .filter_map(|loc| loc["file"].as_str())
                        .all(|file| !file.ends_with(negative))
            })
            .unwrap_or_else(|| {
                panic!(
                    "missing ordered conditional mixed-effect branch family {left}/{right}: {out}"
                )
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
        let has_branch_pair = families.iter().any(|family| {
            let locations = family["locations"].as_array().expect("locations");
            let has_left = locations.iter().any(|loc| {
                loc["file"]
                    .as_str()
                    .is_some_and(|file| file.ends_with(left))
                    && loc["start_line"] == left_start
                    && loc["end_line"] == left_end
            });
            let has_right = locations.iter().any(|loc| {
                loc["file"]
                    .as_str()
                    .is_some_and(|file| file.ends_with(right))
                    && loc["start_line"] == right_start
                    && loc["end_line"] == right_end
            });
            has_left && has_right
        });
        assert!(
            !has_branch_pair,
            "ordered conditional mixed-effect branch boundary must not merge {left}/{right}: {out}"
        );
    };

    assert_branch_pair(
        "cond_mixed_append_a.js",
        "cond_mixed_append_b.js",
        "cond_mixed_append_wrong_order.js",
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
        "cond_mixed_append_a.js",
        "cond_mixed_append_wrong_order.js",
        2,
        7,
        2,
        7,
    );
    assert_no_branch_pair(
        "cond_mixed_append_a.js",
        "cond_mixed_append_wrong_guard.js",
        2,
        7,
        2,
        7,
    );
    assert_no_branch_pair(
        "cond_mixed_append_a.js",
        "cond_mixed_append_wrong_receiver.js",
        2,
        7,
        2,
        7,
    );
    assert_no_branch_pair(
        "cond_mixed_append_a.js",
        "cond_mixed_append_mutated.js",
        2,
        7,
        3,
        8,
    );
    assert_no_branch_pair(
        "cond_mixed_append_a.js",
        "cond_mixed_append_third.js",
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
            "append_pair_a.js",
            "function appendPairLeft(flag, out, x) {\n  if (flag) {\n    out.push(x + 1);\n    out.push(x + 2);\n  }\n  audit(/opaque/);\n}\n",
        ),
        (
            "append_pair_b.js",
            "function appendPairRight(enabled, dst, y) {\n  if (enabled) {\n    dst.push(1 + y);\n    dst.push(2 + y);\n  }\n  trace(/opaque/);\n}\n",
        ),
        (
            "append_pair_wrong_order.js",
            "function appendPairWrongOrder(flag, out, x) {\n  if (flag) {\n    out.push(x + 2);\n    out.push(x + 1);\n  }\n  audit(/opaque/);\n}\n",
        ),
        (
            "append_pair_wrong_receiver.js",
            "function appendPairWrongReceiver(flag, out, other, x) {\n  if (flag) {\n    out.push(x + 1);\n    other.push(x + 2);\n  }\n  audit(/opaque/);\n}\n",
        ),
        (
            "append_pair_mutated.js",
            "function appendPairMutated(flag, out, x) {\n  out.push(0);\n  if (flag) {\n    out.push(x + 1);\n    out.push(x + 2);\n  }\n  audit(/opaque/);\n}\n",
        ),
        (
            "append_temp_pair_a.js",
            "function appendTempPairLeft(flag, out, x) {\n  if (flag) {\n    const first = x + 1;\n    out.push(first);\n    out.push(x + 2);\n  }\n  audit(/opaque/);\n}\n",
        ),
        (
            "append_temp_pair_b.js",
            "function appendTempPairRight(enabled, dst, y) {\n  if (enabled) {\n    dst.push(1 + y);\n    dst.push(2 + y);\n  }\n  trace(/opaque/);\n}\n",
        ),
        (
            "append_temp_pair_wrong.js",
            "function appendTempPairWrong(flag, out, x) {\n  if (flag) {\n    const first = x + 3;\n    out.push(first);\n    out.push(x + 2);\n  }\n  audit(/opaque/);\n}\n",
        ),
        (
            "append_chain_pair_a.js",
            "function appendChainPairLeft(flag, out, x) {\n  if (flag) {\n    const base = x + 1;\n    const first = base * base;\n    out.push(first);\n    out.push(x + 2);\n  }\n  audit(/opaque/);\n}\n",
        ),
        (
            "append_chain_pair_b.js",
            "function appendChainPairRight(enabled, dst, y) {\n  if (enabled) {\n    dst.push((1 + y) * (1 + y));\n    dst.push(2 + y);\n  }\n  trace(/opaque/);\n}\n",
        ),
        (
            "append_chain_pair_wrong.js",
            "function appendChainPairWrong(flag, out, x) {\n  if (flag) {\n    const base = x + 1;\n    const first = base + base;\n    out.push(first);\n    out.push(x + 2);\n  }\n  audit(/opaque/);\n}\n",
        ),
        (
            "append_chain_pair_uses_prior.js",
            "function appendChainPairUsesPrior(flag, out, x) {\n  if (flag) {\n    const base = x + 1;\n    const first = base * base;\n    out.push(first + base);\n    out.push(x + 2);\n  }\n  audit(/opaque/);\n}\n",
        ),
        (
            "append_chain_pair_forward_ref.js",
            "function appendChainPairForwardRef(flag, out, x) {\n  if (flag) {\n    const base = first + 1;\n    const first = x * x;\n    out.push(first);\n    out.push(x + 2);\n  }\n  audit(/opaque/);\n}\n",
        ),
        (
            "append_cond_before.js",
            "function appendCondBefore(flag, out, x) {\n  if (flag) {\n    out.push(x + 1);\n  }\n  out.push(x + 2);\n}\n",
        ),
        (
            "append_cond_after.js",
            "function appendCondAfter(flag, out, x) {\n  out.push(x + 2);\n  if (flag) {\n    out.push(x + 1);\n  }\n}\n",
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
        "append_pair_a.js",
        "append_pair_b.js",
        "append_pair_wrong_order.js",
    );
    assert_block_pair(
        "append_temp_pair_a.js",
        "append_temp_pair_b.js",
        "append_temp_pair_wrong.js",
    );
    assert_block_pair(
        "append_chain_pair_a.js",
        "append_chain_pair_b.js",
        "append_chain_pair_wrong.js",
    );
    assert_no_merge("append_pair_a.js", "append_pair_wrong_order.js", None);
    assert_no_merge("append_pair_a.js", "append_pair_wrong_receiver.js", None);
    assert_no_merge("append_pair_a.js", "append_pair_mutated.js", None);
    assert_no_merge(
        "append_chain_pair_a.js",
        "append_chain_pair_uses_prior.js",
        None,
    );
    assert_no_merge(
        "append_chain_pair_a.js",
        "append_chain_pair_forward_ref.js",
        None,
    );
    assert_no_merge(
        "append_cond_before.js",
        "append_cond_after.js",
        Some("Function"),
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn semantic_scan_reports_exact_safe_three_append_effect_branch_fragments() {
    let dir = std::env::temp_dir().join(format!(
        "nose_three_append_effect_boundary_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    let fixtures = [
        (
            "append_three_a.js",
            "function appendThreeLeft(flag, out, x) {\n  if (flag) {\n    out.push(x + 1);\n    out.push(x + 2);\n    out.push(x + 3);\n  }\n  audit(/opaque/);\n}\n",
        ),
        (
            "append_three_b.js",
            "function appendThreeRight(enabled, dst, y) {\n  if (enabled) {\n    dst.push(1 + y);\n    dst.push(2 + y);\n    dst.push(3 + y);\n  }\n  trace(/opaque/);\n}\n",
        ),
        (
            "append_three_wrong_order.js",
            "function appendThreeWrongOrder(flag, out, x) {\n  if (flag) {\n    out.push(x + 1);\n    out.push(x + 3);\n    out.push(x + 2);\n  }\n  audit(/opaque/);\n}\n",
        ),
        (
            "append_three_wrong_receiver.js",
            "function appendThreeWrongReceiver(flag, out, other, x) {\n  if (flag) {\n    out.push(x + 1);\n    out.push(x + 2);\n    other.push(x + 3);\n  }\n  audit(/opaque/);\n}\n",
        ),
        (
            "append_three_mutated.js",
            "function appendThreeMutated(flag, out, x) {\n  out.push(0);\n  if (flag) {\n    out.push(x + 1);\n    out.push(x + 2);\n    out.push(x + 3);\n  }\n  audit(/opaque/);\n}\n",
        ),
        (
            "append_three_temp_a.js",
            "function appendThreeTempLeft(flag, out, x) {\n  if (flag) {\n    const first = x + 1;\n    out.push(first * first);\n    out.push(x + 2);\n    out.push(x + 3);\n  }\n  audit(/opaque/);\n}\n",
        ),
        (
            "append_three_temp_b.js",
            "function appendThreeTempRight(enabled, dst, y) {\n  if (enabled) {\n    dst.push((1 + y) * (1 + y));\n    dst.push(2 + y);\n    dst.push(3 + y);\n  }\n  trace(/opaque/);\n}\n",
        ),
        (
            "append_three_temp_wrong.js",
            "function appendThreeTempWrong(flag, out, x) {\n  if (flag) {\n    const first = x + 4;\n    out.push(first * first);\n    out.push(x + 2);\n    out.push(x + 3);\n  }\n  audit(/opaque/);\n}\n",
        ),
        (
            "append_three_chain_a.js",
            "function appendThreeChainLeft(flag, out, x) {\n  if (flag) {\n    const base = x + 1;\n    const first = base * base;\n    out.push(first);\n    out.push(x + 2);\n    out.push(x + 3);\n  }\n  audit(/opaque/);\n}\n",
        ),
        (
            "append_three_chain_b.js",
            "function appendThreeChainRight(enabled, dst, y) {\n  if (enabled) {\n    dst.push((1 + y) * (1 + y));\n    dst.push(2 + y);\n    dst.push(3 + y);\n  }\n  trace(/opaque/);\n}\n",
        ),
        (
            "append_three_chain_wrong.js",
            "function appendThreeChainWrong(flag, out, x) {\n  if (flag) {\n    const base = x + 1;\n    const first = base + base;\n    out.push(first);\n    out.push(x + 2);\n    out.push(x + 3);\n  }\n  audit(/opaque/);\n}\n",
        ),
        (
            "append_three_chain_uses_prior.js",
            "function appendThreeChainUsesPrior(flag, out, x) {\n  if (flag) {\n    const base = x + 1;\n    const first = base * base;\n    out.push(first + base);\n    out.push(x + 2);\n    out.push(x + 3);\n  }\n  audit(/opaque/);\n}\n",
        ),
        (
            "append_four_a.js",
            "function appendFourLeft(flag, out, x) {\n  if (flag) {\n    out.push(x + 1);\n    out.push(x + 2);\n    out.push(x + 3);\n    out.push(x + 4);\n  }\n  audit(/opaque/);\n}\n",
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
                panic!("missing three-append-effect branch family {left}/{right}: {out}")
            });
        assert!(
            family["locations"]
                .as_array()
                .expect("locations")
                .iter()
                .all(|loc| loc["kind"] == "Block"),
            "three append-effect branch fragments should report as Block units: {family:?}"
        );
    };

    let assert_no_merge = |left: &str, right: &str| {
        let merged = families.iter().any(|family| {
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
            !merged,
            "semantic mode must not merge three append effects across the boundary ({left}/{right}): {out}"
        );
    };

    assert_block_pair(
        "append_three_a.js",
        "append_three_b.js",
        "append_three_wrong_order.js",
    );
    assert_block_pair(
        "append_three_temp_a.js",
        "append_three_temp_b.js",
        "append_three_temp_wrong.js",
    );
    assert_block_pair(
        "append_three_chain_a.js",
        "append_three_chain_b.js",
        "append_three_chain_wrong.js",
    );
    assert_no_merge("append_three_a.js", "append_three_wrong_order.js");
    assert_no_merge("append_three_a.js", "append_three_wrong_receiver.js");
    assert_no_merge("append_three_a.js", "append_three_mutated.js");
    assert_no_merge(
        "append_three_chain_a.js",
        "append_three_chain_uses_prior.js",
    );
    assert_no_merge("append_three_a.js", "append_four_a.js");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn semantic_scan_reports_exact_safe_ordered_index_assignment_branch_fragments_for_go() {
    let dir = std::env::temp_dir().join(format!(
        "nose_index_effect_order_boundary_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

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
                panic!("missing ordered index-assignment branch family {left}/{right}: {out}")
            });
        assert!(
            family["locations"]
                .as_array()
                .expect("locations")
                .iter()
                .all(|loc| loc["kind"] == "Block"),
            "ordered index-assignment branch fragments should report as Block units: {family:?}"
        );
    };

    let assert_no_merge = |left: &str, right: &str| {
        let merged = families.iter().any(|family| {
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
            !merged,
            "semantic mode must not merge ordered index-assignment effects across the boundary ({left}/{right}): {out}"
        );
    };

    assert_block_pair(
        "index_pair_a.go",
        "index_pair_b.go",
        "index_pair_wrong_order.go",
    );
    assert_block_pair(
        "index_temp_pair_a.go",
        "index_temp_pair_b.go",
        "index_temp_pair_wrong.go",
    );
    assert_block_pair(
        "index_chain_pair_a.go",
        "index_chain_pair_b.go",
        "index_chain_pair_wrong.go",
    );
    assert_no_merge("index_pair_a.go", "index_pair_wrong_order.go");
    assert_no_merge("index_pair_a.go", "index_pair_wrong_receiver.go");
    assert_no_merge("index_pair_a.go", "index_pair_mutated.go");
    assert_no_merge("index_chain_pair_a.go", "index_chain_pair_uses_prior.go");
    assert_no_merge("index_pair_a.go", "index_pair_dynamic_js.js");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn semantic_scan_reports_exact_safe_three_index_assignment_branch_fragments_for_go() {
    let dir = std::env::temp_dir().join(format!(
        "nose_three_index_effect_boundary_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

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
                panic!("missing three-index-assignment branch family {left}/{right}: {out}")
            });
        assert!(
            family["locations"]
                .as_array()
                .expect("locations")
                .iter()
                .all(|loc| loc["kind"] == "Block"),
            "three index-assignment branch fragments should report as Block units: {family:?}"
        );
    };

    let assert_no_merge = |left: &str, right: &str| {
        let merged = families.iter().any(|family| {
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
            !merged,
            "semantic mode must not merge three index-assignment effects across the boundary ({left}/{right}): {out}"
        );
    };

    assert_block_pair(
        "index_three_a.go",
        "index_three_b.go",
        "index_three_wrong_order.go",
    );
    assert_block_pair(
        "index_three_temp_a.go",
        "index_three_temp_b.go",
        "index_three_temp_wrong.go",
    );
    assert_block_pair(
        "index_three_chain_a.go",
        "index_three_chain_b.go",
        "index_three_chain_wrong.go",
    );
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
    let dir = std::env::temp_dir().join(format!(
        "nose_exact_index_assign_fragments_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

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

    let assert_assignment_family = |left: &str, right: &str, negative: &str| {
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
                panic!("missing exact index-assignment fragment family {left}/{right}: {out}")
            });
        let locations = family["locations"].as_array().expect("locations");
        assert!(
            locations
                .iter()
                .filter(|loc| loc["file"].as_str().unwrap_or("").ends_with(left)
                    || loc["file"].as_str().unwrap_or("").ends_with(right))
                .all(|loc| loc["start_line"] == loc["end_line"]
                    || loc["end_line"].as_u64().unwrap_or(0)
                        <= loc["start_line"].as_u64().unwrap_or(0) + 3),
            "index-assignment fragments should stay tightly scoped: {family:?}"
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
    let dir = std::env::temp_dir().join(format!(
        "nose_exact_this_field_assign_fragments_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

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

    let assert_fragment_family = |left: &str, right: &str, negative: &str| {
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
                panic!("missing exact this-field fragment family {left}/{right}: {out}")
            });
        let locations = family["locations"].as_array().expect("locations");
        assert!(
            locations
                .iter()
                .filter(|loc| loc["file"].as_str().unwrap_or("").ends_with(left)
                    || loc["file"].as_str().unwrap_or("").ends_with(right))
                .all(|loc| loc["end_line"].as_u64().unwrap_or(0)
                    <= loc["start_line"].as_u64().unwrap_or(0) + 5),
            "this-field fragments should stay tightly scoped: {family:?}"
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
fn semantic_scan_reports_exact_safe_java_this_field_assignment_body_fragments() {
    let dir = std::env::temp_dir().join(format!(
        "nose_exact_this_field_body_fragments_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

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

    let assert_body_family = |left: &str, right: &str, negative: &str| {
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
                    && locations.iter().any(|loc| {
                        loc["end_line"].as_u64().unwrap_or(0)
                            > loc["start_line"].as_u64().unwrap_or(0) + 1
                    })
                    && files.iter().all(|file| !file.ends_with(negative))
            })
            .unwrap_or_else(|| {
                panic!("missing exact this-field body fragment family {left}/{right}: {out}")
            });
        let locations = family["locations"].as_array().expect("locations");
        assert!(
            locations
                .iter()
                .filter(|loc| loc["file"].as_str().unwrap_or("").ends_with(left)
                    || loc["file"].as_str().unwrap_or("").ends_with(right))
                .all(|loc| loc["end_line"].as_u64().unwrap_or(0)
                    <= loc["start_line"].as_u64().unwrap_or(0) + 7),
            "this-field body fragments should stay tightly scoped: {family:?}"
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
    let dir = std::env::temp_dir().join(format!(
        "nose_exact_this_field_return_this_body_fragments_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

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

    let assert_body_family = |left: &str, right: &str, negative: &str| {
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
                    && locations.iter().any(|loc| {
                        loc["end_line"].as_u64().unwrap_or(0)
                            > loc["start_line"].as_u64().unwrap_or(0) + 1
                    })
                    && files.iter().all(|file| !file.ends_with(negative))
            })
            .unwrap_or_else(|| {
                panic!(
                    "missing exact this-field return-this body fragment family {left}/{right}: {out}"
                )
            });
        let locations = family["locations"].as_array().expect("locations");
        assert!(
            locations
                .iter()
                .filter(|loc| loc["file"].as_str().unwrap_or("").ends_with(left)
                    || loc["file"].as_str().unwrap_or("").ends_with(right))
                .all(|loc| loc["end_line"].as_u64().unwrap_or(0)
                    <= loc["start_line"].as_u64().unwrap_or(0) + 9),
            "this-field return-this body fragments should stay tightly scoped: {family:?}"
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

#[test]
fn scan_mode_semantic_rejects_unproved_regex_predicate_matches() {
    let dir = std::env::temp_dir().join(format!("nose_regex_semantic_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("dot-only.ts"),
        "export function isDotOnlyPathSegment(value: string) {\n    return /^\\.+$/u.test(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("markdown-link.ts"),
        "export function isWorkspaceMarkdownLink(markdown: string) {\n    return /^\\[\\[[^\\]]+\\]\\]$/u.test(markdown);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("dot-only-copy.ts"),
        "export function matchesDotOnly(segment: string) {\n    return /^\\.+$/u.test(segment);\n}\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    assert_eq!(
        semantic_families.len(),
        1,
        "semantic mode should report only the same-regex exact family: {semantic}"
    );
    let semantic_text = semantic_json.to_string();
    assert!(
        semantic_text.contains("dot-only.ts")
            && semantic_text.contains("dot-only-copy.ts")
            && !semantic_text.contains("markdown-link.ts"),
        "semantic mode must distinguish regex pattern semantics: {semantic}"
    );

    let near = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "near:0.5",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    assert!(
        near.contains("dot-only.ts") && near.contains("markdown-link.ts"),
        "near mode may still surface the review candidate: {near}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_allows_proved_js_static_builtins() {
    let dir = std::env::temp_dir().join(format!("nose_static_builtin_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("array_a.ts"),
        "export function isList(value: unknown) {\n    return Array.isArray(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_b.ts"),
        "export function acceptsArray(input: unknown) {\n    return Array.isArray(input);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("typeof_negative.ts"),
        "export function acceptsObject(value: unknown) {\n    return typeof value === \"object\";\n}\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    assert_eq!(
        semantic_families.len(),
        1,
        "semantic mode should report only the identical Array.isArray guard: {semantic}"
    );
    let semantic_text = semantic_json.to_string();
    assert!(
        semantic_text.contains("array_a.ts")
            && semantic_text.contains("array_b.ts")
            && !semantic_text.contains("typeof_negative.ts"),
        "semantic mode must keep static builtin calls exact: {semantic}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_proves_extreme_type4_idioms() {
    let dir = std::env::temp_dir().join(format!("nose_extreme_type4_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("record.ts"),
        "export function recordA(value: unknown) { return value !== null && typeof value === 'object' && Array.isArray(value) === false; }\n\
         export function recordB(input: unknown) { return typeof input === 'object' && input !== null && !Array.isArray(input); }\n\
         export function recordMissingArray(value: unknown) { return typeof value === 'object' && value !== null; }\n",
    )
    .unwrap();
    fs::write(
        dir.join("early.ts"),
        "export function anyLoop(xs: number[]) { let found = false; for (const x of xs) { if (x > 0) { found = true; break; } } return found; }\n\
         export function anySome(xs: number[]) { return xs.some(x => x > 0); }\n\
         export function anyWrongPredicate(xs: number[]) { let found = false; for (const x of xs) { if (x < 0) { found = true; break; } } return found; }\n",
    )
    .unwrap();
    fs::write(
        dir.join("membership.ts"),
        "export function colorOr(value: string) { return value === 'red' || value === 'blue'; }\n\
         export function colorIncludes(value: string) { return ['blue', 'red'].includes(value); }\n\
         export function colorWrongLiteral(value: string) { return value === 'red' || value === 'green'; }\n",
    )
    .unwrap();
    fs::write(
        dir.join("builder.py"),
        concat!(
            "def concat_loop(xs):\n",
            "    out = \"\"\n",
            "    for x in xs:\n",
            "        out += x\n",
            "    return out\n\n",
            "def concat_join(xs):\n",
            "    return \"\".join(xs)\n\n",
            "def concat_prepend(xs):\n",
            "    out = \"\"\n",
            "    for x in xs:\n",
            "        out = x + out\n",
            "    return out\n",
        ),
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    let family = |positives: &[&str], negatives: &[&str]| {
        semantic_families
            .iter()
            .map(serde_json::Value::to_string)
            .find(|text| {
                positives.iter().all(|name| text.contains(name))
                    && negatives.iter().all(|name| !text.contains(name))
            })
            .unwrap_or_else(|| {
                panic!(
                    "semantic mode should report {positives:?} without {negatives:?}: {semantic}"
                )
            })
    };

    family(&["recordA", "recordB"], &["recordMissingArray"]);
    family(&["anyLoop", "anySome"], &["anyWrongPredicate"]);
    family(&["colorOr", "colorIncludes"], &["colorWrongLiteral"]);
    family(&["concat_loop", "concat_join"], &["concat_prepend"]);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_proves_collection_empty_checks() {
    let dir = std::env::temp_dir().join(format!("nose_collection_empty_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("rust_len.rs"),
        "pub fn empty_len(items: &[i32], other: &[i32]) -> bool {\n    items.len() == 0\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("rust_named.rs"),
        "pub fn empty_named(values: &[i32], other: &[i32]) -> bool {\n    values.is_empty()\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("rust_threshold_negative.rs"),
        "pub fn one_item(items: &[i32], other: &[i32]) -> bool {\n    items.len() == 1\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("rust_receiver_negative.rs"),
        "pub fn other_empty(items: &[i32], other: &[i32]) -> bool {\n    other.is_empty()\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("java_size.java"),
        "class JavaSize { static boolean emptySize(java.util.List<Integer> items, java.util.List<Integer> other) { return items.size() == 0; } }\n",
    )
    .unwrap();
    fs::write(
        dir.join("java_named.java"),
        "class JavaNamed { static boolean emptyNamed(java.util.List<Integer> values, java.util.List<Integer> other) { return values.isEmpty(); } }\n",
    )
    .unwrap();
    fs::write(
        dir.join("ruby_length.rb"),
        "def empty_length(items, other)\n  items.length == 0\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("ruby_named.rb"),
        "def empty_named(values, other)\n  values.empty?\nend\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    assert!(
        !semantic_families.is_empty(),
        "semantic mode should report collection emptiness families: {semantic}"
    );
    let semantic_text = semantic_json.to_string();
    assert!(
        semantic_text.contains("rust_len.rs")
            && semantic_text.contains("rust_named.rs")
            && semantic_text.contains("java_size.java")
            && semantic_text.contains("java_named.java")
            && semantic_text.contains("ruby_length.rb")
            && semantic_text.contains("ruby_named.rb")
            && !semantic_text.contains("rust_threshold_negative.rs")
            && !semantic_text.contains("rust_receiver_negative.rs"),
        "semantic mode must prove collection-empty checks without merging boundaries: {semantic}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_proves_string_prefix_checks() {
    let dir = std::env::temp_dir().join(format!("nose_string_prefix_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("prefix.py"),
        "def prefix(value, other):\n    return value.startswith(\"pre\")\n",
    )
    .unwrap();
    fs::write(
        dir.join("prefix.js"),
        "function prefix(value, other) {\n  return value.startsWith(\"pre\");\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("prefix.ts"),
        "function prefix(value: string, other: string): boolean {\n  return value.startsWith(\"pre\");\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("prefix.go"),
        "package p\n\nimport \"strings\"\n\nfunc Prefix(value string, other string) bool {\n    return strings.HasPrefix(value, \"pre\")\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("prefix.rs"),
        "pub fn prefix(value: &str, other: &str) -> bool {\n    value.starts_with(\"pre\")\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("prefix.java"),
        "class Prefix { static boolean prefix(String value, String other) { return value.startsWith(\"pre\"); } }\n",
    )
    .unwrap();
    fs::write(
        dir.join("prefix.rb"),
        "def prefix(value, other)\n  value.start_with?(\"pre\")\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("affix_negative.py"),
        "def prefix_alt(value, other):\n    return value.startswith(\"alt\")\n",
    )
    .unwrap();
    fs::write(
        dir.join("direction_negative.js"),
        "function suffix(value, other) {\n  return value.endsWith(\"pre\");\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("receiver_negative.rs"),
        "pub fn prefix_other(value: &str, other: &str) -> bool {\n    other.starts_with(\"pre\")\n}\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    assert!(
        !semantic_families.is_empty(),
        "semantic mode should report string prefix families: {semantic}"
    );
    let semantic_text = semantic_json.to_string();
    for expected in [
        "prefix.py",
        "prefix.js",
        "prefix.ts",
        "prefix.go",
        "prefix.rs",
        "prefix.java",
        "prefix.rb",
    ] {
        assert!(
            semantic_text.contains(expected),
            "semantic mode should include {expected}: {semantic}"
        );
    }
    for unexpected in [
        "affix_negative.py",
        "direction_negative.js",
        "receiver_negative.rs",
    ] {
        assert!(
            !semantic_text.contains(unexpected),
            "semantic mode must preserve string prefix boundaries: {semantic}"
        );
    }

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_proves_rust_numeric_methods() {
    let dir =
        std::env::temp_dir().join(format!("nose_rust_numeric_methods_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("abs_conditional.py"),
        "def magnitude(value, other):\n    magnitude = value if value >= 0 else -value\n    return magnitude + other\n",
    )
    .unwrap();
    fs::write(
        dir.join("abs_method.rs"),
        "pub fn magnitude(value: i64, other: i64) -> i64 {\n    let magnitude = value.abs();\n    magnitude + other\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("min_conditional.py"),
        "def select(left, right, other):\n    selected = left if left <= right else right\n    return selected + other\n",
    )
    .unwrap();
    fs::write(
        dir.join("min_method.rs"),
        "pub fn select(left: i64, right: i64, other: i64) -> i64 {\n    let selected = left.min(right);\n    selected + other\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("max_conditional.py"),
        "def select(left, right, other):\n    selected = left if left >= right else right\n    return selected + other\n",
    )
    .unwrap();
    fs::write(
        dir.join("max_method.rs"),
        "pub fn select(left: i64, right: i64, other: i64) -> i64 {\n    let selected = left.max(right);\n    selected + other\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("abs_custom_negative.rs"),
        "struct Wrap(i64);\nimpl Wrap { fn abs(&self) -> i64 { 0 } }\npub fn magnitude(value: Wrap) -> i64 {\n    let magnitude = value.abs();\n    magnitude + 1\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("min_custom_negative.rs"),
        "struct Wrap(i64);\nimpl Wrap { fn min(&self, _right: i64) -> i64 { 0 } }\npub fn select(left: Wrap, right: i64, other: i64) -> i64 {\n    let selected = left.min(right);\n    selected + other\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("max_custom_negative.rs"),
        "struct Wrap(i64);\nimpl Wrap { fn max(&self, _right: i64) -> i64 { 0 } }\npub fn select(left: Wrap, right: i64, other: i64) -> i64 {\n    let selected = left.max(right);\n    selected + other\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("min_wrong_value.rs"),
        "pub fn select(left: i64, right: i64, other: i64) -> i64 {\n    let selected = left.min(other);\n    selected + other\n}\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    for expected_pair in [
        ["abs_conditional.py", "abs_method.rs"],
        ["min_conditional.py", "min_method.rs"],
        ["max_conditional.py", "max_method.rs"],
    ] {
        let family = semantic_families
            .iter()
            .find(|family| {
                let text = family.to_string();
                expected_pair.iter().all(|expected| text.contains(expected))
            })
            .unwrap_or_else(|| {
                panic!("semantic mode should report numeric method family: {semantic}")
            });
        let text = family.to_string();
        for unexpected in [
            "abs_custom_negative.rs",
            "min_custom_negative.rs",
            "max_custom_negative.rs",
            "min_wrong_value.rs",
        ] {
            assert!(
                !text.contains(unexpected),
                "semantic mode must preserve Rust numeric method boundaries: {semantic}"
            );
        }
    }

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_proves_literal_collection_membership() {
    let dir = std::env::temp_dir().join(format!("nose_literal_membership_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("membership.py"),
        "def membership(value, other):\n    return value in [\"red\", \"blue\"]\n",
    )
    .unwrap();
    fs::write(
        dir.join("python_set_factory.py"),
        "def python_set_factory(value, other):\n    return set([\"red\", \"blue\"]).__contains__(value)\n",
    )
    .unwrap();
    fs::write(
        dir.join("python_tuple_factory.py"),
        "def python_tuple_factory(value, other):\n    return tuple([\"red\", \"blue\"]).__contains__(value)\n",
    )
    .unwrap();
    fs::write(
        dir.join("python_frozenset_factory.py"),
        "def python_frozenset_factory(value, other):\n    return frozenset([\"red\", \"blue\"]).__contains__(value)\n",
    )
    .unwrap();
    fs::write(
        dir.join("python_deque_import.py"),
        "from collections import deque\n\n\ndef python_deque_import(value, other):\n    return deque([\"red\", \"blue\"]).__contains__(value)\n",
    )
    .unwrap();
    fs::write(
        dir.join("python_deque_alias.py"),
        "from collections import deque as Values\n\n\ndef python_deque_alias(value, other):\n    return Values([\"red\", \"blue\"]).__contains__(value)\n",
    )
    .unwrap();
    fs::write(
        dir.join("python_deque_namespace.py"),
        "import collections\n\n\ndef python_deque_namespace(value, other):\n    return collections.deque([\"red\", \"blue\"]).__contains__(value)\n",
    )
    .unwrap();
    fs::write(
        dir.join("python_module_tuple.py"),
        "VALUES = (\"red\", \"blue\")\n\n\ndef python_module_tuple(value, other):\n    return value in VALUES\n",
    )
    .unwrap();
    fs::write(
        dir.join("python_module_set.py"),
        "VALUES = {\"red\", \"blue\"}\n\n\ndef python_module_set(value, other):\n    return value in VALUES\n",
    )
    .unwrap();
    fs::write(
        dir.join("membership.js"),
        "function membership(value, other) {\n  return [\"red\", \"blue\"].includes(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("membership.ts"),
        "function membership(value: string, other: string): boolean {\n  return [\"red\", \"blue\"].includes(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("membership.go"),
        "package p\n\nimport \"slices\"\n\nfunc Membership(value string, other string) bool {\n    return slices.Contains([]string{\"red\", \"blue\"}, value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("membership.rs"),
        "pub fn membership(value: &str, other: &str) -> bool {\n    [\"red\", \"blue\"].contains(value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("membership.rb"),
        "def membership(value, other)\n  [\"red\", \"blue\"].include?(value)\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("ruby_member.rb"),
        "def ruby_member(value, other)\n  [\"red\", \"blue\"].member?(value)\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("ruby_set_new_include.rb"),
        "require \"set\"\n\ndef ruby_set_new_include(value, other)\n  Set.new([\"red\", \"blue\"]).include?(value)\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("ruby_set_new_member.rb"),
        "require \"set\"\n\ndef ruby_set_new_member(value, other)\n  Set.new([\"red\", \"blue\"]).member?(value)\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("ruby_set_local.rb"),
        "require \"set\"\n\ndef ruby_set_local(value, other)\n  values = Set.new([\"red\", \"blue\"])\n  values.include?(value)\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("module_set.js"),
        "const VALUES = new Set([\"red\", \"blue\"]);\n\nfunction moduleSet(value, other) {\n  return VALUES.has(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("module_set.ts"),
        "const VALUES = new Set<string>([\"red\", \"blue\"]);\n\nfunction moduleSet(value: string, other: string): boolean {\n  return VALUES.has(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_some.js"),
        "function arraySome(value, other) {\n  return [\"red\", \"blue\"].some((item) => item === value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_some.ts"),
        "function arraySome(value: string, other: string): boolean {\n  return [\"red\", \"blue\"].some((item: string) => item === value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_indexof.js"),
        "function arrayIndexOf(value, other) {\n  return [\"red\", \"blue\"].indexOf(value) !== -1;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_indexof.ts"),
        "function arrayIndexOf(value: string, other: string): boolean {\n  return [\"red\", \"blue\"].indexOf(value) >= 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_findindex.js"),
        "function arrayFindIndex(value, other) {\n  return [\"red\", \"blue\"].findIndex((item) => item === value) !== -1;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_findindex.ts"),
        "function arrayFindIndex(value: string, other: string): boolean {\n  return [\"red\", \"blue\"].findIndex((item: string) => item === value) >= 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_filter_length.js"),
        "function arrayFilterLength(value, other) {\n  return [\"red\", \"blue\"].filter((item) => item === value).length !== 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_filter_length.ts"),
        "function arrayFilterLength(value: string, other: string): boolean {\n  return [\"red\", \"blue\"].filter((item: string) => item === value).length >= 1;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("not_membership.py"),
        "def not_membership(value, other):\n    return value not in [\"red\", \"blue\"]\n",
    )
    .unwrap();
    fs::write(
        dir.join("not_includes.js"),
        "function notIncludes(value, other) {\n  return ![\"red\", \"blue\"].includes(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_every.js"),
        "function arrayEvery(value, other) {\n  return [\"red\", \"blue\"].every((item) => item !== value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_every.ts"),
        "function arrayEvery(value: string, other: string): boolean {\n  return [\"red\", \"blue\"].every((item: string) => item !== value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_filter_length_absence.js"),
        "function arrayFilterLengthAbsence(value, other) {\n  return [\"red\", \"blue\"].filter((item) => item === value).length === 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_filter_length_absence.ts"),
        "function arrayFilterLengthAbsence(value: string, other: string): boolean {\n  return [\"red\", \"blue\"].filter((item: string) => item === value).length <= 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("module_list.java"),
        "import java.util.List;\n\nclass ModuleList {\n    static final List<String> VALUES = List.of(\"red\", \"blue\");\n\n    static boolean moduleList(String value, String other) {\n        return VALUES.contains(value);\n    }\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("go_slices_package.go"),
        "package p\n\nimport \"slices\"\n\nvar values = []string{\"red\", \"blue\"}\n\nfunc SlicesPackage(value string, other string) bool {\n    return slices.Contains(values, value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("go_slices_alias.go"),
        "package p\n\nimport sl \"slices\"\n\nvar values = []string{\"red\", \"blue\"}\n\nfunc SlicesAlias(value string, other string) bool {\n    return sl.Contains(values, value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("go_slices_const.go"),
        "package p\n\nimport \"slices\"\n\nconst first = \"red\"\nvar values = []string{first, \"blue\"}\n\nfunc SlicesConst(value string, other string) bool {\n    return slices.Contains(values, value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("go_slices_local.go"),
        "package p\n\nimport \"slices\"\n\nfunc SlicesLocal(value string, other string) bool {\n    values := []string{\"red\", \"blue\"}\n    return slices.Contains(values, value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("java_local_list.java"),
        "import java.util.List;\n\nclass JavaLocalList {\n    static boolean javaLocalList(String value, String other) {\n        var values = List.of(\"red\", \"blue\");\n        return values.contains(value);\n    }\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("rust_local_array.rs"),
        "pub fn rust_local_array(value: &str, other: &str) -> bool {\n    let values = [\"red\", \"blue\"];\n    values.contains(&value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("rust_local_typed_array.rs"),
        "pub fn rust_local_typed_array(value: &str, other: &str) -> bool {\n    let values: [&str; 2] = [\"red\", \"blue\"];\n    values.contains(&value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("rust_local_slice_ref.rs"),
        "pub fn rust_local_slice_ref(value: &str, other: &str) -> bool {\n    let values: &[&str] = &[\"red\", \"blue\"];\n    values.contains(&value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("rust_local_vec.rs"),
        "pub fn rust_local_vec(value: &str, other: &str) -> bool {\n    let values = vec![\"red\", \"blue\"];\n    values.contains(&value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("rust_std_hashset.rs"),
        "pub fn rust_std_hashset(value: &str, other: &str) -> bool {\n    let values = std::collections::HashSet::from([\"red\", \"blue\"]);\n    values.contains(&value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("rust_std_btreeset.rs"),
        "pub fn rust_std_btreeset(value: &str, other: &str) -> bool {\n    let values = std::collections::BTreeSet::from([\"red\", \"blue\"]);\n    values.contains(&value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("rust_std_vecdeque.rs"),
        "pub fn rust_std_vecdeque(value: &str, other: &str) -> bool {\n    let values = std::collections::VecDeque::from([\"red\", \"blue\"]);\n    values.contains(&value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_element.py"),
        "def wrong_element(value, other):\n    return other in [\"red\", \"blue\"]\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_collection.js"),
        "function wrongCollection(value, other) {\n  return [\"green\", \"blue\"].includes(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_some_wrong_element.js"),
        "function arraySomeWrongElement(value, other, third) {\n  return [\"red\", \"blue\"].some((item) => item === third);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_some_wrong_collection.ts"),
        "function arraySomeWrongCollection(value: string, other: string): boolean {\n  return [\"purple\", \"orange\"].some((item: string) => item === value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_indexof_wrong_element.js"),
        "function arrayIndexOfWrongElement(value, other, third) {\n  return [\"red\", \"blue\"].indexOf(value + third) !== -1;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_indexof_wrong_collection.ts"),
        "function arrayIndexOfWrongCollection(value: string, other: string): boolean {\n  return [\"yellow\", \"orange\"].indexOf(value) >= 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_indexof_value.js"),
        "function arrayIndexOfValue(value, other) {\n  return [\"red\", \"blue\"].indexOf(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_findindex_wrong_element.js"),
        "function arrayFindIndexWrongElement(value, other, third) {\n  return [\"red\", \"blue\"].findIndex((item) => item === value + third + other) !== -1;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_findindex_wrong_collection.ts"),
        "function arrayFindIndexWrongCollection(value: string, other: string): boolean {\n  return [\"cyan\", \"magenta\"].findIndex((item: string) => item === value) >= 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_findindex_value.js"),
        "function arrayFindIndexValue(value, other) {\n  return [\"red\", \"blue\"].findIndex((item) => item === value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_filter_length_wrong_element.js"),
        "function arrayFilterLengthWrongElement(value, other, third) {\n  return [\"red\", \"blue\"].filter((item) => item === other + third).length !== 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_filter_length_wrong_collection.ts"),
        "function arrayFilterLengthWrongCollection(value: string, other: string): boolean {\n  return [\"black\", \"white\"].filter((item: string) => item === value).length >= 1;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_filter_length_value.js"),
        "function arrayFilterLengthValue(value, other) {\n  return [\"red\", \"blue\"].filter((item) => item === value).length;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_filter_length_absence_wrong_element.js"),
        "function arrayFilterLengthAbsenceWrongElement(value, other, third) {\n  return [\"red\", \"blue\"].filter((item) => item === other + third).length === 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_filter_length_absence_wrong_collection.ts"),
        "function arrayFilterLengthAbsenceWrongCollection(value: string, other: string): boolean {\n  return [\"black\", \"white\"].filter((item: string) => item === value).length <= 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_every_wrong_element.js"),
        "function arrayEveryWrongElement(value, other, third) {\n  return [\"red\", \"blue\"].every((item) => item !== third);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_every_wrong_collection.ts"),
        "function arrayEveryWrongCollection(value: string, other: string): boolean {\n  return [\"purple\", \"orange\"].every((item: string) => item !== value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("substring.rs"),
        "pub fn substring(value: &str, other: &str) -> bool {\n    value.contains(\"red\")\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("module_set_mutated.js"),
        "const VALUES = new Set([\"red\", \"blue\"]);\nVALUES.add(\"green\");\n\nfunction moduleSetMutated(value, other) {\n  return VALUES.has(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("python_module_mutated.py"),
        "VALUES = [\"red\", \"blue\"]\nVALUES.append(\"green\")\n\n\ndef python_module_mutated(value, other):\n    return value in VALUES\n",
    )
    .unwrap();
    fs::write(
        dir.join("module_set_shadowed.ts"),
        "const Set: any = function(_values: any) {\n  return { has: function() { return false; } };\n};\nconst VALUES = new Set([\"red\", \"blue\"]);\n\nfunction moduleSetShadowed(value: string, other: string): boolean {\n  return VALUES.has(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("module_list_shadowed.java"),
        "class ModuleListShadowed {\n    static final List<String> VALUES = List.of(\"red\", \"blue\");\n\n    static boolean moduleListShadowed(String value, String other) {\n        return VALUES.contains(value);\n    }\n}\n\nclass List<T> {\n    static java.util.List<String> of(String left, String right) {\n        return java.util.List.of(\"green\", right);\n    }\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("python_factory_shadowed.py"),
        "def python_factory_shadowed(value, other):\n    def set(_values):\n        class Box:\n            def __contains__(self, _value):\n                return False\n        return Box()\n    return set([\"red\", \"blue\"]).__contains__(value)\n",
    )
    .unwrap();
    fs::write(
        dir.join("python_deque_wrong_element.py"),
        "from collections import deque\n\n\ndef python_deque_wrong_element(value, other):\n    return deque([\"red\", \"blue\"]).__contains__(other)\n",
    )
    .unwrap();
    fs::write(
        dir.join("python_deque_wrong_collection.py"),
        "from collections import deque\n\n\ndef python_deque_wrong_collection(value, other):\n    return deque([\"green\", \"blue\"]).__contains__(value)\n",
    )
    .unwrap();
    fs::write(
        dir.join("python_deque_missing_import.py"),
        "def python_deque_missing_import(value, other):\n    return deque([\"red\", \"blue\"]).__contains__(value)\n",
    )
    .unwrap();
    fs::write(
        dir.join("python_deque_shadowed.py"),
        "from collections import deque\n\n\ndef deque(_values):\n    class Box:\n        def __contains__(self, _value):\n            return False\n    return Box()\n\n\ndef python_deque_shadowed(value, other):\n    return deque([\"red\", \"blue\"]).__contains__(value)\n",
    )
    .unwrap();
    fs::write(
        dir.join("python_deque_mutated.py"),
        "from collections import deque\n\n\ndef python_deque_mutated(value, other):\n    values = deque([\"red\", \"blue\"])\n    values.append(\"green\")\n    return values.__contains__(value)\n",
    )
    .unwrap();
    fs::write(
        dir.join("go_slices_mutated.go"),
        "package p\n\nimport \"slices\"\n\nvar values = append([]string{\"red\", \"blue\"}, \"green\")\n\nfunc SlicesMutated(value string, other string) bool {\n    return slices.Contains(values, value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("go_slices_local_mutated.go"),
        "package p\n\nimport \"slices\"\n\nfunc SlicesLocalMutated(value string, other string) bool {\n    values := []string{\"red\", \"blue\"}\n    values = append(values, \"green\")\n    return slices.Contains(values, value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("go_slices_unimported.go"),
        "package p\n\ntype fakeSlices struct{}\n\nfunc (fakeSlices) Contains(values []string, value string) bool {\n    return false\n}\n\nvar slices fakeSlices\nvar values = []string{\"red\", \"blue\"}\n\nfunc SlicesUnimported(value string, other string) bool {\n    return slices.Contains(values, value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("java_local_list_mutated.java"),
        "import java.util.ArrayList;\nimport java.util.List;\n\nclass JavaLocalListMutated {\n    static boolean javaLocalListMutated(String value, String other) {\n        var values = new ArrayList<String>(List.of(\"red\", \"blue\"));\n        values.add(\"green\");\n        return values.contains(value);\n    }\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("rust_local_mutated.rs"),
        "pub fn rust_local_mutated(value: &str, other: &str) -> bool {\n    let mut values = vec![\"red\", \"blue\"];\n    values.push(\"green\");\n    values.contains(&value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("rust_local_custom_receiver.rs"),
        "struct Values;\n\nimpl Values {\n    fn contains(&self, _value: &&str) -> bool {\n        false\n    }\n}\n\npub fn rust_local_custom_receiver(value: &str, other: &str) -> bool {\n    let values = Values;\n    values.contains(&value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("rust_std_wrong_element.rs"),
        "pub fn rust_std_wrong_element(value: &str, other: &str) -> bool {\n    let values = std::collections::HashSet::from([\"red\", \"blue\"]);\n    values.contains(&(value.to_owned() + other))\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("rust_std_wrong_collection.rs"),
        "pub fn rust_std_wrong_collection(value: &str, other: &str) -> bool {\n    let values = std::collections::BTreeSet::from([\"silver\", \"gold\"]);\n    values.contains(&value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("rust_std_mutated.rs"),
        "pub fn rust_std_mutated(value: &str, other: &str) -> bool {\n    let mut values = std::collections::HashSet::from([\"red\", \"blue\"]);\n    values.insert(\"green\");\n    values.contains(&value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("ruby_set_wrong_element.rb"),
        "require \"set\"\n\ndef ruby_set_wrong_element(value, other)\n  Set.new([\"red\", \"blue\"]).include?(other)\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("ruby_set_wrong_collection.rb"),
        "require \"set\"\n\ndef ruby_set_wrong_collection(value, other)\n  Set.new([\"green\", \"blue\"]).include?(value)\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("ruby_set_missing_require.rb"),
        "def ruby_set_missing_require(value, other)\n  Set.new([\"red\", \"blue\"]).include?(value)\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("ruby_set_shadowed.rb"),
        "require \"set\"\n\nclass Set\n  def self.new(_values)\n    Box.new\n  end\nend\n\nclass Box\n  def include?(_value)\n    false\n  end\nend\n\ndef ruby_set_shadowed(value, other)\n  Set.new([\"red\", \"blue\"]).include?(value)\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("ruby_set_mutated.rb"),
        "require \"set\"\n\ndef ruby_set_mutated(value, other)\n  values = Set.new([\"red\", \"blue\"])\n  values.add(\"green\")\n  values.include?(value)\nend\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    assert!(
        !semantic_families.is_empty(),
        "semantic mode should report literal membership families: {semantic}"
    );
    let semantic_text = semantic_json.to_string();
    let positive_family = semantic_families
        .iter()
        .map(serde_json::Value::to_string)
        .find(|family| {
            [
                "membership.py",
                "membership.rb",
                "python_set_factory.py",
                "python_deque_import.py",
                "python_deque_alias.py",
                "python_deque_namespace.py",
                "ruby_set_new_include.rb",
                "ruby_set_new_member.rb",
                "ruby_set_local.rb",
                "rust_std_hashset.rs",
            ]
            .iter()
            .all(|expected| family.contains(expected))
        })
        .unwrap_or_else(|| {
            panic!("semantic mode should include positive literal membership family: {semantic}")
        });
    for expected in [
        "membership.py",
        "membership.js",
        "membership.ts",
        "membership.go",
        "membership.rs",
        "membership.rb",
        "ruby_member.rb",
        "ruby_set_new_include.rb",
        "ruby_set_new_member.rb",
        "ruby_set_local.rb",
        "python_set_factory.py",
        "python_tuple_factory.py",
        "python_frozenset_factory.py",
        "python_deque_import.py",
        "python_deque_alias.py",
        "python_deque_namespace.py",
        "python_module_tuple.py",
        "python_module_set.py",
        "module_set.js",
        "module_set.ts",
        "array_some.js",
        "array_some.ts",
        "array_indexof.js",
        "array_indexof.ts",
        "array_findindex.js",
        "array_findindex.ts",
        "array_filter_length.js",
        "array_filter_length.ts",
        "not_membership.py",
        "not_includes.js",
        "array_every.js",
        "array_every.ts",
        "array_filter_length_absence.js",
        "array_filter_length_absence.ts",
        "module_list.java",
        "go_slices_package.go",
        "go_slices_alias.go",
        "go_slices_const.go",
        "go_slices_local.go",
        "java_local_list.java",
        "rust_local_array.rs",
        "rust_local_typed_array.rs",
        "rust_local_slice_ref.rs",
        "rust_local_vec.rs",
        "rust_std_hashset.rs",
        "rust_std_btreeset.rs",
        "rust_std_vecdeque.rs",
    ] {
        assert!(
            semantic_text.contains(expected),
            "semantic mode should include {expected}: {semantic}"
        );
    }
    for unexpected in [
        "wrong_element.py",
        "wrong_collection.js",
        "array_some_wrong_element.js",
        "array_some_wrong_collection.ts",
        "array_indexof_wrong_element.js",
        "array_indexof_wrong_collection.ts",
        "array_indexof_value.js",
        "array_findindex_wrong_element.js",
        "array_findindex_wrong_collection.ts",
        "array_findindex_value.js",
        "array_filter_length_wrong_element.js",
        "array_filter_length_wrong_collection.ts",
        "array_filter_length_value.js",
        "array_filter_length_absence_wrong_element.js",
        "array_filter_length_absence_wrong_collection.ts",
        "array_every_wrong_element.js",
        "array_every_wrong_collection.ts",
        "substring.rs",
        "module_set_mutated.js",
        "python_module_mutated.py",
        "module_set_shadowed.ts",
        "module_list_shadowed.java",
        "python_factory_shadowed.py",
        "python_deque_wrong_element.py",
        "python_deque_wrong_collection.py",
        "python_deque_missing_import.py",
        "python_deque_shadowed.py",
        "python_deque_mutated.py",
        "go_slices_mutated.go",
        "go_slices_local_mutated.go",
        "go_slices_unimported.go",
        "java_local_list_mutated.java",
        "rust_local_mutated.rs",
        "rust_local_custom_receiver.rs",
        "rust_std_wrong_element.rs",
        "rust_std_wrong_collection.rs",
        "rust_std_mutated.rs",
        "ruby_set_wrong_element.rb",
        "ruby_set_wrong_collection.rb",
        "ruby_set_missing_require.rb",
        "ruby_set_shadowed.rb",
        "ruby_set_mutated.rb",
    ] {
        assert!(
            !positive_family.contains(unexpected),
            "semantic mode must preserve literal membership boundaries: {semantic}"
        );
    }
    let absence_family = semantic_families
        .iter()
        .map(serde_json::Value::to_string)
        .find(|family| family.contains("not_membership.py"))
        .unwrap_or_else(|| {
            panic!("semantic mode should include negated membership family: {semantic}")
        });
    for expected in [
        "not_includes.js",
        "array_every.js",
        "array_every.ts",
        "array_filter_length_absence.js",
        "array_filter_length_absence.ts",
    ] {
        assert!(
            absence_family.contains(expected),
            "semantic mode should include negated membership {expected}: {semantic}"
        );
    }
    for unexpected in [
        "array_every_wrong_element.js",
        "array_every_wrong_collection.ts",
        "array_filter_length_absence_wrong_element.js",
        "array_filter_length_absence_wrong_collection.ts",
    ] {
        assert!(
            !absence_family.contains(unexpected),
            "semantic mode must preserve negated membership boundaries: {semantic}"
        );
    }

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_keeps_unproven_contains_calls_distinct() {
    let dir = std::env::temp_dir().join(format!("nose_unproven_contains_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("java_list.java"),
        "import java.util.List;\n\nclass C { static boolean f(List<String> values, String value) { return values.contains(value); } }\n",
    )
    .unwrap();
    fs::write(
        dir.join("java_string_negative.java"),
        "class C { static boolean f(String values, String value) { return values.contains(value); } }\n",
    )
    .unwrap();
    fs::write(
        dir.join("rust_slice.rs"),
        "pub fn f(values: &[&str], value: &str) -> bool {\n    values.contains(&value)\n}\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json: serde_json::Value =
        serde_json::from_str(&semantic).expect("semantic scan should emit JSON");
    let semantic_text = semantic_json.to_string();
    assert!(
        !semantic_text.contains("java_string_negative.java"),
        "semantic mode must not merge unproven collection membership with substring contains: {semantic}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_proves_typed_dynamic_collection_membership() {
    let dir = std::env::temp_dir().join(format!(
        "nose_typed_dynamic_membership_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("membership.py"),
        "def f(values: list[str], value: str, other: str) -> bool:\n    return value in values\n",
    )
    .unwrap();
    fs::write(
        dir.join("membership.ts"),
        "function f(values: string[], value: string, other: string): boolean {\n  return values.includes(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("membership.go"),
        "package p\n\nimport \"slices\"\n\nfunc F(values []string, value string, other string) bool {\n    return slices.Contains(values, value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("membership.rs"),
        "pub fn f(values: &[&str], value: &str, other: &str) -> bool {\n    values.contains(&value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("membership.java"),
        "import java.util.List;\n\nclass C { static boolean f(List<String> values, String value, String other) { return values.contains(value); } }\n",
    )
    .unwrap();
    fs::write(
        dir.join("membership_tuple.py"),
        "def f(values: tuple[str, ...], value: str, other: str) -> bool:\n    return value in values\n",
    )
    .unwrap();
    fs::write(
        dir.join("membership_alias_sequence.py"),
        "from typing import Sequence as Values\n\ndef f(values: Values[str], value: str, other: str, other_values: Values[str]) -> bool:\n    return value in values\n",
    )
    .unwrap();
    fs::write(
        dir.join("membership_alias_container.py"),
        "from collections.abc import Container as Values\n\ndef f(values: Values[str], value: str, other: str, other_values: Values[str]) -> bool:\n    return value in values\n",
    )
    .unwrap();
    fs::write(
        dir.join("membership_alias_set.py"),
        "from typing import Set as Values\n\ndef f(values: Values[str], value: str, other: str, other_values: Values[str]) -> bool:\n    return value in values\n",
    )
    .unwrap();
    fs::write(
        dir.join("membership_queue.java"),
        "import java.util.Queue;\n\nclass C { static boolean f(Queue<String> values, String value, String other) { return values.contains(value); } }\n",
    )
    .unwrap();
    fs::write(
        dir.join("membership_vecdeque.rs"),
        "use std::collections::VecDeque;\n\npub fn f(values: &VecDeque<&str>, value: &str, other: &str) -> bool {\n    values.contains(&value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_element.ts"),
        "function f(values: string[], value: string, other: string): boolean {\n  return values.includes(other);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("membership_alias_wrong_element.py"),
        "from typing import Sequence as Values\n\ndef f(values: Values[str], value: str, other: str, other_values: Values[str]) -> bool:\n    return other in values\n",
    )
    .unwrap();
    fs::write(
        dir.join("membership_alias_wrong_receiver.py"),
        "from typing import Sequence as Values\n\ndef f(values: Values[str], value: str, other: str, other_values: Values[str]) -> bool:\n    return value in other_values\n",
    )
    .unwrap();
    fs::write(
        dir.join("membership_alias_unresolved.py"),
        "def f(values: Values[str], value: str, other: str, other_values: Values[str]) -> bool:\n    return value in values\n",
    )
    .unwrap();
    fs::write(
        dir.join("membership_alias_shadowed.py"),
        "from typing import Sequence as Values\nValues = str\n\ndef f(values: Values[str], value: str, other: str, other_values: Values[str]) -> bool:\n    return value in values\n",
    )
    .unwrap();
    fs::write(
        dir.join("string_negative.java"),
        "class C { static boolean f(String values, String value, String other) { return values.contains(value); } }\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    let expected = [
        "membership.py",
        "membership.ts",
        "membership.go",
        "membership.rs",
        "membership.java",
        "membership_tuple.py",
        "membership_alias_sequence.py",
        "membership_alias_container.py",
        "membership_alias_set.py",
        "membership_queue.java",
        "membership_vecdeque.rs",
    ];
    let positive_family = semantic_families
        .iter()
        .find(|family| {
            let family_text = family.to_string();
            expected
                .iter()
                .all(|expected| family_text.contains(expected))
        })
        .unwrap_or_else(|| {
            panic!("semantic mode should report one typed dynamic membership family: {semantic}")
        });
    let positive_text = positive_family.to_string();
    for expected in expected {
        assert!(
            positive_text.contains(expected),
            "semantic mode should include typed dynamic membership {expected}: {semantic}"
        );
    }
    for unexpected in [
        "wrong_element.ts",
        "membership_alias_wrong_element.py",
        "membership_alias_wrong_receiver.py",
        "membership_alias_unresolved.py",
        "membership_alias_shadowed.py",
        "string_negative.java",
    ] {
        assert!(
            !positive_text.contains(unexpected),
            "semantic mode must preserve typed dynamic membership boundaries: {semantic}"
        );
    }

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_proves_set_membership_when_receiver_is_proven() {
    let dir = std::env::temp_dir().join(format!("nose_set_membership_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("literal.py"),
        "def f(value, other):\n    return value in [\"red\", \"blue\"]\n",
    )
    .unwrap();
    fs::write(
        dir.join("set_inline.js"),
        "function f(value, other) {\n  return new Set([\"red\", \"blue\"]).has(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("set_local.js"),
        "function f(value, other) {\n  const values = new Set([\"red\", \"blue\"]);\n  return values.has(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("typed_array.ts"),
        "function f(values: string[], value: string, other: string): boolean {\n  return values.includes(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("typed_set.ts"),
        "function f(values: Set<string>, value: string, other: string): boolean {\n  return values.has(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("java_list_of.java"),
        "import java.util.List;\n\nclass JavaListOf { static boolean f(String value, String other) { return List.of(\"red\", \"blue\").contains(value); } }\n",
    )
    .unwrap();
    fs::write(
        dir.join("java_set_of.java"),
        "import java.util.Set;\n\nclass JavaSetOf { static boolean f(String value, String other) { return Set.of(\"red\", \"blue\").contains(value); } }\n",
    )
    .unwrap();
    fs::write(
        dir.join("java_arrays_aslist.java"),
        "import java.util.Arrays;\n\nclass JavaArraysAsList { static boolean f(String value, String other) { return Arrays.asList(\"red\", \"blue\").contains(value); } }\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_element.js"),
        "function f(value, other) {\n  return new Set([\"red\", \"blue\"]).has(other);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_collection.js"),
        "function f(value, other) {\n  return new Set([\"green\", \"blue\"]).has(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("untyped_receiver.ts"),
        "function f(values, value, other) {\n  return values.has(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("shadowed_set.js"),
        "function f(Set, value, other) {\n  return new Set([\"red\", \"blue\"]).has(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("java_wrong_element.java"),
        "import java.util.List;\n\nclass JavaWrongElement { static boolean f(String value, String other) { return List.of(\"red\", \"blue\").contains(other); } }\n",
    )
    .unwrap();
    fs::write(
        dir.join("java_shadowed_list.java"),
        "class JavaShadowedList { static boolean f(Object List, String value, String other) { return List.of(\"red\", \"blue\").contains(value); } }\n",
    )
    .unwrap();
    fs::write(
        dir.join("java_local_list.java"),
        "class JavaLocalList { static boolean f(String value, String other) { return List.of(\"red\", \"blue\").contains(value); } }\nclass List { static Box of(String a, String b) { return new Box(); } }\nclass Box { boolean contains(String value) { return false; } }\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    let expected_positive = [
        "literal.py",
        "set_inline.js",
        "set_local.js",
        "java_list_of.java",
        "java_set_of.java",
        "java_arrays_aslist.java",
    ];
    let positive_family = semantic_families
        .iter()
        .find(|family| {
            let family_text = family.to_string();
            expected_positive
                .iter()
                .all(|expected| family_text.contains(expected))
        })
        .unwrap_or_else(|| {
            panic!("semantic mode should include proven Set membership family: {semantic}")
        });
    let positive_text = positive_family.to_string();
    let typed_family = semantic_families
        .iter()
        .find(|family| {
            let family_text = family.to_string();
            family_text.contains("typed_array.ts") && family_text.contains("typed_set.ts")
        })
        .unwrap_or_else(|| {
            panic!("semantic mode should include typed Set membership family: {semantic}")
        });
    let typed_text = typed_family.to_string();
    for unexpected in [
        "wrong_element.js",
        "wrong_collection.js",
        "untyped_receiver.ts",
        "shadowed_set.js",
        "java_wrong_element.java",
        "java_shadowed_list.java",
        "java_local_list.java",
    ] {
        assert!(
            !positive_text.contains(unexpected),
            "semantic mode must preserve Set membership boundaries: {semantic}"
        );
        assert!(
            !typed_text.contains(unexpected),
            "semantic mode must preserve typed Set membership boundaries: {semantic}"
        );
    }

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_handles_shadowed_callback_collection_name() {
    let dir = std::env::temp_dir().join(format!(
        "nose_shadowed_callback_collection_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("shadowed_callback.js"),
        r#"function clean(stdout, original) {
  const words = stdout
    ? stdout
        .split("\n")
        .filter((word, _, words) => {
          const lowerCased = word.toLowerCase();
          return lowerCased === word || !words.includes(lowerCased);
        })
        .sort((a, b) => a.toLowerCase().localeCompare(b.toLowerCase()))
    : [];
  const removed = original.filter((word) => !words.includes(word));
  return removed.length;
}
"#,
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let _: serde_json::Value =
        serde_json::from_str(&semantic).expect("semantic scan should emit JSON");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_proves_typed_typescript_map_key_membership() {
    let dir = std::env::temp_dir().join(format!("nose_typed_ts_map_key_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("map_key.py"),
        "def f(lookup, other_lookup, key, other):\n    return key in lookup\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_key.java"),
        "import java.util.Map;\n\nclass C { static boolean f(Map<String, String> lookup, Map<String, String> other_lookup, String key, String other) { return lookup.containsKey(key); } }\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_key.ts"),
        "function f(lookup: Map<string, string>, other_lookup: Map<string, string>, key: string, other: string): boolean {\n  return lookup.has(key);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_key_python_keys_in.py"),
        "def f(lookup: dict[str, str], other_lookup: dict[str, str], key: str, other: str) -> bool:\n    return key in lookup.keys()\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_key_python_keys_contains.py"),
        "def f(lookup: dict[str, str], other_lookup: dict[str, str], key: str, other: str) -> bool:\n    return lookup.keys().__contains__(key)\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_key_ts_array_from_keys.ts"),
        "function f(lookup: Map<string, string>, other_lookup: Map<string, string>, key: string, other: string): boolean {\n  return Array.from(lookup.keys()).includes(key);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_key.ts"),
        "function f(lookup: Map<string, string>, other_lookup: Map<string, string>, key: string, other: string): boolean {\n  return lookup.has(other);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("python_keys_wrong_key.py"),
        "def f(lookup: dict[str, str], other_lookup: dict[str, str], key: str, other: str) -> bool:\n    return other in lookup.keys()\n",
    )
    .unwrap();
    fs::write(
        dir.join("python_keys_wrong_map.py"),
        "def f(lookup: dict[str, str], other_lookup: dict[str, str], key: str, other: str) -> bool:\n    return key in other_lookup.keys()\n",
    )
    .unwrap();
    fs::write(
        dir.join("python_values_view.py"),
        "def f(lookup: dict[str, str], other_lookup: dict[str, str], key: str, other: str) -> bool:\n    return key in lookup.values()\n",
    )
    .unwrap();
    fs::write(
        dir.join("ts_array_from_keys_wrong_key.ts"),
        "function f(lookup: Map<string, string>, other_lookup: Map<string, string>, key: string, other: string): boolean {\n  return Array.from(lookup.keys()).includes(other);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("ts_array_from_keys_wrong_map.ts"),
        "function f(lookup: Map<string, string>, other_lookup: Map<string, string>, key: string, other: string): boolean {\n  return Array.from(other_lookup.keys()).includes(key);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("ts_array_from_values.ts"),
        "function f(lookup: Map<string, string>, other_lookup: Map<string, string>, key: string, other: string): boolean {\n  return Array.from(lookup.values()).includes(key);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("set_negative.ts"),
        "function f(lookup: Set<string>, other_lookup: Set<string>, key: string, other: string): boolean {\n  return lookup.has(key);\n}\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    let positive_family = semantic_families
        .iter()
        .map(serde_json::Value::to_string)
        .find(|family| {
            [
                "map_key.py",
                "map_key.java",
                "map_key.ts",
                "map_key_python_keys_in.py",
                "map_key_python_keys_contains.py",
                "map_key_ts_array_from_keys.ts",
            ]
            .iter()
            .all(|expected| family.contains(expected))
        })
        .unwrap_or_else(|| panic!("semantic mode should include typed map-key family: {semantic}"));
    for expected in [
        "map_key.py",
        "map_key.java",
        "map_key.ts",
        "map_key_python_keys_in.py",
        "map_key_python_keys_contains.py",
        "map_key_ts_array_from_keys.ts",
    ] {
        assert!(
            positive_family.contains(expected),
            "semantic mode should include typed TS Map.has map-key membership {expected}: {semantic}"
        );
    }
    for unexpected in [
        "wrong_key.ts",
        "set_negative.ts",
        "python_keys_wrong_key.py",
        "python_keys_wrong_map.py",
        "python_values_view.py",
        "ts_array_from_keys_wrong_key.ts",
        "ts_array_from_keys_wrong_map.ts",
        "ts_array_from_values.ts",
    ] {
        assert!(
            !positive_family.contains(unexpected),
            "semantic mode must preserve typed TS Map.has boundaries: {semantic}"
        );
    }

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_proves_typed_typescript_map_default_lookup() {
    let dir =
        std::env::temp_dir().join(format!("nose_typed_ts_map_default_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("map_default.go"),
        "package p\n\nfunc F(lookup map[string]int, otherLookup map[string]int, key string, otherKey string, fallback int, otherDefault int) int { value, ok := lookup[key]; if !ok { value = fallback }; return value }\n",
    )
    .unwrap();
    fs::write(
        dir.join("ts_nullish.ts"),
        "function f(lookup: Map<string, number>, other_lookup: Map<string, number>, key: string, other_key: string, fallback: number, other_default: number): number {\n  return lookup.get(key) ?? fallback;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("ts_has_get.ts"),
        "function f(lookup: Map<string, number>, other_lookup: Map<string, number>, key: string, other_key: string, fallback: number, other_default: number): number {\n  return lookup.has(key) ? lookup.get(key) : fallback;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("ts_temp_guard.ts"),
        "function f(lookup: Map<string, number>, other_lookup: Map<string, number>, key: string, other_key: string, fallback: number, other_default: number): number {\n  const selected = lookup.get(key);\n  return selected === undefined ? fallback : selected;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("ts_guard_return.ts"),
        "function f(lookup: Map<string, number>, other_lookup: Map<string, number>, key: string, other_key: string, fallback: number, other_default: number): number {\n  if (lookup.has(key)) {\n    return lookup.get(key)!;\n  }\n  return fallback;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("java_guard_return.java"),
        "import java.util.Map;\n\nclass C { static int f(Map<String, Integer> lookup, Map<String, Integer> other_lookup, String key, String other_key, int fallback, int other_default) { if (lookup.containsKey(key)) { return lookup.get(key); } return fallback; } }\n",
    )
    .unwrap();
    fs::write(
        dir.join("ts_wrong_key.ts"),
        "function f(lookup: Map<string, number>, other_lookup: Map<string, number>, key: string, other_key: string, fallback: number, other_default: number): number {\n  return lookup.get(other_key) ?? fallback;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("ts_wrong_default.ts"),
        "function f(lookup: Map<string, number>, other_lookup: Map<string, number>, key: string, other_key: string, fallback: number, other_default: number): number {\n  return lookup.get(key) ?? other_default;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("ts_wrong_map.ts"),
        "function f(lookup: Map<string, number>, other_lookup: Map<string, number>, key: string, other_key: string, fallback: number, other_default: number): number {\n  return other_lookup.get(key) ?? fallback;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("ts_untyped.ts"),
        "function f(lookup, other_lookup, key, other_key, fallback, other_default) {\n  return lookup.get(key) ?? fallback;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("ts_guard_wrong_key.ts"),
        "function f(lookup: Map<string, number>, other_lookup: Map<string, number>, key: string, other_key: string, fallback: number, other_default: number): number {\n  if (lookup.has(other_key)) {\n    return lookup.get(other_key)!;\n  }\n  return fallback;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("java_guard_wrong_default.java"),
        "import java.util.Map;\n\nclass C { static int f(Map<String, Integer> lookup, Map<String, Integer> other_lookup, String key, String other_key, int fallback, int other_default) { if (lookup.containsKey(key)) { return lookup.get(key); } return other_default; } }\n",
    )
    .unwrap();
    fs::write(
        dir.join("py_dict.py"),
        "def f(lookup: dict[str, int], other_lookup: dict[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return lookup.get(key, fallback)\n",
    )
    .unwrap();
    fs::write(
        dir.join("py_guard_return.py"),
        "def f(lookup: dict[str, int], other_lookup: dict[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    if key in lookup:\n        return lookup[key]\n    return fallback\n",
    )
    .unwrap();
    fs::write(
        dir.join("py_mapping.py"),
        "from collections.abc import Mapping\n\ndef f(lookup: Mapping[str, int], other_lookup: Mapping[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return lookup.get(key, fallback)\n",
    )
    .unwrap();
    fs::write(
        dir.join("py_mutable_mapping.py"),
        "from collections.abc import MutableMapping\n\ndef f(lookup: MutableMapping[str, int], other_lookup: MutableMapping[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return lookup.get(key, fallback)\n",
    )
    .unwrap();
    fs::write(
        dir.join("py_alias_mapping.py"),
        "from collections.abc import Mapping as MapLike\n\ndef f(lookup: MapLike[str, int], other_lookup: MapLike[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return lookup.get(key, fallback)\n",
    )
    .unwrap();
    fs::write(
        dir.join("py_alias_mutable_mapping.py"),
        "from collections.abc import MutableMapping as MapLike\n\ndef f(lookup: MapLike[str, int], other_lookup: MapLike[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return lookup.get(key, fallback)\n",
    )
    .unwrap();
    fs::write(
        dir.join("py_alias_dict.py"),
        "from typing import Dict as MapLike\n\ndef f(lookup: MapLike[str, int], other_lookup: MapLike[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return lookup.get(key, fallback)\n",
    )
    .unwrap();
    fs::write(
        dir.join("py_wrong_key.py"),
        "def f(lookup: dict[str, int], other_lookup: dict[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return lookup.get(other_key, fallback)\n",
    )
    .unwrap();
    fs::write(
        dir.join("py_wrong_default.py"),
        "def f(lookup: dict[str, int], other_lookup: dict[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return lookup.get(key, other_default)\n",
    )
    .unwrap();
    fs::write(
        dir.join("py_wrong_map.py"),
        "def f(lookup: dict[str, int], other_lookup: dict[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return other_lookup.get(key, fallback)\n",
    )
    .unwrap();
    fs::write(
        dir.join("py_guard_wrong_map.py"),
        "def f(lookup: dict[str, int], other_lookup: dict[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    if key in other_lookup:\n        return other_lookup[key]\n    return fallback\n",
    )
    .unwrap();
    fs::write(
        dir.join("py_untyped.py"),
        "def f(lookup, other_lookup, key, other_key, fallback, other_default):\n    return lookup.get(key, fallback)\n",
    )
    .unwrap();
    fs::write(
        dir.join("py_alias_wrong_key.py"),
        "from collections.abc import Mapping as MapLike\n\ndef f(lookup: MapLike[str, int], other_lookup: MapLike[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return lookup.get(other_key, fallback)\n",
    )
    .unwrap();
    fs::write(
        dir.join("py_alias_wrong_default.py"),
        "from collections.abc import Mapping as MapLike\n\ndef f(lookup: MapLike[str, int], other_lookup: MapLike[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return lookup.get(key, other_default)\n",
    )
    .unwrap();
    fs::write(
        dir.join("py_alias_wrong_map.py"),
        "from collections.abc import Mapping as MapLike\n\ndef f(lookup: MapLike[str, int], other_lookup: MapLike[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return other_lookup.get(key, fallback)\n",
    )
    .unwrap();
    fs::write(
        dir.join("py_alias_unresolved.py"),
        "def f(lookup: MapLike[str, int], other_lookup: MapLike[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return lookup.get(key, fallback)\n",
    )
    .unwrap();
    fs::write(
        dir.join("py_alias_shadowed.py"),
        "from collections.abc import Mapping as MapLike\nMapLike = list\n\ndef f(lookup: MapLike[str, int], other_lookup: MapLike[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return lookup.get(key, fallback)\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    let expected = [
        "map_default.go",
        "ts_nullish.ts",
        "ts_has_get.ts",
        "ts_temp_guard.ts",
        "ts_guard_return.ts",
        "java_guard_return.java",
        "py_dict.py",
        "py_guard_return.py",
        "py_mapping.py",
        "py_mutable_mapping.py",
        "py_alias_mapping.py",
        "py_alias_mutable_mapping.py",
        "py_alias_dict.py",
    ];
    let positive_family = semantic_families
        .iter()
        .find(|family| {
            let family_text = family.to_string();
            expected
                .iter()
                .all(|expected| family_text.contains(expected))
        })
        .unwrap_or_else(|| {
            panic!("semantic mode should report one typed map default family: {semantic}")
        });
    let positive_text = positive_family.to_string();
    for expected in expected {
        assert!(
            positive_text.contains(expected),
            "semantic mode should include typed map default lookup {expected}: {semantic}"
        );
    }
    for unexpected in [
        "ts_wrong_key.ts",
        "ts_wrong_default.ts",
        "ts_wrong_map.ts",
        "ts_untyped.ts",
        "ts_guard_wrong_key.ts",
        "java_guard_wrong_default.java",
        "py_wrong_key.py",
        "py_wrong_default.py",
        "py_wrong_map.py",
        "py_guard_wrong_map.py",
        "py_untyped.py",
        "py_alias_wrong_key.py",
        "py_alias_wrong_default.py",
        "py_alias_wrong_map.py",
        "py_alias_unresolved.py",
        "py_alias_shadowed.py",
    ] {
        assert!(
            !positive_text.contains(unexpected),
            "semantic mode must preserve typed map default boundaries: {semantic}"
        );
    }

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_proves_literal_map_default_lookup() {
    let dir = std::env::temp_dir().join(format!("nose_map_default_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("map_default.py"),
        "def lookup(key, other):\n    return {\"red\": 1, \"blue\": 2}.get(key, 0)\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default.rb"),
        "def lookup(key, other)\n  {\"red\" => 1, \"blue\" => 2}.fetch(key, 0)\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_block.rb"),
        "def lookup(key, other)\n  {\"red\" => 1, \"blue\" => 2}.fetch(key) { 0 }\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_inline.js"),
        "function lookup(key, other) {\n  return new Map([[\"red\", 1], [\"blue\", 2]]).get(key) ?? 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_local.js"),
        "function lookup(key, other) {\n  const values = new Map([[\"red\", 1], [\"blue\", 2]]);\n  return values.get(key) ?? 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_has_get.js"),
        "function lookup(key, other) {\n  const values = new Map([[\"red\", 1], [\"blue\", 2]]);\n  return values.has(key) ? values.get(key) : 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_inline.ts"),
        "function lookup(key: string, other: string): number {\n  return new Map<string, number>([[\"red\", 1], [\"blue\", 2]]).get(key) ?? 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_java_of.java"),
        "import java.util.Map;\n\nclass JavaMapOf {\n  static int lookup(String key, String other) {\n    return Map.of(\"red\", 1, \"blue\", 2).getOrDefault(key, 0);\n  }\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_java_entries.java"),
        "import java.util.Map;\n\nclass JavaMapEntries {\n  static int lookup(String key, String other) {\n    return Map.ofEntries(Map.entry(\"red\", 1), Map.entry(\"blue\", 2)).getOrDefault(key, 0);\n  }\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_java_local.java"),
        "import java.util.Map;\n\nclass JavaMapLocal {\n  static int lookup(String key, String other) {\n    Map<String, Integer> values = Map.of(\"red\", 1, \"blue\", 2);\n    return values.getOrDefault(key, 0);\n  }\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_module.js"),
        "const LOOKUP = new Map([[\"red\", 1], [\"blue\", 2]]);\n\nfunction lookup(key, other) {\n  return LOOKUP.get(key) ?? 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_module.ts"),
        "const LOOKUP = new Map<string, number>([[\"red\", 1], [\"blue\", 2]]);\n\nfunction lookup(key: string, other: string): number {\n  return LOOKUP.get(key) ?? 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_module.java"),
        "import java.util.Map;\n\nclass JavaModuleMap {\n  static final Map<String, Integer> LOOKUP = Map.of(\"red\", 1, \"blue\", 2);\n\n  static int lookup(String key, String other) {\n    return LOOKUP.getOrDefault(key, 0);\n  }\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_rust_hashmap.rs"),
        "pub fn lookup(key: &str, other: &str) -> i32 {\n    *std::collections::HashMap::from([(\"red\", 1), (\"blue\", 2)]).get(key).unwrap_or(&0)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_rust_btreemap.rs"),
        "pub fn lookup(key: &str, other: &str) -> i32 {\n    *std::collections::BTreeMap::from([(\"red\", 1), (\"blue\", 2)]).get(key).unwrap_or(&0)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_rust_local.rs"),
        "pub fn lookup(key: &str, other: &str) -> i32 {\n    let values = std::collections::HashMap::from([(\"red\", 1), (\"blue\", 2)]);\n    *values.get(key).unwrap_or(&0)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_go_inline.go"),
        "package p\n\nfunc Lookup(key string, other string) int {\n    return map[string]int{\"red\": 1, \"blue\": 2}[key]\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_go_local.go"),
        "package p\n\nfunc Lookup(key string, other string) int {\n    lookup := map[string]int{\"red\": 1, \"blue\": 2}\n    return lookup[key]\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_go_var.go"),
        "package p\n\nfunc Lookup(key string, other string) int {\n    var lookup = map[string]int{\"red\": 1, \"blue\": 2}\n    return lookup[key]\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_string.py"),
        "def lookup(key, other):\n    return {\"red\": \"apple\", \"blue\": \"berry\"}.get(key, \"\")\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_string.rb"),
        "def lookup(key, other)\n  {\"red\" => \"apple\", \"blue\" => \"berry\"}.fetch(key, \"\")\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_string_block.rb"),
        "def lookup(key, other)\n  {\"red\" => \"apple\", \"blue\" => \"berry\"}.fetch(key) { \"\" }\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_go_string_inline.go"),
        "package p\n\nfunc Lookup(key string, other string) string {\n    return map[string]string{\"red\": \"apple\", \"blue\": \"berry\"}[key]\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_go_string_local.go"),
        "package p\n\nfunc Lookup(key string, other string) string {\n    lookup := map[string]string{\"red\": \"apple\", \"blue\": \"berry\"}\n    return lookup[key]\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_bool.py"),
        "def lookup(key, other):\n    return {\"red\": True, \"blue\": False}.get(key, False)\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_bool.rb"),
        "def lookup(key, other)\n  {\"red\" => true, \"blue\" => false}.fetch(key, false)\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_bool_block.rb"),
        "def lookup(key, other)\n  {\"red\" => true, \"blue\" => false}.fetch(key) { false }\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_go_bool_inline.go"),
        "package p\n\nfunc Lookup(key string, other string) bool {\n    return map[string]bool{\"red\": true, \"blue\": false}[key]\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_float.py"),
        "def lookup(key, other):\n    return {\"red\": 1.5, \"blue\": 2.5}.get(key, 0.0)\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_float.rb"),
        "def lookup(key, other)\n  {\"red\" => 1.5, \"blue\" => 2.5}.fetch(key, 0.0)\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_go_float_inline.go"),
        "package p\n\nfunc Lookup(key string, other string) float64 {\n    return map[string]float64{\"red\": 1.5, \"blue\": 2.5}[key]\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_go_float_local.go"),
        "package p\n\nfunc Lookup(key string, other string) float64 {\n    lookup := map[string]float64{\"red\": 1.5, \"blue\": 2.5}\n    return lookup[key]\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_nil.py"),
        "def lookup(key, other):\n    return {\"red\": None, \"blue\": None}.get(key, None)\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_nil.rb"),
        "def lookup(key, other)\n  {\"red\" => nil, \"blue\" => nil}.fetch(key, nil)\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_nil_block.rb"),
        "def lookup(key, other)\n  {\"red\" => nil, \"blue\" => nil}.fetch(key) { nil }\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_go_nil_inline.go"),
        "package p\n\ntype Item struct{}\n\nfunc Lookup(key string, other string) *Item {\n    return map[string]*Item{\"red\": nil, \"blue\": nil}[key]\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("object_hasown.js"),
        "function lookup(key, other) {\n  const values = { \"red\": 1, \"blue\": 2 };\n  return Object.hasOwn(values, key) ? values[key] : 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("object_hasown_call.js"),
        "function lookup(key, other) {\n  const values = { \"red\": 1, \"blue\": 2 };\n  return Object.prototype.hasOwnProperty.call(values, key) ? values[key] : 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("object_negated.ts"),
        "function lookup(key: string, other: string): number {\n  const values: Record<string, number> = { \"red\": 1, \"blue\": 2 };\n  return !Object.hasOwn(values, key) ? 0 : values[key];\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_key.py"),
        "def wrong_key(key, other):\n    return {\"red\": 1, \"blue\": 2}.get(other, 0)\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_default.rb"),
        "def wrong_default(key, other)\n  {\"red\" => 1, \"blue\" => 2}.fetch(key, 9)\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_default_block.rb"),
        "def wrong_default(key, other)\n  {\"red\" => 1, \"blue\" => 2}.fetch(key) { 9 }\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_map.py"),
        "def wrong_map(key, other):\n    return {\"red\": 9, \"blue\": 2}.get(key, 0)\n",
    )
    .unwrap();
    fs::write(
        dir.join("ruby_fetch_block_param.rb"),
        "def wrong(key, other)\n  {\"red\" => 1, \"blue\" => 2}.fetch(key) { |missing| missing.to_s }\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("ruby_fetch_raise_block.rb"),
        "def wrong(key, other)\n  {\"red\" => 1, \"blue\" => 2}.fetch(key) { raise KeyError }\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_js_key.js"),
        "function wrong_key(key, other) {\n  return new Map([[\"red\", 1], [\"blue\", 2]]).get(other) ?? 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_js_default.js"),
        "function wrong_default(key, other) {\n  return new Map([[\"red\", 1], [\"blue\", 2]]).get(key) ?? 9;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_js_map.js"),
        "function wrong_map(key, other) {\n  return new Map([[\"red\", 9], [\"blue\", 2]]).get(key) ?? 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("untyped_receiver.js"),
        "function untyped_receiver(values, key, other) {\n  return values.get(key) ?? 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("shadowed_map.js"),
        "function shadowed_map(key, other, Map) {\n  return new Map([[\"red\", 1], [\"blue\", 2]]).get(key) ?? 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_java_key.java"),
        "import java.util.Map;\n\nclass WrongJavaKey {\n  static int wrong(String key, String other) {\n    return Map.of(\"red\", 1, \"blue\", 2).getOrDefault(other, 0);\n  }\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_java_default.java"),
        "import java.util.Map;\n\nclass WrongJavaDefault {\n  static int wrong(String key, String other) {\n    return Map.of(\"red\", 1, \"blue\", 2).getOrDefault(key, 9);\n  }\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_java_map.java"),
        "import java.util.Map;\n\nclass WrongJavaMap {\n  static int wrong(String key, String other) {\n    return Map.of(\"red\", 9, \"blue\", 2).getOrDefault(key, 0);\n  }\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("shadowed_java_map.java"),
        "class ShadowedJavaMap {\n  static class MapFactory {\n    java.util.Map<String, Integer> of(Object... values) { return java.util.Map.of(); }\n  }\n  static int wrong(String key, String other, MapFactory Map) {\n    return Map.of(\"red\", 1, \"blue\", 2).getOrDefault(key, 0);\n  }\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("local_java_map_type.java"),
        "class LocalJavaMapType {\n  static int wrong(String key, String other) {\n    return Map.of(\"red\", 1, \"blue\", 2).getOrDefault(key, 0);\n  }\n}\nclass Map {\n  static java.util.Map<String, Integer> of(Object... values) { return java.util.Map.of(); }\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("module_map_mutated.js"),
        "const LOOKUP = new Map([[\"red\", 1], [\"blue\", 2]]);\nLOOKUP.set(\"red\", 9);\n\nfunction wrong(key, other) {\n  return LOOKUP.get(key) ?? 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("module_map_shadowed.ts"),
        "const Map: any = function(_entries: any) {\n  return { get: function() { return 9; } };\n};\nconst LOOKUP = new Map([[\"red\", 1], [\"blue\", 2]]);\n\nfunction wrong(key: string, other: string): number {\n  return LOOKUP.get(key) ?? 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("module_map_shadowed.java"),
        "class JavaShadowedModuleMap {\n  static final Map<String, Integer> LOOKUP = Map.of(\"red\", 1, \"blue\", 2);\n\n  static int wrong(String key, String other) {\n    return LOOKUP.getOrDefault(key, 0);\n  }\n}\nclass Map {\n  static java.util.Map<String, Integer> of(Object... values) { return java.util.Map.of(); }\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_rust_key.rs"),
        "pub fn wrong(key: &str, other: &str) -> i32 {\n    *std::collections::HashMap::from([(\"red\", 1), (\"blue\", 2)]).get(other).unwrap_or(&0)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_rust_default.rs"),
        "pub fn wrong(key: &str, other: &str) -> i32 {\n    *std::collections::HashMap::from([(\"red\", 1), (\"blue\", 2)]).get(key).unwrap_or(&9)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_rust_map.rs"),
        "pub fn wrong(key: &str, other: &str) -> i32 {\n    *std::collections::HashMap::from([(\"red\", 9), (\"blue\", 2)]).get(key).unwrap_or(&0)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("rust_map_mutated.rs"),
        "pub fn wrong(key: &str, other: &str) -> i32 {\n    let mut values = std::collections::HashMap::from([(\"red\", 1), (\"blue\", 2)]);\n    values.insert(\"red\", 9);\n    *values.get(key).unwrap_or(&0)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_go_key.go"),
        "package p\n\nfunc Wrong(key string, other string) int {\n    return map[string]int{\"red\": 1, \"blue\": 2}[other]\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_go_map.go"),
        "package p\n\nfunc Wrong(key string, other string) int {\n    return map[string]int{\"red\": 9, \"blue\": 2}[key]\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("go_keyed_slice.go"),
        "package p\n\nfunc Wrong(key int, other int) int {\n    return []int{0: 1, 1: 2}[key]\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_go_string_key.go"),
        "package p\n\nfunc Wrong(key string, other string) string {\n    return map[string]string{\"red\": \"apple\", \"blue\": \"berry\"}[other]\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_go_bool_map.go"),
        "package p\n\nfunc Wrong(key string, other string) bool {\n    return map[string]bool{\"red\": false, \"blue\": false}[key]\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_go_float_key.go"),
        "package p\n\nfunc Wrong(key string, other string) float64 {\n    return map[string]float64{\"red\": 1.5, \"blue\": 2.5}[other]\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_go_nil_map.go"),
        "package p\n\nfunc Wrong(key string, other string) string {\n    return map[string]string{\"red\": \"apricot\", \"blue\": \"berry\"}[key]\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("go_mixed_value_map.go"),
        "package p\n\nfunc Wrong(key string, other string) interface{} {\n    return map[string]interface{}{\"red\": \"apple\", \"blue\": false}[key]\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("go_string_keyed_slice.go"),
        "package p\n\nfunc Wrong(key int, other int) string {\n    return []string{0: \"apple\", 1: \"berry\"}[key]\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("object_wrong_key.js"),
        "function wrong_key(key, other) {\n  const values = { \"red\": 1, \"blue\": 2 };\n  return Object.hasOwn(values, other) ? values[other] : 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("object_wrong_default.js"),
        "function wrong_default(key, other) {\n  const values = { \"red\": 1, \"blue\": 2 };\n  return Object.hasOwn(values, key) ? values[key] : 9;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("object_wrong_map.js"),
        "function wrong_map(key, other) {\n  const values = { \"red\": 9, \"blue\": 2 };\n  return Object.hasOwn(values, key) ? values[key] : 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("object_unguarded.js"),
        "function unguarded(key, other) {\n  const values = { \"red\": 1, \"blue\": 2 };\n  return values[key] ?? 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("object_in.js"),
        "function object_in(key, other) {\n  const values = { \"red\": 1, \"blue\": 2 };\n  return key in values ? values[key] : 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("object_method.js"),
        "function object_method(key, other) {\n  const values = { \"red\": 1, \"blue\": 2 };\n  return values.hasOwnProperty(key) ? values[key] : 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("object_shadowed.js"),
        "function object_shadowed(key, other, Object) {\n  const values = { \"red\": 1, \"blue\": 2 };\n  return Object.hasOwn(values, key) ? values[key] : 0;\n}\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    let expected = [
        "map_default.py",
        "map_default.rb",
        "map_default_block.rb",
        "map_default_inline.js",
        "map_default_local.js",
        "map_default_has_get.js",
        "map_default_inline.ts",
        "map_default_java_of.java",
        "map_default_java_entries.java",
        "map_default_java_local.java",
        "map_default_module.js",
        "map_default_module.ts",
        "map_default_module.java",
        "map_default_rust_hashmap.rs",
        "map_default_rust_btreemap.rs",
        "map_default_rust_local.rs",
        "map_default_go_inline.go",
        "map_default_go_local.go",
        "map_default_go_var.go",
        "object_hasown.js",
        "object_hasown_call.js",
        "object_negated.ts",
    ];
    let positive_family = semantic_families
        .iter()
        .find(|family| {
            let family_text = family.to_string();
            expected
                .iter()
                .all(|expected| family_text.contains(expected))
        })
        .unwrap_or_else(|| {
            panic!("semantic mode should report one literal map-default family: {semantic}")
        });
    let positive_text = positive_family.to_string();
    for expected in expected {
        assert!(
            positive_text.contains(expected),
            "semantic mode should include {expected}: {semantic}"
        );
    }
    let string_expected = [
        "map_default_string.py",
        "map_default_string.rb",
        "map_default_string_block.rb",
        "map_default_go_string_inline.go",
        "map_default_go_string_local.go",
    ];
    let string_family = semantic_families
        .iter()
        .find(|family| {
            let family_text = family.to_string();
            string_expected
                .iter()
                .all(|expected| family_text.contains(expected))
        })
        .unwrap_or_else(|| {
            panic!("semantic mode should report one string map-default family: {semantic}")
        });
    let string_text = string_family.to_string();
    for expected in string_expected {
        assert!(
            string_text.contains(expected),
            "semantic mode should include string map-default {expected}: {semantic}"
        );
    }

    let bool_expected = [
        "map_default_bool.py",
        "map_default_bool.rb",
        "map_default_bool_block.rb",
        "map_default_go_bool_inline.go",
    ];
    let bool_family = semantic_families
        .iter()
        .find(|family| {
            let family_text = family.to_string();
            bool_expected
                .iter()
                .all(|expected| family_text.contains(expected))
        })
        .unwrap_or_else(|| {
            panic!("semantic mode should report one bool map-default family: {semantic}")
        });
    let bool_text = bool_family.to_string();
    for expected in bool_expected {
        assert!(
            bool_text.contains(expected),
            "semantic mode should include bool map-default {expected}: {semantic}"
        );
    }

    let float_expected = [
        "map_default_float.py",
        "map_default_float.rb",
        "map_default_go_float_inline.go",
        "map_default_go_float_local.go",
    ];
    let float_family = semantic_families
        .iter()
        .find(|family| {
            let family_text = family.to_string();
            float_expected
                .iter()
                .all(|expected| family_text.contains(expected))
        })
        .unwrap_or_else(|| {
            panic!("semantic mode should report one float map-default family: {semantic}")
        });
    let float_text = float_family.to_string();
    for expected in float_expected {
        assert!(
            float_text.contains(expected),
            "semantic mode should include float map-default {expected}: {semantic}"
        );
    }

    let nil_expected = [
        "map_default_nil.py",
        "map_default_nil.rb",
        "map_default_nil_block.rb",
        "map_default_go_nil_inline.go",
    ];
    let nil_family = semantic_families
        .iter()
        .find(|family| {
            let family_text = family.to_string();
            nil_expected
                .iter()
                .all(|expected| family_text.contains(expected))
        })
        .unwrap_or_else(|| {
            panic!("semantic mode should report one nil map-default family: {semantic}")
        });
    let nil_text = nil_family.to_string();
    for expected in nil_expected {
        assert!(
            nil_text.contains(expected),
            "semantic mode should include nil map-default {expected}: {semantic}"
        );
    }

    let boundary_files = [
        "wrong_key.py",
        "wrong_default.rb",
        "wrong_default_block.rb",
        "wrong_map.py",
        "ruby_fetch_block_param.rb",
        "ruby_fetch_raise_block.rb",
        "wrong_js_key.js",
        "wrong_js_default.js",
        "wrong_js_map.js",
        "untyped_receiver.js",
        "shadowed_map.js",
        "wrong_java_key.java",
        "wrong_java_default.java",
        "wrong_java_map.java",
        "shadowed_java_map.java",
        "local_java_map_type.java",
        "module_map_mutated.js",
        "module_map_shadowed.ts",
        "module_map_shadowed.java",
        "wrong_rust_key.rs",
        "wrong_rust_default.rs",
        "wrong_rust_map.rs",
        "rust_map_mutated.rs",
        "wrong_go_key.go",
        "wrong_go_map.go",
        "go_keyed_slice.go",
        "wrong_go_string_key.go",
        "wrong_go_bool_map.go",
        "wrong_go_float_key.go",
        "wrong_go_nil_map.go",
        "go_mixed_value_map.go",
        "go_string_keyed_slice.go",
        "object_wrong_key.js",
        "object_wrong_default.js",
        "object_wrong_map.js",
        "object_unguarded.js",
        "object_in.js",
        "object_method.js",
        "object_shadowed.js",
    ];
    for family_text in [
        &positive_text,
        &string_text,
        &bool_text,
        &float_text,
        &nil_text,
    ] {
        for unexpected in &boundary_files {
            assert!(
                !family_text.contains(*unexpected),
                "semantic mode must preserve literal map-default boundaries: {semantic}"
            );
        }
    }

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_proves_null_presence_predicates() {
    let dir = std::env::temp_dir().join(format!("nose_null_presence_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("none_compare.py"),
        "def is_missing(value, other):\n    return value is None\n",
    )
    .unwrap();
    fs::write(
        dir.join("null_compare.c"),
        "#include <stddef.h>\n\nint is_missing(void *value, void *other) {\n    return value == NULL;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("nil_method.rb"),
        "def is_missing(value, other)\n  value.nil?\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("none_method.rs"),
        "pub fn is_missing(value: Option<i32>, other: Option<i32>) -> bool {\n    value.is_none()\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("iflet_none.rs"),
        "pub fn is_missing(value: Option<i32>, other: Option<i32>) -> bool {\n    if let None = value { true } else { false }\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("some_method.rs"),
        "pub fn is_present(value: Option<i32>, other: Option<i32>) -> bool {\n    value.is_some()\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_value.py"),
        "def wrong_value(value, other):\n    return other is None\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    assert_eq!(
        semantic_families.len(),
        1,
        "semantic mode should report one null-presence family: {semantic}"
    );
    let semantic_text = semantic_json.to_string();
    for expected in [
        "none_compare.py",
        "null_compare.c",
        "nil_method.rb",
        "none_method.rs",
        "iflet_none.rs",
    ] {
        assert!(
            semantic_text.contains(expected),
            "semantic mode should include {expected}: {semantic}"
        );
    }
    for unexpected in ["some_method.rs", "wrong_value.py"] {
        assert!(
            !semantic_text.contains(unexpected),
            "semantic mode must preserve null-presence boundaries: {semantic}"
        );
    }

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_reports_flattened_guard_span_only() {
    let dir = std::env::temp_dir().join(format!("nose_guard_span_semantic_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    fs::write(
        dir.join("large_branch.rb"),
        "def table(rows, errors)\n  if rows.length == 0\n    return nil\n  else\n    title = \"Potential problems\"\n    if errors.length > 0\n      title = title.red\n    else\n      title = title.yellow\n    end\n    return Terminal::Table.new(title: title, rows: rows).to_s\n  end\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("small_guard.rb"),
        "def payload(data)\n  return nil if data.empty?\n  data\nend\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let families = scan_families(&semantic_json);
    let text = semantic_json.to_string();
    assert!(
        text.contains("large_branch.rb") && text.contains("small_guard.rb"),
        "semantic mode should still report the strict guard-clause clone: {semantic}"
    );
    assert!(
        families.iter().all(|family| {
            let Some(locations) = family["locations"].as_array() else {
                return true;
            };
            locations.iter().all(|loc| {
                let Some(file) = loc["file"].as_str() else {
                    return true;
                };
                !file.ends_with("large_branch.rb")
                    || (loc["start_line"] == 2 && loc["end_line"] == 3)
            })
        }),
        "semantic mode must not report the flattened guard as the whole if/else span: {semantic}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_preserves_js_typeof_operator() {
    let dir = std::env::temp_dir().join(format!("nose_typeof_semantic_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("typeof_a.ts"),
        "export function isString(value: unknown) {\n    return typeof value === \"string\";\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("typeof_b.ts"),
        "export function acceptsString(input: unknown) {\n    return typeof input === \"string\";\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("plain_equality_negative.ts"),
        "export function equalsString(value: unknown) {\n    return value === \"string\";\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("void_a.ts"),
        "export function eraseValue(value: unknown) {\n    return void value;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("void_b.ts"),
        "export function eraseInput(input: unknown) {\n    return void input;\n}\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    assert_eq!(
        semantic_families.len(),
        1,
        "semantic mode should report only the identical typeof guard: {semantic}"
    );
    let semantic_text = semantic_json.to_string();
    assert!(
        semantic_text.contains("typeof_a.ts")
            && semantic_text.contains("typeof_b.ts")
            && !semantic_text.contains("plain_equality_negative.ts")
            && !semantic_text.contains("void_a.ts")
            && !semantic_text.contains("void_b.ts"),
        "semantic mode must preserve typeof and reject unproved unary JS operators: {semantic}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_allows_safe_uninterpreted_calls() {
    let dir = std::env::temp_dir().join(format!("nose_uninterpreted_call_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("call_a.py"),
        "def report(helper, value):\n    adjusted = value + 1\n    return helper(adjusted)\n",
    )
    .unwrap();
    fs::write(
        dir.join("call_b.py"),
        "def build(callback, input):\n    adjusted = input + 1\n    return callback(adjusted)\n",
    )
    .unwrap();
    fs::write(
        dir.join("call_negative.py"),
        "def other(value, callback):\n    adjusted = value + 1\n    return callback(adjusted)\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "3",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    assert_eq!(
        semantic_families.len(),
        1,
        "semantic mode should report only same helper-call identity: {semantic}"
    );
    let semantic_text = semantic_json.to_string();
    assert!(
        semantic_text.contains("call_a.py")
            && semantic_text.contains("call_b.py")
            && !semantic_text.contains("call_negative.py"),
        "semantic mode must preserve uninterpreted callee identity: {semantic}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_allows_safe_uninterpreted_method_calls() {
    let dir =
        std::env::temp_dir().join(format!("nose_uninterpreted_method_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("method_a.js"),
        "function normalize(value) { return value; }\nexport function assertLine(t, stderr) {\n    const line = normalize(stderr);\n    t.is(line, \"ok\");\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("method_b.js"),
        "function normalize(value) { return value; }\nexport function checkLine(assertion, output) {\n    const line = normalize(output);\n    assertion.is(line, \"ok\");\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("method_negative.js"),
        "function normalize(value) { return value; }\nexport function assertDeep(t, stderr) {\n    const line = normalize(stderr);\n    t.deepEqual(line, \"ok\");\n}\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "3",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    assert_eq!(
        semantic_families.len(),
        1,
        "semantic mode should report only same method-call identity: {semantic}"
    );
    let semantic_text = semantic_json.to_string();
    assert!(
        semantic_text.contains("method_a.js")
            && semantic_text.contains("method_b.js")
            && !semantic_text.contains("method_negative.js"),
        "semantic mode must preserve uninterpreted method identity: {semantic}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_distinguishes_sequence_kinds() {
    let dir = std::env::temp_dir().join(format!("nose_seq_semantic_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("list_a.py"),
        "def pair(a, b):\n    return [a, b]\n",
    )
    .unwrap();
    fs::write(
        dir.join("list_b.py"),
        "def make_pair(x, y):\n    return [x, y]\n",
    )
    .unwrap();
    fs::write(
        dir.join("tuple.py"),
        "def tuple_pair(a, b):\n    return (a, b)\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    assert_eq!(
        semantic_families.len(),
        1,
        "semantic mode should report only the same list-construction family: {semantic}"
    );
    let semantic_text = semantic_json.to_string();
    assert!(
        semantic_text.contains("list_a.py")
            && semantic_text.contains("list_b.py")
            && !semantic_text.contains("tuple.py"),
        "semantic mode must preserve list-vs-tuple sequence kind: {semantic}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_allows_static_import_identity() {
    let dir = std::env::temp_dir().join(format!("nose_import_identity_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("import_a.py"),
        "from shared_math import unused_helper, helper\n\ndef report(value):\n    shifted = value + 1\n    return helper(shifted)\n",
    )
    .unwrap();
    fs::write(
        dir.join("import_b.py"),
        "from shared_math import unused_helper, helper as calc\n\ndef build(input):\n    shifted = input + 1\n    return calc(shifted)\n",
    )
    .unwrap();
    fs::write(
        dir.join("import_negative.py"),
        "from shared_math import unused_helper, other_helper as calc\n\ndef other(input):\n    shifted = input + 1\n    return calc(shifted)\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    assert_eq!(
        semantic_families.len(),
        1,
        "semantic mode should report only same import coordinate: {semantic}"
    );
    let semantic_text = semantic_json.to_string();
    assert!(
        semantic_text.contains("import_a.py")
            && semantic_text.contains("import_b.py")
            && !semantic_text.contains("import_negative.py"),
        "semantic mode must preserve static import coordinates: {semantic}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_allows_named_namespace_import_identity() {
    let dir = std::env::temp_dir().join(format!(
        "nose_import_namespace_member_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("named.js"),
        "import { helper } from \"./shared-math\";\n\nfunction report(value) {\n  return helper(value + 1);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("namespace.js"),
        "import * as mathOps from \"./shared-math\";\n\nfunction build(input) {\n  return mathOps.helper(input + 1);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("typed_namespace.ts"),
        "import * as mathOps from \"./shared-math\";\n\nfunction build(input: number): number {\n  return mathOps.helper(input + 1);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_member.js"),
        "import * as mathOps from \"./shared-math\";\n\nfunction other(input) {\n  return mathOps.otherHelper(input + 1);\n}\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json: serde_json::Value =
        serde_json::from_str(&semantic).expect("semantic scan should emit JSON");
    let semantic_text = semantic_json.to_string();
    for expected in ["named.js", "namespace.js", "typed_namespace.ts"] {
        assert!(
            semantic_text.contains(expected),
            "semantic mode should include static import member positive {expected}: {semantic}"
        );
    }
    assert!(
        !semantic_text.contains("wrong_member.js"),
        "semantic mode must preserve imported member coordinate boundaries: {semantic}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_allows_static_projection_identity() {
    let dir = std::env::temp_dir().join(format!("nose_projection_identity_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("projection_direct.js"),
        "function direct(record, value) {\n  return value + record.today;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("projection_keyed.js"),
        "function keyed(row, amount) {\n  return amount + row['today'];\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("projection_destructured.js"),
        "function destructured(row, amount) {\n  const { today: selected } = row;\n  return amount + selected;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("projection_rust_direct.rs"),
        "pub fn rust_direct(record: Reading, value: i32) -> i32 {\n    value + record.today\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("projection_rust_shorthand.rs"),
        "pub fn rust_shorthand(row: Reading, amount: i32) -> i32 {\n    let Reading { today, .. } = row;\n    amount + today\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("projection_negative.js"),
        "function negative(row, amount, key) {\n  return amount + row[key];\n}\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    assert!(
        !semantic_families.is_empty(),
        "semantic mode should report static projection identities: {semantic}"
    );
    let semantic_text = semantic_json.to_string();
    for expected in [
        "projection_direct.js",
        "projection_keyed.js",
        "projection_destructured.js",
        "projection_rust_direct.rs",
        "projection_rust_shorthand.rs",
    ] {
        assert!(
            semantic_text.contains(expected),
            "semantic mode should include {expected}: {semantic}"
        );
    }
    assert!(
        !semantic_text.contains("projection_negative.js"),
        "semantic mode must not treat dynamic keys as fixed projections: {semantic}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_distinguishes_nullish_from_truthy_defaults() {
    let dir = std::env::temp_dir().join(format!("nose_nullish_default_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("nullish_coalesce.js"),
        "function coalesce(value, fallback) {\n  return value ?? fallback;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("nullish_ternary.js"),
        "function ternary(value, fallback) {\n  return value == null ? fallback : value;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("nullish_guard.js"),
        "function guard(value, fallback) {\n  if (value == null) {\n    return fallback;\n  }\n  return value;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("truthy_negative.js"),
        "function truthy(value, fallback) {\n  return value || fallback;\n}\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    assert_eq!(
        semantic_families.len(),
        1,
        "semantic mode should report one nullish-default family: {semantic}"
    );
    let semantic_text = semantic_json.to_string();
    for expected in [
        "nullish_coalesce.js",
        "nullish_ternary.js",
        "nullish_guard.js",
    ] {
        assert!(
            semantic_text.contains(expected),
            "semantic mode should include {expected}: {semantic}"
        );
    }
    assert!(
        !semantic_text.contains("truthy_negative.js"),
        "semantic mode must not merge nullish and truthy defaults: {semantic}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_proves_js_record_shape_guards() {
    let dir = std::env::temp_dir().join(format!("nose_record_guard_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("guard_direct.js"),
        "function direct(value) {\n  return typeof value === \"object\" && value !== null && !Array.isArray(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("guard_reordered.js"),
        "function reordered(candidate) {\n  return !Array.isArray(candidate) && candidate !== null && typeof candidate === \"object\";\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("guard_truthy.js"),
        "function truthy(input) {\n  return Boolean(input) && typeof input === \"object\" && !Array.isArray(input);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_allowed_negative.js"),
        "function arrayAllowed(value) {\n  return typeof value === \"object\" && value !== null;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("null_allowed_negative.js"),
        "function nullAllowed(value) {\n  return typeof value === \"object\" && !Array.isArray(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_typeof_literal_negative.js"),
        "function wrongLiteral(value) {\n  return typeof value === \"ob ject\" && value !== null && !Array.isArray(value);\n}\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    assert_eq!(
        semantic_families.len(),
        1,
        "semantic mode should report one proved record-shape guard family: {semantic}"
    );
    let semantic_text = semantic_json.to_string();
    for expected in ["guard_direct.js", "guard_reordered.js", "guard_truthy.js"] {
        assert!(
            semantic_text.contains(expected),
            "semantic mode should include {expected}: {semantic}"
        );
    }
    for unexpected in [
        "array_allowed_negative.js",
        "null_allowed_negative.js",
        "wrong_typeof_literal_negative.js",
    ] {
        assert!(
            !semantic_text.contains(unexpected),
            "semantic mode must reject invalid or incomplete record-shape guards: {semantic}"
        );
    }

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_proves_js_own_property_guards() {
    let dir = std::env::temp_dir().join(format!("nose_own_property_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("has_own.js"),
        "function hasOwn(value) {\n  return Object.hasOwn(value, \"ready\");\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("has_own_call.js"),
        "function hasOwnCall(candidate) {\n  return Object.prototype.hasOwnProperty.call(candidate, \"ready\");\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("in_operator_negative.js"),
        "function inOperator(value) {\n  return \"ready\" in value;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("direct_method_negative.js"),
        "function directMethod(value) {\n  return value.hasOwnProperty(\"ready\");\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("different_key_negative.js"),
        "function differentKey(value) {\n  return Object.hasOwn(value, \"enabled\");\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("shadowed_object_negative.js"),
        "function shadowedObject(Object, value) {\n  return Object.hasOwn(value, \"ready\");\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("shadowed_global_object_negative.js"),
        "const Object = { hasOwn() { return false; } };\nfunction shadowedGlobal(value) {\n  return Object.hasOwn(value, \"ready\");\n}\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    assert_eq!(
        semantic_families.len(),
        1,
        "semantic mode should report one proved own-property guard family: {semantic}"
    );
    let semantic_text = semantic_json.to_string();
    for expected in ["has_own.js", "has_own_call.js"] {
        assert!(
            semantic_text.contains(expected),
            "semantic mode should include {expected}: {semantic}"
        );
    }
    for unexpected in [
        "in_operator_negative.js",
        "direct_method_negative.js",
        "different_key_negative.js",
        "shadowed_object_negative.js",
        "shadowed_global_object_negative.js",
    ] {
        assert!(
            !semantic_text.contains(unexpected),
            "semantic mode must reject non-own or different-key property guards: {semantic}"
        );
    }

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_converges_cross_language_list_literals() {
    let dir = std::env::temp_dir().join(format!("nose_list_cross_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("pair.js"),
        "export function pair(a, b) {\n    return [a, b];\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("pair.py"),
        "def make_pair(x, y):\n    return [x, y]\n",
    )
    .unwrap();
    fs::write(
        dir.join("pair.rb"),
        "def build_pair(first, second)\n  [first, second]\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("tuple_negative.py"),
        "def tuple_pair(a, b):\n    return (a, b)\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    assert_eq!(
        semantic_families.len(),
        1,
        "semantic mode should report one cross-language list literal family: {semantic}"
    );
    let semantic_text = semantic_json.to_string();
    assert!(
        semantic_text.contains("pair.js")
            && semantic_text.contains("pair.py")
            && semantic_text.contains("pair.rb")
            && !semantic_text.contains("tuple_negative.py"),
        "semantic mode must converge list-like literals without merging tuples: {semantic}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_preserves_js_object_keys() {
    let dir = std::env::temp_dir().join(format!("nose_object_semantic_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("object_a.ts"),
        "export function example(command: string, description: string) {\n    return { command, description };\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("object_b.ts"),
        "export function makeExample(cmd: string, desc: string) {\n    return { command: cmd, description: desc };\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("object_key_negative.ts"),
        "export function makeParam(name: string, description: string) {\n    return { name, description };\n}\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    assert_eq!(
        semantic_families.len(),
        1,
        "semantic mode should report only same-key object construction: {semantic}"
    );
    let semantic_text = semantic_json.to_string();
    assert!(
        semantic_text.contains("object_a.ts")
            && semantic_text.contains("object_b.ts")
            && !semantic_text.contains("object_key_negative.ts"),
        "semantic mode must preserve object keys: {semantic}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_converges_cross_language_map_literals() {
    let dir = std::env::temp_dir().join(format!("nose_map_cross_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("map.ts"),
        "export function example(command: string, description: string) {\n    return { command, description };\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("map.py"),
        "def make_example(cmd, desc):\n    return {\"command\": cmd, \"description\": desc}\n",
    )
    .unwrap();
    fs::write(
        dir.join("map.rb"),
        "def build_example(command, description)\n  { command: command, description: description }\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_key_negative.ts"),
        "export function makeParam(name: string, description: string) {\n    return { name, description };\n}\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    assert_eq!(
        semantic_families.len(),
        1,
        "semantic mode should report one cross-language map literal family: {semantic}"
    );
    let semantic_text = semantic_json.to_string();
    assert!(
        semantic_text.contains("map.ts")
            && semantic_text.contains("map.py")
            && semantic_text.contains("map.rb")
            && !semantic_text.contains("map_key_negative.ts"),
        "semantic mode must converge map-like literals without dropping key identity: {semantic}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_captures_module_literal_bindings() {
    let dir = std::env::temp_dir().join(format!("nose_module_const_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("locale_a.ts"),
        "const labels = { today: \"today\", tomorrow: \"tomorrow\" };\nexport function label(token: string) {\n    return labels[token];\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("locale_b.ts"),
        "const labels = { today: \"heute\", tomorrow: \"morgen\" };\nexport function label(token: string) {\n    return labels[token];\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("locale_a_copy.ts"),
        "const labels = { today: \"today\", tomorrow: \"tomorrow\" };\nexport function relativeLabel(key: string) {\n    return labels[key];\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("locale_mutated.ts"),
        "let labels = { today: \"today\", tomorrow: \"tomorrow\" };\nlabels = { today: \"heute\", tomorrow: \"morgen\" };\nexport function mutatedLabel(key: string) {\n    return labels[key];\n}\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    assert_eq!(
        semantic_families.len(),
        1,
        "semantic mode should report only same module-literal binding behavior: {semantic}"
    );
    let semantic_text = semantic_json.to_string();
    assert!(
        semantic_text.contains("locale_a.ts")
            && semantic_text.contains("locale_a_copy.ts")
            && !semantic_text.contains("locale_b.ts")
            && !semantic_text.contains("locale_mutated.ts"),
        "semantic mode must include captured module literal values: {semantic}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_preserves_python_dict_keys() {
    let dir = std::env::temp_dir().join(format!("nose_dict_semantic_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("dict_a.py"),
        "def example(command, description):\n    return {\"command\": command, \"description\": description}\n",
    )
    .unwrap();
    fs::write(
        dir.join("dict_b.py"),
        "def make_example(cmd, desc):\n    return {\"command\": cmd, \"description\": desc}\n",
    )
    .unwrap();
    fs::write(
        dir.join("dict_key_negative.py"),
        "def make_param(name, description):\n    return {\"name\": name, \"description\": description}\n",
    )
    .unwrap();
    fs::write(
        dir.join("dict_spread_a.py"),
        "def with_spread(base, command):\n    return {**base, \"command\": command}\n",
    )
    .unwrap();
    fs::write(
        dir.join("dict_spread_b.py"),
        "def copy_spread(other, cmd):\n    return {**other, \"command\": cmd}\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    assert_eq!(
        semantic_families.len(),
        1,
        "semantic mode should report only same-key dict construction: {semantic}"
    );
    let semantic_text = semantic_json.to_string();
    assert!(
        semantic_text.contains("dict_a.py")
            && semantic_text.contains("dict_b.py")
            && !semantic_text.contains("dict_key_negative.py")
            && !semantic_text.contains("dict_spread_a.py")
            && !semantic_text.contains("dict_spread_b.py"),
        "semantic mode must preserve dict keys and reject unproved unpacking: {semantic}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_preserves_ruby_hash_keys() {
    let dir = std::env::temp_dir().join(format!("nose_hash_semantic_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("hash_a.rb"),
        "def example(command, description)\n  { command: command, description: description }\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("hash_b.rb"),
        "def make_example(cmd, desc)\n  { command: cmd, description: desc }\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("hash_key_negative.rb"),
        "def make_param(name, description)\n  { name: name, description: description }\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("hash_splat_a.rb"),
        "def with_splat(base, command)\n  { **base, command: command }\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("hash_splat_b.rb"),
        "def copy_splat(other, cmd)\n  { **other, command: cmd }\nend\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    assert_eq!(
        semantic_families.len(),
        1,
        "semantic mode should report only same-key hash construction: {semantic}"
    );
    let semantic_text = semantic_json.to_string();
    assert!(
        semantic_text.contains("hash_a.rb")
            && semantic_text.contains("hash_b.rb")
            && !semantic_text.contains("hash_key_negative.rb")
            && !semantic_text.contains("hash_splat_a.rb")
            && !semantic_text.contains("hash_splat_b.rb"),
        "semantic mode must preserve hash keys and reject unproved splats: {semantic}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn default_mode_runs_syntax_and_semantic() {
    let dir = make_mode_project("default_modes");
    let p = dir.to_str().unwrap();
    let out = run(&["scan", p, "--min-size", "12", "--format", "json"]);
    assert!(
        out.contains("copy_a.py") && out.contains("copy_b.py"),
        "default mode includes syntax: {out}"
    );
    assert!(
        out.contains("renamed_a.py") && out.contains("renamed_b.py"),
        "default mode includes semantic: {out}"
    );
    let repeated = run(&[
        "scan",
        p,
        "--mode",
        "syntax",
        "--mode",
        "semantic",
        "--min-size",
        "12",
        "--format",
        "json",
    ]);
    assert!(
        repeated.contains("copy_a.py") && repeated.contains("copy_b.py"),
        "repeated --mode includes syntax: {repeated}"
    );
    assert!(
        repeated.contains("renamed_a.py") && repeated.contains("renamed_b.py"),
        "repeated --mode includes semantic: {repeated}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn non_near_scan_modes_reject_similarity_thresholds() {
    let dir = make_mode_project("exact_threshold");
    // Only `near` carries an inline threshold; `syntax:0.5` / `semantic:0.5` are invalid.
    for mode in ["syntax:0.5", "semantic:0.5"] {
        let out = Command::new(bin())
            .args(["scan", dir.to_str().unwrap(), "--mode", mode])
            .output()
            .expect("run nose");
        assert!(
            !out.status.success(),
            "{mode} must not accept a fuzzy similarity threshold"
        );
        let stderr = String::from_utf8(out.stderr).unwrap();
        assert!(
            stderr.contains("unknown mode"),
            "specific error explains the invalid threshold for {mode}: {stderr}"
        );
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn near_scan_mode_accepts_similarity_threshold() {
    let dir = make_mode_project("near_threshold");
    let out = Command::new(bin())
        .args([
            "scan",
            dir.to_str().unwrap(),
            "--mode",
            "near:0.5",
            "--min-size",
            "12",
            "--format",
            "json",
        ])
        .output()
        .expect("run nose");
    assert!(
        out.status.success(),
        "near mode should accept --threshold: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_reports_the_clone_family() {
    let dir = make_project("fam");
    let out = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--min-size",
        "12",
        "--top",
        "10",
    ]);
    assert!(out.contains("clone"), "has a header: {out}");
    assert!(out.contains("copies"), "lists a family: {out}");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn exclude_drops_matching_paths() {
    let dir = make_project("excl");
    let p = dir.to_str().unwrap();
    // The family spans a/, b/, tests/. Excluding `tests` must shrink it.
    let with = run(&[
        "scan",
        p,
        "--min-size",
        "12",
        "--format",
        "json",
        "--top",
        "1",
    ]);
    let without = run(&[
        "scan",
        p,
        "--min-size",
        "12",
        "--format",
        "json",
        "--top",
        "1",
        "--exclude",
        "tests",
    ]);
    let count = |s: &str| s.matches("\"file\"").count();
    assert!(
        count(&with) > count(&without),
        "excluding tests removes a site: {} vs {}",
        count(&with),
        count(&without)
    );
    assert!(
        !without.contains("/tests/"),
        "no tests/ path in excluded output"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn exclude_supports_globs() {
    // `--exclude` takes gitignore-style globs, not just substrings: `a/**` excludes
    // the a/ directory specifically, leaving the family in b/ and tests/.
    let dir = make_project("glob");
    let p = dir.to_str().unwrap();
    let out = run(&[
        "scan",
        p,
        "--min-size",
        "12",
        "--format",
        "json",
        "--exclude",
        "a/**",
    ]);
    assert!(
        !out.contains("/a/f.py"),
        "glob `a/**` must exclude a/f.py: {out}"
    );
    assert!(out.contains("/b/f.py"), "b/ must remain: {out}");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn min_value_filters_low_value_families() {
    let dir = make_project("minval");
    let p = dir.to_str().unwrap();
    // A value floor above any family's value hides them all; zero keeps them.
    let all = run(&["scan", p, "--min-size", "12", "--min-value", "0"]);
    let none = run(&["scan", p, "--min-size", "12", "--min-value", "100000"]);
    assert!(all.contains("copies"), "unfiltered shows a family: {all}");
    assert!(
        !none.contains("copies"),
        "a high value floor hides every family: {none}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn hotspots_lists_modules() {
    let dir = make_project("hot");
    let out = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--min-size",
        "12",
        "--show",
        "hotspots",
    ]);
    assert!(
        out.contains("duplication hotspots"),
        "hotspots section present: {out}"
    );
    assert!(out.contains("dup lines"), "hotspot rows present: {out}");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn empty_corpus_warns_on_stderr() {
    // A path with no supported source must not silently look like "no duplication".
    let dir = std::env::temp_dir().join(format!("nose_empty_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("README.txt"), "no code here\n").unwrap();
    let out = Command::new(bin())
        .args(["scan", dir.to_str().unwrap()])
        .output()
        .expect("run nose");
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(
        stderr.contains("no supported source files found"),
        "expected an empty-corpus warning, got: {stderr}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn output_is_independent_of_thread_count() {
    // Parallel lowering/detection must not affect output: a single-threaded run and
    // a multi-threaded run must produce byte-identical JSON. A release invariant.
    let dir = make_project("threads");
    let p = dir.to_str().unwrap();
    let run_with_threads = |n: &str| {
        let out = Command::new(bin())
            .args(["scan", p, "--min-size", "12", "--format", "json"])
            .env("RAYON_NUM_THREADS", n)
            .output()
            .expect("run nose");
        assert!(out.status.success());
        String::from_utf8(out.stdout).unwrap()
    };
    assert_eq!(
        run_with_threads("1"),
        run_with_threads("8"),
        "1-thread and 8-thread runs must produce identical output"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn cache_output_matches_uncached_cold_and_warm() {
    // --cache-dir must not change results: cold (populating) and warm (reusing)
    // runs must both equal the non-cached output. This is the cache's correctness
    // contract — a stale/incorrect entry would diverge here.
    let dir = make_project("cache");
    // make_project's clones are *renamed* copies — matched only by the value-graph
    // channel. Add files with a *verbatim* duplicated block so the contiguous
    // (copy-paste) channel also produces a family: the cache once stored only units and
    // silently dropped that channel, so without the streams cached this run would diverge
    // here. (Identical variable names → identical raw token stream → a contiguous match,
    // unlike the renamed value-graph clones.)
    let block = "def handle(events):\n    out = []\n    for e in events:\n        if e.kind == 1:\n            out.append(e.payload)\n            record(e.id, e.kind)\n    return out\n";
    for name in ["dup_a", "dup_b", "dup_c"] {
        fs::write(dir.join(format!("{name}.py")), block).unwrap();
    }
    let p = dir.to_str().unwrap();
    let cache = std::env::temp_dir().join(format!("nose_cachedir_{}", std::process::id()));
    let _ = fs::remove_dir_all(&cache);
    let cd = cache.to_str().unwrap();

    let uncached = run(&["scan", p, "--min-size", "12", "--format", "json"]);
    let cold = run(&[
        "scan",
        p,
        "--min-size",
        "12",
        "--format",
        "json",
        "--cache-dir",
        cd,
    ]);
    let warm = run(&[
        "scan",
        p,
        "--min-size",
        "12",
        "--format",
        "json",
        "--cache-dir",
        cd,
    ]);

    assert_eq!(
        uncached, cold,
        "cold cache run must match the uncached output"
    );
    assert_eq!(cold, warm, "warm cache run must match the cold run");
    let _ = fs::remove_dir_all(&cache);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn fail_flag_sets_exit_code_as_ci_gate() {
    let dir = make_project("fail");
    let p = dir.to_str().unwrap();
    // A family exists → --fail must exit non-zero.
    let found = Command::new(bin())
        .args(["scan", p, "--min-size", "12", "--fail-on", "any"])
        .output()
        .expect("run");
    assert!(
        !found.status.success(),
        "--fail must fail when a family is found"
    );
    // Filter everything out → nothing to fail on → success.
    let clean = Command::new(bin())
        .args([
            "scan",
            p,
            "--min-size",
            "12",
            "--min-value",
            "1e9",
            "--fail-on",
            "any",
        ])
        .output()
        .expect("run");
    assert!(
        clean.status.success(),
        "--fail must pass when no family survives filters"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn output_paths_are_relative_to_cwd() {
    // Even when scanned via an absolute path, reported paths should be relative to
    // the working directory (readable in CI logs / reviews).
    let dir = make_project("relpath");
    let abs = dir.canonicalize().unwrap();
    let out = Command::new(bin())
        .args([
            "scan",
            abs.to_str().unwrap(),
            "--min-size",
            "12",
            "--format",
            "json",
        ])
        .current_dir(&abs)
        .output()
        .expect("run");
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        stdout.contains("\"a/f.py\"") || stdout.contains("\"b/f.py\""),
        "paths should be relative to cwd, got: {stdout}"
    );
    assert!(
        !stdout.contains(abs.to_str().unwrap()),
        "no absolute path should leak: {stdout}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn baseline_hides_accepted_families() {
    // Write a baseline accepting today's families, then a re-run reports nothing new.
    let dir = make_project("baseline");
    let p = dir.to_str().unwrap();
    let bl = std::env::temp_dir().join(format!("nose_bl_{}.json", std::process::id()));
    let bls = bl.to_str().unwrap();

    // Sanity: without a baseline there IS a family.
    assert!(run(&["scan", p, "--min-size", "12"]).contains("copies"));

    // Accept current state…
    let _ = Command::new(bin())
        .args([
            "scan",
            p,
            "--min-size",
            "12",
            "--baseline",
            bls,
            "--write-baseline",
        ])
        .output()
        .expect("run");
    assert!(bl.exists(), "baseline file should be written");
    let baseline_text = fs::read_to_string(&bl).expect("read baseline");
    assert!(
        baseline_text.contains("\"members\""),
        "baseline should record member identities for changed/resolved comparison: {baseline_text}"
    );

    // …then a re-run shows no *new* families.
    let after = run(&["scan", p, "--min-size", "12", "--baseline", bls]);
    assert!(
        !after.contains("sites"),
        "baselined families must be hidden, got: {after}"
    );
    let _ = fs::remove_file(&bl);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn new_only_json_marks_new_families_against_baseline() {
    let dir = make_project("new_only");
    let p = dir.to_str().unwrap();
    let bl = std::env::temp_dir().join(format!("nose_new_only_bl_{}.json", std::process::id()));
    let bls = bl.to_str().unwrap();

    let baseline = Command::new(bin())
        .args([
            "scan",
            p,
            "--min-size",
            "12",
            "--baseline",
            bls,
            "--write-baseline",
        ])
        .output()
        .expect("write baseline");
    assert!(baseline.status.success());

    add_distinct_clone_family(&dir);
    let out = run(&[
        "scan",
        p,
        "--min-size",
        "12",
        "--baseline",
        bls,
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let json = scan_json(&out);
    let families = scan_families(&json);
    assert!(
        !families.is_empty(),
        "new-only scan should report the introduced family: {out}"
    );
    assert!(
        out.contains("fresh_a.py") && out.contains("fresh_b.py") && !out.contains("a/f.py"),
        "new-only JSON should include new sites, not accepted baseline sites: {out}"
    );
    assert_eq!(json["baseline"]["mode"], "new-only");
    assert!(json["baseline"]["new_families"].as_u64().unwrap() >= 1);
    assert!(json["baseline"]["unchanged_families"].as_u64().unwrap() >= 1);
    assert_eq!(json["baseline"]["changed_families"], 0);
    assert_eq!(json["baseline"]["resolved_families"], 0);
    assert!(
        families
            .iter()
            .all(|f| f["baseline_status"].as_str() == Some("new")),
        "all reportable families should be marked new: {out}"
    );

    let _ = fs::remove_file(&bl);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn fail_on_new_fails_for_changed_family_and_passes_when_clean() {
    let dir = make_project("fail_on_new");
    let p = dir.to_str().unwrap();
    let bl = std::env::temp_dir().join(format!("nose_fail_on_new_bl_{}.json", std::process::id()));
    let bls = bl.to_str().unwrap();

    let baseline = Command::new(bin())
        .args([
            "scan",
            p,
            "--min-size",
            "12",
            "--baseline",
            bls,
            "--write-baseline",
        ])
        .output()
        .expect("write baseline");
    assert!(baseline.status.success());

    let clean = Command::new(bin())
        .args([
            "scan",
            p,
            "--min-size",
            "12",
            "--baseline",
            bls,
            "--fail-on",
            "new",
        ])
        .output()
        .expect("clean run");
    assert!(
        clean.status.success(),
        "--fail-on-new should pass when every family is accepted"
    );

    add_member_to_existing_family(&dir);
    let changed = Command::new(bin())
        .args([
            "scan",
            p,
            "--min-size",
            "12",
            "--baseline",
            bls,
            "--fail-on",
            "new",
            "--format",
            "json",
            "--top",
            "0",
        ])
        .output()
        .expect("changed run");
    assert!(
        !changed.status.success(),
        "--fail-on-new should fail when a family changes"
    );
    let stdout = String::from_utf8(changed.stdout).unwrap();
    let stderr = String::from_utf8(changed.stderr).unwrap();
    let json = scan_json(&stdout);
    assert_eq!(json["baseline"]["new_families"], 0);
    assert_eq!(json["baseline"]["changed_families"], 1);
    assert_eq!(json["baseline"]["resolved_families"], 0);
    assert_eq!(scan_families(&json)[0]["baseline_status"], "changed");
    assert!(
        stderr.contains("--fail-on new"),
        "stderr should name the explicit gate: {stderr}"
    );

    let _ = fs::remove_file(&bl);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn config_file_supplies_defaults() {
    // A nose.toml in the working dir provides defaults (here: an exclude glob and
    // min-size); a CLI flag still overrides.
    let dir = make_project("cfg");
    fs::write(
        dir.join("nose.toml"),
        "[scan]\nexclude = [\"a/**\"]\nmin-size = 12\n",
    )
    .unwrap();
    let out = Command::new(bin())
        .args(["scan", ".", "--format", "json"])
        .current_dir(&dir)
        .output()
        .expect("run");
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        !stdout.contains("a/f.py"),
        "config exclude a/** should drop a/f.py: {stdout}"
    );
    assert!(stdout.contains("b/f.py"), "b/ remains: {stdout}");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn inline_nose_ignore_suppresses_a_site() {
    // A `nose-ignore` marker above a function drops that site; with only one copy
    // left there's no family.
    let dir = std::env::temp_dir().join(format!("nose_sup_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let body = "def f(items):\n    t = 0\n    for x in items:\n        if x > 0:\n            t = t + x * x\n    return t\n";
    fs::create_dir_all(dir.join("a")).unwrap();
    fs::create_dir_all(dir.join("b")).unwrap();
    fs::write(dir.join("a/f.py"), body).unwrap();
    fs::write(dir.join("b/f.py"), format!("# nose-ignore\n{body}")).unwrap();
    let p = dir.to_str().unwrap();
    assert!(
        run(&["scan", p, "--min-size", "12"]).contains("0 clone"),
        "the marked copy must be suppressed, leaving no family"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn structured_ignore_suppresses_family_id_with_metadata() {
    let dir = make_project("structured_ignore_id");
    let p = dir.to_str().unwrap();
    let before = run(&[
        "scan",
        p,
        "--mode",
        "semantic",
        "--min-size",
        "12",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let before_json = scan_json(&before);
    let family_id = scan_families(&before_json)[0]["family_id"]
        .as_str()
        .expect("family_id should be exposed")
        .to_string();
    let ignore_file = std::env::temp_dir().join(format!(
        "nose_structured_ignore_{}.json",
        std::process::id()
    ));
    fs::write(
        &ignore_file,
        format!(
            "{{\"ignores\":[{{\"family_id\":\"{family_id}\",\"reason\":\"generated-code\",\"note\":\"Generated from the same template.\",\"owner\":\"platform\",\"expires_at\":\"2099-01-01\"}}]}}\n"
        ),
    )
    .unwrap();

    let after = run(&[
        "scan",
        p,
        "--mode",
        "semantic",
        "--min-size",
        "12",
        "--format",
        "json",
        "--top",
        "0",
        "--ignore-file",
        ignore_file.to_str().unwrap(),
    ]);
    let after_json = scan_json(&after);
    assert!(
        scan_families(&after_json).is_empty(),
        "the ignored family should be absent from active findings: {after}"
    );
    assert_eq!(after_json["ignore"]["active_entries"], 1);
    assert_eq!(after_json["ignore"]["ignored_families"], 1);
    assert_eq!(after_json["ignored_families"][0]["family_id"], family_id);
    assert_eq!(
        after_json["ignored_families"][0]["ignore"]["reason"],
        "generated-code"
    );
    assert_eq!(
        after_json["ignored_families"][0]["ignore"]["owner"],
        "platform"
    );

    let _ = fs::remove_file(&ignore_file);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn default_structured_ignore_file_matches_paths() {
    let dir = make_project("structured_ignore_paths");
    fs::write(
        dir.join("nose.ignore.json"),
        "{\"ignores\":[{\"paths\":[\"a/**\"],\"reason\":\"template-copy\",\"note\":\"a/ is generated.\"}]}\n",
    )
    .unwrap();
    let out = Command::new(bin())
        .args([
            "scan",
            ".",
            "--mode",
            "semantic",
            "--min-size",
            "12",
            "--format",
            "json",
            "--top",
            "0",
        ])
        .current_dir(&dir)
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "scan should succeed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    let json = scan_json(&stdout);
    assert!(
        scan_families(&json).is_empty(),
        "path ignore should suppress the family: {stdout}"
    );
    assert_eq!(json["ignore"]["active_entries"], 1);
    assert_eq!(json["ignore"]["ignored_families"], 1);
    assert_eq!(
        json["ignored_families"][0]["ignore"]["matched_paths"][0],
        "./a/f.py"
    );
    assert_eq!(
        json["ignored_families"][0]["ignore"]["selectors"]["paths"][0],
        "a/**"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn expired_structured_ignore_is_reported_but_not_applied() {
    let dir = make_project("structured_ignore_expired");
    fs::write(
        dir.join("nose.ignore.json"),
        "{\"ignores\":[{\"paths\":[\"a/**\"],\"reason\":\"temporary-waiver\",\"owner\":\"platform\",\"expires_at\":\"2000-01-01\"}]}\n",
    )
    .unwrap();
    let out = Command::new(bin())
        .args([
            "scan",
            ".",
            "--mode",
            "semantic",
            "--min-size",
            "12",
            "--format",
            "json",
            "--top",
            "0",
        ])
        .current_dir(&dir)
        .output()
        .expect("run");
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    let stderr = String::from_utf8(out.stderr).unwrap();
    let json = scan_json(&stdout);
    assert!(
        !scan_families(&json).is_empty(),
        "expired ignore must not suppress the family: {stdout}"
    );
    assert_eq!(json["ignore"]["active_entries"], 0);
    assert_eq!(json["ignore"]["expired_entries"], 1);
    assert_eq!(json["ignore"]["ignored_families"], 0);
    assert_eq!(json["ignore"]["expired"][0]["reason"], "temporary-waiver");
    assert!(
        stderr.contains("expired on 2000-01-01") && stderr.contains("not applied"),
        "stderr should explain the expired entry: {stderr}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn malformed_structured_ignore_file_fails_clearly() {
    let dir = make_project("structured_ignore_bad");
    let ignore_file = dir.join("bad-ignore.json");
    fs::write(
        &ignore_file,
        "{\"ignores\":[{\"paths\":[\"a/**\"],\"note\":\"missing reason\"}]}\n",
    )
    .unwrap();
    let out = Command::new(bin())
        .args([
            "scan",
            dir.to_str().unwrap(),
            "--min-size",
            "12",
            "--ignore-file",
            ignore_file.to_str().unwrap(),
        ])
        .output()
        .expect("run");
    assert!(!out.status.success(), "malformed ignore files must fail");
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(
        stderr.contains("parsing ignore file") || stderr.contains("validating ignore file"),
        "error should name the ignore file problem: {stderr}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn sarif_output_is_well_formed() {
    let dir = make_project("sarif");
    let out = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--min-size",
        "12",
        "--format",
        "sarif",
    ]);
    let v: serde_json::Value = serde_json::from_str(&out).expect("SARIF must be valid JSON");
    assert_eq!(v["version"], "2.1.0");
    assert!(v["runs"][0]["tool"]["driver"]["name"] == "nose");
    assert!(
        !v["runs"][0]["results"].as_array().unwrap().is_empty(),
        "should have at least one result: {out}"
    );
    // The run records the full family count, so a consumer can tell a complete upload
    // from a truncated one. This single-family project is not truncated (default --top 30),
    // so total == shown and there is no truncation notification.
    let props = &v["runs"][0]["properties"];
    let total = props["total_families"].as_u64().expect("total_families");
    let shown = props["shown_families"].as_u64().expect("shown_families");
    assert_eq!(shown, total, "untruncated run: shown == total ({out})");
    assert_eq!(
        shown as usize,
        v["runs"][0]["results"].as_array().unwrap().len()
    );
    assert!(
        v["runs"][0].get("invocations").is_none(),
        "no truncation note when nothing is hidden: {out}"
    );
    let _ = fs::remove_dir_all(&dir);
}

/// A project with several behaviorally-distinct duplicated functions, so `nose scan`
/// reports multiple clone families. `--top N` can then truncate the report.
fn make_multi_family_project(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nose_cli_{tag}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    // Four distinct computations (distinct value fingerprints, so they don't merge into
    // one family), each duplicated across two directories → four families.
    let logics = [
        ("sq", "def f(items):\n    a = 0\n    for x in items:\n        if x > 0:\n            a = a + x * x\n    return a\n"),
        ("prod", "def f(items):\n    a = 1\n    for x in items:\n        a = a * x\n    return a\n"),
        ("cnt", "def f(items):\n    a = 0\n    for x in items:\n        if x < 0:\n            a = a + 1\n    return a\n"),
        ("join", "def f(items):\n    a = ''\n    for x in items:\n        a = a + x + ','\n    return a\n"),
    ];
    for sub in ["x", "y"] {
        for (name, src) in logics {
            let d = dir.join(sub);
            fs::create_dir_all(&d).unwrap();
            fs::write(d.join(format!("{name}.py")), src).unwrap();
        }
    }
    dir
}

#[test]
fn sarif_records_and_notes_top_truncation() {
    let dir = make_multi_family_project("sarif_trunc");

    // --top 1: only one family is emitted, but the run must record the true total and
    // carry an explicit truncation note pointing at --top 0.
    let out = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--min-size",
        "12",
        "--format",
        "sarif",
        "--top",
        "1",
    ]);
    let v: serde_json::Value = serde_json::from_str(&out).expect("SARIF must be valid JSON");
    let props = &v["runs"][0]["properties"];
    let total = props["total_families"].as_u64().expect("total_families");
    let shown = props["shown_families"].as_u64().expect("shown_families");
    assert!(total >= 2, "fixture should yield multiple families: {out}");
    assert_eq!(shown, 1, "--top 1 shows exactly one family: {out}");
    assert_eq!(v["runs"][0]["results"].as_array().unwrap().len(), 1);
    let note = v["runs"][0]["invocations"][0]["toolExecutionNotifications"][0].clone();
    assert_eq!(note["level"], "note", "truncation note present: {out}");
    assert!(
        note["message"]["text"]
            .as_str()
            .unwrap()
            .contains("--top 0"),
        "note points the reader at --top 0: {out}"
    );

    // --top 0: the whole set is emitted, so there is no truncation note.
    let full = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--min-size",
        "12",
        "--format",
        "sarif",
        "--top",
        "0",
    ]);
    let fv: serde_json::Value = serde_json::from_str(&full).expect("SARIF must be valid JSON");
    let fp = &fv["runs"][0]["properties"];
    assert_eq!(
        fp["shown_families"], fp["total_families"],
        "--top 0 emits every family: {full}"
    );
    assert!(
        fv["runs"][0].get("invocations").is_none(),
        "no truncation note with --top 0: {full}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn diff_shows_the_differing_line() {
    // Two near-identical functions differing in one line → --diff marks it +/-.
    let dir = std::env::temp_dir().join(format!("nose_diff_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("a")).unwrap();
    fs::create_dir_all(dir.join("b")).unwrap();
    // The two sum-loops differ only in a literal (the per-element weight), so they
    // are a clear near-duplicate family. (A `+`-loop vs a `*`-loop is deliberately
    // *not* used here: the value graph now treats those as distinct reductions — see
    // §AH — so they are no longer a single family.)
    fs::write(
        dir.join("a/f.py"),
        "def f(items):\n    t = 0\n    for x in items:\n        t = t + x * 2\n    return t\n",
    )
    .unwrap();
    fs::write(
        dir.join("b/f.py"),
        "def g(items):\n    t = 0\n    for x in items:\n        t = t + x * 3\n    return t\n",
    )
    .unwrap();
    let out = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "near:0.5",
        "--min-size",
        "10",
        "--show",
        "diff",
    ]);
    assert!(out.contains("diff  "), "should print a diff header: {out}");
    assert!(
        out.contains("-         t = t + x * 2") && out.contains("+         t = t + x * 3"),
        "diff should mark the changed line: {out}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn version_flag_works() {
    let out = run(&["--version"]);
    assert!(out.starts_with("nose "), "version line: {out}");
}

#[test]
fn capabilities_command_emits_machine_readable_contract() {
    let out = run(&["capabilities"]);
    let json: serde_json::Value =
        serde_json::from_str(&out).expect("capabilities must emit valid JSON");

    assert_eq!(json["schema_version"], 1);
    assert_eq!(json["tool"]["name"], "nose");
    assert_eq!(json["tool"]["version"], env!("CARGO_PKG_VERSION"));
    assert!(
        json["platform"]["os"]
            .as_str()
            .is_some_and(|s| !s.is_empty()),
        "platform.os should be non-empty: {out}"
    );
    assert!(
        json["platform"]["arch"]
            .as_str()
            .is_some_and(|s| !s.is_empty()),
        "platform.arch should be non-empty: {out}"
    );
    assert_eq!(json["interfaces"]["capabilities_json"], true);
    assert_eq!(json["interfaces"]["version_json"], false);
    assert_eq!(json["interfaces"]["doctor_json"], false);

    assert_eq!(
        json_array_strings(&json["commands"], "stable"),
        vec!["capabilities", "il", "review", "scan", "stats"]
    );
    assert_eq!(json["schemas"]["capabilities"][0], 1);
    assert_eq!(json["schemas"]["scan_json"][0], 1);
    assert_eq!(
        json_array_strings(&json["scan"], "modes"),
        vec!["syntax", "semantic", "near"]
    );
    assert_eq!(
        json_array_strings(&json["scan"], "default_modes"),
        vec!["syntax", "semantic"]
    );
    assert_eq!(
        json_array_strings(&json["scan"], "output_formats"),
        vec!["human", "json", "markdown", "sarif"]
    );
    assert_eq!(
        json_array_strings(&json["scan"], "sort_keys"),
        vec!["extractability", "value", "sites", "hazard"]
    );
    assert_eq!(json["scan"]["capabilities"]["baseline"], true);
    assert_eq!(json["scan"]["capabilities"]["structured_ignores"], true);
    assert_eq!(
        json_array_strings(&json["il"], "output_formats"),
        vec!["sexpr", "json"]
    );
    assert_eq!(
        json_array_strings(&json["stats"], "output_formats"),
        vec!["human", "json"]
    );
}

#[test]
fn broken_pipe_exits_cleanly() {
    // `nose scan … | head` (or quitting a pager) closes the read end of stdout while
    // nose is still writing. That write then fails with `BrokenPipe`, which `println!`
    // turns into a panic — the `failed printing to stdout` crash users hit. The Unix
    // convention is to stop quietly; main()'s panic hook must catch this and exit 0.
    // We reproduce it by closing the pipe's read end outright, so every write to the
    // now-readerless pipe gets EPIPE.
    let dir = std::env::temp_dir().join(format!("nose_pipe_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    // A handful of large, distinct clone families → plenty of `--diff` output, so the
    // child keeps writing past any buffering and reliably hits the dead pipe.
    for i in 0..40 {
        let body = |delta: &str| {
            let mut s = format!("def fam{i}(xs):\n    acc{i} = 0\n");
            for k in 0..120 {
                s.push_str(&format!("    acc{i} = acc{i} + xs[{k}] * {i}\n"));
            }
            s.push_str(&format!("    acc{i} = acc{i} {delta}\n    return acc{i}\n"));
            s
        };
        for (sub, delta) in [("a", "+ 1"), ("b", "- 1")] {
            let d = dir.join(sub);
            fs::create_dir_all(&d).unwrap();
            fs::write(d.join(format!("f{i}.py")), body(delta)).unwrap();
        }
    }

    let mut child = Command::new(bin())
        .args([
            "scan",
            dir.to_str().unwrap(),
            "--mode",
            "near:0.5",
            "--show",
            "diff",
            "--top",
            "40",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn nose");
    // Close the read end immediately — the moral equivalent of `| head` exiting early.
    drop(child.stdout.take());
    let out = child.wait_with_output().expect("wait for nose");
    let _ = fs::remove_dir_all(&dir);

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.contains("panicked") && !stderr.contains("Broken pipe"),
        "broken pipe must not panic, got stderr:\n{stderr}"
    );
    assert!(
        out.status.success(),
        "broken pipe should exit cleanly (0), got {:?}\nstderr:\n{stderr}",
        out.status
    );
}

#[test]
fn deeply_nested_file_does_not_overflow() {
    // A pathologically deep expression (minified bundle / generated code) must not
    // crash the recursive lowering on rayon's small worker stack. Regression for the
    // stack-overflow on real repos (prettier test fixtures); main() sizes the pool's
    // stack so this completes instead of aborting (`run` asserts a clean exit).
    let dir = std::env::temp_dir().join(format!("nose_deep_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let depth = 40_000;
    let body = format!("const x = {}1{};\n", "[".repeat(depth), "]".repeat(depth));
    fs::write(dir.join("deep.js"), body).unwrap();
    let _ = run(&["scan", dir.to_str().unwrap()]);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn proposal_shows_shared_skeleton_and_parameters() {
    // Two near-identical functions differing in one line → an extraction proposal:
    // the shared skeleton plus a ⟨param⟩ for the varying spot.
    let dir = std::env::temp_dir().join(format!("nose_prop_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("a")).unwrap();
    fs::create_dir_all(dir.join("b")).unwrap();
    let mk = |op: &str| {
        format!("def f(xs):\n    t = 0\n    for x in xs:\n        if x > 0:\n            t = t {op} x\n    return t\n")
    };
    fs::write(dir.join("a/m.py"), mk("+")).unwrap();
    fs::write(dir.join("b/m.py"), mk("*")).unwrap();
    let out = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "near:0.5",
        "--show",
        "proposal",
        "--min-size",
        "12",
    ]);
    assert!(out.contains("proposal"), "should print a proposal: {out}");
    assert!(
        out.contains("parameter(s) vary"),
        "should report parameters: {out}"
    );
    assert!(
        out.contains("⟨param 1⟩"),
        "should show a parameter placeholder: {out}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn sort_keys_label_the_ranking_and_reject_garbage() {
    let dir = make_project("sort");
    let p = dir.to_str().unwrap();
    // Default ranking is extractability — the header says so, in plain language.
    let def = run(&["scan", p, "--min-size", "12"]);
    assert!(
        def.contains("ranked by extractability"),
        "default header names the ranking: {def}"
    );
    // --sort hazard is available and switches the header.
    let byhaz = run(&["scan", p, "--min-size", "12", "--sort", "hazard"]);
    assert!(
        byhaz.contains("ranked by divergent-edit hazard"),
        "--sort hazard names the ranking: {byhaz}"
    );
    // Families are described in plain language (copies + removable lines), not a
    // wall of internal metrics.
    assert!(
        def.contains("copies") && def.contains("lines removable"),
        "family summary is plain-language: {def}"
    );
    // --sort value switches the header.
    let byval = run(&["scan", p, "--min-size", "12", "--sort", "value"]);
    assert!(
        byval.contains("raw duplicated volume"),
        "--sort value names the ranking: {byval}"
    );
    // An unknown sort key is rejected by clap.
    let bad = Command::new(bin())
        .args(["scan", p, "--sort", "bogus"])
        .output()
        .expect("run nose");
    assert!(!bad.status.success(), "unknown --sort value is rejected");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_reports_what_it_scanned() {
    let dir = make_project("scope");
    let p = dir.to_str().unwrap();
    // The scope line states the file count and per-language breakdown (make_project
    // writes four Python files), so a `.gitignore`-pruned scope is visible, not silent.
    let out = run(&["scan", p, "--min-size", "12"]);
    assert!(
        out.contains("scanned 4 files") && out.contains("python 4"),
        "header reports scanned count and languages: {out}"
    );
    // The scope line must not corrupt machine-readable output.
    let json = run(&["scan", p, "--min-size", "12", "--format", "json"]);
    let report = scan_json(&json);
    assert!(
        json.trim_start().starts_with('{') && !json.contains("scanned"),
        "json output stays a pure object without human text: {json}"
    );
    assert_eq!(report["scope"]["files"], 4);
    assert_eq!(report["scope"]["languages"][0]["language"], "python");
    assert_eq!(report["scope"]["languages"][0]["files"], 4);
    let _ = fs::remove_dir_all(&dir);
}

/// A determinism guard with teeth: the toy `make_project` is too small to exercise the
/// order-sensitive paths (shared-line IDF summation, RANSAC offset ties, cross-family
/// ordering) where byte-identical output has actually broken before. Generate a project
/// with many same-language near-duplicate functions — several clone families whose copies
/// differ in a few lines — and assert the JSON report is identical across a range of
/// thread counts (each a distinct process, so this also covers `HashMap` seed variation).
#[test]
fn output_is_byte_identical_across_thread_counts_on_a_rich_project() {
    let dir = std::env::temp_dir().join(format!("nose_determinism_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    // 8 families × 5 near-duplicate copies. Each copy renames variables and tweaks a
    // couple of lines, so families form, RANSAC alignment runs, and the honest
    // shared-line counter compares several members.
    for fam in 0..8 {
        for copy in 0..5 {
            let v = format!("v{copy}");
            let src = format!(
                "def family{fam}_{copy}(items):\n    \
                 {v}_total = {copy}\n    \
                 {v}_seen = []\n    \
                 for {v}_x in items:\n        \
                 if {v}_x > {fam}:\n            \
                 {v}_total = {v}_total + {v}_x * {fam}\n            \
                 {v}_seen.append({v}_x)\n        \
                 else:\n            \
                 {v}_total = {v}_total - {copy}\n    \
                 return ({v}_total, {v}_seen)\n"
            );
            fs::write(dir.join(format!("f{fam}_{copy}.py")), src).unwrap();
        }
    }
    let p = dir.to_str().unwrap();
    let run_threads = |n: &str| {
        let out = Command::new(bin())
            .args([
                "scan",
                p,
                "--mode",
                "near:0.5",
                "--min-size",
                "12",
                "--format",
                "json",
                "--top",
                "1000",
            ])
            .env("RAYON_NUM_THREADS", n)
            .output()
            .expect("run nose");
        assert!(out.status.success());
        String::from_utf8(out.stdout).unwrap()
    };
    let baseline = run_threads("1");
    assert!(
        baseline.contains("\"members\""),
        "the fixture forms families: {baseline}"
    );
    for n in ["2", "3", "4", "8"] {
        assert_eq!(
            run_threads(n),
            baseline,
            "{n}-thread output must be byte-identical to the 1-thread run"
        );
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn top_zero_shows_all_families() {
    // `--top 0` is documented as "no limit" (docs/usage.md). It must return every
    // family, not an empty set. Regression: `.take(0)` used to silently drop all.
    let dir = std::env::temp_dir().join(format!("nose_top0_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    // Build several *structurally distinct* clone families so "all" is clearly more
    // than "top 1". nose finds semantic (Type-4) clones, so each family must have a
    // different shape (statement count / control flow) or they collapse into one.
    let bodies = [
        "def f0(items):\n    acc = 0\n    for x in items:\n        acc = acc + x\n    return acc\n",
        "def f1(items):\n    acc = 1\n    for x in items:\n        if x > 0:\n            acc = acc * x\n    return acc\n",
        "def f2(s):\n    out = []\n    for c in s:\n        out.append(c)\n        out.append(c)\n    return out\n",
        "def f3(a, b):\n    r = 0\n    while a < b:\n        r = r + a\n        a = a + 1\n    return r\n",
        "def f4(d):\n    keys = []\n    for k in d:\n        if k is not None:\n            keys.append(k)\n    keys.sort()\n    return keys\n",
        "def f5(n):\n    total = 0\n    i = 0\n    while i < n:\n        total = total + i * i\n        i = i + 1\n    return total\n",
    ];
    for (i, body) in bodies.iter().enumerate() {
        for sub in ["a", "b"] {
            let d = dir.join(sub);
            fs::create_dir_all(&d).unwrap();
            fs::write(d.join(format!("f{i}.py")), body).unwrap();
        }
    }
    let p = dir.to_str().unwrap();
    let count = |args: &[&str]| -> usize {
        let out = run(args);
        let v = scan_json(&out);
        scan_families(&v).len()
    };

    let all = count(&[
        "scan",
        p,
        "--min-size",
        "12",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let one = count(&[
        "scan",
        p,
        "--min-size",
        "12",
        "--format",
        "json",
        "--top",
        "1",
    ]);
    assert!(all > 1, "--top 0 must show all families, got {all}");
    assert_eq!(one, 1, "--top 1 must still cap at one family, got {one}");
    assert!(
        all >= 6,
        "--top 0 should include every distinct family (expected >=6, got {all})"
    );
    let _ = fs::remove_dir_all(&dir);
}

/// Helper: run git in a fixture dir, asserting success.
fn git_in(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .current_dir(dir)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_OBJECT_DIRECTORY")
        .env_remove("GIT_COMMON_DIR")
        .args(args)
        .output()
        .expect("run git");
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Turn a fixture dir into a committed git repo.
fn init_git_repo(dir: &Path) {
    git_in(dir, &["init", "-q", "-b", "main"]);
    git_in(dir, &["config", "user.email", "t@example.com"]);
    git_in(dir, &["config", "user.name", "Test"]);
    git_in(dir, &["add", "-A"]);
    git_in(dir, &["commit", "-q", "-m", "init"]);
}

fn nose_review(dir: &Path, extra: &[&str]) -> std::process::Output {
    let mut args = vec!["review", ".", "--min-size", "8"];
    args.extend_from_slice(extra);
    Command::new(bin())
        .current_dir(dir)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_OBJECT_DIRECTORY")
        .env_remove("GIT_COMMON_DIR")
        .args(&args)
        .output()
        .expect("run nose review")
}

#[test]
fn review_flags_a_clone_changed_in_one_copy_only() {
    let dir = make_project("review_flag");
    init_git_repo(&dir);

    // Edit ONE copy of the clone family (a/f.py) — a fix not propagated to b/f.py.
    let a = dir.join("a/f.py");
    let src = fs::read_to_string(&a).unwrap();
    fs::write(
        &a,
        src.replace(
            "    return total",
            "    total = total + 1\n    return total",
        ),
    )
    .unwrap();

    let out = nose_review(&dir, &[]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("changed inconsistently"),
        "should flag the inconsistently-changed clone: {stdout}"
    );
    assert!(
        stdout.contains("a/f.py"),
        "names the changed copy: {stdout}"
    );
    assert!(
        stdout.contains("b/f.py"),
        "lists the un-updated sibling: {stdout}"
    );

    // --fail turns it into a non-zero CI gate.
    let gated = nose_review(&dir, &["--fail"]);
    assert!(
        !gated.status.success(),
        "--fail should exit non-zero when flagged"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn review_is_quiet_when_a_clone_changes_consistently() {
    let dir = make_project("review_consistent");
    init_git_repo(&dir);

    // Apply the *same* edit to every copy — a consistent change, nothing to flag.
    for sub in ["a", "b", "tests"] {
        let f = dir.join(sub).join("f.py");
        let src = fs::read_to_string(&f).unwrap();
        fs::write(&f, src.replace("    return", "    pass\n    return")).unwrap();
    }

    let out = nose_review(&dir, &[]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.contains("not updated"),
        "a consistent change must not be flagged: {stdout}"
    );
    assert!(
        out.status.success(),
        "no --fail trip on a consistent change"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn review_needs_a_git_repository() {
    let dir = make_project("review_nogit");
    let out = Command::new(bin())
        .current_dir(&dir)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_OBJECT_DIRECTORY")
        .env_remove("GIT_COMMON_DIR")
        .args(["review", "."])
        .output()
        .expect("run nose review");
    assert!(!out.status.success(), "review must fail outside a git repo");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("git repository"),
        "explains the git requirement: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn review_respects_structured_ignores() {
    let dir = make_project("review_ignore");
    init_git_repo(&dir);

    // Edit one copy so a family is flagged.
    let a = dir.join("a/f.py");
    let src = fs::read_to_string(&a).unwrap();
    fs::write(
        &a,
        src.replace(
            "    return total",
            "    total = total + 1\n    return total",
        ),
    )
    .unwrap();

    // Grab the stable family_id from JSON, then suppress it.
    let json_out = nose_review(&dir, &["--format", "json"]);
    let json: serde_json::Value = serde_json::from_slice(&json_out.stdout).expect("review JSON");
    let findings = json["findings"].as_array().expect("findings");
    assert!(!findings.is_empty(), "expected a flagged family first");
    let fid = findings[0]["family_id"].as_str().unwrap();

    let ignore = dir.join("nose.ignore.json");
    fs::write(
        &ignore,
        format!(r#"{{"ignores":[{{"family_id":"{fid}","reason":"intentional"}}]}}"#),
    )
    .unwrap();

    let out = nose_review(&dir, &["--fail"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.contains("not updated"),
        "the ignored family must be suppressed: {stdout}"
    );
    assert!(
        out.status.success(),
        "a fully-suppressed review must not trip --fail"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn fail_on_new_requires_a_baseline() {
    let dir = make_project("fail_on_new_nobaseline");
    let p = dir.to_str().unwrap();
    // `--fail-on new` compares against a baseline; with no --baseline the gate can never
    // fire, so it must error rather than silently pass (a CI gate that always passes).
    let out = Command::new(bin())
        .args(["scan", p, "--min-size", "12", "--fail-on", "new"])
        .output()
        .expect("run scan");
    assert!(
        !out.status.success(),
        "`--fail-on new` without --baseline must error, not silently pass: stdout={} stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn c_u16_byte_pack_recognized_in_either_operand_order() {
    // The byte-pack idiom must be recognized whichever way its commutative operands sort by
    // value-hash. With the base at param 1 the shifted lane sorts second; a `+` form and a
    // `|` form then cluster into one Type-4 family only if both normalize to the byte-pack op.
    let dir = std::env::temp_dir().join(format!("nose_bytepack_order_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("add2.c"),
        "unsigned int add2(int d, const unsigned char *a) {\n  return (a[0] << 8) + a[1];\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("or2.c"),
        "unsigned int or2(int d, unsigned char *a) {\n  return (a[0] << 8) | a[1];\n}\n",
    )
    .unwrap();
    let out = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let json = scan_json(&out);
    let families = scan_families(&json);
    assert_eq!(
        families.len(),
        1,
        "byte-pack must be recognized in either operand order (+ and | should cluster): {out}"
    );
    let _ = fs::remove_dir_all(&dir);
}
