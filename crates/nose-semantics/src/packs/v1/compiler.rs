use super::*;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

pub(in crate::packs) fn compile_manifest_v1(
    manifest: &SemanticPackManifestV1,
) -> Result<CompiledSemanticPackV1, String> {
    validate_manifest_v1(manifest)?;

    let mut contracts = manifest.declares.api_contracts.clone();
    for contract in &mut contracts {
        contract.call.arity.canonicalize();
    }
    contracts.sort_by(|left, right| left.id.cmp(&right.id));
    let semantic_digest = semantic_digest(manifest, &contracts)?;
    let row_digests_by_id = contracts
        .iter()
        .map(|contract| Ok((contract.id.clone(), digest_json(contract)?)))
        .collect::<Result<BTreeMap<_, _>, String>>()?;
    let packages_by_coordinate = manifest
        .packages
        .iter()
        .cloned()
        .map(|package| {
            (
                SemanticPackV1PackageCoordinate {
                    ecosystem: package.ecosystem,
                    name: package.name.clone(),
                },
                package,
            )
        })
        .collect();
    let mut contracts_by_id = BTreeMap::new();
    let mut contract_ids_by_coordinate = BTreeMap::<_, Vec<_>>::new();
    let mut contract_ids_by_operation = BTreeMap::<_, Vec<_>>::new();
    for contract in contracts {
        let coordinate = contract.coordinate();
        contract_ids_by_coordinate
            .entry(coordinate)
            .or_default()
            .push(contract.id.clone());
        contract_ids_by_operation
            .entry(contract.operation)
            .or_default()
            .push(contract.id.clone());
        contracts_by_id.insert(contract.id.clone(), contract);
    }
    Ok(CompiledSemanticPackV1 {
        pack_id: manifest.pack.id.clone(),
        pack_version: manifest.pack.version.clone(),
        nose_compatibility: manifest.compatibility.nose.clone(),
        semantic_digest,
        row_digests_by_id,
        packages_by_coordinate,
        contracts_by_id,
        contract_ids_by_coordinate,
        contract_ids_by_operation,
    })
}

impl SemanticPackV1Contract {
    fn coordinate(&self) -> SemanticPackV1Coordinate {
        SemanticPackV1Coordinate {
            language: self.language,
            package: self.package.clone(),
            import: self.import.clone(),
            call_shape: self.call.shape,
            member: self.call.member.clone(),
            receiver: self.call.receiver,
        }
    }
}

impl SemanticPackV1Arity {
    fn canonicalize(&mut self) {
        self.values.sort_unstable();
    }

    fn validate(&self, contract_id: &str) -> Result<(), String> {
        match self.kind {
            SemanticPackV1ArityKind::Range => {
                let (Some(min), Some(max)) = (self.min, self.max) else {
                    return Err(format!(
                        "contract `{contract_id}` range arity requires `min` and `max`"
                    ));
                };
                if !self.values.is_empty() || min > max || max > MAX_V1_ARITY {
                    return Err(format!(
                        "contract `{contract_id}` range arity must satisfy min <= max <= {MAX_V1_ARITY} and omit `values`"
                    ));
                }
            }
            SemanticPackV1ArityKind::Set => {
                if self.min.is_some() || self.max.is_some() || self.values.is_empty() {
                    return Err(format!(
                        "contract `{contract_id}` set arity requires non-empty `values` and omits `min`/`max`"
                    ));
                }
                let unique = self.values.iter().copied().collect::<BTreeSet<_>>();
                if unique.len() != self.values.len()
                    || self.values.iter().any(|value| *value > MAX_V1_ARITY)
                {
                    return Err(format!(
                        "contract `{contract_id}` set arity values must be unique and <= {MAX_V1_ARITY}"
                    ));
                }
            }
        }
        Ok(())
    }

    fn all_values_satisfy(&self, predicate: impl Fn(u8) -> bool) -> bool {
        match self.kind {
            SemanticPackV1ArityKind::Range => {
                let (Some(min), Some(max)) = (self.min, self.max) else {
                    return false;
                };
                (min..=max).all(predicate)
            }
            SemanticPackV1ArityKind::Set => self.values.iter().copied().all(predicate),
        }
    }
}

