use super::*;

fn site_label(s: &Site) -> String {
    match &s.name {
        Some(n) if !n.is_empty() => format!("{} ({}:{}-{})", n, s.file, s.start_line, s.end_line),
        _ => format!("{}:{}-{}", s.file, s.start_line, s.end_line),
    }
}

pub(super) fn fragment_context(s: &Site) -> Option<String> {
    if !s.is_fragment {
        return None;
    }
    let kind = s
        .fragment_kind
        .map(|k| {
            k.reason_code()
                .strip_prefix("exact-")
                .unwrap_or(k.reason_code())
                .to_string()
        })
        .unwrap_or_else(|| "fragment".to_string());
    let reason = s.reason_code.unwrap_or("unknown");
    let parent = s.enclosing_unit.as_ref().map(|p| {
        let name = p
            .name
            .as_deref()
            .filter(|n| !n.is_empty())
            .map(|n| format!(" `{n}`"))
            .unwrap_or_default();
        format!(
            " in {:?}{name} {}:{}-{}",
            p.kind, p.file, p.start_line, p.end_line
        )
    });
    Some(format!(
        "{kind} fragment ({reason}){}",
        parent.unwrap_or_default()
    ))
}

/// The flagged divergences as JSON item objects inside query-JSON's `base` view.
pub(crate) fn divergence_items_json(flagged: &[Divergence]) -> Vec<serde_json::Value> {
    use serde_json::json;
    let site = |s: &Site| {
        json!({
            "tree": "base",
            "file": s.file, "name": s.name,
            "start_line": s.start_line, "end_line": s.end_line, "lang": s.lang,
            "kind": s.kind,
            "span_lines": s.span_lines,
            "span_tokens": s.span_tokens,
            "is_fragment": s.is_fragment,
            "fragment_kind": s.fragment_kind,
            "reason_code": s.reason_code,
            "enclosing_unit": s.enclosing_unit,
            "touches_shared": s.touches_shared,
        })
    };
    flagged
        .iter()
        .map(|d| {
            let tier = d.tier();
            json!({
                "family_id": d.family_id,
                "lane": "base-divergence",
                "base_family_id": d.family_id,
                "similarity": d.similarity,
                "complexity": d.complexity,
                "scope": d.scope,
                "witness_kind": d.witness_kind,
                "fire_eligible": d.fire_eligible,
                "tier": tier.as_str(),
                "tier_reasons": d.tier_reasons(),
                "taxonomy_hint": d.taxonomy_hint(),
                "gate": {
                    "eligible": tier.gate_eligible(),
                    "fail_default": d.gate_fail_default(),
                    "policy": DIVERGENT_EDIT_V2_POLICY,
                },
                "suppression": null,
                "graded": d.graded,
                "changed": d.changed.iter().map(&site).collect::<Vec<_>>(),
                "not_updated": d.not_updated.iter().map(&site).collect::<Vec<_>>(),
            })
        })
        .collect()
}

fn shown_divergences(flagged: &[Divergence], top: Option<usize>) -> &[Divergence] {
    let limit = top.unwrap_or(30);
    if limit == 0 || flagged.len() <= limit {
        flagged
    } else {
        &flagged[..limit]
    }
}

pub(super) fn divergence_sarif(
    flagged: &[Divergence],
    top: Option<usize>,
    top_zero_spelling: &str,
) -> Result<String> {
    use serde_json::json;
    let phys = |s: &Site| {
        let message = fragment_context(s).unwrap_or_else(|| site_label(s));
        json!({
            "message": { "text": message },
            "physicalLocation": {
                "artifactLocation": { "uri": s.file },
                "region": { "startLine": s.start_line, "endLine": s.end_line }
            }
        })
    };
    // The SARIF *location* is each un-updated sibling (where a fix may be missing), so a CI
    // annotation lands on the copy the change skipped; the changed copies are related.
    let shown = shown_divergences(flagged, top);
    let results: Vec<_> = shown
        .iter()
        .map(|d| {
            let tier = d.tier();
            let changed = d
                .changed
                .iter()
                .map(site_label)
                .collect::<Vec<_>>()
                .join(", ");
            json!({
                "ruleId": tier.sarif_rule_id(),
                "level": tier.sarif_level(),
                "message": { "text": format!(
                    "A clone of this code was changed ({changed}) but this copy was not — \
                     inspect whether the change should propagate here."
                ) },
                "locations": d.not_updated.iter().map(&phys).collect::<Vec<_>>(),
                "relatedLocations": d.changed.iter().map(&phys).collect::<Vec<_>>(),
                "properties": {
                    "family_id": d.family_id,
                    "base_family_id": d.family_id,
                    "lane": "base-divergence",
                    "tier": tier.as_str(),
                    "tier_reasons": d.tier_reasons(),
                    "taxonomy_hint": d.taxonomy_hint(),
                    "gate": {
                        "eligible": tier.gate_eligible(),
                        "fail_default": d.gate_fail_default(),
                        "policy": DIVERGENT_EDIT_V2_POLICY,
                    },
                    "policy": DIVERGENT_EDIT_V2_POLICY,
                    "fire_eligible": d.fire_eligible,
                },
            })
        })
        .collect();
    let rules = [
        DivergenceTier::Strict,
        DivergenceTier::Review,
        DivergenceTier::ReportOnly,
    ]
    .into_iter()
    .map(|tier| {
        json!({
            "id": tier.sarif_rule_id(),
            "name": tier.sarif_rule_name(),
            "shortDescription": { "text": match tier {
                DivergenceTier::Strict => "A likely missed clone-sibling edit",
                DivergenceTier::Review => "A divergent clone edit needing review",
                DivergenceTier::ReportOnly => "Advisory divergent clone evidence",
            } }
        })
    })
    .collect::<Vec<_>>();
    let mut run = json!({
        "tool": { "driver": {
            "name": "nose",
            "informationUri": "https://github.com/corca-ai/nose",
            "version": env!("CARGO_PKG_VERSION"),
            "rules": rules
        }},
        "results": results,
        "properties": {
            "inconsistent_families": flagged.len(),
            "total_families": flagged.len(),
            "shown_families": shown.len(),
        },
    });
    if shown.len() < flagged.len() {
        run["invocations"] = json!([{
            "executionSuccessful": true,
            "toolExecutionNotifications": [{
                "level": "note",
                "message": { "text": format!(
                    "Showing {} of {} divergent clone families (the row limit). \
                     Pass {top_zero_spelling} to emit every finding.",
                    shown.len(),
                    flagged.len(),
                ) }
            }]
        }]);
    }
    Ok(serde_json::to_string_pretty(&json!({
        "version": "2.1.0",
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "runs": [run],
    }))?)
}
