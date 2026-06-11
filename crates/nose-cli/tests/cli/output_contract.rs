use super::*;

#[test]
fn checked_in_scan_json_v1_example_matches_contract() {
    let json = scan_json(include_str!("../fixtures/scan-json-v1.json"));
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
    assert_eq!(json["ranking"]["surface_counts"]["default"], 1);
    assert_eq!(json["ranking"]["surface_counts"]["review"], 0);
    assert_eq!(json["ranking"]["surface_counts"]["hidden"], 0);
    assert_eq!(json["ranking"]["surface_counts"]["fragments"]["total"], 0);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_json_exposes_exact_fragment_metadata() {
    let dir = make_fragment_project("json");

    let out = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--format",
        "json",
        "--top",
        "0",
        "--min-size",
        "1",
    ]);
    let json = scan_json(&out);
    assert_eq!(json["ranking"]["surface_counts"]["default"], 0);
    assert_eq!(json["ranking"]["surface_counts"]["hidden"], 1);
    assert_eq!(json["ranking"]["surface_counts"]["fragments"]["total"], 1);
    assert_eq!(json["ranking"]["surface_counts"]["fragments"]["hidden"], 1);
    let family = scan_families(&json)
        .iter()
        .find(|family| family["recommended_surface"] == "hidden")
        .expect("tiny exact fragment family should be present");
    assert_eq!(family["mean_lines"], 2);
    let locs = family["locations"].as_array().expect("locations");
    assert_eq!(locs.len(), 2);
    for loc in locs {
        assert_eq!(loc["kind"], "Block");
        assert_eq!(loc["span_lines"], 2);
        assert!(loc["span_tokens"].as_u64().is_some_and(|n| n > 0));
        assert_eq!(loc["is_fragment"], true);
        assert_eq!(loc["fragment_kind"], "conditional-guard");
        assert_eq!(loc["reason_code"], "exact-conditional-guard");
        assert!(
            loc.get("proof_facts").is_none(),
            "proof facts must not be stable scan JSON: {loc}"
        );
        let parent = &loc["enclosing_unit"];
        assert_eq!(parent["kind"], "Function");
        assert!(parent["name"]
            .as_str()
            .is_some_and(|name| { name == "first" || name == "second" }));
        assert!(parent["unit_key"]
            .as_str()
            .is_some_and(|key| { key.contains(":Function:1-5:") }));
    }

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_human_hides_hidden_exact_fragments() {
    let dir = make_fragment_project("human_hidden");

    let out = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-size",
        "1",
    ]);
    assert!(
        out.contains("no semantic clone families found"),
        "hidden proof fragments should not be top-level human findings: {out}"
    );
    assert!(
        out.contains("omitted 1 hidden proof-only family"),
        "human report should explain the omitted diagnostic family: {out}"
    );
    assert!(
        !out.contains("a/f.py") && !out.contains("b/f.py"),
        "hidden proof fragments must not expose report locations: {out}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn fail_on_any_ignores_hidden_exact_fragments() {
    let dir = make_fragment_project("fail_hidden");

    let out = Command::new(bin())
        .args([
            "scan",
            dir.to_str().unwrap(),
            "--mode",
            "semantic",
            "--min-size",
            "1",
            "--fail-on",
            "any",
        ])
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "hidden proof-only fragments should not trip the default CI gate: stdout={} stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let _ = fs::remove_dir_all(&dir);
}

