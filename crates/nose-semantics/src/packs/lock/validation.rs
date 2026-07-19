use super::paths::*;
use super::version_ranges::requirements_may_overlap;
use super::*;
use crate::packs::{
    discover_manifest_paths, CompiledSemanticPackV1, SemanticPackV1Arity, SemanticPackV1ArityKind,
    SemanticPackV1Contract, SEMANTIC_PACK_API_VERSION_V1,
};
use std::collections::BTreeMap;
use std::path::Path;

pub fn create_project_lock(
    output_path: &Path,
    manifest_paths: &[PathBuf],
    options: SemanticPackLockOptions,
) -> Result<ValidatedSemanticPackProjectLock, SemanticPackLockError> {
    let document = build_lock_document(output_path, manifest_paths, options)?;
    let validated = validate_lock_document(output_path, document.clone())?;
    let mut encoded =
        serde_json::to_string_pretty(&document).map_err(|source| SemanticPackLockError::Json {
            path: output_path.to_path_buf(),
            source,
        })?;
    encoded.push('\n');
    std::fs::write(output_path, encoded).map_err(|source| SemanticPackLockError::Io {
        path: output_path.to_path_buf(),
        source,
    })?;
    Ok(validated)
}

pub fn validate_project_lock(
    lock_path: &Path,
) -> Result<ValidatedSemanticPackProjectLock, SemanticPackLockError> {
    let text = std::fs::read_to_string(lock_path).map_err(|source| SemanticPackLockError::Io {
        path: lock_path.to_path_buf(),
        source,
    })?;
    let document = serde_json::from_str(&text).map_err(|source| SemanticPackLockError::Json {
        path: lock_path.to_path_buf(),
        source,
    })?;
    validate_lock_document(lock_path, document)
}

fn build_lock_document(
    output_path: &Path,
    manifest_paths: &[PathBuf],
    options: SemanticPackLockOptions,
) -> Result<SemanticPackProjectLockV1, SemanticPackLockError> {
    let root = canonical_lock_root(output_path)?;
    let manifest_paths = discover_manifest_paths(manifest_paths)?;
    let semantic_packs = SemanticPackSet::new_local(&manifest_paths)?;
    let mut channels = options.allowed_channels;
    channels.sort();
    channels.dedup();
    require_non_empty(
        &channels,
        output_path,
        "at least one allowed channel is required",
    )?;
    require_non_empty(
        &options.dependency_paths,
        output_path,
        "at least one dependency evidence file is required",
    )?;
    let mut dependencies = options
        .dependency_paths
        .iter()
        .map(|path| pin_file(&root, path, output_path))
        .collect::<Result<Vec<_>, _>>()?;
    dependencies.sort_by(|left, right| left.path.cmp(&right.path));
    ensure_unique_paths(&dependencies, output_path, "dependency")?;

    let mut compiled = semantic_packs
        .compiled_external_v1_packs()
        .iter()
        .collect::<Vec<_>>();
    compiled.sort_by(|left, right| left.pack_id().cmp(right.pack_id()));
    if compiled.len() != manifest_paths.len() {
        return invalid(
            output_path,
            "project locks accept only typed nose.semantic-pack.v1 manifests; v0 remains metadata-only",
        );
    }
    if compiled.is_empty() {
        return invalid(output_path, "at least one v1 manifest is required");
    }
    if options.exact_receipt.is_some() && compiled.len() != 1 {
        return invalid(
            output_path,
            "--exact-receipt is accepted only when creating a lock for one pack",
        );
    }
    let selected = select_rows(&compiled, &channels, &options.selected_rows, output_path)?;
    let receipt = options
        .exact_receipt
        .as_ref()
        .map(|path| pin_file(&root, path, output_path))
        .transpose()?;
    let summaries = semantic_packs
        .packs()
        .iter()
        .filter_map(|summary| summary.api_version.map(|_| (summary.id.as_str(), summary)))
        .collect::<BTreeMap<_, _>>();
    let mut packs = Vec::with_capacity(compiled.len());
    for pack in compiled {
        let summary = summaries
            .get(pack.pack_id())
            .expect("compiled v1 pack has a summary");
        let manifest_path = summary
            .manifest_path
            .as_ref()
            .expect("external v1 summary has a manifest path");
        packs.push(SemanticPackLockedEntryV1 {
            manifest: relative_path(&root, manifest_path, output_path)?,
            manifest_api_version: SEMANTIC_PACK_API_VERSION_V1.to_string(),
            pack_id: pack.pack_id().to_string(),
            pack_version: pack.pack_version().to_string(),
            nose_compatibility: pack.nose_compatibility().to_string(),
            semantic_digest: pack.semantic_digest().to_string(),
            allowed_channels: channels.clone(),
            selected_rows: selected.get(pack.pack_id()).cloned().unwrap_or_default(),
            exact_receipt: receipt.clone(),
        });
    }
    Ok(SemanticPackProjectLockV1 {
        schema: None,
        api_version: SEMANTIC_PACK_LOCK_API_VERSION_V1.to_string(),
        dependencies,
        packs,
    })
}

