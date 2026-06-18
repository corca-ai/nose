//! Smoke/eval runner: `cargo run --example mddup -- <paths...> [--json]`
//! Recursively finds `.md`/`.markdown` files under the given paths, runs the pipeline, and
//! prints ranked families (human summary, or `--json` for the machine contract).

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
    let args: Vec<String> = std::env::args().skip(1).collect();
    let json = args.iter().any(|a| a == "--json");
    let roots: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    if roots.is_empty() {
        eprintln!("usage: mddup <paths...> [--json]");
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

    let fams = nose_markdown::detect(&docs, &nose_markdown::Options::default());

    if json {
        println!("{}", serde_json::to_string_pretty(&fams).unwrap());
        return;
    }

    eprintln!(
        "scanned {} markdown files → {} near-duplicate families",
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
