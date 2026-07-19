use serde_json::{json, Value};

pub(crate) fn semantic_packs_json(
    semantic_packs: &nose_semantics::SemanticPackSet,
    near_report: Option<&nose_semantics::SemanticPackNearReport>,
    exact_report: Option<&nose_semantics::SemanticPackExternalExactReport>,
) -> Vec<Value> {
    semantic_packs
        .packs()
        .iter()
        .map(|pack| semantic_pack_summary_json(pack, semantic_packs, near_report, exact_report))
        .collect()
}

pub(crate) fn with_semantic_packs(mut report: Value, semantic_packs: &[Value]) -> Value {
    if let Some(object) = report.as_object_mut() {
        object.insert(
            "semantic_packs".to_string(),
            Value::Array(semantic_packs.to_vec()),
        );
    }
    report
}

fn semantic_pack_summary_json(
    pack: &nose_semantics::SemanticPackSummary,
    semantic_packs: &nose_semantics::SemanticPackSet,
    near_report: Option<&nose_semantics::SemanticPackNearReport>,
    exact_report: Option<&nose_semantics::SemanticPackExternalExactReport>,
) -> Value {
    let mut summary = json!({
        "id": &pack.id,
        "hash": pack.hash_hex(),
        "kind": pack.kind.as_str(),
        "version": &pack.version,
        "display_name": &pack.display_name,
        "trust": pack.trust.as_manifest_str(),
        "enabled_by_default": pack.enabled_by_default,
        "source": pack.source.as_str(),
        "influence": pack.influence.as_str(),
        "path": pack.manifest_path.as_ref().map(|path| path.display().to_string()),
        "provider": &pack.provider,
        "repository": &pack.repository,
        "license": &pack.license,
        "supported_languages": &pack.supported_languages,
        "counts": {
            "evidence_producers": pack.counts.evidence_producers,
            "contracts": pack.counts.contracts,
            "value_laws": pack.counts.value_laws,
            "positive_fixtures": pack.counts.positive_fixtures,
            "hard_negatives": pack.counts.hard_negatives,
        },
    });
    let object = summary
        .as_object_mut()
        .expect("semantic-pack summary is an object");
    if let Some(api_version) = pack.api_version {
        object.insert("api_version".to_string(), json!(api_version));
    }
    if let Some(semantic_digest) = &pack.semantic_digest {
        object.insert("semantic_digest".to_string(), json!(semantic_digest));
    }
    if let Some(authorization) = semantic_packs.external_v1_authorization(&pack.id) {
        let project_lock = semantic_packs
            .project_lock()
            .expect("v1 authorization requires a validated project lock");
        object.insert(
            "lock".to_string(),
            json!({
                "status": "valid",
                "api_version": project_lock.api_version(),
                "decision_digest": project_lock.decision_digest(),
                "allowed_channels": authorization
                    .allowed_channels()
                    .iter()
                    .map(|channel| channel.as_str())
                    .collect::<Vec<_>>(),
                "selected_rows": authorization.selected_rows(),
                "dependencies": authorization.dependencies().iter().map(|dependency| json!({
                    "path": dependency.declared_path(),
                    "content_digest": dependency.content_digest(),
                })).collect::<Vec<_>>(),
                "exact_receipt": authorization.exact_receipt().map(|receipt| json!({
                    "path": receipt.declared_path(),
                    "content_digest": receipt.content_digest(),
                })),
            }),
        );
    }
    if let Some(counts) = near_report.and_then(|report| report.pack(&pack.id)) {
        object.insert(
            "near_influence".to_string(),
            json!({
                "lane": "near",
                "trust": "external-opt-in",
                "selected_rows": counts.selected_rows,
                "admitted_rows": counts.admitted_rows,
                "rejected_rows": counts.rejected_rows,
                "admitted_occurrences": counts.admitted_occurrences,
                "influential_occurrences": counts.influential_occurrences,
                "caveats": [
                    "near-only",
                    "not-an-equivalence-proof",
                    "provider-claim-user-authorized",
                    "exact-output-unchanged"
                ],
            }),
        );
    }
    if let Some(counts) = exact_report.and_then(|report| report.pack(&pack.id)) {
        object.insert(
            "external_exact_influence".to_string(),
            json!({
                "lane": "external-claim-exact",
                "trust": "external-opt-in",
                "assurance": "kernel-conformance-receipt",
                "selected_rows": counts.selected_rows,
                "admitted_rows": counts.admitted_rows,
                "rejected_rows": counts.rejected_rows,
                "admitted_occurrences": counts.admitted_occurrences,
                "influential_occurrences": counts.influential_occurrences,
                "caveats": [
                    "external-claim-not-builtin-certification",
                    "provider-claim-user-authorized",
                    "local-content-pinned"
                ],
            }),
        );
    }
    summary
}
