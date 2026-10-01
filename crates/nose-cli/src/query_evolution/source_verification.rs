//! Summarize explicit verified lookups without changing analysis completeness.
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn describe(items: &[Value]) -> Value {
    let mut attempted = false;
    let mut verified = 0;
    let mut unavailable = 0;
    let mut files: BTreeMap<&str, BTreeMap<&str, BTreeSet<&str>>> = BTreeMap::new();
    for item in items {
        if item["source_lookup"].is_null() {
            continue;
        }
        attempted = true;
        verified += item["source_lookup"]["verified"].as_u64().unwrap_or(0);
        unavailable += item["source_lookup"]["unavailable"].as_u64().unwrap_or(0);
        for (side, member) in super::source_view::members(item) {
            if member["source_body"]["status"] != "unavailable" {
                continue;
            }
            if let Some(file) = member["file"].as_str() {
                let reasons = files.entry(file).or_default().entry(side).or_default();
                if let Some(reason) = member["source_body"]["reason"].as_str() {
                    reasons.insert(reason);
                }
            }
        }
    }
    if !attempted {
        return Value::Null;
    }
    let status = match (verified, unavailable) {
        (0, 0) => "not-attempted",
        (0, _) => "unavailable",
        (_, 0) => "complete",
        _ => "partial",
    };
    let files: Vec<_> = files
        .into_iter()
        .map(|(file, reasons)| json!({"file":file,"sides":reasons.keys().collect::<Vec<_>>(),"reasons":reasons}))
        .collect();
    json!({"status":status,"scope":"shown-observations",
        "lookup_counts":{"verified":verified,"unavailable":unavailable},
        "unavailable_files":files,
        "meaning":"Counts are member reads across explicitly requested before/after source sides. This verifies only shown observations, independently of capture coverage and comparison completeness; unavailable files may be missing, stale or otherwise unreadable."})
}

pub(super) fn render(output: &Value) {
    let verification = &output["source_verification"];
    if verification.is_null() {
        return;
    }
    println!(
        "Source verification (shown observations): {} · {} verified / {} unavailable member reads.",
        super::render::text(&verification["status"]),
        verification["lookup_counts"]["verified"],
        verification["lookup_counts"]["unavailable"]
    );
    for file in verification["unavailable_files"]
        .as_array()
        .into_iter()
        .flatten()
    {
        println!(
            "  Unavailable: {} · sides {} · reasons {}",
            super::render::text(&file["file"]),
            file["sides"],
            file["reasons"]
        );
    }
}
