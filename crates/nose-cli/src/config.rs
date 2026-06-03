//! Project config file (`nose.toml` / `.nose.toml`) so settings can be committed
//! per-project instead of repeated on every command line. CLI flags always win;
//! the config supplies defaults; anything unset falls back to the built-in default.
//!
//! ```toml
//! [scan]
//! exclude = ["tests/**", "**/*.generated.ts", "vendor/**"]
//! min-value = 200
//! min-members = 3
//! threshold = 0.72
//! min-lines = 8
//! min-tokens = 30
//! ```

use serde::Deserialize;
use std::path::{Path, PathBuf};

/// The `[scan]` table. Every field is optional — absent means "no opinion,
/// use the CLI value or the built-in default".
#[derive(Deserialize, Default)]
#[serde(rename_all = "kebab-case", default)]
pub(crate) struct ScanConfig {
    pub exclude: Vec<String>,
    pub min_value: Option<f64>,
    pub min_members: Option<usize>,
    pub threshold: Option<f64>,
    pub min_lines: Option<u32>,
    pub min_tokens: Option<usize>,
    pub top: Option<usize>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
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