/// #222: every reported family carries an equivalence witness naming WHY its
/// members merged — an exact value-graph proof is distinguishable from surface
/// similarity without opening any source file.
#[test]
fn scan_json_families_carry_equivalence_witness() {
    let dir = std::env::temp_dir().join(format!("nose_witness_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    // Exact pair: identical pure functions (strict-exact-safe, above the size floor).
    fs::write(
        dir.join("a.py"),
        "def total(xs, lo):\n    out = 0\n    for x in xs:\n        if x > lo:\n            out += x\n    return out\n",
    )
    .unwrap();
    fs::write(
        dir.join("b.py"),
        "def total_again(values, floor):\n    out = 0\n    for v in values:\n        if v > floor:\n            out += v\n    return out\n",
    )
    .unwrap();

    let out = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let json = scan_json(&out);
    let fams = scan_families(&json);
    assert_eq!(fams.len(), 1, "exact twin pair expected: {out}");
    let witness = &fams[0]["witness"];
    assert_eq!(
        witness["kind"], "exact-value-graph",
        "exact merge must carry the exact witness: {out}"
    );
    assert!(
        witness["value_nodes"].as_u64().unwrap_or(0) >= 4,
        "witness carries the shared multiset size: {out}"
    );
}

/// #223: families expose WHAT differs — each varying spot with per-side line
/// ranges and text, consistent with `params` (same representative pair) — so a
/// data-table family is classifiable from JSON alone.
#[test]
fn scan_json_families_carry_varying_spot_diff_evidence() {
    let dir = std::env::temp_dir().join(format!("nose_spots_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("a.py"),
        "def msg_ko(name, n):\n    head = \"KO-HEAD \" + name\n    body = \"KO-BODY \" + str(n)\n    return head + body\n\n\ndef msg_en(name, n):\n    head = \"EN-HEAD \" + name\n    body = \"EN-BODY \" + str(n)\n    return head + body\n",
    )
    .unwrap();

    let out = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "near",
        "--min-lines",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let json = scan_json(&out);
    let fams = scan_families(&json);
    assert_eq!(fams.len(), 1, "literal-varied twin pair expected: {out}");
    let spots = fams[0]["varying_spots"]
        .as_array()
        .unwrap_or_else(|| panic!("varying_spots expected: {out}"));
    assert!(!spots.is_empty(), "at least one spot: {out}");
    let text = spots
        .iter()
        .map(|s| format!("{} {}", s["a_text"], s["b_text"]))
        .collect::<String>();
    assert!(
        text.contains("KO-HEAD") && text.contains("EN-HEAD"),
        "spot text shows the differing literals: {out}"
    );
    assert_eq!(
        fams[0]["params"].as_u64().unwrap_or(0),
        spots.len() as u64,
        "spots and params come from the same representative pair: {out}"
    );
}

/// #224: a family that is generated end-to-end (file-head marker) leaves the
/// default surface as `recommended_surface: "generated"`; a partly-generated
/// family stays default with its generated members flagged — hand-written
/// logic leaking into generated output is a real finding.
#[test]
fn scan_surfaces_generated_families_off_default_and_flags_partial_ones() {
    let dir = std::env::temp_dir().join(format!("nose_gen_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let body = "def pick(xs, lo):\n    out = 0\n    for x in xs:\n        if x > lo:\n            out += x\n    return out\n";
    fs::write(
        dir.join("gen_a.py"),
        format!("# Code generated by mktool. DO NOT EDIT.\n{body}"),
    )
    .unwrap();
    fs::write(
        dir.join("gen_b.py"),
        format!(
            "# Code generated by mktool. DO NOT EDIT.\n{}",
            body.replace("pick", "pick_again")
        ),
    )
    .unwrap();
    fs::write(dir.join("hand.py"), body.replace("pick", "pick_by_hand")).unwrap();

    let out = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let json = scan_json(&out);
    let fams = scan_families(&json);
    // The three copies form ONE exact family; it stays on the default surface
    // because one member is hand-written, and the generated members carry the flag.
    assert_eq!(fams.len(), 1, "partly-generated family is kept: {out}");
    assert_eq!(
        fams[0]["recommended_surface"], "default",
        "a partly-generated family stays actionable: {out}"
    );
    let gen_flags: Vec<bool> = fams[0]["locations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["looks_generated"].as_bool().unwrap_or(false))
        .collect();
    assert_eq!(
        gen_flags.iter().filter(|&&g| g).count(),
        2,
        "both generated members flagged: {out}"
    );

    // Remove the hand-written copy: the family becomes all-generated and leaves
    // the default surface (`recommended_surface: "generated"`), matching the
    // families the human report omits from default output.
    fs::remove_file(dir.join("hand.py")).unwrap();
    let out = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let json = scan_json(&out);
    let fams = scan_families(&json);
    assert_eq!(
        fams.len(),
        1,
        "all-generated family stays visible in JSON: {out}"
    );
    assert_eq!(
        fams[0]["recommended_surface"], "generated",
        "all-generated family must leave the default surface: {out}"
    );
    assert_eq!(
        json["ranking"]["surface_counts"]["generated"].as_u64(),
        Some(1),
        "surface_counts accounts the generated family: {out}"
    );
}

/// #225: plain Block locations carry their enclosing function/method, so an
/// agent can NAME the region a block family lives in without opening files.
#[test]
fn scan_json_block_locations_carry_enclosing_unit() {
    let dir = std::env::temp_dir().join(format!("nose_encl_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    // Two DIFFERENT functions sharing one identical inner block: the whole
    // functions must not merge (different surrounding statements), so the
    // family's reported members are the Block units — which must name their
    // host functions.
    let block = "    for x in items:\n        if x > 100:\n            out.append(x * 2 + 7)\n        elif x > 50:\n            out.append(x * 3 + 11)\n        elif x > 10:\n            out.append(x * 5 + 13)\n        else:\n            out.append(x - 17)\n";
    fs::write(
        dir.join("a.py"),
        format!("def host_one(items, label):\n    out = []\n    print(\"first\" + label)\n{block}    return out\n"),
    )
    .unwrap();
    fs::write(
        dir.join("b.py"),
        format!("def host_two(items, n):\n    out = []\n    total = n * 3\n{block}    return (out, total)\n"),
    )
    .unwrap();

    // Pin the exact surface: with `near` in the default mix the two host
    // functions also merge as one near family, which subsumes the Block
    // members this contract is about.
    let out = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "syntax,semantic",
        "--min-lines",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let json = scan_json(&out);
    let fams = scan_families(&json);
    let mut named = 0;
    for f in fams {
        for l in f["locations"].as_array().unwrap() {
            if l["kind"] == "Block" {
                let host = l["enclosing_unit"]["name"].as_str().unwrap_or("");
                assert!(
                    host.starts_with("host_"),
                    "block location must name its enclosing function: {out}"
                );
                named += 1;
            }
        }
    }
    assert!(named >= 2, "expected named block locations: {out}");
}

/// #226: Rust keeps tests inside production files — units under an inline
/// `mod tests` must classify as test scope (the path heuristic alone tagged
/// them `prod`), and their locations carry `in_test_module`.
#[test]
fn rust_inline_test_module_families_classify_as_test_scope() {
    let dir = std::env::temp_dir().join(format!("nose_modtests_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let body = "        let mut grid = vec![0i64; 16];\n        grid[0] = 1;\n        grid[1] = 2;\n        grid[2] = 3;\n        assert_eq!(grid[0] + grid[1] + grid[2], 6);\n";
    fs::write(
        dir.join("engine.rs"),
        format!(
            "pub fn run(x: i64) -> i64 {{\n    x + 1\n}}\n\n#[cfg(test)]\nmod tests {{\n    #[test]\n    fn first() {{\n{body}    }}\n\n    #[test]\n    fn second() {{\n{body}    }}\n}}\n"
        ),
    )
    .unwrap();

    let out = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--min-lines",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let json = scan_json(&out);
    let fams = scan_families(&json);
    assert!(
        !fams.is_empty(),
        "twin test bodies should form a family: {out}"
    );
    for f in fams {
        assert_eq!(
            f["scope"], "test",
            "inline mod-tests units must classify as test scope: {out}"
        );
        for l in f["locations"].as_array().unwrap() {
            assert_eq!(
                l["in_test_module"], true,
                "locations carry the in_test_module flag: {out}"
            );
        }
    }
}
