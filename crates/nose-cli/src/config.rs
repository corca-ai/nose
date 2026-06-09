//! Project config file (`nose.toml` / `.nose.toml`) so settings can be committed
//! per-project instead of repeated on every command line. CLI flags always win;
//! the config supplies defaults; anything unset falls back to the built-in default.
//!
//! ```toml
//! [scan]
//! exclude = ["tests/**", "**/*.generated.ts", "vendor/**"]
//! mode = ["syntax", "semantic", "near:0.8"] # fuzzy thresholds ride on the mode
//! min-value = 200
//! sort = "extractability"
//! min-members = 3
//! min-size = 30                             # minimum unit size in IL tokens
//! ignore-file = "nose.ignore.json"
//! semantic-packs = ["semantic-packs/python-math-prod.json"]
//! ```

use serde::Deserialize;
use std::path::{Path, PathBuf};

/// The `[scan]` table. Every field is optional — absent means "no opinion,
/// use the CLI value or the built-in default".
#[derive(Deserialize, Default)]
#[serde(rename_all = "kebab-case", default, deny_unknown_fields)]
pub(crate) struct ScanConfig {
    pub exclude: Vec<String>,
    pub mode: Vec<crate::ScanMode>,
    pub min_value: Option<f64>,
    pub sort: Option<crate::SortKey>,
    pub min_members: Option<usize>,
    /// Advanced: minimum source-line span (most users only set `min-size`).
    pub min_lines: Option<u32>,
    /// Minimum unit size in IL tokens.
    pub min_size: Option<usize>,
    pub top: Option<usize>,
    pub ignore_file: Option<PathBuf>,
    /// Local semantic-pack v0 manifest files or directories. These are explicit opt-ins.
    pub semantic_packs: Vec<PathBuf>,
}

#[derive(Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct File {
    scan: ScanConfig,
}

/// Load the `[scan]` config: from `explicit` if given, else the first of
/// `nose.toml` / `.nose.toml` found in the current directory. Returns the default
/// (all-unset) config when there is no file. A malformed file is a hard error —
/// silently ignoring it would hide a typo'd setting.
pub(crate) fn load_scan(explicit: Option<&Path>) -> anyhow::Result<ScanConfig> {
    let path = match explicit {
        Some(p) => Some(p.to_path_buf()),
        None => discover(),
    };
    let Some(path) = path else {
        return Ok(ScanConfig::default());
    };
    let text = std::fs::read_to_string(&path)
        .map_err(|e| anyhow::anyhow!("reading config {}: {e}", path.display()))?;
    let file: File =
        toml::from_str(&text).map_err(|e| anyhow::anyhow!("parsing {}: {e}", path.display()))?;
    Ok(file.scan)
}

fn discover() -> Option<PathBuf> {
    ["nose.toml", ".nose.toml"]
        .iter()
        .map(PathBuf::from)
        .find(|p| p.is_file())
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
    fn unknown_scan_key_is_a_hard_error() {
        // `min-valeu` is a typo for `min-value`; silently dropping it would hide the setting.
        let p = write_cfg("badkey", "[scan]\nmin-valeu = 200\n");
        assert!(
            load_scan(Some(&p)).is_err(),
            "a typo'd key must be a hard error, not silently dropped"
        );
    }

    #[test]
    fn unknown_table_is_a_hard_error() {
        let p = write_cfg("badtable", "[scna]\nmin-value = 200\n");
        assert!(
            load_scan(Some(&p)).is_err(),
            "a typo'd table must be a hard error"
        );
    }

    #[test]
    fn valid_config_still_loads() {
        let p = write_cfg(
            "ok",
            "[scan]\nmin-value = 200\nmin-size = 30\nsemantic-packs = [\"packs\"]\n",
        );
        let cfg = load_scan(Some(&p)).expect("valid config must load");
        assert_eq!(cfg.min_value, Some(200.0));
        assert_eq!(cfg.min_size, Some(30));
        assert_eq!(cfg.semantic_packs, vec![PathBuf::from("packs")]);
    }
}
