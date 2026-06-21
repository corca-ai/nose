use super::*;
use std::collections::HashSet;

pub(super) fn validate_manifest(manifest: &SemanticPackManifest) -> Result<(), String> {
    validate_manifest_header(manifest)?;
    validate_manifest_targets(manifest)?;
    let known_refs = collect_declared_refs(manifest)?;
    validate_manifest_conformance(manifest)?;
    let conformance_refs = collect_conformance_fixture_refs(manifest)?;
    validate_executable_conformance(manifest, &conformance_refs)?;

    for producer in &manifest.declares.evidence_producers {
        validate_evidence_producer(producer, &known_refs)?;
    }
    for contract in &manifest.declares.contracts {
        if !contract.surface.is_object() {
            return Err(format!(
                "contract `{}` surface must be an object",
                contract.id
            ));
        }
        for unsupported in &contract.known_unsupported {
            require_non_empty("contract.known_unsupported[]", unsupported)?;
        }
        optional_non_empty("contract.notes", contract.notes.as_deref())?;
        validate_contract(
            "contract",
            &contract.id,
            &contract.requires,
            &contract.semantics,
            contract.channel,
            contract.proof_status,
            &contract.conformance_refs,
            &known_refs,
            &conformance_refs,
        )?;
    }
    for law in &manifest.declares.value_laws {
        validate_contract(
            "value law",
            &law.id,
            &law.requires,
            &law.semantics,
            law.channel,
            law.proof_status,
            &law.conformance_refs,
            &known_refs,
            &conformance_refs,
        )?;
    }
    Ok(())
}

fn validate_manifest_header(manifest: &SemanticPackManifest) -> Result<(), String> {
    if manifest.api_version != SEMANTIC_PACK_API_VERSION {
        return Err(format!(
            "`api_version` must be {SEMANTIC_PACK_API_VERSION}, got `{}`",
            manifest.api_version
        ));
    }
    require_stable_id("pack.id", &manifest.pack.id)?;
    require_non_empty("pack.version", &manifest.pack.version)?;
    require_non_empty("pack.display_name", &manifest.pack.display_name)?;
    optional_non_empty("pack.description", manifest.pack.description.as_deref())?;
    require_non_empty(
        "provenance.provider.name",
        &manifest.provenance.provider.name,
    )?;
    optional_non_empty(
        "provenance.provider.contact",
        manifest.provenance.provider.contact.as_deref(),
    )?;
    require_non_empty("provenance.license", &manifest.provenance.license)?;
    require_non_empty("provenance.repository", &manifest.provenance.repository)?;
    optional_non_empty(
        "provenance.source_revision",
        manifest.provenance.source_revision.as_deref(),
    )?;
    validate_nose_version_requirement("compatibility.nose", &manifest.compatibility.nose)?;
    optional_non_empty(
        "compatibility.notes",
        manifest.compatibility.notes.as_deref(),
    )?;
    if manifest.pack.trust != PackTrust::ExternalOptIn || manifest.pack.enabled_by_default {
        return Err(
            "local semantic pack manifests must be external-opt-in and disabled by default"
                .to_string(),
        );
    }
    Ok(())
}

fn validate_manifest_targets(manifest: &SemanticPackManifest) -> Result<(), String> {
    if manifest.supported_languages.is_empty() {
        return Err("`supported_languages` must contain at least one language".to_string());
    }
    for language in &manifest.supported_languages {
        require_non_empty("supported_languages[].id", &language.id)?;
        optional_non_empty(
            "supported_languages[].language_version",
            language.language_version.as_deref(),
        )?;
        optional_non_empty("supported_languages[].runtime", language.runtime.as_deref())?;
        for version in &language.runtime_versions {
            require_non_empty("supported_languages[].runtime_versions[]", version)?;
        }
    }
    for package in &manifest.packages {
        require_non_empty("packages[].ecosystem", &package.ecosystem)?;
        require_non_empty("packages[].name", &package.name)?;
        require_non_empty("packages[].versions", &package.versions)?;
    }
    for dependency in &manifest.dependencies {
        require_stable_id("dependencies[].id", &dependency.id)?;
        require_non_empty("dependencies[].version", &dependency.version)?;
        let _required = dependency.required;
    }
    Ok(())
}

