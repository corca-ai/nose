use anyhow::{Context, Result};
use std::path::PathBuf;

use crate::path_utils::{paths_as_refs, warn_if_empty};
use crate::timing::time_lower;

pub(crate) struct DetectArgs {
    pub(crate) paths: Vec<PathBuf>,
    pub(crate) min_lines: u32,
    pub(crate) min_tokens: usize,
    pub(crate) threshold: Option<f64>,
    pub(crate) candidates: bool,
    pub(crate) query_accepted: bool,
    pub(crate) minhash_k: usize,
    pub(crate) bands: usize,
    pub(crate) no_cfg_norm: bool,
    pub(crate) dce: bool,
    pub(crate) no_blocks: bool,
    pub(crate) out: Option<PathBuf>,
    pub(crate) summary: bool,
    pub(crate) bench_schema: bool,
    pub(crate) repos_root: Option<PathBuf>,
    pub(crate) dump: Option<PathBuf>,
}

pub(crate) fn cmd_detect(args: DetectArgs) -> Result<()> {
    let refs = paths_as_refs(&args.paths);
    let corpus = time_lower(|| nose_frontend::lower_corpus_many(&refs));
    warn_if_empty(&corpus, &args.paths);

    let (opts, detector) = detection_config(&args)?;

    // Diagnostic dump: units + candidates + predictions to a directory.
    if let Some(dir) = &args.dump {
        let root = args
            .repos_root
            .as_ref()
            .context("--dump requires --repos-root")?;
        let (report, dump) = nose_detect::detect_with_dump(&corpus, &opts, detector.as_ref());
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;

        let units: Vec<nose_eval::UnitRegion> = dump
            .units
            .iter()
            .map(|u| match map_to_repo(root, &u.path) {
                // keep index alignment for unmappable units with a sentinel repo
                Some((repo, file)) => nose_eval::UnitRegion {
                    repo,
                    file,
                    start_line: u.start_line,
                    end_line: u.end_line,
                },
                None => nose_eval::UnitRegion {
                    repo: String::new(),
                    file: u.path.clone(),
                    start_line: u.start_line,
                    end_line: u.end_line,
                },
            })
            .collect();
        std::fs::write(
            dir.join("units.json"),
            serde_json::to_string(&nose_eval::UnitsDump { units })?,
        )?;
        std::fs::write(
            dir.join("candidates.json"),
            serde_json::to_string(&nose_eval::CandidatesDump {
                candidates: dump.candidates,
            })?,
        )?;
        let preds = to_benchmark_predictions(&report, root);
        std::fs::write(dir.join("predictions.json"), serde_json::to_string(&preds)?)?;
        eprintln!(
            "dump → {}: {} units, {} candidate pairs, {} predictions",
            dir.display(),
            report.metrics.units,
            report.metrics.candidate_pairs,
            preds.duplicates.len()
        );
        return Ok(());
    }

    let report = nose_detect::detect(&corpus, &opts, detector.as_ref());

    if args.bench_schema {
        let root = args
            .repos_root
            .as_ref()
            .context("--bench-schema requires --repos-root")?;
        let preds = to_benchmark_predictions(&report, root);
        let json = serde_json::to_string_pretty(&preds)?;
        match &args.out {
            Some(path) => {
                std::fs::write(path, json).with_context(|| format!("writing {}", path.display()))?
            }
            None => println!("{json}"),
        }
        eprintln!(
            "emitted {} benchmark-schema predictions",
            preds.duplicates.len()
        );
        return Ok(());
    }

    if args.summary {
        print_summary(&report);
        return Ok(());
    }

    let json = serde_json::to_string_pretty(&report)?;
    match args.out {
        Some(path) => {
            std::fs::write(&path, json).with_context(|| format!("writing {}", path.display()))?
        }
        None => println!("{json}"),
    }
    Ok(())
}