fn select_rows(
    packs: &[&CompiledSemanticPackV1],
    channels: &[SemanticPackV1Channel],
    requested: &[String],
    lock_path: &Path,
) -> Result<BTreeMap<String, Vec<String>>, SemanticPackLockError> {
    let mut selected = BTreeMap::<String, Vec<String>>::new();
    if requested.is_empty() {
        for pack in packs {
            let rows = pack
                .contracts_by_id()
                .values()
                .filter(|contract| channels.binary_search(&contract.channel).is_ok())
                .map(|contract| contract.id.clone())
                .collect::<Vec<_>>();
            selected.insert(pack.pack_id().to_string(), rows);
        }
    } else {
        for row in requested {
            let (pack_id, row_id) = qualify_row(packs, row, lock_path)?;
            selected.entry(pack_id).or_default().push(row_id);
        }
    }
    for pack in packs {
        let rows = selected.entry(pack.pack_id().to_string()).or_default();
        rows.sort();
        if rows.is_empty() {
            return invalid(
                lock_path,
                format!("pack `{}` has no selected rows", pack.pack_id()),
            );
        }
        if rows.windows(2).any(|pair| pair[0] == pair[1]) {
            return invalid(
                lock_path,
                format!(
                    "pack `{}` contains a duplicate selected row",
                    pack.pack_id()
                ),
            );
        }
        for row_id in rows.iter() {
            let Some(contract) = pack.contracts_by_id().get(row_id) else {
                return invalid(
                    lock_path,
                    format!("pack `{}` has no row `{row_id}`", pack.pack_id()),
                );
            };
            if channels.binary_search(&contract.channel).is_err() {
                return invalid(
                    lock_path,
                    format!(
                        "row `{}/{row_id}` requests channel `{:?}`, which is not authorized",
                        pack.pack_id(),
                        contract.channel
                    ),
                );
            }
        }
    }
    Ok(selected)
}

fn qualify_row(
    packs: &[&CompiledSemanticPackV1],
    requested: &str,
    lock_path: &Path,
) -> Result<(String, String), SemanticPackLockError> {
    if let Some((pack_id, row_id)) = requested.split_once('/') {
        if pack_id.is_empty() || row_id.is_empty() {
            return invalid(lock_path, format!("invalid selected row `{requested}`"));
        }
        if !packs.iter().any(|pack| pack.pack_id() == pack_id) {
            return invalid(lock_path, format!("unknown selected-row pack `{pack_id}`"));
        }
        return Ok((pack_id.to_string(), row_id.to_string()));
    }
    if packs.len() == 1 {
        return Ok((packs[0].pack_id().to_string(), requested.to_string()));
    }
    invalid(
        lock_path,
        format!("selected row `{requested}` must use PACK_ID/ROW_ID with multiple packs"),
    )
}

