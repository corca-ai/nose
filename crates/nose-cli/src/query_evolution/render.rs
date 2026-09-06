use serde_json::Value;

pub(super) fn reason(code: &str) -> &str {
    match code {
        "profile-changed" => "Analysis settings or pack authorization differ; review continuity is unavailable.",
        "incomplete-coverage" => "Input coverage or the candidate search is incomplete.",
        "membership-changed" => "The candidate families contain different numbers of member occurrences.",
        "evidence-population-changed" => "Aggregated evidence differs with member multiplicity. This does not establish a pack-policy or semantic change; older captures may lack distinct-evidence projections.",
        "member-content-changed" => "The distinct selected member contents differ; this is not a causal edit attribution.",
        "source-address-changed" => "Member paths, byte ranges, or containing source snapshots differ.",
        "scope-changed" => "Production/test scope differs.",
        "witness-changed" => "The detector witness kind or admitted value size differs.",
        "analysis-changed" => "Member analysis fingerprints differ; internal semantic edits are not reconstructed.",
        "packs-changed" => "Member pack evidence differs; inspect the recorded dependency and receipt provenance.",
        "laws-changed" => "The semantic-law provenance differs.",
        "abstraction-changed" => "The abstraction claim, template, holes or caveats differ.",
        "review-evidence-changed" => "The review-content/evidence signature differs.",
        "evidence-unavailable" => "Required source or proof evidence is unavailable.",
        "review-evidence-retained" => "Unambiguous matched members retain content/evidence and test scope; caller policy still applies.",
        "candidate" => "A unique related family is a candidate; occurrence continuity is not established.",
        "ambiguous" => "Multiple candidates or competing observations prevent a unique correspondence.",
        "unresolved" => "No family correspondence was established; this does not mean the code was deleted.",
        "unmatched-current" => "A current family has no asserted predecessor in the captured population.",
        "budget-exceeded" => "The complete candidate bucket could not be examined within the work budget.",
        _ => "Additional evidence; consult the installed capabilities contract.",
    }
}

pub(super) fn render(output: &Value, full: bool) {
    let s = &output["summary"];
    println!(
        "{}: {} · {} recheck · {} evidence unchanged.",
        if output["exploration"] == true {
            "Saved analysis exploration"
        } else {
            "Analysis comparison"
        },
        observation_count(&s["total"]),
        s["recheck"],
        s["retained"]
    );
    let showing = if output["view"] == "group" {
        "group view below".into()
    } else {
        format!("{} shown", s["shown"])
    };
    println!(
        "Selection: {} ({} recheck, {} retained); {showing}.",
        observation_count(&s["selected"]),
        s["selected_recheck"],
        s["selected_retained"]
    );
    println!(
        "Population: admitted code families ({} before, {} after).",
        s["before_families"], s["after_families"]
    );
    println!("Profile matches: {}; coverage complete: {}; candidate search complete: {}; candidates: {}/{}.",
        output["profile_matches"], output["complete"], output["candidate_search_complete"], output["candidates_examined"], output["max_candidates"]);
    if let Some(message) = output["empty_message"].as_str() {
        println!("{message}");
    }
    if full {
        capture_context(output);
    }
    for side in ["before", "after"] {
        if full || output["coverage"][side]["complete"] == false {
            coverage(side, &output["coverage"][side]);
        }
    }
    if output["view"] == "group" {
        println!(
            "Showing {} / {} groups (counts may overlap).",
            s["groups_shown"], s["groups_total"]
        );
        for group in output["groups"].as_array().unwrap() {
            println!(
                "  {}={} · {}\n    next: {}",
                text(&output["group_field"]),
                text(&group["key"]),
                observation_count(&group["count"]),
                text(&group["next"][0])
            );
        }
    }
    for item in output["items"].as_array().unwrap() {
        item_summary(item, full);
    }
    if let Some(path) = output["reviews"]["written"].as_str() {
        println!("Saved caller review to {path}.");
    }
    for path in output["reviews"]["unrelated"]
        .as_array()
        .into_iter()
        .flatten()
    {
        println!(
            "Review {} belongs to another capture; compare its original analysis to evaluate it.",
            text(path)
        );
    }
    if output["view"] == "change" && output["reviews"]["written"].is_null() {
        println!("Record your decision: add --write-review FILE --decision keep-separate|refactor|defer --reason TEXT. A new file records this current family; it does not suppress findings.");
    }
    println!("\nRetained evidence is not approval. Unmatched observations do not establish deletion or ancestry.");
    if !full {
        println!("Add `full` to inspect capture context, reason explanations and member evidence.");
    }
    println!("next:");
    for action in output["actions"].as_array().unwrap() {
        if !full && output["view"] == "dashboard" && action["kind"] == "group-evidence" {
            continue;
        }
        println!(
            "  {}\n    {}",
            text(&action["label"]),
            text(&action["command"])
        );
    }
}

