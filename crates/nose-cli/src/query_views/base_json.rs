use super::*;

pub(super) fn render(
    flagged: &[divergence::Divergence],
    changed_files: usize,
    base_ref: &str,
    path: &str,
    top: Option<usize>,
    semantic_packs: &[serde_json::Value],
) {
    let limit = query_row_limit(top);
    let items: Vec<_> = divergence::divergence_items_json(flagged)
        .into_iter()
        .take(limit)
        .collect();
    let limit_value = match top {
        Some(0) => serde_json::Value::Null,
        Some(n) => serde_json::json!(n),
        None => serde_json::json!(30),
    };
    println!(
        "{}",
        with_semantic_packs(
            serde_json::json!({
                "schema_version": schema_versions::QUERY_BASE_JSON_SCHEMA_VERSION,
                "tool": "nose",
                "view": "base",
                "path": path,
                "base": base_ref,
                "summary": {
                    "changed_files": changed_files,
                    "divergences": flagged.len(),
                    "shown_divergences": items.len(),
                    "limit": limit_value,
                    "fire_eligible": flagged.iter().filter(|d| d.fire_eligible).count(),
                    "strict": flagged.iter().filter(|d| d.gate_fail_default()).count(),
                },
                "items": items,
                "next": [format!("nose query {path} base={base_ref} --fail-on any")],
            }),
            semantic_packs
        )
    );
}