fn collect_declared_refs(manifest: &SemanticPackManifest) -> Result<HashSet<String>, String> {
    let mut known_refs = HashSet::new();
    collect_unique_refs(
        "dependencies",
        manifest.dependencies.iter().map(|dep| &dep.id),
        &mut known_refs,
    )?;
    collect_unique_refs(
        "declares.evidence_producers",
        manifest
            .declares
            .evidence_producers
            .iter()
            .map(|producer| &producer.id),
        &mut known_refs,
    )?;
    collect_unique_refs(
        "declares.contracts",
        manifest
            .declares
            .contracts
            .iter()
            .map(|contract| &contract.id),
        &mut known_refs,
    )?;
    collect_unique_refs(
        "declares.value_laws",
        manifest.declares.value_laws.iter().map(|law| &law.id),
        &mut known_refs,
    )?;
    Ok(known_refs)
}

fn validate_manifest_conformance(manifest: &SemanticPackManifest) -> Result<(), String> {
    if manifest.conformance.positive_fixtures.is_empty() {
        return Err("`conformance.positive_fixtures` must not be empty".to_string());
    }
    if manifest.conformance.hard_negatives.is_empty() {
        return Err("`conformance.hard_negatives` must not be empty".to_string());
    }
    for fixture in manifest
        .conformance
        .positive_fixtures
        .iter()
        .chain(&manifest.conformance.hard_negatives)
    {
        require_stable_id("conformance fixture id", &fixture.id)?;
        require_non_empty("conformance fixture description", &fixture.description)?;
        optional_non_empty("conformance fixture path", fixture.path.as_deref())?;
        optional_non_empty(
            "conformance fixture expectation",
            fixture.expectation.as_deref(),
        )?;
    }
    for unsupported in &manifest.conformance.known_unsupported {
        require_non_empty("conformance.known_unsupported[]", unsupported)?;
    }
    optional_non_empty(
        "conformance.command",
        manifest.conformance.command.as_deref(),
    )?;
    for proof in &manifest.conformance.proofs {
        require_non_empty("conformance.proofs[]", proof)?;
    }
    for gate in &manifest.conformance.executable {
        require_stable_id("conformance.executable[].id", &gate.id)?;
        require_stable_id("conformance.executable[].row_ref", &gate.row_ref)?;
        require_non_empty(
            "conformance.executable[].expected_positive",
            &gate.expected_positive,
        )?;
        require_non_empty(
            "conformance.executable[].expected_hard_negative",
            &gate.expected_hard_negative,
        )?;
    }
    Ok(())
}

fn validate_executable_conformance(
    manifest: &SemanticPackManifest,
    fixture_refs: &ConformanceFixtureRefs,
) -> Result<(), String> {
    let mut gate_ids = HashSet::new();
    let exact_rows = collect_exact_capable_row_refs(manifest);
    for gate in &manifest.conformance.executable {
        if !gate_ids.insert(gate.id.clone()) {
            return Err(format!(
                "duplicate id `{}` in `conformance.executable`",
                gate.id
            ));
        }
        if !exact_rows.contains(&gate.row_ref) {
            return Err(format!(
                "executable conformance gate `{}` row_ref must reference an exact-capable declared row",
                gate.id
            ));
        }
        if gate.positive_fixtures.is_empty() {
            return Err(format!(
                "executable conformance gate `{}` must reference at least one positive fixture",
                gate.id
            ));
        }
        if gate.hard_negatives.is_empty() {
            return Err(format!(
                "executable conformance gate `{}` must reference at least one hard-negative fixture",
                gate.id
            ));
        }
        for id in &gate.positive_fixtures {
            require_stable_id("conformance.executable[].positive_fixtures[]", id)?;
            if !fixture_refs.positive.contains(id) {
                return Err(format!(
                    "executable conformance gate `{}` references missing positive fixture `{id}`",
                    gate.id
                ));
            }
        }
        for id in &gate.hard_negatives {
            require_stable_id("conformance.executable[].hard_negatives[]", id)?;
            if !fixture_refs.hard_negative.contains(id) {
                return Err(format!(
                    "executable conformance gate `{}` references missing hard-negative fixture `{id}`",
                    gate.id
                ));
            }
        }
    }
    Ok(())
}

fn collect_exact_capable_row_refs(manifest: &SemanticPackManifest) -> HashSet<String> {
    let mut rows = HashSet::new();
    for producer in &manifest.declares.evidence_producers {
        if producer.channel.exact_capable() {
            rows.insert(producer.id.clone());
        }
    }
    for contract in &manifest.declares.contracts {
        if contract.channel.exact_capable() {
            rows.insert(contract.id.clone());
        }
    }
    for law in &manifest.declares.value_laws {
        if law.channel.exact_capable() {
            rows.insert(law.id.clone());
        }
    }
    rows
}

#[derive(Default)]
struct ConformanceFixtureRefs {
    all: HashSet<String>,
    positive: HashSet<String>,
    hard_negative: HashSet<String>,
}

