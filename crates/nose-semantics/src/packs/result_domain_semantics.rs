use super::*;
use std::collections::HashMap;

const RESULT_DOMAIN_KEYS: &[&str] = &["kind", "domain", "subject", "notes"];
const FIXED_RESULT_DOMAINS: &[&str] = &[
    "Array",
    "Boolean",
    "ByteArray",
    "Collection",
    "Float",
    "FutureLike",
    "Integer",
    "Iterable",
    "Iterator",
    "Map",
    "Number",
    "Option",
    "PromiseLike",
    "Record",
    "Result",
    "Set",
    "String",
];

pub(super) fn validate_result_domain_semantics(
    kind: &str,
    id: &str,
    semantics: &serde_json::Map<String, serde_json::Value>,
    requirements: &[ManifestRequirement],
    evidence_producer_kinds: &HashMap<String, String>,
) -> Result<(), String> {
    let Some(value) = semantics.get("result_domain") else {
        return Ok(());
    };
    let Some(object) = value.as_object() else {
        return Ok(());
    };
    for key in object.keys() {
        if !RESULT_DOMAIN_KEYS.contains(&key.as_str()) {
            return Err(format!(
                "{kind} `{id}` semantics.result_domain has unknown key `{key}`"
            ));
        }
    }
    require_result_domain_string(kind, id, object, "kind").and_then(|value| {
        (value == "fixed")
            .then_some(())
            .ok_or_else(|| format!("{kind} `{id}` semantics.result_domain.kind must be `fixed`"))
    })?;
    let domain = require_result_domain_string(kind, id, object, "domain")?;
    if !FIXED_RESULT_DOMAINS.contains(&domain) {
        return Err(format!(
            "{kind} `{id}` semantics.result_domain.domain has unknown domain `{domain}`"
        ));
    }
    if let Some(subject) = optional_result_domain_string(kind, id, object, "subject")? {
        if subject != "call" {
            return Err(format!(
                "{kind} `{id}` semantics.result_domain.subject must be `call`"
            ));
        }
    }
    optional_result_domain_string(kind, id, object, "notes")?;
    if !requirements.iter().any(|requirement| {
        requirement.required
            && (requirement.ref_id == "LibraryApi.Contract"
                || evidence_producer_kinds
                    .get(&requirement.ref_id)
                    .is_some_and(|producer_kind| producer_kind == "LibraryApi.Contract"))
    }) {
        return Err(format!(
            "{kind} `{id}` semantics.result_domain requires required LibraryApi.Contract evidence"
        ));
    }
    Ok(())
}

pub(super) fn collect_evidence_producer_kinds(
    manifest: &SemanticPackManifest,
) -> HashMap<String, String> {
    manifest
        .declares
        .evidence_producers
        .iter()
        .map(|producer| (producer.id.clone(), producer.kind.clone()))
        .collect()
}

fn require_result_domain_string<'a>(
    kind: &str,
    id: &str,
    object: &'a serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Result<&'a str, String> {
    optional_result_domain_string(kind, id, object, key)?.ok_or_else(|| {
        format!("{kind} `{id}` semantics.result_domain.{key} must be a non-empty string")
    })
}

fn optional_result_domain_string<'a>(
    kind: &str,
    id: &str,
    object: &'a serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Result<Option<&'a str>, String> {
    let Some(value) = object.get(key) else {
        return Ok(None);
    };
    value
        .as_str()
        .filter(|value| !value.is_empty())
        .map(Some)
        .ok_or_else(|| {
            format!("{kind} `{id}` semantics.result_domain.{key} must be a non-empty string")
        })
}
