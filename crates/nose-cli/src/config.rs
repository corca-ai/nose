//! Project config file (`nose.toml` / `.nose.toml`) so settings can be committed
//! per-project instead of repeated on every command line. CLI flags always win;
//! the config supplies defaults; anything unset falls back to the built-in default.
//!
//! ```toml
//! [query]
//! exclude = ["tests/**", "**/*.generated.ts", "vendor/**"]
//! generated-paths = ["generated/**", "**/snapshots/mypy/**"]
//! mode = ["syntax", "semantic", "near:0.8"] # fuzzy thresholds ride on the mode
//! min-value = 200
//! sort = "extractability"
//! min-members = 3
//! min-size = 30                             # minimum unit size in IL tokens
//! ignore-file = "nose.ignore.json"
//! semantic-packs = ["semantic-packs/python-math-prod.json"]
//! semantic-pack-lock = "nose.semantic-pack-lock.json"
//! cache-max-bytes = 5368709120
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
    /// Root-anchored caller assertions; unlike excludes, these retain findings.
    pub generated_paths: Vec<String>,
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
    /// Maximum managed `--cache-dir` storage. The default is 5GiB.
    pub cache_max_bytes: Option<u64>,
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

pub(crate) fn discover_for_roots(roots: &[PathBuf]) -> anyhow::Result<PathBuf> {
    let mut bases = roots
        .iter()
        .map(|root| {
            let path = std::fs::canonicalize(root)?;
            Ok(if path.is_file() {
                path.parent().unwrap().to_path_buf()
            } else {
                path
            })
        })
        .collect::<std::io::Result<Vec<_>>>()?;
    let mut common = bases
        .pop()
        .ok_or_else(|| anyhow::anyhow!("configuration needs an analysis root"))?;
    for base in bases {
        while !base.starts_with(&common) {
            anyhow::ensure!(
                common.pop(),
                "analysis roots have no common configuration directory"
            );
        }
    }
    ["nose.toml", ".nose.toml"]
        .iter()
        .map(|name| common.join(name))
        .find(|path| path.is_file())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "no nose.toml or .nose.toml at common analysis root {}",
                common.display()
            )
        })
}

pub(crate) fn print_effective(args: &crate::cli_args::QueryArgs) -> anyhow::Result<()> {
    let source = args.config.clone().or_else(discover);
    let cfg = load_query(source.as_deref())?;
    let (settings, _) = crate::query_dataset::resolve_query_settings(
        args,
        crate::query_options::QUERY_DEFAULT_MODES,
    )?;
    let mut modes = Vec::new();
    if settings.channels.syntax {
        modes.push("syntax".to_owned());
    }
    if settings.channels.semantic {
        modes.push("semantic".to_owned());
    }
    if settings.channels.near {
        modes.push(format!("near:{}", settings.channels.threshold()));
    }
    if settings.channels.abstraction {
        modes.push(format!("abstraction:{}", settings.channels.threshold()));
    }
    let mut generated = cfg.generated_paths;
    generated.extend(args.generated_path.iter().cloned());
    let mut packs = cfg.semantic_packs;
    packs.extend(args.semantic_pack.iter().cloned());
    println!(
        "{}",
        serde_json::json!({
            "schema": "nose.query-config/v1", "config_file": source,
            "roots": args.paths, "cache_dir": args.cache_dir,
            "query": {
                "mode": modes, "min-members": settings.min_members,
                "min-value": settings.min_value, "min-lines": settings.min_lines,
                "min-size": settings.min_tokens, "sort": settings.sort,
                "exclude": settings.exclude, "generated-paths": generated,
                "cache-max-bytes": settings.cache_max_bytes,
                "ignore-file": args.ignore_file.clone().or(cfg.ignore_file).or_else(|| {
                    let path = PathBuf::from(crate::ignores::DEFAULT_IGNORE_FILE);
                    path.is_file().then_some(path)
                }),
                "semantic-packs": packs,
                "semantic-pack-lock": args.semantic_pack_lock.clone().or(cfg.semantic_pack_lock)
            }
        })
    );
    Ok(())
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
            "[query]\nmin-value = 200\nmin-size = 30\ncache-max-bytes = 1048576\ngenerated-paths = [\"generated/**\"]\nignore-file = \"nose.ignore.json\"\nsemantic-packs = [\"packs\"]\n",
        );
        let cfg = load_query(Some(&p)).expect("valid config must load");
        assert_eq!(cfg.min_value, Some(200.0));
        assert_eq!(cfg.min_size, Some(30));
        assert_eq!(cfg.cache_max_bytes, Some(1_048_576));
        assert_eq!(cfg.generated_paths, vec!["generated/**"]);
        assert_eq!(
            cfg.ignore_file,
            Some(p.parent().unwrap().join("nose.ignore.json"))
        );
        assert_eq!(cfg.semantic_packs, vec![p.parent().unwrap().join("packs")]);
        assert!(cfg.semantic_pack_lock.is_none());
    }
}