fn validate_lock_document(
    lock_path: &Path,
    mut document: SemanticPackProjectLockV1,
) -> Result<ValidatedSemanticPackProjectLock, SemanticPackLockError> {
    let root = canonical_lock_root(lock_path)?;
    if document.api_version != SEMANTIC_PACK_LOCK_API_VERSION_V1 {
        return invalid(
            lock_path,
            format!(
                "unsupported `api_version` `{}`; expected {SEMANTIC_PACK_LOCK_API_VERSION_V1}",
                document.api_version
            ),
        );
    }
    require_non_empty(
        &document.dependencies,
        lock_path,
        "at least one dependency pin is required",
    )?;
    require_non_empty(
        &document.packs,
        lock_path,
        "at least one pack entry is required",
    )?;
    document
        .dependencies
        .sort_by(|left, right| left.path.cmp(&right.path));
    ensure_unique_paths(&document.dependencies, lock_path, "dependency")?;
    let dependencies = document
        .dependencies
        .iter()
        .map(|pin| validate_pin(&root, pin, lock_path))
        .collect::<Result<Vec<_>, _>>()?;
    document
        .packs
        .sort_by(|left, right| left.pack_id.cmp(&right.pack_id));
    ensure_unique_entries(&document.packs, lock_path)?;
    let manifest_paths = document
        .packs
        .iter()
        .map(|entry| resolve_relative(&root, &entry.manifest, lock_path))
        .collect::<Result<Vec<_>, _>>()?;
    let semantic_packs = SemanticPackSet::new_local(&manifest_paths)?;
    if semantic_packs.compiled_external_v1_packs().len() != document.packs.len() {
        return invalid(
            lock_path,
            "every lock entry must reference a typed v1 manifest; v0 can never be locked for influence",
        );
    }
    let authorizations = validate_authorizations(
        &root,
        &mut document.packs,
        &dependencies,
        &semantic_packs,
        lock_path,
    )?;
    validate_conflicts(
        semantic_packs.compiled_external_v1_packs(),
        &authorizations,
        lock_path,
    )?;
    document.schema = None;
    let decision_digest = digest_json(&document, lock_path)?;
    let lock_path = canonical_or_joined_lock_path(&root, lock_path)?;
    Ok(ValidatedSemanticPackProjectLock {
        summary: SemanticPackProjectLockSummary {
            api_version: SEMANTIC_PACK_LOCK_API_VERSION_V1,
            lock_path,
            decision_digest,
        },
        authorizations,
        semantic_packs,
    })
}

fn validate_authorizations(
    root: &Path,
    entries: &mut [SemanticPackLockedEntryV1],
    dependencies: &[SemanticPackLockedFile],
    semantic_packs: &SemanticPackSet,
    lock_path: &Path,
) -> Result<Vec<SemanticPackV1Authorization>, SemanticPackLockError> {
    let compiled = semantic_packs
        .compiled_external_v1_packs()
        .iter()
        .map(|pack| (pack.pack_id(), pack))
        .collect::<BTreeMap<_, _>>();
    let pack_ids_by_manifest = semantic_packs
        .packs()
        .iter()
        .filter_map(|summary| {
            summary
                .manifest_path
                .as_ref()
                .map(|path| (path.as_path(), summary.id.as_str()))
        })
        .collect::<BTreeMap<_, _>>();
    let mut authorizations = Vec::with_capacity(entries.len());
    for entry in entries {
        let resolved_manifest = resolve_relative(root, &entry.manifest, lock_path)?;
        if pack_ids_by_manifest
            .get(resolved_manifest.as_path())
            .copied()
            != Some(entry.pack_id.as_str())
        {
            return invalid(
                lock_path,
                format!(
                    "lock pack id `{}` does not match manifest `{}`",
                    entry.pack_id, entry.manifest
                ),
            );
        }
        let Some(pack) = compiled.get(entry.pack_id.as_str()).copied() else {
            return invalid(
                lock_path,
                format!(
                    "lock pack id `{}` does not match its manifest",
                    entry.pack_id
                ),
            );
        };
        validate_entry_identity(entry, pack, lock_path)?;
        entry.allowed_channels.sort();
        entry.selected_rows.sort();
        validate_entry_selection(entry, pack, lock_path)?;
        let exact_receipt = entry
            .exact_receipt
            .as_ref()
            .map(|pin| validate_pin(root, pin, lock_path))
            .transpose()?;
        authorizations.push(SemanticPackV1Authorization {
            pack_id: entry.pack_id.clone(),
            allowed_channels: entry.allowed_channels.clone(),
            selected_rows: entry.selected_rows.clone(),
            dependencies: dependencies.to_vec(),
            exact_receipt,
        });
    }
    Ok(authorizations)
}

fn validate_entry_identity(
    entry: &SemanticPackLockedEntryV1,
    pack: &CompiledSemanticPackV1,
    lock_path: &Path,
) -> Result<(), SemanticPackLockError> {
    let checks = [
        (
            entry.manifest_api_version.as_str(),
            SEMANTIC_PACK_API_VERSION_V1,
            "manifest API version",
        ),
        (
            entry.pack_version.as_str(),
            pack.pack_version(),
            "pack version",
        ),
        (
            entry.nose_compatibility.as_str(),
            pack.nose_compatibility(),
            "nose compatibility range",
        ),
        (
            entry.semantic_digest.as_str(),
            pack.semantic_digest(),
            "semantic content digest",
        ),
    ];
    for (locked, actual, label) in checks {
        if locked != actual {
            return invalid(
                lock_path,
                format!(
                    "pack `{}` {label} is stale: lock has `{locked}`, manifest has `{actual}`",
                    entry.pack_id
                ),
            );
        }
    }
    Ok(())
}

