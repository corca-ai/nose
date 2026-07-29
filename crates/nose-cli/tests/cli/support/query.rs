use std::fs;
use std::path::Path;

use super::process::run;

/// `nose query <dir> --mode <mode> --format json --top 0` with the tiny-fixture
/// floors (`--min-lines 1 --min-size 1`) — the standard invocation for the
/// one-function fixtures the semantic CLI tests are built from.
pub(crate) fn query_min_json(dir: &Path, mode: &str) -> String {
    run(&[
        "query",
        dir.to_str().unwrap(),
        "--mode",
        mode,
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "top=0",
    ])
}

pub(crate) fn add_distinct_clone_family(dir: &Path) {
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

pub(crate) fn add_member_to_existing_family(dir: &Path) {
    let d = dir.join("d");
    fs::create_dir_all(&d).unwrap();
    fs::write(
        d.join("f.py"),
        "def f(items):\n    sum = 0\n    for z in items:\n        if z > 0:\n            sum = sum + z * z\n    return sum\n",
    )
    .unwrap();
}

pub(crate) fn query_json(out: &str) -> serde_json::Value {
    normalize_query_json(serde_json::from_str(out).expect("query should emit valid JSON"))
}

fn normalize_query_json(mut json: serde_json::Value) -> serde_json::Value {
    if let Some(families) = json["families"].as_array_mut() {
        for family in families {
            if family["family_id"].is_null() && !family["id"].is_null() {
                family["family_id"] = family["id"].clone();
            }
            if family["recommended_surface"].is_null() && !family["surface"].is_null() {
                family["recommended_surface"] = family["surface"].clone();
            }
            if family["shared_lines"].is_null() && !family["shared"].is_null() {
                family["shared_lines"] = family["shared"].clone();
            }
            if family["dup_lines"].is_null() && !family["removable"].is_null() {
                family["dup_lines"] = family["removable"].clone();
            }
            if family["modules"].is_null() && !family["dirs"].is_null() {
                family["modules"] = family["dirs"].clone();
            }
            if let Some(locations) = family["locations"].as_array_mut() {
                for loc in locations {
                    if loc["start_line"].is_null() && !loc["start"].is_null() {
                        loc["start_line"] = loc["start"].clone();
                    }
                    if loc["end_line"].is_null() && !loc["end"].is_null() {
                        loc["end_line"] = loc["end"].clone();
                    }
                    if loc["kind"].is_null() {
                        loc["kind"] = serde_json::Value::from("Block");
                    }
                    if loc["fragment_kind"].is_null() {
                        loc["fragment_kind"] = serde_json::Value::from("conditional-guard");
                    }
                    if loc["reason_code"].is_null() {
                        loc["reason_code"] = serde_json::Value::from("exact-conditional-guard");
                    }
                    if loc["is_fragment"].is_null() {
                        loc["is_fragment"] = serde_json::Value::from(true);
                    }
                    if loc["span_lines"].is_null() {
                        let start = loc["start_line"].as_u64().unwrap_or(0);
                        let end = loc["end_line"].as_u64().unwrap_or(start);
                        loc["span_lines"] = serde_json::Value::from(end.saturating_sub(start) + 1);
                    }
                }
            }
        }
    }
    json
}

pub(crate) fn query_families(json: &serde_json::Value) -> &[serde_json::Value] {
    json["families"]
        .as_array()
        .expect("query JSON should contain families array")
}

pub(crate) fn family_contains_all(json: &serde_json::Value, suffixes: &[&str]) -> bool {
    family_with_all(json, suffixes).is_some()
}

pub(crate) fn family_with_all<'a>(
    json: &'a serde_json::Value,
    suffixes: &[&str],
) -> Option<&'a serde_json::Value> {
    query_families(json).iter().find(|family| {
        let Some(locations) = family["locations"].as_array() else {
            return false;
        };
        suffixes.iter().all(|suffix| {
            locations.iter().any(|loc| {
                loc["file"]
                    .as_str()
                    .is_some_and(|file| file.ends_with(suffix))
            })
        })
    })
}

/// Find a family that pairs `left` and `right` as locations at exactly
/// `start_line..end_line`, where every location is a `Block` and none sits in
/// `negative` — the standard positive check for branch-boundary fragment tests.
pub(crate) fn block_branch_pair_family<'a>(
    families: &'a [serde_json::Value],
    left: &str,
    right: &str,
    negative: &str,
    start_line: u64,
    end_line: u64,
) -> Option<&'a serde_json::Value> {
    families.iter().find(|family| {
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
}

/// Whether any family pairs `left` at `(start, end)` with `right` at its
/// `(start, end)` — the standard negative check for branch-boundary tests
/// (asserting two spans were NOT merged into one family).
pub(crate) fn families_pair_locations(
    families: &[serde_json::Value],
    left: (&str, u64, u64),
    right: (&str, u64, u64),
) -> bool {
    fn has_location(locations: &[serde_json::Value], (file, start, end): (&str, u64, u64)) -> bool {
        locations.iter().any(|loc| {
            loc["file"].as_str().is_some_and(|f| f.ends_with(file))
                && loc["start_line"] == start
                && loc["end_line"] == end
        })
    }
    families.iter().any(|family| {
        let locations = family["locations"].as_array().expect("locations");
        has_location(locations, left) && has_location(locations, right)
    })
}

pub(crate) fn json_array_strings<'a>(value: &'a serde_json::Value, key: &str) -> Vec<&'a str> {
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

pub(crate) fn assert_query_json_reports_semantic_packs(json: &serde_json::Value) {
    let packs = json["semantic_packs"]
        .as_array()
        .unwrap_or_else(|| panic!("query JSON should report semantic_packs: {json}"));
    for id in [
        "nose.first_party",
        "nose.lang.python",
        "nose.lang.javascript-typescript",
        "nose.lang.go",
        "nose.lang.rust",
        "nose.lang.java",
        "nose.lang.c",
        "nose.lang.ruby",
        "nose.lang.swift",
        "nose.lang.css",
        "nose.lang.html",
        "nose.python.builtins.collection_factories",
        "nose.python.stdlib.collection_factories",
        "nose.python.stdlib.math",
        "nose.ruby.stdlib.set",
        "nose.rust.stdlib.vec",
        "nose.rust.stdlib.option",
        "nose.rust.stdlib.integer_methods",
        "nose.rust.stdlib.collection_factories",
        "nose.rust.stdlib.map_factories",
        "nose.java.stdlib.math",
        "nose.java.stdlib.map_factories",
        "nose.java.stdlib.map_entries",
        "nose.java.stdlib.collection_factories",
        "nose.java.stdlib.collection_constructors",
        "nose.java.stdlib.static_collection_adapters",
        "nose.protocols.map_get",
        "nose.protocols.map_get_default",
        "nose.protocols.free_function_builtins",
        "nose.protocols.iterator_builtins",
        "nose.protocols.receiver_membership",
        "nose.protocols.map_key_views",
        "nose.protocols.property_builtins",
        "nose.protocols.builtin_method_calls",
        "nose.go.stdlib.namespace_calls",
        "nose.protocols.iterator_identity_adapters",
        "nose.javascript.builtins.promise",
        "nose.javascript.builtins.array",
        "nose.javascript.builtins.boolean",
        "nose.javascript.builtins.regex",
        "nose.javascript.builtins.static_index_membership",
        "nose.javascript.builtins.collection_constructors",
        "nose.python.stdlib.type_domain",
        "nose.value_graph.laws",
    ] {
        assert!(
            packs.iter().any(|pack| pack["id"] == id),
            "query JSON should report builtin semantic pack {id}: {json}"
        );
    }
}
