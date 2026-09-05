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

pub(super) fn render(output: &Value) {
    let s = &output["summary"];
    println!("Analysis changes: {} total · {} recheck · {} retain review evidence.\nSelection: {} observations ({} recheck, {} retained); {} shown.\nPopulation: admitted code families ({} before, {} after).\nProfile matches: {}; coverage complete: {}; candidate search complete: {}; candidates: {}/{}.",
        s["total"],s["recheck"],s["retained"],s["selected"],s["selected_recheck"],s["selected_retained"],s["shown"],s["before_families"],s["after_families"],
        output["profile_matches"],output["complete"],output["candidate_search_complete"],output["candidates_examined"],output["max_candidates"]);
    if let Some(message) = output["empty_message"].as_str() {
        println!("{message}");
    }
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
    coverage("Before", &output["coverage"]["before"]);
    coverage("After", &output["coverage"]["after"]);
    if output["profile_matches"] == false {
        println!(
            "Captured settings: {} → {}",
            output["profiles"]["before"], output["profiles"]["after"]
        );
    }
    println!(
        "Showing {} changes and {} / {} groups (counts may overlap).",
        s["shown"], s["groups_shown"], s["groups_total"]
    );
    for group in output["groups"].as_array().unwrap() {
        println!(
            "  {}={} · {} changes\n    next: {}",
            text(&output["group_field"]),
            text(&group["key"]),
            group["count"],
            text(&group["next"][0])
        );
    }
    for item in output["items"].as_array().unwrap() {
        println!(
            "\n{} · {}\n  paths: {}",
            &text(&item["id"])[..12],
            text(&item["correspondence"]),
            item["paths"]
        );
        for detail in item["reason_details"].as_array().unwrap() {
            println!("  {}: {}", text(&detail["code"]), text(&detail["meaning"]));
        }
        if let Some(changes) = item.get("member_changes") {
            println!(
                "  Member counts: {} → {}",
                changes["before_members"], changes["after_member_counts"]
            );
            for member in changes["members"].as_array().unwrap() {
                let before = location(&member["before"]);
                let after = member["after"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(location)
                    .collect::<Vec<_>>()
                    .join(", ");
                println!(
                    "    {}: {before} → {}",
                    text(&member["status"]),
                    if after.is_empty() {
                        "no established counterpart"
                    } else {
                        &after
                    }
                );
            }
        }
        if let Some(before) = item.get("before_observation") {
            observation("before", before);
            for after in item["after_observations"].as_array().unwrap() {
                observation("after", after);
            }
            println!("  Source bodies: not stored. Analysis digest internals: opaque.");
        }
        println!("  next: {}", text(&item["next"][0]));
    }
    println!("\nRetained evidence is not approval or ancestry. Missing findings do not establish deletion or successful refactoring.\nnext:");
    for action in output["actions"].as_array().unwrap() {
        println!(
            "  {}\n    {}",
            text(&action["label"]),
            text(&action["command"])
        );
    }
}

fn observation(side: &str, f: &Value) {
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
        println!("  Inspect the source diagnostics and recapture with the same analysis settings. Increasing the comparison budget cannot restore missing input evidence.");
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