fn validate_manifest_v1(manifest: &SemanticPackManifestV1) -> Result<(), String> {
    if manifest.api_version != SEMANTIC_PACK_API_VERSION_V1 {
        return Err(format!(
            "`api_version` must be {SEMANTIC_PACK_API_VERSION_V1}, got `{}`",
            manifest.api_version
        ));
    }
    require_stable_id("pack.id", &manifest.pack.id)?;
    require_semver("pack.version", &manifest.pack.version)?;
    require_non_empty("pack.display_name", &manifest.pack.display_name)?;
    optional_non_empty("pack.description", manifest.pack.description.as_deref())?;
    if manifest.pack.kind != SemanticPackKind::LibraryPack {
        return Err("semantic-pack v1 currently accepts only LibraryPack manifests".to_string());
    }
    if manifest.pack.enabled_by_default {
        return Err("local semantic-pack v1 manifests must be disabled by default".to_string());
    }
    let _trust = manifest.pack.trust;
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
    validate_version_requirement("compatibility.nose", &manifest.compatibility.nose, true)?;
    validate_targets(manifest)?;
    validate_contracts(manifest)
}

fn validate_targets(manifest: &SemanticPackManifestV1) -> Result<(), String> {
    if manifest.supported_languages.is_empty() {
        return Err("`supported_languages` must contain at least one language".to_string());
    }
    let languages = manifest
        .supported_languages
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if languages.len() != manifest.supported_languages.len() {
        return Err("`supported_languages` must not contain duplicates".to_string());
    }
    if manifest.packages.is_empty() {
        return Err("`packages` must contain at least one package coordinate".to_string());
    }
    let mut packages = BTreeSet::new();
    for package in &manifest.packages {
        validate_maven_coordinate("packages[].name", &package.name)?;
        validate_version_requirement("packages[].versions", &package.versions, false)?;
        if !packages.insert((package.ecosystem, package.name.clone())) {
            return Err(format!(
                "duplicate package coordinate `{:?}:{}`",
                package.ecosystem, package.name
            ));
        }
    }
    Ok(())
}

fn validate_contracts(manifest: &SemanticPackManifestV1) -> Result<(), String> {
    if manifest.declares.api_contracts.is_empty() {
        return Err("`declares.api_contracts` must contain at least one contract".to_string());
    }
    let languages = manifest
        .supported_languages
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let packages = manifest
        .packages
        .iter()
        .map(|package| (package.ecosystem, package.name.as_str()))
        .collect::<BTreeSet<_>>();
    let mut ids = BTreeSet::new();
    let mut exact_rows = BTreeSet::new();
    for contract in &manifest.declares.api_contracts {
        require_stable_id("declares.api_contracts[].id", &contract.id)?;
        if !ids.insert(contract.id.as_str()) {
            return Err(format!("duplicate v1 contract id `{}`", contract.id));
        }
        if !languages.contains(&contract.language) {
            return Err(format!(
                "contract `{}` language is not declared in `supported_languages`",
                contract.id
            ));
        }
        validate_maven_coordinate(
            "declares.api_contracts[].package.name",
            &contract.package.name,
        )?;
        if !packages.contains(&(contract.package.ecosystem, contract.package.name.as_str())) {
            return Err(format!(
                "contract `{}` package is not declared in `packages`",
                contract.id
            ));
        }
        validate_java_path(
            "declares.api_contracts[].import.module",
            &contract.import.module,
        )?;
        validate_java_identifier(
            "declares.api_contracts[].import.name",
            &contract.import.name,
        )?;
        validate_java_identifier(
            "declares.api_contracts[].call.member",
            &contract.call.member,
        )?;
        validate_call_shape(contract)?;
        contract.call.arity.validate(&contract.id)?;
        validate_operation_domain(contract)?;
        let mut arity = contract.call.arity.clone();
        arity.canonicalize();
        let exact_key = (contract.coordinate(), arity);
        if !exact_rows.insert(exact_key) {
            return Err(format!(
                "contract `{}` duplicates an existing package API coordinate and arity",
                contract.id
            ));
        }
    }
    Ok(())
}

fn validate_call_shape(contract: &SemanticPackV1Contract) -> Result<(), String> {
    let valid = matches!(
        (
            contract.import.role,
            contract.call.shape,
            contract.call.receiver
        ),
        (
            SemanticPackV1ImportRole::Type,
            SemanticPackV1CallShape::StaticMethod,
            SemanticPackV1ReceiverRole::ImportedType
        ) | (
            SemanticPackV1ImportRole::StaticMember,
            SemanticPackV1CallShape::FreeFunction,
            SemanticPackV1ReceiverRole::None
        )
    );
    if valid {
        Ok(())
    } else {
        Err(format!(
            "contract `{}` has an incompatible import role, call shape, and receiver role",
            contract.id
        ))
    }
}

