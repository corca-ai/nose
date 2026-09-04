//! Markdown near-duplicate detection as a **`nose query` domain** (epic #435).
//!
//! Converged from the former standalone `nose markdown` subcommand: per "capabilities over
//! features", duplication has one entry point (`nose query`). `nose query` discovers `.md` and
//! reports markdown near-duplicate families alongside code clones, using the separate
//! `nose-markdown` engine (char-n-gram MinHash/winnowing/TF-IDF/alignment — prose is not code).
//! Honesty contract: near-dup score + span witness + commonness evidence, never "same meaning"
//! or "worth removing". Dev golden-build/eval tooling lives in `nose-markdown`'s `mddup` example.

use anyhow::{Context, Result};
use nose_markdown::{detect, Family, Options};
use rayon::prelude::*;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Vendor/build directories never worth checking for prose duplication — excluded even without a
/// `.gitignore` (a non-git project's `node_modules` otherwise floods the report).
const DEFAULT_EXCLUDE_DIRS: &[&str] = &[
    "node_modules",
    "vendor",
    ".venv",
    "venv",
    "dist",
    "build",
    "target",
    "bower_components",
    ".next",
    ".nuxt",
    "site-packages",
    ".tox",
    ".mypy_cache",
    "__pycache__",
    ".cache",
    ".git",
];

/// Discover `.md`/`.markdown` files under `root`, respecting `.gitignore`, default vendor-dir
/// excludes, and the query's `exclude` globs (config + `--exclude`).
fn discover(root: &Path, excludes: &[String]) -> Result<Vec<PathBuf>> {
    use ignore::overrides::OverrideBuilder;
    let mut builder = ignore::WalkBuilder::new(root);
    builder.parents(false).require_git(false);
    let mut ob = OverrideBuilder::new(root);
    for d in DEFAULT_EXCLUDE_DIRS {
        let _ = ob.add(&format!("!**/{d}/**"));
        let _ = ob.add(&format!("!**/{d}"));
    }
    for g in excludes {
        let _ = ob.add(&format!("!{g}"));
    }
    if let Ok(ov) = ob.build() {
        builder.overrides(ov);
    }
    let mut out = Vec::new();
    for dent in builder.build() {
        let dent =
            dent.with_context(|| format!("discovering Markdown under {}", root.display()))?;
        if let Some(error) = dent.error() {
            anyhow::bail!("discovering Markdown under {}: {error}", root.display());
        }
        let p = dent.path();
        let is_md = matches!(
            p.extension().and_then(|e| e.to_str()),
            Some("md") | Some("markdown")
        );
        // Safety net: never report files under a vendor dir even if the override missed it.
        let relative = p.strip_prefix(root).unwrap_or(p);
        let vendored = relative.components().any(|c| {
            c.as_os_str()
                .to_str()
                .is_some_and(|s| DEFAULT_EXCLUDE_DIRS.contains(&s))
        });
        if dent.file_type().is_some_and(|t| t.is_file()) && is_md && !vendored {
            out.push(p.to_path_buf());
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

/// Query-facing Markdown domain report. The dashboard owns presentation order; this adapter owns
/// discovery, the `markdown[]` JSON field, and the prose-domain honesty wording.
pub(crate) struct QueryMarkdownReport {
    families: Vec<Family>,
}

impl QueryMarkdownReport {
    /// Detect Markdown near-duplicate families under the `nose query` roots for the dashboard.
    pub(crate) fn detect_under(roots: &[PathBuf], excludes: &[String]) -> Result<Self> {
        let files = discover_roots(roots, excludes)?;
        let docs = files
            .par_iter()
            .map(|path| {
                let bytes = std::fs::read(path)
                    .with_context(|| format!("reading Markdown {}", path.display()))?;
                Ok((
                    path.to_string_lossy().into_owned(),
                    String::from_utf8_lossy(&bytes).into_owned(),
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(QueryMarkdownReport {
            families: detect(&docs, &Options::default()),
        })
    }

    pub(crate) fn has_findings(&self) -> bool {
        !self.families.is_empty()
    }

    /// The `markdown` array for the query-JSON dashboard (additive, backwards-compatible).
    /// `Family` already derives `Serialize`, so this is its faithful structured form.
    pub(crate) fn dashboard_json(&self) -> serde_json::Value {
        serde_json::to_value(&self.families).unwrap_or(serde_json::Value::Array(vec![]))
    }

    /// Human "Markdown near-duplicates" section appended to the `nose query` dashboard.
    pub(crate) fn print_dashboard_section(&self, path: &str) {
        if self.families.is_empty() {
            return;
        }
        let (templates, dups): (Vec<&Family>, Vec<&Family>) =
            self.families.iter().partition(|f| f.template);
        println!(
            "\n{} ({}, {} templated)",
            crate::style::bold("markdown near-duplicates"),
            plural(dups.len(), "family", "families"),
            templates.len(),
        );
        println!(
            "  {}",
            crate::style::dim(
                "prose near-dup: score + span witness + commonness; not a worth-it verdict"
            )
        );
        for f in dups.iter().take(5) {
            let common = if f.commonness > 0.25 {
                "  [common]"
            } else {
                ""
            };
            let head = f
                .members
                .first()
                .and_then(|m| m.heading.as_deref())
                .map(|h| short(h, 48))
                .unwrap_or_default();
            let loc = f
                .members
                .first()
                .map(|m| format!("{}:{}-{}", file_only(&m.path), m.start_line, m.end_line))
                .unwrap_or_default();
            println!(
                "  {loc:<40}  {} copies · {} · ~{} removable · {}{}",
                f.members.len(),
                crate::style::blue(f.tier),
                f.removable,
                short(&head, 40),
                crate::style::dim(common),
            );
        }
        if !templates.is_empty() {
            println!(
                "  {}",
                crate::style::dim(&format!(
                    "+ {} templated section(s) (one skeleton repeated across files)",
                    templates.len()
                ))
            );
        }
        println!(
            "  {}",
            crate::style::dim(&format!(
                "see all: nose query {path} --format json  # top-level markdown array"
            ))
        );
    }
}

fn discover_roots(roots: &[PathBuf], excludes: &[String]) -> Result<Vec<PathBuf>> {
    let mut seen = HashSet::new();
    let mut files = Vec::new();
    for root in roots {
        for path in discover(root, excludes)? {
            let key = path.canonicalize().unwrap_or_else(|_| path.clone());
            if seen.insert(key) {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

fn short(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let t: String = s.chars().take(n).collect();
        format!("{t}\u{2026}")
    }
}

fn file_only(p: &str) -> &str {
    Path::new(p)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or(p)
}
