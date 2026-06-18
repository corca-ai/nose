//! `nose markdown` — same-language near-duplicate detection for Markdown documents.
//!
//! Runs the `nose-markdown` pipeline (units → MinHash/winnowing candidates → TF-IDF/containment
//! verify → local-alignment witness) and surfaces ranked near-duplicate families with a span
//! witness + orthogonal evidence (commonness, removable, files). Honesty contract (epic #435):
//! it reports near-duplication, never "same meaning" and never "worth removing".

use anyhow::{Context, Result};
use nose_markdown::{detect, Family, Options};
use std::path::{Path, PathBuf};

/// Discover `.md`/`.markdown` files under the given paths, respecting `.gitignore`.
fn discover(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut builder = ignore::WalkBuilder::new(&paths[0]);
    for p in &paths[1..] {
        builder.add(p);
    }
    let mut out = Vec::new();
    for dent in builder.build().flatten() {
        let p = dent.path();
        if dent.file_type().is_some_and(|t| t.is_file())
            && matches!(
                p.extension().and_then(|e| e.to_str()),
                Some("md") | Some("markdown")
            )
        {
            out.push(p.to_path_buf());
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
    println!(
        "scanned {scanned} markdown files \u{2192} {} near-duplicate {} \
         (near-dup score + span witness + commonness; not a judgment of intent)",
        families.len(),
        if families.len() == 1 {
            "family"
        } else {
            "families"
        }
    );
    for (n, f) in families.iter().take(top).enumerate() {
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
            println!("   … and {} more", f.members.len() - 6);
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
