//! Markdown near-duplicate detection as a **`nose query` domain** (epic #435).
//!
//! Converged from the former standalone `nose markdown` subcommand: per "capabilities over
//! features", duplication has one entry point (`nose query`). `nose query` discovers `.md` and
//! reports markdown near-duplicate families alongside code clones, using the separate
//! `nose-markdown` engine (char-n-gram MinHash/winnowing/TF-IDF/alignment — prose is not code).
//! Honesty contract: near-dup score + span witness + commonness evidence, never "same meaning"
//! or "worth removing". Dev golden-build/eval tooling lives in `nose-markdown`'s `mddup` example.

use nose_markdown::{detect, Family, Options};
use std::path::{Path, PathBuf};

/// Vendor/build directories never worth scanning for prose duplication — excluded even without a
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
fn discover(root: &Path, excludes: &[String]) -> Vec<PathBuf> {
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
    for dent in builder.build().flatten() {
        let p = dent.path();
        let is_md = matches!(
            p.extension().and_then(|e| e.to_str()),
            Some("md") | Some("markdown")
        );
        // Safety net: never report files under a vendor dir even if the override missed it.
        let vendored = p.components().any(|c| {
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
    out
}

/// Detect markdown near-duplicate families under `root` for the `nose query` dashboard.
pub(crate) fn detect_under(root: &Path, excludes: &[String]) -> Vec<Family> {
    let files = discover(root, excludes);
    let docs: Vec<(String, String)> = files
        .iter()
        .filter_map(|p| {
            std::fs::read(p).ok().map(|b| {
                (
                    p.to_string_lossy().into_owned(),
                    String::from_utf8_lossy(&b).into_owned(),
                )
            })
        })
        .collect();
    detect(&docs, &Options::default())
}

/// The `markdown` array for the query-JSON dashboard (additive, backwards-compatible). `Family`
/// already derives `Serialize`, so this is its faithful structured form.
pub(crate) fn families_json(fams: &[Family]) -> serde_json::Value {
    serde_json::to_value(fams).unwrap_or(serde_json::Value::Array(vec![]))
}

/// Human "Markdown near-duplicates" section appended to the `nose query` dashboard.
pub(crate) fn print_section(fams: &[Family], path: &str) {
    if fams.is_empty() {
        return;
    }
    let (templates, dups): (Vec<&Family>, Vec<&Family>) = fams.iter().partition(|f| f.template);
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
            "see all: nose query {path} --format json  # markdown[] array"
        ))
    );
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
