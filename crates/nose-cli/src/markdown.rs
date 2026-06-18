//! `nose markdown` — same-language near-duplicate detection for Markdown documents.
//!
//! Runs the `nose-markdown` pipeline (units → MinHash/winnowing candidates → TF-IDF/containment
//! verify → local-alignment witness) and surfaces ranked near-duplicate families with a span
//! witness + orthogonal evidence (commonness, removable, files). Honesty contract (epic #435):
//! it reports near-duplication, never "same meaning" and never "worth removing".

use anyhow::{Context, Result};
use nose_markdown::{detect, Family, Options};
use std::path::{Path, PathBuf};

/// Vendor/build directories never worth scanning for prose duplication. Excluded even without a
/// `.gitignore` — the field eval hit a non-git project whose `node_modules` flooded the report.
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

/// Discover `.md`/`.markdown` files under the given paths, respecting `.gitignore`, default
/// vendor-dir excludes, and `nose.toml` `exclude` globs.
fn discover(paths: &[PathBuf]) -> Vec<PathBuf> {
    use ignore::overrides::OverrideBuilder;
    let cfg_excludes = crate::config::load_scan(None)
        .map(|c| c.exclude)
        .unwrap_or_default();

    let mut out = Vec::new();
    for root in paths {
        let mut builder = ignore::WalkBuilder::new(root);
        builder.parents(false).require_git(false);
        let mut ob = OverrideBuilder::new(root);
        for d in DEFAULT_EXCLUDE_DIRS {
            let _ = ob.add(&format!("!**/{d}/**"));
            let _ = ob.add(&format!("!**/{d}"));
        }
        for g in &cfg_excludes {
            let _ = ob.add(&format!("!{g}"));
        }
        if let Ok(ov) = ob.build() {
            builder.overrides(ov);
        }
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
    }
    out.sort();
    out.dedup();
    out
}

fn read_docs(files: &[PathBuf]) -> Vec<(String, String)> {
    files
        .iter()
        .filter_map(|p| {
            std::fs::read(p).ok().map(|b| {
                (
                    p.to_string_lossy().into_owned(),
                    String::from_utf8_lossy(&b).into_owned(),
                )
            })
        })
        .collect()
}

pub(crate) struct Args {
    pub paths: Vec<PathBuf>,
    pub json: bool,
    pub min_words: usize,
    pub threshold: f64,
    pub top: usize,
    pub dump_pairs: bool,
    pub eval: Option<PathBuf>,
}

pub(crate) fn cmd_markdown(args: Args) -> Result<()> {
    let files = discover(&args.paths);
    let docs = read_docs(&files);

    // Golden-building mode: emit all scored candidate pairs (with text) as JSON.
    if args.dump_pairs {
        let pairs = nose_markdown::dump_pairs(&docs, args.min_words);
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "scanned_files": docs.len(),
                "pairs": pairs,
            }))?
        );
        return Ok(());
    }

    // Measurement mode: score candidates and evaluate against a labeled golden.
    if let Some(golden_path) = args.eval {
        let golden: nose_markdown::Golden =
            serde_json::from_str(&std::fs::read_to_string(&golden_path)?)
                .with_context(|| format!("parsing golden {}", golden_path.display()))?;
        let scored = nose_markdown::score_pairs(&docs, args.min_words);
        let metrics = nose_markdown::evaluate(&scored, &golden);
        println!("{}", serde_json::to_string_pretty(&metrics)?);
        return Ok(());
    }

    let opts = Options {
        min_words: args.min_words,
        threshold: args.threshold,
        ..Options::default()
    };
    let families = detect(&docs, &opts);
    if args.json {
        let out = serde_json::json!({ "scanned_files": docs.len(), "families": families });
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        print_human(docs.len(), &families, args.top);
    }
    Ok(())
}

fn print_human(scanned: usize, families: &[Family], top: usize) {
    let (templates, dups): (Vec<&Family>, Vec<&Family>) = families.iter().partition(|f| f.template);
    println!(
        "scanned {scanned} markdown files \u{2192} {} near-duplicate {}, {} templated {} \
         (score + span witness + commonness; not a judgment of intent)",
        dups.len(),
        if dups.len() == 1 {
            "family"
        } else {
            "families"
        },
        templates.len(),
        if templates.len() == 1 {
            "section"
        } else {
            "sections"
        },
    );
    for (n, f) in dups.iter().take(top).enumerate() {
        print_family(n, f);
    }
    if !templates.is_empty() {
        println!(
            "\n\u{2014} templated sections (one section skeleton repeated across files; \
             consider a single source / generator) \u{2014}"
        );
        for f in templates.iter().take(top) {
            let h = f
                .members
                .first()
                .and_then(|m| m.heading.as_deref())
                .unwrap_or("(section)");
            println!(
                "  \u{2022} \"{}\" repeated {}\u{00d7} across {} files (score {:.2}, removable~{})",
                short(h, 60),
                f.members.len(),
                f.files,
                f.score,
                f.removable
            );
        }
    }
}

fn print_family(n: usize, f: &Family) {
    let common = if f.commonness > 0.25 {
        "  [common / likely boilerplate]"
    } else {
        ""
    };
    println!(
        "\n#{n} [{}] score={:.2}  members={} files={} removable~{} commonness={:.2}{}",
        f.tier,
        f.score,
        f.members.len(),
        f.files,
        f.removable,
        f.commonness,
        common
    );
    if let Some(h) = f.members.first().and_then(|m| m.heading.as_deref()) {
        println!("   heading: {}", short(h, 70));
    }
    for m in f.members.iter().take(6) {
        println!("   - {}:{}-{}", m.path, m.start_line, m.end_line);
    }
    if f.members.len() > 6 {
        println!("   \u{2026} and {} more", f.members.len() - 6);
    }
    if let Some(w) = &f.witness {
        println!(
            "   witness: {} shared lines (e.g. {}:{}-{} \u{2248} {}:{}-{})",
            w.span.matched_lines,
            file_only(&w.a_path),
            w.span.a_start,
            w.span.a_end,
            file_only(&w.b_path),
            w.span.b_start,
            w.span.b_end,
        );
    }
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
