use crate::cli_args::QueryArgs;
use crate::path_utils::paths_as_refs;
use crate::query_dataset::build_query_dataset;
use crate::query_options::ReportFormat;
use anyhow::{ensure, Context, Result};
use nose_detect::regions::evolution::{AnalysisSnapshot, FamilyObservation};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::Path;

const MAX_BYTES: u64 = 128 * 1024 * 1024;

pub(super) fn read(path: &Path) -> Result<AnalysisSnapshot> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .with_context(|| format!("opening analysis {}", path.display()))?
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)?;
    ensure!(bytes.len() as u64 <= MAX_BYTES, "analysis exceeds 128 MiB");
    let snapshot: AnalysisSnapshot = serde_json::from_slice(&bytes).context(
        "expected --save-analysis artifact (nose.analysis/v1); dashboard, baseline and region snapshots are not complete family analyses")?;
    snapshot.validate().map_err(anyhow::Error::msg)?;
    Ok(snapshot)
}

pub(crate) fn capture(args: &QueryArgs, path: &Path) -> Result<()> {
    ensure!(
        !path.exists(),
        "analysis output already exists: {}; choose a new file",
        path.display()
    );
    let dataset = build_query_dataset(args, &paths_as_refs(&args.paths))?;
    let profile = profile(&dataset)?;
    let mut families: Vec<_> = dataset
        .families
        .iter()
        .map(FamilyObservation::capture)
        .collect();
    families.sort_by_key(|f| f.id);
    families.dedup_by_key(|f| f.id);
    let mut roots: Vec<_> = args
        .paths
        .iter()
        .map(|p| std::fs::canonicalize(p).map(|p| p.to_string_lossy().into_owned()))
        .collect::<std::io::Result<_>>()?;
    roots.sort();
    roots.dedup();
    let snapshot = AnalysisSnapshot {
        schema: "nose.analysis/v1".into(),
        profile,
        roots,
        path_base: std::env::current_dir()?.to_string_lossy().into_owned(),
        scanned_files: dataset.scope.files,
        skipped_sources: dataset.scope.skipped_sources.len(),
        population: "admitted-query-families".into(),
        complete: dataset.scope.skipped_sources.is_empty(),
        families,
    };
    snapshot.validate().map_err(anyhow::Error::msg)?;
    let bytes = serde_json::to_vec(&snapshot)?;
    ensure!(
        bytes.len() as u64 <= MAX_BYTES,
        "analysis exceeds 128 MiB; narrow the analysis roots"
    );
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    if let Err(e) = file.write_all(&bytes) {
        drop(file);
        let _ = std::fs::remove_file(path);
        return Err(e.into());
    }
    let path = std::fs::canonicalize(path)?;
    let next = super::navigation::command(&path, &path, 100_000, &[]);
    if args.format == ReportFormat::Json {
        println!(
            "{}",
            serde_json::json!({"schema":"nose.analysis-capture/v1", "file":path,
            "families":snapshot.families.len(), "complete":snapshot.complete,
            "population":snapshot.population, "next":[next]})
        );
    } else {
        println!("Saved {} admitted code families to {}.
All surfaces included; reviews and source bodies are not stored.
next: {next}
Compare this capture to itself to explore its evidence; supply a later --after capture to inspect changes.",
            snapshot.families.len(), path.display());
    }
    Ok(())
}

fn profile(dataset: &crate::query_dataset::QueryDataset) -> Result<BTreeMap<String, String>> {
    let s = &dataset.settings;
    let channels = s.channels;
    let mut exclude = s.exclude.clone();
    exclude.sort();
    exclude.dedup();
    Ok(BTreeMap::from([
        (
            "engine".into(),
            format!("nose/{}/analysis-v1", env!("CARGO_PKG_VERSION")),
        ),
        (
            "channels".into(),
            format!(
                "syntax={},semantic={},near={},abstraction={},threshold={}",
                channels.syntax,
                channels.semantic,
                channels.near,
                channels.abstraction,
                channels.threshold()
            ),
        ),
        ("min-lines".into(), s.min_lines.to_string()),
        ("min-size".into(), s.min_tokens.to_string()),
        ("min-members".into(), s.min_members.to_string()),
        ("min-value".into(), s.min_value.to_string()),
        ("exclude".into(), serde_json::to_string(&exclude)?),
        (
            "semantic-packs".into(),
            crate::cache::semantic_pack_digest(&dataset.semantic_packs).hex(),
        ),
        (
            "pack-lock".into(),
            dataset
                .semantic_packs
                .project_lock()
                .map(|l| l.decision_digest().to_string())
                .unwrap_or_default(),
        ),
        (
            "discovery".into(),
            "frontend-gitignore/supported-sources; source changes require a new capture".into(),
        ),
    ]))
}
