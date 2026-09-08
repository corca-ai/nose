//! Present captured observations and explicitly verified source text.
use super::sources::Sources;
use crate::source_lines::{line_diff, LINE_DIFF_LIMIT};
use nose_detect::regions::evolution::FamilyObservation;
use serde_json::{json, Value};

pub(super) fn family(family: &FamilyObservation, sources: Option<&mut Sources>) -> Value {
    let mut output = json!(family);
    if let Some(sources) = sources {
        for (i, member) in family.members.iter().enumerate() {
            let rendered = &mut output["members"][i];
            rendered["observation_id"] = json!(member.observation_id());
            let mut body = json!({"file":member.file,"region":member.source});
            match sources.text(member) {
                Ok(text) => {
                    body["status"] = json!("verified");
                    body["text"] = json!(text);
                }
                Err(error) => {
                    body["status"] = json!("unavailable");
                    body["reason"] = json!(error.to_string());
                }
            }
            rendered["source_body"] = body;
        }
    }
    output
}

/// Derive display summaries only from bodies that passed explicit verification.
pub(super) fn summarize(item: &mut Value) {
    let diffs: Vec<_> = item["member_changes"]["members"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|row| source_diff(item, row))
        .collect();
    item["source_diffs"] = json!(diffs);
    item["source_body_status"] = json!("explicit-verified-lookup");
    let members = std::iter::once(&item["before_observation"])
        .chain(item["after_observations"].as_array().into_iter().flatten())
        .flat_map(|f| f["members"].as_array().into_iter().flatten());
    let mut verified = 0;
    let mut unavailable = 0;
    for member in members {
        match member["source_body"]["status"].as_str() {
            Some("verified") => verified += 1,
            Some("unavailable") => unavailable += 1,
            _ => {}
        }
    }
    item["source_lookup"] = json!({"verified":verified,"unavailable":unavailable});
}

fn source_diff(item: &Value, row: &Value) -> Option<Value> {
    let before = body_at(&item["before_observation"], &row["before"])?;
    let [after_location] = row["after"].as_array()?.as_slice() else {
        return None;
    };
    let after = item["after_observations"]
        .as_array()?
        .iter()
        .find_map(|family| body_at(family, after_location))?;
    let a: Vec<_> = before.lines().collect();
    let b: Vec<_> = after.lines().collect();
    let lines: Vec<_> = line_diff(&a, &b)
        .into_iter()
        .map(|(tag, text)| json!({"tag":tag.to_string(),"text":text}))
        .collect();
    Some(json!({
        "before":row["before"], "after":after_location, "correspondence":row["status"],
        "same_content":before == after, "lines":lines,
        "truncated":a.len() > LINE_DIFF_LIMIT || b.len() > LINE_DIFF_LIMIT,
        "line_limit_per_side":LINE_DIFF_LIMIT,
        "meaning":"Text alignment of verified selected regions; correspondence remains advisory where labeled candidate."
    }))
}

fn body_at<'a>(family: &'a Value, location: &Value) -> Option<&'a str> {
    let id = location["observation_id"].as_str()?;
    family["members"]
        .as_array()?
        .iter()
        .find(|m| m["observation_id"].as_str() == Some(id))?["source_body"]["text"]
        .as_str()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(before: &str, after: &str) -> Value {
        json!({
            "before_observation":{"members":[
                {"observation_id":"old", "source_body":{"status":"verified", "text":before}}
            ]},
            "after_observations":[{"members":[
                {"observation_id":"new", "source_body":{"status":"verified", "text":after}}
            ]}],
            "member_changes":{"members":[{
                "before":{"observation_id":"old"},
                "after":[{"observation_id":"new"}], "status":"candidate"
            }]}
        })
    }

    #[test]
    fn alignment_does_not_hide_changes_beyond_the_display_limit() {
        let prefix = "shared\n".repeat(120);
        let mut report = item(&format!("{prefix}old\n"), &format!("{prefix}new\n"));
        summarize(&mut report);
        let diff = &report["source_diffs"][0];
        assert_eq!(diff["same_content"], false);
        assert_eq!(diff["correspondence"], "candidate");
        assert_eq!(diff["truncated"], true);
        assert_eq!(diff["line_limit_per_side"], 120);
        assert_eq!(diff["lines"].as_array().unwrap().len(), 120);
        assert!(diff["lines"]
            .as_array()
            .unwrap()
            .iter()
            .all(|l| l["tag"] == " "));

        let mut boundary = item(&prefix, &prefix);
        summarize(&mut boundary);
        assert_eq!(boundary["source_diffs"][0]["same_content"], true);
        assert_eq!(boundary["source_diffs"][0]["truncated"], false);
    }

    #[test]
    fn ambiguous_correspondence_does_not_choose_a_source_for_alignment() {
        let mut report = item("old", "new");
        report["member_changes"]["members"][0]["after"] =
            json!([{"observation_id":"new"}, {"observation_id":"other"}]);
        summarize(&mut report);
        assert_eq!(report["source_diffs"], json!([]));
        assert_eq!(
            report["source_lookup"],
            json!({"verified":2,"unavailable":0})
        );
    }

    #[test]
    fn unrequested_and_unavailable_sources_are_counted_separately() {
        let mut report = item("old", "new");
        report["before_observation"]["members"][0]["source_body"] =
            json!({"status":"unavailable", "reason":"source snapshot mismatch"});
        report["after_observations"][0]["members"][0]
            .as_object_mut()
            .unwrap()
            .remove("source_body");
        summarize(&mut report);
        assert_eq!(report["source_diffs"], json!([]));
        assert_eq!(
            report["source_lookup"],
            json!({"verified":0,"unavailable":1})
        );
    }
}
