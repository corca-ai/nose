use super::*;

#[test]
fn query_top_zero_shows_all_families() {
    // Regression for the v0.10.0 query-subsumes-query gap: `top=0` must mean *all* (matching
    // `query --top 0`, and the `top: Some(0)` the dataset build already uses for "every
    // family"), not "zero rows". The display paths previously truncated it to an empty set.
    let dir = make_mode_project("top_zero");
    let p = dir.to_str().unwrap();
    let count = |term: &str| -> usize {
        let v: serde_json::Value = serde_json::from_str(&run(&[
            "query",
            p,
            "all",
            term,
            "--min-size",
            "1",
            "--min-lines",
            "1",
            "--format",
            "json",
        ]))
        .unwrap();
        v["families"].as_array().unwrap().len()
    };
    let total = count("top=999");
    assert!(
        total >= 2,
        "fixture should surface multiple families (got {total})"
    );
    assert_eq!(
        count("top=0"),
        total,
        "query top=0 shows ALL families, like query --top 0"
    );
    assert_eq!(count("top=1"), 1, "a finite top still truncates");
}

#[test]
fn query_accepts_explicit_multi_roots() {
    let dir = make_project("multi_roots");
    let a = dir.join("a");
    let b = dir.join("b");
    let a = a.to_str().unwrap();
    let b = b.to_str().unwrap();

    let out = run_raw(&[
        "query",
        "-r",
        a,
        "-r",
        b,
        "all",
        "top=0",
        "--mode",
        "semantic",
        "--min-size",
        "1",
        "--min-lines",
        "1",
        "--format",
        "json",
    ]);
    let json = query_json(&out);
    assert_eq!(json["view"], "list");
    assert_eq!(json["path"], format!("-r {a} -r {b}"));
    assert!(
        family_contains_all(&json, &["a/f.py", "b/f.py"]),
        "multi-root query should analyze both explicit roots: {json}"
    );

    let dash = run(&[
        "query",
        "-r",
        a,
        "-r",
        b,
        "--mode",
        "semantic",
        "--min-size",
        "1",
        "--min-lines",
        "1",
    ]);
    assert!(
        dash.contains(&format!("nose query -r {a} -r {b} id=")),
        "multi-root drill links should remain runnable: {dash}"
    );
}

#[test]
fn query_second_path_suggests_explicit_roots() {
    let dir = make_project("second_path_hint");
    let a = dir.join("a");
    let b = dir.join("b");
    let err = run_fail(&["query", a.to_str().unwrap(), b.to_str().unwrap()]);
    assert!(
        err.contains("looks like another path") && err.contains("nose query -r"),
        "second positional path should explain explicit multi-root syntax: {err}"
    );

    let explicit_err = run_fail(&["query", "-r", a.to_str().unwrap(), b.to_str().unwrap()]);
    assert!(
        explicit_err.contains("When using `--root`/`-r`")
            && explicit_err.contains("bare arguments are query terms"),
        "bare path after --root should explain that all roots need -r: {explicit_err}"
    );
}

#[test]
fn query_json_grades_async_mirror_shared_core_families() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../bench/type4/fixtures/async_sync/async_sync");

    let json = query_json(&run(&[
        "query",
        fixture.to_str().unwrap(),
        "all",
        "spotclass=structural",
        "--mode",
        "near",
        "--min-size",
        "1",
        "--min-lines",
        "1",
        "--format",
        "json",
    ]));
    let families = json["families"]
        .as_array()
        .expect("query JSON should carry a families array");
    let async_mirror_subdag: Vec<_> = families
        .iter()
        .filter(|family| {
            family["witness"] == "subdag"
                && family["spotclass"] == "structural"
                && family["graded"]["equal_modulo_holes"] == false
                && json_array_strings(&family["graded"], "patterns").contains(&"async-mirror")
        })
        .collect();
    for family in &async_mirror_subdag {
        let locations = family["locations"].as_array().unwrap();
        let pair = &family["graded_pair"];
        let a_idx = pair["a_index"].as_u64().unwrap() as usize;
        let b_idx = pair["b_index"].as_u64().unwrap() as usize;
        let a = &locations[a_idx];
        let b = &locations[b_idx];
        assert_eq!(pair["a_member_id"], a["id"]);
        assert_eq!(pair["b_member_id"], b["id"]);
        let a_file = a["file"].as_str().unwrap();
        let b_file = b["file"].as_str().unwrap();
        assert_ne!(a_idx, b_idx);
        assert!(
            a_file != b_file
                && (a_file.ends_with(".py") || a_file.ends_with(".ts"))
                && (b_file.ends_with(".py") || b_file.ends_with(".ts")),
            "graded_pair should identify the concrete representatives behind the witness: {family}"
        );
    }
    assert_eq!(
        async_mirror_subdag.len(),
        5,
        "all shared-core async/sync fixture families should carry async-mirror evidence: {json}"
    );
}

