use super::*;

#[test]
fn value_census_attributes_opaque_to_constructs() {
    // #391 prevalence probe: `nose value-census` reports, per IL construct, how many value-graph
    // `Opaque` fallbacks were minted. JS `instanceof` is an unmodeled BinOp → a semantic opaque;
    // a plain numeric function mints none.
    let dir = std::env::temp_dir().join(format!("nose_vgcensus_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("a.js"),
        "function f(x, y) { return x instanceof y; }\nfunction g(a, b) { return a + b * 2; }\n",
    )
    .unwrap();
    let census: serde_json::Value =
        serde_json::from_str(&run(&["value-census", dir.to_str().unwrap()])).unwrap();
    assert_eq!(census["function_units"], 2, "two functions: {census}");
    assert_eq!(
        census["units_with_opaque"], 1,
        "only the instanceof function mints an opaque: {census}"
    );
    let has_binop_opaque = census["by_construct"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["construct"] == "BinOp" && r["opaque_nodes"].as_u64().unwrap_or(0) >= 1);
    assert!(
        has_binop_opaque,
        "instanceof attributes an opaque to the BinOp construct: {census}"
    );
}

#[test]
fn rust_binding_patterns_lower_without_raw() {
    // #390 lowering fidelity: a `match`/`if let`/`while let` test on a binding constructor
    // pattern (`Some(v)`, `Ok(v)`, `Point { x, y }`) used to lower the whole pattern to an
    // opaque Raw node. It now lowers to the constructor path, so stats reports no
    // tuple_struct_pattern / struct_pattern Raw for these sites.
    let dir = std::env::temp_dir().join(format!("nose_pat_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("p.rs"),
        "fn a(x: Option<i32>) -> i32 { match x { Some(v) => v + 1, None => 0 } }\n\
         fn b(x: Option<i32>) -> i32 { if let Some(v) = x { v } else { 0 } }\n\
         fn c(p: P) -> i32 { match p { Point { x, y } => x + y, _ => 0 } }\n",
    )
    .unwrap();
    let stats: serde_json::Value = serde_json::from_str(&run(&[
        "stats",
        dir.to_str().unwrap(),
        "--format",
        "json",
        "--top",
        "50",
    ]))
    .unwrap();
    let pattern_raw: u64 = stats["top_unhandled"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|u| {
            matches!(
                u["surface_kind"].as_str(),
                Some("tuple_struct_pattern" | "struct_pattern")
            )
        })
        .map(|u| u["count"].as_u64().unwrap())
        .sum();
    assert_eq!(
        pattern_raw, 0,
        "binding constructor patterns must not lower to Raw: {stats}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn query_reinvented_view_lists_call_the_helper_findings() {
    // A function that reimplements an existing pure helper inline (the reinvented channel),
    // surfaced as a query view — the action is "call the helper", complementing the clustered
    // `shape=call-existing-helper` families.
    let dir = std::env::temp_dir().join(format!("nose_reinv_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("a.js"),
        "function big(x, y) {\n    return ((x * 2 + 3) * (x - 4)) / ((x + 5) * (y - 7) + (y * y + 11))\n}\n\nfunction use(x, y) {\n    return big(x, y) + 1\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("b.js"),
        "function manual(x, y) {\n    return (((x * 2 + 3) * (x - 4)) / ((x + 5) * (y - 7) + (y * y + 11))) * 7\n}\n",
    )
    .unwrap();
    let p = dir.to_str().unwrap();
    let human = run(&[
        "query",
        p,
        "reinvented",
        "--min-size",
        "1",
        "--min-lines",
        "1",
    ]);
    assert!(
        human.contains("call big") && human.contains("reimplements an existing helper"),
        "reinvented view names the helper to call: {human}"
    );
    let j: serde_json::Value = serde_json::from_str(&run(&[
        "query",
        p,
        "reinvented",
        "--min-size",
        "1",
        "--min-lines",
        "1",
        "--format",
        "json",
    ]))
    .unwrap();
    assert_eq!(j["view"], "reinvented");
    assert_eq!(
        j["items"][0]["helper"]["name"], "big",
        "json names the helper: {j}"
    );
    assert_eq!(j["items"][0]["site"]["container"], "manual");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn query_reinvented_omits_prod_sites_that_only_match_test_helpers() {
    let dir = std::env::temp_dir().join(format!("nose_reinv_test_helper_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::create_dir_all(dir.join("tests")).unwrap();
    fs::write(
        dir.join("tests/redaction.test.js"),
        "function redactPayload(x, y) {\n    return ((x * 2 + 3) * (x - 4)) / ((x + 5) * (y - 7) + (y * y + 11))\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("src/worker.js"),
        "function buildPayload(x, y) {\n    return (((x * 2 + 3) * (x - 4)) / ((x + 5) * (y - 7) + (y * y + 11))) * 7\n}\n",
    )
    .unwrap();
    let p = dir.to_str().unwrap();
    let human = run(&[
        "query",
        p,
        "reinvented",
        "--min-size",
        "1",
        "--min-lines",
        "1",
    ]);
    assert!(
        human.contains("test-only helpers")
            && !human.contains("→ call redactPayload")
            && human.contains("rehome a helper before calling it from production"),
        "prod code must not be told to call a test helper: {human}"
    );
    let j: serde_json::Value = serde_json::from_str(&run(&[
        "query",
        p,
        "reinvented",
        "--min-size",
        "1",
        "--min-lines",
        "1",
        "--format",
        "json",
    ]))
    .unwrap();
    assert_eq!(
        j["summary"]["test_helper"], 1,
        "json counts omitted test-helper targets: {j}"
    );
    assert!(
        j["items"].as_array().unwrap().is_empty(),
        "json does not surface unsafe call-it action items: {j}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
#[allow(clippy::too_many_lines)] // one end-to-end walk of the since= temporal lens
fn query_since_status_classifies_against_a_snapshot() {
    // Family X (3 near copies of `process`, one operator each) exists at snapshot time; a
    // structurally distinct family Y (3 exact `banner` copies) is added after — so `since=`
    // grades X unchanged and Y new. (Y must be distinct: a near-identical Y would *merge*
    // into X, which would correctly read as `changed`, not a second family.)
    let dir = std::env::temp_dir().join(format!("nose_since_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    for d in ["a", "b", "c", "d", "e", "f"] {
        fs::create_dir_all(dir.join(d)).unwrap();
    }
    let process = |op: &str| {
        format!(
            "def process(items):\n    total = 0\n    count = 0\n    best = None\n    for it in items:\n        v = it.value\n        total = total {op} v\n        count = count + 1\n        if best is None or v > best:\n            best = v\n    return total, count, best\n"
        )
    };
    fs::write(dir.join("a/m.py"), process("+")).unwrap();
    fs::write(dir.join("b/m.py"), process("*")).unwrap();
    fs::write(dir.join("c/m.py"), process("-")).unwrap();
    let p = dir.to_str().unwrap();
    let bl = dir.join("base.json");
    let bls = bl.to_str().unwrap();

    // Snapshot the current state (only family X).
    run(&["query", p, "--baseline", bls, "--write-baseline"]);

    // Add a structurally distinct family Y (exact copies) after the snapshot.
    let banner = "def banner(title):\n    line = \"=\" * 40\n    print(line)\n    print(\"  \" + title)\n    print(line)\n    print(\"done\")\n    return title\n";
    fs::write(dir.join("d/n.py"), banner).unwrap();
    fs::write(dir.join("e/n.py"), banner).unwrap();
    fs::write(dir.join("f/n.py"), banner).unwrap();

    // `status` without `since=` is a hard error (the field is unresolvable).
    assert!(
        run_fail(&["query", p, "status=new"]).contains("needs a snapshot"),
        "status without since= errors"
    );

    // group=status facets the diff against the snapshot.
    let grouped = run(&["query", p, &format!("since={bls}"), "group=status"]);
    assert!(
        grouped.contains("by status"),
        "group=status facets the snapshot diff: {grouped}"
    );

    // status=new keeps the family added after the snapshot; the JSON family carries status.
    let newj: serde_json::Value = serde_json::from_str(&run(&[
        "query",
        p,
        &format!("since={bls}"),
        "status=new",
        "--format",
        "json",
    ]))
    .unwrap();
    let newfams = newj["families"].as_array().unwrap();
    assert!(
        !newfams.is_empty() && newfams.iter().all(|f| f["status"] == "new"),
        "status=new selects only new families, each tagged: {newj}"
    );
    // The new family is Y (the post-snapshot `banner` family in d/e/f — byte-identical copies,
    // so the `exact` witness), not the snapshotted `process` family (the `similar` witness in a/b/c).
    assert!(
        newfams.iter().all(|f| f["witness"] == "exact"
            && f["locations"][0]["file"].as_str().unwrap().contains("/d/")),
        "the post-snapshot family (Y) is flagged new, not the snapshotted X: {newj}"
    );

    // status=unchanged keeps the snapshotted family X (`process`, the `similar` witness).
    let unchanged: serde_json::Value = serde_json::from_str(&run(&[
        "query",
        p,
        &format!("since={bls}"),
        "status=unchanged",
        "--format",
        "json",
    ]))
    .unwrap();
    assert!(
        unchanged["families"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["locations"][0]["name"] == "process" && f["status"] == "unchanged"),
        "the snapshotted family grades unchanged: {unchanged}"
    );

    let _ = fs::remove_dir_all(&dir);
}
