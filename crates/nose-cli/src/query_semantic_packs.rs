use serde_json::{json, Value};

pub(crate) fn semantic_packs_json(semantic_packs: &nose_semantics::SemanticPackSet) -> Vec<Value> {
    semantic_packs
        .packs()
        .iter()
        .map(semantic_pack_summary_json)
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

fn semantic_pack_summary_json(pack: &nose_semantics::SemanticPackSummary) -> Value {
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
    summary
}