fn observation_count(value: &Value) -> String {
    format!(
        "{value} {}",
        if value == 1 {
            "observation"
        } else {
            "observations"
        }
    )
}

fn capture_context(output: &Value) {
    println!(
        "Before: {}\nAfter: {}",
        text(&output["inputs"]["before"]),
        text(&output["inputs"]["after"])
    );
    println!(
        "Roots: {} → {}",
        output["roots"]["before"], output["roots"]["after"]
    );
    println!(
        "Path bases: {} → {}",
        output["path_bases"]["before"], output["path_bases"]["after"]
    );
    if output["profile_matches"] == false {
        println!(
            "Captured settings: {} → {}",
            output["profiles"]["before"], output["profiles"]["after"]
        );
    }
}

fn item_summary(item: &Value, full: bool) {
    let paths = item["paths"]
        .as_array()
        .unwrap()
        .iter()
        .map(text)
        .collect::<Vec<_>>()
        .join(", ");
    println!(
        "\n{} · {} · {paths}",
        &text(&item["id"])[..12],
        text(&item["correspondence"])
    );
    if full {
        for detail in item["reason_details"].as_array().unwrap() {
            println!("  {}: {}", text(&detail["code"]), text(&detail["meaning"]));
        }
    } else {
        let reasons = item["reasons"]
            .as_array()
            .unwrap()
            .iter()
            .map(text)
            .collect::<Vec<_>>()
            .join(", ");
        println!("  {reasons}");
    }
    if let Some(changes) = item.get("member_changes") {
        member_changes(changes);
    }
    if let Some(before) = item.get("before_observation") {
        let mut shown_sources = std::collections::BTreeMap::new();
        observation("before", before, &mut shown_sources);
        for after in item["after_observations"].as_array().unwrap() {
            observation("after", after, &mut shown_sources);
        }
        if let Some(lookup) = item.get("source_lookup") {
            println!(
                "  Source lookup: {} verified / {} unavailable.",
                lookup["verified"], lookup["unavailable"]
            );
        } else {
            println!("  Source bodies: {}.", text(&item["source_body_status"]));
        }
    }
    for diff in item["source_diffs"].as_array().into_iter().flatten() {
        if diff["same_content"] == true {
            println!(
                "  verified source unchanged: {} → {}",
                location(&diff["before"]),
                location(&diff["after"])
            );
            continue;
        }
        println!(
            "  verified source diff: {} → {} ({})",
            location(&diff["before"]),
            location(&diff["after"]),
            text(&diff["correspondence"])
        );
        for line in diff["lines"].as_array().unwrap() {
            println!("    {} {}", text(&line["tag"]), text(&line["text"]));
        }
        if diff["truncated"] == true {
            println!("    … alignment limited to 120 lines per side; verified member bodies are shown above");
        }
    }
    if let Some(reviews) = item["reviews"].as_array() {
        for review in reviews {
            println!(
                "  review {}: {} — {} ({})",
                text(&review["status"]),
                text(&review["decision"]),
                text(&review["reason"]),
                text(&review["file"])
            );
            println!("    {}", text(&review["basis"]));
        }
    }
    if let Some(actions) = item["actions"].as_array() {
        for action in actions {
            println!(
                "  {}\n    {}",
                text(&action["label"]),
                text(&action["command"])
            );
        }
    }
    println!("  next: {}", text(&item["next"][0]));
}