impl ConformanceFixtureRefs {
    fn has_positive_and_hard_negative_refs(&self, refs: &[String]) -> bool {
        refs.iter().any(|id| self.positive.contains(id))
            && refs.iter().any(|id| self.hard_negative.contains(id))
    }
}

fn collect_conformance_fixture_refs(
    manifest: &SemanticPackManifest,
) -> Result<ConformanceFixtureRefs, String> {
    let mut refs = ConformanceFixtureRefs::default();
    for fixture in &manifest.conformance.positive_fixtures {
        collect_conformance_fixture_ref("conformance.positive_fixtures", &fixture.id, &mut refs)?;
        refs.positive.insert(fixture.id.clone());
    }
    for fixture in &manifest.conformance.hard_negatives {
        collect_conformance_fixture_ref("conformance.hard_negatives", &fixture.id, &mut refs)?;
        refs.hard_negative.insert(fixture.id.clone());
    }
    Ok(refs)
}

fn collect_conformance_fixture_ref(
    label: &str,
    id: &str,
    refs: &mut ConformanceFixtureRefs,
) -> Result<(), String> {
    require_stable_id(label, id)?;
    if !refs.all.insert(id.to_string()) {
        return Err(format!("duplicate id `{id}` in `conformance fixtures`"));
    }
    Ok(())
}

fn validate_evidence_producer(
    producer: &ManifestEvidenceProducer,
    known_refs: &HashSet<String>,
) -> Result<(), String> {
    require_stable_id("declares.evidence_producers[].id", &producer.id)?;
    if !is_valid_evidence_kind(&producer.kind) {
        return Err(format!(
            "evidence producer `{}` has unknown kind `{}`",
            producer.id, producer.kind
        ));
    }
    if producer.anchors.is_empty() {
        return Err(format!(
            "evidence producer `{}` must declare at least one anchor",
            producer.id
        ));
    }
    if producer.stable_hash_inputs.is_empty()
        || !producer
            .stable_hash_inputs
            .iter()
            .any(|input| input == "pack.id")
        || !producer
            .stable_hash_inputs
            .iter()
            .any(|input| input == "producer.id")
    {
        return Err(format!(
            "evidence producer `{}` stable_hash_inputs must include pack.id and producer.id",
            producer.id
        ));
    }
    if producer.conflict_policy != "fail-closed" && producer.conflict_policy != "near-only" {
        return Err(format!(
            "evidence producer `{}` conflict_policy must be fail-closed or near-only",
            producer.id
        ));
    }
    if producer.channel.exact_capable() && producer.conflict_policy != "fail-closed" {
        return Err(format!(
            "exact-capable evidence producer `{}` must fail closed on conflicts",
            producer.id
        ));
    }
    for emitted in &producer.emits {
        if !is_valid_evidence_kind(emitted) {
            return Err(format!(
                "evidence producer `{}` emits unknown evidence kind `{emitted}`",
                producer.id
            ));
        }
    }
    for requirement in &producer.requires {
        validate_requirement(&producer.id, requirement, known_refs)?;
    }
    optional_non_empty("producer.notes", producer.notes.as_deref())?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_contract(
    kind: &str,
    id: &str,
    requirements: &[ManifestRequirement],
    semantics: &serde_json::Value,
    channel: SemanticPackChannel,
    _proof_status: SemanticPackProofStatus,
    conformance_refs: &[String],
    known_refs: &HashSet<String>,
    fixture_refs: &ConformanceFixtureRefs,
) -> Result<(), String> {
    require_stable_id("declares.contracts[].id", id)?;
    let semantics = semantics
        .as_object()
        .ok_or_else(|| format!("{kind} `{id}` semantics must be an object"))?;
    for ref_id in conformance_refs {
        if !fixture_refs.all.contains(ref_id) {
            return Err(format!(
                "{kind} `{id}` references missing conformance fixture `{ref_id}`"
            ));
        }
    }
    if channel.exact_capable() {
        if !requirements.iter().any(|requirement| requirement.required) {
            return Err(format!(
                "exact-capable {kind} `{id}` must declare at least one required evidence requirement"
            ));
        }
        if !semantics.contains_key("demand") || !semantics.contains_key("effects") {
            return Err(format!(
                "exact-capable {kind} `{id}` must declare demand and effects"
            ));
        }
        if !fixture_refs.has_positive_and_hard_negative_refs(conformance_refs) {
            return Err(format!(
                "exact-capable {kind} `{id}` must reference at least one positive and one hard-negative conformance fixture"
            ));
        }
    }
    for requirement in requirements {
        validate_requirement(id, requirement, known_refs)?;
    }
    Ok(())
}

fn validate_requirement(
    context: &str,
    requirement: &ManifestRequirement,
    known_refs: &HashSet<String>,
) -> Result<(), String> {
    require_non_empty("requirement.ref", &requirement.ref_id)?;
    require_non_empty("requirement.subject", &requirement.subject)?;
    let _required = requirement.required;
    optional_non_empty(
        "requirement.same_anchor_as",
        requirement.same_anchor_as.as_deref(),
    )?;
    optional_non_empty(
        "requirement.within_scope",
        requirement.within_scope.as_deref(),
    )?;
    optional_non_empty("requirement.before", requirement.before.as_deref())?;
    optional_non_empty("requirement.after", requirement.after.as_deref())?;
    if !known_refs.contains(&requirement.ref_id)
        && !ALLOWED_REQUIREMENT_PREFIXES
            .iter()
            .any(|prefix| requirement.ref_id.starts_with(prefix))
    {
        return Err(format!(
            "`{context}` requirement references unknown id `{}`",
            requirement.ref_id
        ));
    }
    if requirement.ref_id.starts_with_evidence_prefix()
        && !is_valid_evidence_kind(&requirement.ref_id)
    {
        return Err(format!(
            "`{context}` requirement has invalid evidence ref `{}`",
            requirement.ref_id
        ));
    }
    Ok(())
}

fn collect_unique_refs<'a>(
    label: &str,
    ids: impl Iterator<Item = &'a String>,
    out: &mut HashSet<String>,
) -> Result<(), String> {
    for id in ids {
        require_stable_id(label, id)?;
        if !out.insert(id.clone()) {
            return Err(format!("duplicate id `{id}` in `{label}`"));
        }
    }
    Ok(())
}

