use serde_json::Value;

pub(super) fn reason(code: &str) -> &str {
    match code {
        "profile-changed" => "Analysis settings or pack authorization differ; review continuity is unavailable.",
        "incomplete-coverage" => "Input coverage or the candidate search is incomplete.",
        "membership-changed" => "The candidate families contain different numbers of member occurrences.",
        "member-content-changed" => "The multiset of selected member contents differs.",
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
    println!("Analysis changes: {} selected / {} total; {} retain review evidence.\nPopulation: admitted code families ({} before, {} after).\nProfile matches: {}; coverage complete: {}; candidates: {}/{}.",
        s["selected"],s["total"],s["retained"],s["before_families"],s["after_families"],
        output["profile_matches"],output["complete"],output["candidates_examined"],output["max_candidates"]);
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
    for next in output["next"].as_array().unwrap() {
        println!("  {}", text(next));
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