fn member_changes(changes: &Value) {
    let before = changes["before_members"]
        .as_u64()
        .map(|n| n.to_string())
        .unwrap_or_else(|| "no established predecessor".into());
    let counts = changes["after_member_counts"].as_array().unwrap();
    let after = match counts.as_slice() {
        [] => "no established counterpart".into(),
        [count] => count.to_string(),
        many => format!(
            "candidate family counts: {}",
            many.iter()
                .map(Value::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
    };
    println!("  Member counts: {before} → {after}");
    for member in changes["members"].as_array().unwrap() {
        let after = member["after"]
            .as_array()
            .unwrap()
            .iter()
            .map(location)
            .collect::<Vec<_>>()
            .join(", ");
        println!(
            "    {}: {} → {}",
            text(&member["status"]),
            location(&member["before"]),
            if after.is_empty() {
                "no established counterpart"
            } else {
                &after
            }
        );
    }
}

fn observation<'a>(
    side: &str,
    f: &'a Value,
    shown: &mut std::collections::BTreeMap<&'a str, String>,
) {
    if f.is_null() {
        println!("  {side}: no asserted observation");
        return;
    }
    println!(
        "  {side}: {} · {} · {} members",
        text(&f["witness"]),
        text(&f["scope"]),
        f["members"].as_array().unwrap().len()
    );
    for m in f["members"].as_array().unwrap() {
        println!(
            "    {}:{}-{} · {} · {}",
            text(&m["file"]),
            m["start_line"],
            m["end_line"],
            text(&m["name"]),
            text(&m["lang"])
        );
    }
    for member in f["members"].as_array().unwrap() {
        if let Some(body) = member.get("source_body") {
            println!(
                "    source {}: {}",
                text(&member["file"]),
                text(&body["status"])
            );
            if let Some(source) = body["text"].as_str() {
                if let Some(previous) = shown.get(source) {
                    println!("      Same verified content as {previous}; shown once.");
                } else {
                    shown.insert(
                        source,
                        format!("{side} {}:{}", text(&member["file"]), member["start_line"]),
                    );
                    for line in source.lines().take(120) {
                        println!("      {line}");
                    }
                    if source.lines().count() > 120 {
                        println!("      … source display limited to 120 lines; --format json includes the complete verified body within the source byte limit.");
                    }
                }
            } else {
                println!("      {}", text(&body["reason"]));
            }
        }
    }
    for key in [
        "laws",
        "near_provenance",
        "exact_provenance",
        "abstraction_template",
    ] {
        if !f[key].as_array().unwrap().is_empty() {
            println!("    {key}: {}", f[key]);
        }
    }
}
fn text(value: &Value) -> &str {
    value.as_str().unwrap_or("unnamed")
}

pub(super) fn coverage(label: &str, data: &Value) {
    println!(
        "{label} coverage: {} · {} scanned files · {} skipped sources · {} members without source.",
        if data["complete"] == true {
            "complete"
        } else {
            "INCOMPLETE"
        },
        data["scanned_files"],
        data["skipped_sources"],
        data["members_without_source"]
    );
    if let Some(rows) = data["diagnostics"].as_array() {
        for row in rows {
            println!("  {}: {}", text(&row["path"]), text(&row["reason"]));
        }
    } else if data["skipped_sources"].as_u64().unwrap_or(0) > 0 {
        println!("  This older capture did not record skipped-source details; capture again to inspect them.");
    }
    if data["complete"] == false {
        println!(
            "  Population discovery complete: {}; member source evidence complete: {}.",
            data["population_complete"], data["source_evidence_complete"]
        );
        let mut files = std::collections::BTreeMap::<&str, usize>::new();
        for member in data["unavailable_members"].as_array().into_iter().flatten() {
            *files.entry(text(&member["file"])).or_default() += 1;
        }
        for (file, count) in files.iter().take(8) {
            println!("  {file}: {count} member references lack a captured source address or content key.");
        }
        if files.len() > 8 {
            println!(
                "  {} more paths; --format json exposes all unavailable_members.",
                files.len() - 8
            );
        }
        println!("  Inspect recorded diagnostics before recapturing; a larger comparison budget cannot restore missing source evidence.");
        println!("  A complete selected family can record a current decision. Carrying decisions to another capture still requires complete evidence.");
    }
}

fn location(value: &Value) -> String {
    if value.is_null() {
        return "no established predecessor".into();
    }
    format!(
        "{}:{}-{}",
        text(&value["file"]),
        value["start_line"],
        value["end_line"]
    )
}
