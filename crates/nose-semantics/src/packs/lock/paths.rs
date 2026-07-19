use super::{
    SemanticPackLockError, SemanticPackLockedEntryV1, SemanticPackLockedFile,
    SemanticPackLockedFileV1,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::{Component, Path, PathBuf};

pub(super) fn canonical_lock_root(lock_path: &Path) -> Result<PathBuf, SemanticPackLockError> {
    let parent = lock_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    parent
        .canonicalize()
        .map_err(|source| SemanticPackLockError::Io {
            path: parent.to_path_buf(),
            source,
        })
}

pub(super) fn canonical_or_joined_lock_path(
    root: &Path,
    lock_path: &Path,
) -> Result<PathBuf, SemanticPackLockError> {
    if lock_path.exists() {
        lock_path
            .canonicalize()
            .map_err(|source| SemanticPackLockError::Io {
                path: lock_path.to_path_buf(),
                source,
            })
    } else {
        let name = lock_path
            .file_name()
            .ok_or_else(|| SemanticPackLockError::Invalid {
                path: lock_path.to_path_buf(),
                message: "lock output must name a file".to_string(),
            })?;
        Ok(root.join(name))
    }
}

pub(super) fn relative_path(
    root: &Path,
    path: &Path,
    lock_path: &Path,
) -> Result<String, SemanticPackLockError> {
    let canonical = path
        .canonicalize()
        .map_err(|source| SemanticPackLockError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    let relative = canonical
        .strip_prefix(root)
        .map_err(|_| SemanticPackLockError::Invalid {
            path: lock_path.to_path_buf(),
            message: format!(
                "locked path {} escapes project lock root {}",
                path.display(),
                root.display()
            ),
        })?;
    if relative.as_os_str().is_empty() {
        return invalid(
            lock_path,
            "locked paths must name files below the lock root",
        );
    }
    Ok(relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/"))
}

pub(super) fn resolve_relative(
    root: &Path,
    declared: &str,
    lock_path: &Path,
) -> Result<PathBuf, SemanticPackLockError> {
    let relative = Path::new(declared);
    if declared.is_empty()
        || relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return invalid(
            lock_path,
            format!("locked path `{declared}` must be a project-relative non-escaping path"),
        );
    }
    let joined = root.join(relative);
    let canonical = joined
        .canonicalize()
        .map_err(|source| SemanticPackLockError::Io {
            path: joined.clone(),
            source,
        })?;
    if !canonical.starts_with(root) {
        return invalid(
            lock_path,
            format!("locked path `{declared}` escapes through a symlink"),
        );
    }
    Ok(canonical)
}

pub(super) fn pin_file(
    root: &Path,
    path: &Path,
    lock_path: &Path,
) -> Result<SemanticPackLockedFileV1, SemanticPackLockError> {
    let relative = relative_path(root, path, lock_path)?;
    let resolved = resolve_relative(root, &relative, lock_path)?;
    Ok(SemanticPackLockedFileV1 {
        path: relative,
        content_digest: digest_file(&resolved)?,
    })
}

pub(super) fn validate_pin(
    root: &Path,
    pin: &SemanticPackLockedFileV1,
    lock_path: &Path,
) -> Result<SemanticPackLockedFile, SemanticPackLockError> {
    let resolved_path = resolve_relative(root, &pin.path, lock_path)?;
    let actual = digest_file(&resolved_path)?;
    if pin.content_digest != actual {
        return invalid(
            lock_path,
            format!(
                "locked file `{}` changed: lock has `{}`, current content is `{actual}`",
                pin.path, pin.content_digest
            ),
        );
    }
    Ok(SemanticPackLockedFile {
        declared_path: pin.path.clone(),
        resolved_path,
        content_digest: actual,
    })
}

fn digest_file(path: &Path) -> Result<String, SemanticPackLockError> {
    let bytes = std::fs::read(path).map_err(|source| SemanticPackLockError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(format_digest(Sha256::digest(bytes)))
}

pub(super) fn digest_json(
    value: &impl Serialize,
    path: &Path,
) -> Result<String, SemanticPackLockError> {
    let bytes = serde_json::to_vec(value).map_err(|source| SemanticPackLockError::Json {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(format_digest(Sha256::digest(bytes)))
}

fn format_digest(bytes: impl IntoIterator<Item = u8>) -> String {
    let mut hex = String::with_capacity(64);
    for byte in bytes {
        write!(&mut hex, "{byte:02x}").expect("writing to String cannot fail");
    }
    format!("sha256:{hex}")
}

pub(super) fn ensure_unique_paths(
    pins: &[SemanticPackLockedFileV1],
    lock_path: &Path,
    label: &str,
) -> Result<(), SemanticPackLockError> {
    if pins.windows(2).any(|pair| pair[0].path == pair[1].path) {
        invalid(lock_path, format!("duplicate {label} path in project lock"))
    } else {
        Ok(())
    }
}

pub(super) fn ensure_unique_entries(
    entries: &[SemanticPackLockedEntryV1],
    lock_path: &Path,
) -> Result<(), SemanticPackLockError> {
    let ids = entries
        .iter()
        .map(|entry| entry.pack_id.as_str())
        .collect::<BTreeSet<_>>();
    let paths = entries
        .iter()
        .map(|entry| entry.manifest.as_str())
        .collect::<BTreeSet<_>>();
    if ids.len() != entries.len() || paths.len() != entries.len() {
        invalid(lock_path, "pack ids and manifest paths must be unique")
    } else {
        Ok(())
    }
}

pub(super) fn require_sorted_unique_non_empty<T: Ord>(
    values: &[T],
    lock_path: &Path,
    label: &str,
) -> Result<(), SemanticPackLockError> {
    if values.is_empty() {
        return invalid(lock_path, format!("{label} must not be empty"));
    }
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return invalid(lock_path, format!("{label} must not contain duplicates"));
    }
    Ok(())
}

pub(super) fn require_non_empty<T>(
    values: &[T],
    lock_path: &Path,
    message: impl Into<String>,
) -> Result<(), SemanticPackLockError> {
    if values.is_empty() {
        invalid(lock_path, message)
    } else {
        Ok(())
    }
}

pub(super) fn invalid<T>(
    path: &Path,
    message: impl Into<String>,
) -> Result<T, SemanticPackLockError> {
    Err(SemanticPackLockError::Invalid {
        path: path.to_path_buf(),
        message: message.into(),
    })
}