fn detection_config(
    args: &DetectArgs,
) -> Result<(nose_detect::DetectOptions, Box<dyn nose_detect::Detector>)> {
    if args.query_accepted {
        let channels = crate::query_options::DetectionChannels::resolve(
            Vec::new(),
            Vec::new(),
            crate::query_options::QUERY_DEFAULT_MODES,
        )?;
        let mut opts =
            crate::detect_pipeline::detection_options(channels, args.min_tokens, args.min_lines);
        opts.emit_pairs = true;
        let detector = crate::detect_pipeline::detection_engine(channels, &opts);
        Ok((opts, detector))
    } else {
        let opts = nose_detect::DetectOptions {
            min_lines: args.min_lines,
            min_tokens: args.min_tokens,
            threshold: args
                .threshold
                .unwrap_or(if args.candidates { 0.70 } else { 0.86 }),
            minhash_k: args.minhash_k,
            bands: args.bands,
            cfg_norm: !args.no_cfg_norm,
            dce: args.dce,
            block_units: !args.no_blocks,
            ..Default::default()
        };
        let detector: Box<dyn nose_detect::Detector> = if args.candidates {
            Box::new(
                nose_detect::StructuralDetector::candidates(opts.jaccard_weight)
                    .with_threshold(opts.threshold),
            )
        } else {
            Box::new(
                nose_detect::StructuralDetector::strict(opts.jaccard_weight)
                    .with_threshold(opts.threshold),
            )
        };
        Ok((opts, detector))
    }
}

/// Map an absolute analyzed path to `(repo_id, repo_relative_path)` given the root
/// whose immediate children are repo ids.
pub(crate) fn map_to_repo(root: &std::path::Path, path: &str) -> Option<(String, String)> {
    let rel = std::path::Path::new(path).strip_prefix(root).ok()?;
    let mut comps = rel.components();
    let repo = comps.next()?.as_os_str().to_str()?.to_string();
    let relpath = comps.as_path().to_str()?.to_string();
    if relpath.is_empty() {
        return None;
    }
    Some((repo, relpath))
}

/// Convert nose's clone pairs into benchmark predictions (repo id +
/// repo-relative paths, 1-based lines, `nose_semantic` channel).
pub(crate) fn to_benchmark_predictions(
    report: &nose_detect::Report,
    root: &std::path::Path,
) -> nose_eval::Predictions {
    let mut duplicates = Vec::new();
    for d in &report.duplicates {
        let (lrepo, lfile) = match map_to_repo(root, &d.left.file) {
            Some(x) => x,
            None => continue,
        };
        let (rrepo, rfile) = match map_to_repo(root, &d.right.file) {
            Some(x) => x,
            None => continue,
        };
        duplicates.push(nose_eval::PredPair {
            repo: Some(lrepo.clone()),
            channel: Some("nose_semantic".to_string()),
            score: Some(d.score),
            left: nose_eval::PredRegion {
                repo: Some(lrepo),
                file: lfile,
                start_line: d.left.start_line,
                end_line: d.left.end_line,
                symbol: d.left.name.clone(),
            },
            right: nose_eval::PredRegion {
                repo: Some(rrepo),
                file: rfile,
                start_line: d.right.start_line,
                end_line: d.right.end_line,
                symbol: d.right.name.clone(),
            },
        });
    }
    nose_eval::Predictions {
        schema_version: "0.1.0".to_string(),
        tool: "nose".to_string(),
        duplicates,
    }
}

pub(crate) fn print_summary(report: &nose_detect::Report) {
    let m = &report.metrics;
    eprintln!(
        "nose [{}]: {} files, {} units, {} candidate pairs, {} accepted, {} clone groups",
        report.detector, m.files, m.units, m.candidate_pairs, m.accepted_pairs, m.groups
    );
    for (n, g) in report.groups.iter().enumerate() {
        println!(
            "\nGroup {} (score {:.3}, {} members):",
            n + 1,
            g.score,
            g.members.len()
        );
        for mem in &g.members {
            let name = mem.name.as_deref().unwrap_or("");
            println!(
                "  {}:{}-{}  [{}] {}",
                mem.file, mem.start_line, mem.end_line, mem.lang, name
            );
        }
    }
}