fn validate_operation_domain(contract: &SemanticPackV1Contract) -> Result<(), String> {
    let valid = match (contract.operation, contract.result_domain) {
        (
            SemanticPackV1ProtocolOperation::CollectionFactory,
            SemanticPackV1ResultDomain::Collection,
        ) => true,
        (SemanticPackV1ProtocolOperation::MapFactory, SemanticPackV1ResultDomain::Map) => contract
            .call
            .arity
            .all_values_satisfy(|arity| arity % 2 == 0),
        (SemanticPackV1ProtocolOperation::CollectionFactory, SemanticPackV1ResultDomain::Map)
        | (SemanticPackV1ProtocolOperation::MapFactory, SemanticPackV1ResultDomain::Collection) => {
            false
        }
    };
    if valid {
        Ok(())
    } else {
        Err(format!(
            "contract `{}` operation and fixed result domain are incompatible",
            contract.id
        ))
    }
}

#[derive(Serialize)]
struct CanonicalSemanticContent<'a> {
    api_version: &'static str,
    pack_kind: &'static str,
    supported_languages: Vec<SemanticPackV1Language>,
    packages: Vec<SemanticPackV1Package>,
    api_contracts: &'a [SemanticPackV1Contract],
}

fn semantic_digest(
    manifest: &SemanticPackManifestV1,
    contracts: &[SemanticPackV1Contract],
) -> Result<String, String> {
    let mut supported_languages = manifest.supported_languages.clone();
    supported_languages.sort();
    let mut packages = manifest.packages.clone();
    packages.sort();
    let canonical = CanonicalSemanticContent {
        api_version: SEMANTIC_PACK_API_VERSION_V1,
        pack_kind: manifest.pack.kind.as_str(),
        supported_languages,
        packages,
        api_contracts: contracts,
    };
    digest_json(&canonical)
}

fn digest_json(value: &impl Serialize) -> Result<String, String> {
    let bytes = serde_json::to_vec(value)
        .map_err(|err| format!("serializing canonical v1 semantic content: {err}"))?;
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(64);
    for byte in digest {
        write!(&mut hex, "{byte:02x}").expect("writing to String cannot fail");
    }
    Ok(format!("sha256:{hex}"))
}

fn require_non_empty(label: &str, value: &str) -> Result<(), String> {
    if value.is_empty() {
        Err(format!("`{label}` must be a non-empty string"))
    } else {
        Ok(())
    }
}

fn optional_non_empty(label: &str, value: Option<&str>) -> Result<(), String> {
    if matches!(value, Some("")) {
        Err(format!("`{label}` must be a non-empty string when present"))
    } else {
        Ok(())
    }
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

fn require_semver(label: &str, value: &str) -> Result<(), String> {
    semver::Version::parse(value)
        .map(|_| ())
        .map_err(|err| format!("`{label}` must be a semantic version: {err}"))
}

fn validate_version_requirement(
    label: &str,
    value: &str,
    require_current: bool,
) -> Result<(), String> {
    require_non_empty(label, value)?;
    let normalized = value
        .replace(',', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(",");
    let requirement = semver::VersionReq::parse(&normalized).map_err(|err| {
        format!("`{label}` contains unsupported version requirement `{value}`: {err}")
    })?;
    if require_current {
        let current = semver::Version::parse(env!("CARGO_PKG_VERSION"))
            .map_err(|err| format!("current nose version is invalid: {err}"))?;
        if !requirement.matches(&current) {
            return Err(format!(
                "`{label}` range `{value}` does not include this nose binary version `{current}`"
            ));
        }
    }
    Ok(())
}

fn validate_maven_coordinate(label: &str, value: &str) -> Result<(), String> {
    let Some((group, artifact)) = value.split_once(':') else {
        return Err(format!(
            "`{label}` must be an exact Maven `group:artifact` coordinate"
        ));
    };
    if artifact.contains(':') || !valid_java_path(group) || !valid_maven_artifact(artifact) {
        return Err(format!("`{label}` has invalid Maven coordinate `{value}`"));
    }
    Ok(())
}

fn validate_java_path(label: &str, value: &str) -> Result<(), String> {
    if valid_java_path(value) {
        Ok(())
    } else {
        Err(format!(
            "`{label}` has invalid exact Java module path `{value}`"
        ))
    }
}

fn valid_java_path(value: &str) -> bool {
    !value.is_empty() && value.split('.').all(valid_coordinate_segment)
}

fn validate_java_identifier(label: &str, value: &str) -> Result<(), String> {
    if valid_coordinate_segment(value) {
        Ok(())
    } else {
        Err(format!(
            "`{label}` has invalid exact Java identifier `{value}`"
        ))
    }
}

fn valid_coordinate_segment(value: &str) -> bool {
    let mut chars = value.chars();
    chars
        .next()
        .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_' || ch == '$')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '$')
}

fn valid_maven_artifact(value: &str) -> bool {
    let mut chars = value.chars();
    chars
        .next()
        .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '$')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | '$'))
}
