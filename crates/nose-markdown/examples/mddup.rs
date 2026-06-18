//! Dev runner / golden-build tooling for the markdown near-dup engine.
//!
//!   cargo run -p nose-markdown --example mddup -- <paths...>            # human summary
//!   cargo run -p nose-markdown --example mddup -- <paths...> --json     # families as JSON
//!   cargo run -p nose-markdown --example mddup -- <paths...> --dump-pairs   # candidate pairs (golden build)
//!   cargo run -p nose-markdown --example mddup -- <paths...> --eval <golden.json>  # metrics
//!
//! (User-facing markdown detection lives in `nose query`; this is the dev/benchmark surface.)

use std::fs;
use std::path::{Path, PathBuf};

fn collect_md(root: &Path, out: &mut Vec<PathBuf>) {
    if root.is_file() {
        if matches!(
            root.extension().and_then(|e| e.to_str()),
            Some("md") | Some("markdown")
        ) {
            out.push(root.to_path_buf());
        }
        return;
    }
    let Ok(rd) = fs::read_dir(root) else { return };
    let mut entries: Vec<_> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort();
    for p in entries {
        collect_md(&p, out);
    }
}

fn main() {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let json = raw.iter().any(|a| a == "--json");
    let dump = raw.iter().any(|a| a == "--dump-pairs");
    let eval_idx = raw.iter().position(|a| a == "--eval");
    let eval_golden = eval_idx.and_then(|i| raw.get(i + 1).cloned());
    // roots = positional args, excluding flags and the value after `--eval`.
    let roots: Vec<&String> = raw
        .iter()
        .enumerate()
        .filter(|(i, a)| !a.starts_with("--") && Some(*i) != eval_idx.map(|e| e + 1))
        .map(|(_, a)| a)
        .collect();
    if roots.is_empty() {
        eprintln!("usage: mddup <paths...> [--json | --dump-pairs | --eval <golden.json>]");
        std::process::exit(2);
    }

    let mut paths = Vec::new();
    for r in &roots {
        collect_md(Path::new(r), &mut paths);
    }
    paths.sort();
    paths.dedup();

    let mut docs: Vec<(String, String)> = Vec::new();
    for p in &paths {
        if let Ok(bytes) = fs::read(p) {
            docs.push((
                p.to_string_lossy().into_owned(),
                String::from_utf8_lossy(&bytes).into_owned(),
            ));
        }
    }

    // Golden-build: dump all scored candidate pairs (with text).
    if dump {
        let pairs = nose_markdown::dump_pairs(&docs, 8);
        let out = serde_json::json!({ "scanned_files": docs.len(), "pairs": pairs });
        println!("{}", serde_json::to_string_pretty(&out).unwrap());
        return;
    }
    // Measurement: evaluate the detector against a labeled golden.
    if let Some(golden_path) = eval_golden {
        let golden: nose_markdown::Golden =
            serde_json::from_str(&fs::read_to_string(&golden_path).expect("read golden"))
                .expect("parse golden");
        let metrics = nose_markdown::evaluate(&nose_markdown::score_pairs(&docs, 8), &golden);
        println!("{}", serde_json::to_string_pretty(&metrics).unwrap());
        return;
    }

    let fams = nose_markdown::detect(&docs, &nose_markdown::Options::default());

    if json {
        println!("{}", serde_json::to_string_pretty(&fams).unwrap());
        return;
    }

    eprintln!(
        "scanned {} markdown files \u{2192} {} near-duplicate families",
        docs.len(),
        fams.len()
    );
    for (n, f) in fams.iter().take(25).enumerate() {
        let common = if f.commonness > 0.25 {
            "  [common/boilerplate]"
        } else {
            ""
        };
        println!(
            "\n#{n} [{}] score={:.2} members={} files={} removable~{} commonness={:.2}{}",
            f.tier,
            f.score,
            f.members.len(),
            f.files,
            f.removable,
            f.commonness,
            common
        );
        if let Some(h) = f.members.first().and_then(|m| m.heading.as_deref()) {
            println!("   heading: {h}");
        }
        for m in f.members.iter().take(6) {
            println!("   - {}:{}-{}", m.path, m.start_line, m.end_line);
        }
        if let Some(w) = &f.witness {
            println!(
                "   witness: {} shared lines ({} {}-{} ~ {} {}-{})",
                w.span.matched_lines,
                w.a_path,
                w.span.a_start,
                w.span.a_end,
                w.b_path,
                w.span.b_start,
                w.span.b_end
            );
        }
    }
}
