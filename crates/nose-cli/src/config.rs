//! Project config file (`nose.toml` / `.nose.toml`) so settings can be committed
//! per-project instead of repeated on every command line. CLI flags always win;
//! the config supplies defaults; anything unset falls back to the built-in default.
//!
//! ```toml
//! [query]
//! exclude = ["tests/**", "**/*.generated.ts", "vendor/**"]
//! mode = ["syntax", "semantic", "near:0.8"] # fuzzy thresholds ride on the mode
//! min-value = 200
//! sort = "extractability"
//! min-members = 3
//! min-size = 30                             # minimum unit size in IL tokens
//! ignore-file = "nose.ignore.json"
//! semantic-packs = ["semantic-packs/python-math-prod.json"]
//! semantic-pack-lock = "nose.semantic-pack-lock.json"
//! ```

use serde::Deserialize;
use std::path::{Path, PathBuf};

use crate::query_options::{DetectionMode, SortKey};

/// The `[query]` table. Every field is optional — absent means "no opinion,
/// use the CLI value or the built-in default".
#[derive(Deserialize, Default)]
#[serde(rename_all = "kebab-case", default, deny_unknown_fields)]
pub(crate) struct QueryConfig {
    pub exclude: Vec<String>,
    pub mode: Vec<DetectionMode>,
    pub min_value: Option<f64>,
    pub sort: Option<SortKey>,
    pub min_members: Option<usize>,
    /// Advanced: minimum source-line span (most users only set `min-size`).
    pub min_lines: Option<u32>,
    /// Minimum unit size in IL tokens.
    pub min_size: Option<usize>,
    pub ignore_file: Option<PathBuf>,
    /// Local semantic-pack v0/v1 manifest files or directories. These are explicit opt-ins.
    pub semantic_packs: Vec<PathBuf>,
    /// Content-pinned v1 project lock. It is mutually exclusive with `semantic-packs`.
    pub semantic_pack_lock: Option<PathBuf>,
}

#[derive(Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct File {
    query: QueryConfig,
}

/// Load the `[query]` config: from `explicit` if given, else the first of
/// `nose.toml` / `.nose.toml` found in the current directory. Returns the default
/// (all-unset) config when there is no file. A malformed file is a hard error —
/// silently ignoring it would hide a typo'd setting.
pub(crate) fn load_query(explicit: Option<&Path>) -> anyhow::Result<QueryConfig> {
    let path = match explicit {
        Some(p) => Some(p.to_path_buf()),
        None => discover(),
    };
    let Some(path) = path else {
        return Ok(QueryConfig::default());
    };
    let text = std::fs::read_to_string(&path)
        .map_err(|e| anyhow::anyhow!("reading config {}: {e}", path.display()))?;
    let file: File =
        toml::from_str(&text).map_err(|e| anyhow::anyhow!("parsing {}: {e}", path.display()))?;
    Ok(resolve_config_relative_paths(file.query, &path))
}

fn discover() -> Option<PathBuf> {
    ["nose.toml", ".nose.toml"]
        .iter()
        .map(PathBuf::from)
        .find(|p| p.is_file())
}

fn resolve_config_relative_paths(mut cfg: QueryConfig, path: &Path) -> QueryConfig {
    let base = path.parent().unwrap_or_else(|| Path::new(""));
    if let Some(ignore_file) = &mut cfg.ignore_file {
        if ignore_file.is_relative() {
            *ignore_file = base.join(&ignore_file);
        }
    }
    for pack in &mut cfg.semantic_packs {
        if pack.is_relative() {
            *pack = base.join(&pack);
        }
    }
    if let Some(lock) = &mut cfg.semantic_pack_lock {
        if lock.is_relative() {
            *lock = base.join(&lock);
        }
    }
    cfg
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_cfg(tag: &str, body: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nose_cfg_{tag}_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let p = dir.join("nose.toml");
        std::fs::write(&p, body).unwrap();
        p
    }

    #[test]
    fn unknown_query_key_is_a_hard_error() {
        // `min-valeu` is a typo for `min-value`; silently dropping it would hide the setting.
        let p = write_cfg("badkey", "[query]\nmin-valeu = 200\n");
        assert!(
            load_query(Some(&p)).is_err(),
            "a typo'd key must be a hard error, not silently dropped"
        );
    }

    #[test]
    fn unknown_table_is_a_hard_error() {
        let p = write_cfg("badtable", "[scna]\nmin-value = 200\n");
        assert!(
            load_query(Some(&p)).is_err(),
            "a typo'd table must be a hard error"
        );
    }

    #[test]
    fn valid_config_still_loads() {
        let p = write_cfg(
            "ok",
            "[query]\nmin-value = 200\nmin-size = 30\nignore-file = \"nose.ignore.json\"\nsemantic-packs = [\"packs\"]\n",
        );
        let cfg = load_query(Some(&p)).expect("valid config must load");
        assert_eq!(cfg.min_value, Some(200.0));
        assert_eq!(cfg.min_size, Some(30));
        assert_eq!(
            cfg.ignore_file,
            Some(p.parent().unwrap().join("nose.ignore.json"))
        );
        assert_eq!(cfg.semantic_packs, vec![p.parent().unwrap().join("packs")]);
        assert!(cfg.semantic_pack_lock.is_none());
    }
}