fn validate_entry_selection(
    entry: &SemanticPackLockedEntryV1,
    pack: &CompiledSemanticPackV1,
    lock_path: &Path,
) -> Result<(), SemanticPackLockError> {
    require_sorted_unique_non_empty(
        &entry.allowed_channels,
        lock_path,
        &format!("pack `{}` allowed_channels", entry.pack_id),
    )?;
    require_sorted_unique_non_empty(
        &entry.selected_rows,
        lock_path,
        &format!("pack `{}` selected_rows", entry.pack_id),
    )?;
    for row_id in &entry.selected_rows {
        let Some(contract) = pack.contracts_by_id().get(row_id) else {
            return invalid(
                lock_path,
                format!(
                    "pack `{}` no longer declares selected row `{row_id}`",
                    entry.pack_id
                ),
            );
        };
        if entry
            .allowed_channels
            .binary_search(&contract.channel)
            .is_err()
        {
            return invalid(
                lock_path,
                format!(
                    "selected row `{}/{row_id}` requests an unauthorized channel",
                    entry.pack_id
                ),
            );
        }
    }
    Ok(())
}

fn validate_conflicts(
    packs: &[CompiledSemanticPackV1],
    authorizations: &[SemanticPackV1Authorization],
    lock_path: &Path,
) -> Result<(), SemanticPackLockError> {
    let pack_map = packs
        .iter()
        .map(|pack| (pack.pack_id(), pack))
        .collect::<BTreeMap<_, _>>();
    let mut rows = Vec::new();
    for authorization in authorizations {
        let pack = pack_map[authorization.pack_id.as_str()];
        for row_id in &authorization.selected_rows {
            rows.push((pack, &pack.contracts_by_id()[row_id]));
        }
    }
    rows.sort_by(|left, right| {
        (left.0.pack_id(), left.1.id.as_str()).cmp(&(right.0.pack_id(), right.1.id.as_str()))
    });
    for left_index in 0..rows.len() {
        for right_index in (left_index + 1)..rows.len() {
            let (left_pack, left) = rows[left_index];
            let (right_pack, right) = rows[right_index];
            if left_pack.pack_id() == right_pack.pack_id() {
                continue;
            }
            if semantic_coordinates_overlap(left_pack, left, right_pack, right) {
                return invalid(
                    lock_path,
                    format!(
                        "selected rows `{}/{}` and `{}/{}` overlap a semantic coordinate; load order and provider precedence are forbidden",
                        left_pack.pack_id(),
                        left.id,
                        right_pack.pack_id(),
                        right.id
                    ),
                );
            }
        }
    }
    Ok(())
}

fn semantic_coordinates_overlap(
    left_pack: &CompiledSemanticPackV1,
    left: &SemanticPackV1Contract,
    right_pack: &CompiledSemanticPackV1,
    right: &SemanticPackV1Contract,
) -> bool {
    if left.language != right.language
        || left.package != right.package
        || left.import != right.import
        || left.call.shape != right.call.shape
        || left.call.member != right.call.member
        || left.call.receiver != right.call.receiver
        || !arities_overlap(&left.call.arity, &right.call.arity)
    {
        return false;
    }
    let left_versions = &left_pack.packages_by_coordinate()[&left.package].versions;
    let right_versions = &right_pack.packages_by_coordinate()[&right.package].versions;
    requirements_may_overlap(left_versions, right_versions)
}

fn arities_overlap(left: &SemanticPackV1Arity, right: &SemanticPackV1Arity) -> bool {
    match (left.kind, right.kind) {
        (SemanticPackV1ArityKind::Range, SemanticPackV1ArityKind::Range) => {
            left.min <= right.max && right.min <= left.max
        }
        (SemanticPackV1ArityKind::Set, SemanticPackV1ArityKind::Set) => left
            .values
            .iter()
            .any(|value| right.values.binary_search(value).is_ok()),
        (SemanticPackV1ArityKind::Range, SemanticPackV1ArityKind::Set) => right
            .values
            .iter()
            .any(|value| left.min <= Some(*value) && Some(*value) <= left.max),
        (SemanticPackV1ArityKind::Set, SemanticPackV1ArityKind::Range) => {
            arities_overlap(right, left)
        }
    }
}