#[test]
#[allow(clippy::too_many_lines, clippy::cognitive_complexity)] // one end-to-end walk of the query surface
fn query_dashboard_filter_and_family() {
    // A sizeable 3-copy near family (one call target each) survives the default size floor.
    let dir = std::env::temp_dir().join(format!("nose_query_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    for d in ["a", "b", "c"] {
        fs::create_dir_all(dir.join(d)).unwrap();
    }
    let mk = |combine: &str| {
        format!(
            "def process(items):\n    total = 0\n    count = 0\n    best = None\n    for it in items:\n        v = it.value\n        total = {combine}(total, v)\n        count = count + 1\n        if best is None or v > best:\n            best = v\n    avg = total / count\n    return total, count, best, avg\n"
        )
    };
    fs::write(dir.join("a/m.py"), mk("adjust")).unwrap();
    fs::write(dir.join("b/m.py"), mk("scale")).unwrap();
    fs::write(dir.join("c/m.py"), mk("offset")).unwrap();
    let p = dir.to_str().unwrap();

    // Dashboard: self-describing, with a real candidate carrying a runnable drill link.
    let dash = run(&["query", p]);
    assert!(
        // The count noun is pluralized ("1 duplicated-code family" / "N … families"),
        // so match the stem rather than a fixed plural.
        dash.contains("duplicated-code famil"),
        "dashboard names the dataset: {dash}"
    );
    // Colour is TTY-gated: piped output (as the test harness captures it) must be plain ASCII
    // with no ANSI escape sequences, so the JSON contract and these string checks stay stable.
    assert!(
        !dash.contains('\u{1b}'),
        "piped human output must carry no ANSI colour codes: {dash:?}"
    );
    assert!(
        dash.contains("nose query"),
        "dashboard teaches the grammar: {dash}"
    );
    assert!(
        dash.contains("verified = machine-checked evidence")
            && !dash.contains("proven = same behavior, machine-verified")
            && !dash.contains("proven families (same behavior"),
        "dashboard must not flatten exact and shared-core evidence: {dash}"
    );
    // Suggested commands echo the path so they're runnable verbatim (the surface takes
    // the path positionally) — every drill link is `nose query <path> id=…`.
    assert!(
        dash.contains(&format!("nose query {p} id=")),
        "dashboard's drill links carry the path: {dash}"
    );

    // A filter narrows to a ranked list; a facet groups it. (The count noun is
    // pluralized — "1 family" / "N families" — so match the stem.)
    assert!(run(&["query", p, "members>1"]).contains("famil"));
    assert!(
        run(&["query", p, "group=dir"]).contains("famil")
            && run(&["query", p, "group=dir"]).contains("by dir")
    );

    // An unknown term is a hard error (a typo must not silently widen the result).
    assert!(run_fail(&["query", p, "wat"]).contains("unrecognized term"));

    // Negation (`!~`): a path substring matched by every copy drops the family; a
    // non-matching one keeps it (so a typo'd exclusion can't silently empty the result).
    let excluded = run(&["query", p, "path!~m.py"]);
    assert!(
        excluded.contains("0 families") || !excluded.contains("nose query"),
        "path!~m.py excludes the all-m.py family"
    );
    let excluded_gate = run(&["query", p, "path!~m.py", "--fail-on", "any"]);
    assert!(
        excluded_gate.contains("0 families") || !excluded_gate.contains("nose query"),
        "query --fail-on any gates the filtered selection, not hidden families"
    );
    assert!(
        run(&["query", p, "path!~zzz_absent"]).contains(&format!("nose query {p} id=")),
        "path!~<absent> keeps the family"
    );
    // Negated equality still validates the value (a typo errors, never silently matches).
    assert!(run_fail(&["query", p, "witness!=nonsense"]).contains("unknown witness value"));

    // Set-membership OR: comma is "any of". The fixture family is `similar`, so a set that
    // includes `similar` keeps it and one that excludes it drops it — and a typo in any
    // comma-part errors (never silently narrows the set).
    assert!(
        run(&["query", p, "witness=exact,similar", "--mode", "near"])
            .contains(&format!("nose query {p} id=")),
        "witness=exact,similar matches the similar fixture (OR)"
    );
    let none = run(&["query", p, "witness=exact,copy-paste", "--mode", "near"]);
    assert!(
        none.contains("0 families") || !none.contains(&format!("nose query {p} id=")),
        "a set without `similar` excludes the only family"
    );
    assert!(
        run(&["query", p, "witness!=exact,copy-paste", "--mode", "near"])
            .contains(&format!("nose query {p} id=")),
        "witness!=<set> keeps a family outside the set"
    );
    assert!(
        run_fail(&["query", p, "witness=similar,bogus", "--mode", "near"])
            .contains("unknown witness value")
    );

    // `at=FILE:LINE` opens the family whose copy covers that location; a miss errors loudly.
    let at = run(&["query", p, &format!("at={p}/a/m.py:3")]);
    assert!(
        at.contains("copies:") && at.contains("shared"),
        "at= opens the covering family: {at}"
    );
    assert!(
        run_fail(&["query", p, "at=nope.rs:999"]).contains("no family has a copy covering"),
        "a location with no family errors"
    );

    // same_symbol: the three `process` copies share a name → the parallel-variant signal.
    assert!(
        run(&["query", p, "same_symbol=true", "--mode", "near"])
            .contains(&format!("nose query {p} id=")),
        "same_symbol=true matches the same-named family"
    );
    assert!(
        run(&["query", p, "group=same_symbol", "--mode", "near"]).contains("by same_symbol"),
        "group=same_symbol facets"
    );
    assert!(
        run_fail(&["query", p, "same_symbol=oops", "--mode", "near"])
            .contains("unknown same_symbol value")
    );

    // query takes query's analysis flags (dataset parity): `--min-members 99` floors out the
    // 3-copy family, and `--mode syntax` is accepted (different channel mix).
    assert!(
        run(&["query", p, "--min-members", "99"]).contains("0 duplicated-code families"),
        "query respects --min-members"
    );
    assert!(
        run(&["query", p, "--mode", "syntax"]).contains("duplicated-code famil"),
        "query accepts --mode"
    );

    // query gates like query: `--fail-on any` exits non-zero on a reported family, and
    // `--fail-on new` without `--baseline` errors (parity with query).
    assert!(
        run_fail(&["query", p, "--fail-on", "any"]).contains("--fail-on any"),
        "query --fail-on any fires on a reported family"
    );
    assert!(
        run_fail(&["query", p, "--fail-on", "new"]).contains("requires --baseline"),
        "query --fail-on new requires --baseline"
    );

    // The JSON form is the structured, versioned query-v7 contract (every view).
    let dash: serde_json::Value =
        serde_json::from_str(&run_raw(&["query", p, "--format", "json"])).unwrap();
    assert_eq!(
        dash["schema_version"], 7,
        "dashboard json is schema v7: {dash}"
    );
    assert_eq!(dash["view"], "dashboard");
    assert_query_json_reports_semantic_packs(&dash);
    assert!(dash["summary"]["families"].is_number());
    assert!(
        dash["families"].is_array() && dash["top_candidates"].is_array(),
        "dashboard json exposes the family array under the stable `families` key: {dash}"
    );
    assert_eq!(
        dash["families"], dash["top_candidates"],
        "dashboard keeps top_candidates as a compatibility alias for families: {dash}"
    );
    let dashboard_count = dash["summary"]["families"].as_u64().unwrap();
    let gate = run_fail(&["query", p, "--fail-on", "any"]);
    assert!(
        gate.contains(&format!("nose: {dashboard_count} ")),
        "--fail-on any count must match the dashboard default-surface count: {gate}"
    );
    // A filtered list emits structured family objects (not human `where` strings).
    let list: serde_json::Value =
        serde_json::from_str(&run(&["query", p, "members>1", "--format", "json"])).unwrap();
    assert_eq!(list["view"], "list");
    assert_query_json_reports_semantic_packs(&list);
    let grouped: serde_json::Value =
        serde_json::from_str(&run(&["query", p, "group=dir", "--format", "json"])).unwrap();
    assert_eq!(grouped["view"], "group");
    assert_query_json_reports_semantic_packs(&grouped);
    let fam = &list["families"][0];
    for k in [
        "id",
        "scope",
        "witness",
        "members",
        "shared",
        "params",
        "removable",
        "locations",
        "extraction_shape",
        "same_symbol",
    ] {
        assert!(!fam[k].is_null(), "query-json family.{k} present: {list}");
    }
    // `full` carries the all-copies extraction skeleton on the family object.
    let famid = fam["id"].as_str().unwrap().to_string();
    let opened: serde_json::Value = serde_json::from_str(&run(&[
        "query",
        p,
        &format!("id={famid}"),
        "full",
        "--format",
        "json",
    ]))
    .unwrap();
    assert_eq!(opened["view"], "family");
    assert_query_json_reports_semantic_packs(&opened);
    assert!(
        opened["family"]["skeleton"].is_array(),
        "id=…full json carries skeleton: {opened}"
    );

    // spotclass (#374 item 2): the near family's only varying spot is a call target — a clean
    // value-leaf — so it grades `leaf-only`. Grouping/filtering by spotclass triggers the
    // on-demand graded-witness enrichment (skipped on the common path for cost).
    assert!(
        run(&["query", p, "group=spotclass", "--mode", "near"]).contains("by spotclass"),
        "group=spotclass facets the near families"
    );
    let leaf: serde_json::Value = serde_json::from_str(&run(&[
        "query",
        p,
        "spotclass=leaf-only",
        "--mode",
        "near",
        "--format",
        "json",
    ]))
    .unwrap();
    assert_eq!(
        leaf["families"][0]["spotclass"], "leaf-only",
        "the operator-varying near family grades leaf-only: {leaf}"
    );
    assert!(run_fail(&["query", p, "spotclass=bogus"]).contains("unknown spotclass value"));

    // Report formats (#374 + query parity): query reuses query's markdown and SARIF formatters
    // over the selected family set.
    let md = run(&["query", p, "--format", "markdown"]);
    assert!(
        md.contains("duplicated lines"),
        "markdown report has the header: {md}"
    );
    // #422: the bulk markdown report stays a compact location list (no per-family skeleton)…
    assert!(
        !md.contains("**proposal**"),
        "bulk markdown stays compact — no extraction skeleton: {md}"
    );
    // …but `id=<fam>` drills into one family and renders the extraction skeleton, and `full`
    // adds the representative diff — so markdown composes with `id=`/`full` like human/JSON.
    let fid = {
        let j: serde_json::Value =
            serde_json::from_str(&run(&["query", p, "top=0", "--format", "json"])).unwrap();
        j["families"][0]["id"].as_str().unwrap().to_string()
    };
    let drill = run(&["query", p, &format!("id={fid}"), "--format", "markdown"]);
    assert!(
        drill.contains("**proposal**") && drill.contains("⟨param"),
        "id=<fam> markdown renders the extraction skeleton with parameter slots: {drill}"
    );
    assert!(
        !drill.contains("**diff**"),
        "id=<fam> without `full` omits the representative diff: {drill}"
    );
    let drill_full = run(&[
        "query",
        p,
        &format!("id={fid}"),
        "full",
        "--format",
        "markdown",
    ]);
    assert!(
        drill_full.contains("**proposal**") && drill_full.contains("**diff**"),
        "id=<fam> full markdown adds the representative diff: {drill_full}"
    );
    let sarif: serde_json::Value =
        serde_json::from_str(&run(&["query", p, "--format", "sarif"])).unwrap();
    assert_eq!(
        sarif["version"], "2.1.0",
        "query emits a SARIF 2.1.0 doc: {sarif}"
    );
    assert!(
        sarif["runs"][0]["results"].is_array(),
        "SARIF has a results array: {sarif}"
    );

    // group= is a hotspot map: each bucket carries summed removable lines and ranks by them,
    // and `file` is a group key (object-model PR).
    assert!(
        run(&["query", p, "group=dir"]).contains("removable"),
        "group=dir aggregates removable economics per bucket"
    );
    let byfile: serde_json::Value =
        serde_json::from_str(&run(&["query", p, "group=file", "--format", "json"])).unwrap();
    assert_eq!(byfile["field"], "file");
    assert!(
        byfile["groups"][0]["removable"].is_number(),
        "group buckets carry removable: {byfile}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn query_cross_language_rows_show_repeated_volume_not_zero_removable() {
    let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let examples = examples.to_str().unwrap();
    let human = run(&["query", examples]);
    assert!(
        human.contains("cross-language · ~30 repeated")
            && !human.contains("0/7 shared, 0p · ~0 removable"),
        "cross-language rows should not look like zero-removal same-language extracts: {human}"
    );
    let dashboard: serde_json::Value =
        serde_json::from_str(&run_raw(&["query", examples, "--format", "json"])).unwrap();
    let family = &dashboard["families"][0];
    assert_eq!(
        family["source_comparable"], false,
        "json marks the basis: {dashboard}"
    );
    assert_eq!(
        family["removable"], 30,
        "cross-language removable carries repeated source volume, not zero: {dashboard}"
    );
}