fn require_non_empty(label: &str, value: &str) -> Result<(), String> {
    if value.is_empty() {
        return Err(format!("`{label}` must be a non-empty string"));
    }
    Ok(())
}

fn optional_non_empty(label: &str, value: Option<&str>) -> Result<(), String> {
    if matches!(value, Some("")) {
        return Err(format!("`{label}` must be a non-empty string when present"));
    }
    Ok(())
}

fn validate_nose_version_requirement(label: &str, value: &str) -> Result<(), String> {
    require_non_empty(label, value)?;
    for constraint in value
        .split(|c: char| c.is_ascii_whitespace() || c == ',')
        .filter(|constraint| !constraint.is_empty())
    {
        let version = constraint
            .strip_prefix(">=")
            .or_else(|| constraint.strip_prefix("<="))
            .or_else(|| constraint.strip_prefix('>'))
            .or_else(|| constraint.strip_prefix('<'))
            .or_else(|| constraint.strip_prefix('='))
            .or_else(|| constraint.strip_prefix('^'))
            .or_else(|| constraint.strip_prefix('~'))
            .unwrap_or(constraint);
        if !is_version_like(version) {
            return Err(format!(
                "`{label}` contains unsupported version constraint `{constraint}`"
            ));
        }
    }
    Ok(())
}

fn is_version_like(value: &str) -> bool {
    let value = value.strip_prefix('v').unwrap_or(value);
    value == "*"
        || value.chars().next().is_some_and(|c| c.is_ascii_digit())
            && value.chars().all(|c| {
                c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+' | '*' | 'x' | 'X')
            })
}

fn require_stable_id(label: &str, value: &str) -> Result<(), String> {
    require_non_empty(label, value)?;
    let mut chars = value.chars();
    if !chars.next().is_some_and(|c| c.is_ascii_alphanumeric())
        || !chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | ':' | '-'))
    {
        return Err(format!("`{label}` has invalid stable id `{value}`"));
    }
    Ok(())
}

trait EvidenceKindPrefix {
    fn starts_with_evidence_prefix(&self) -> bool;
}

impl EvidenceKindPrefix for str {
    fn starts_with_evidence_prefix(&self) -> bool {
        ALLOWED_REQUIREMENT_PREFIXES[..ALLOWED_REQUIREMENT_PREFIXES.len() - 1]
            .iter()
            .any(|prefix| self.starts_with(prefix))
    }
}

fn is_valid_evidence_kind(value: &str) -> bool {
    let Some(prefix) = ALLOWED_REQUIREMENT_PREFIXES[..ALLOWED_REQUIREMENT_PREFIXES.len() - 1]
        .iter()
        .find(|prefix| value.starts_with(**prefix))
    else {
        return false;
    };
    let suffix = &value[prefix.len()..];
    !suffix.is_empty()
        && suffix
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | ':' | '-'))
}
