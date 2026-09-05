use crate::cli_args::QueryArgs;
use crate::path_utils::{paths_as_refs, relativize};
use crate::query_commands::{
    activate_query_families, discard_accepted_coverage, query_opportunities,
    query_surface_overrides, semantic_packs_for_output,
};
use crate::query_dashboard::query_dashboard_json;
use crate::query_dataset::{build_query_dataset, QueryAnalysisSession, QueryDataset};
use crate::query_options::ReportFormat;
use crate::query_terms::Query;
use crate::schema_versions::QUERY_WATCH_JSONL_SCHEMA;
use anyhow::{Context, Result};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const DEBOUNCE: Duration = Duration::from_millis(40);
const MAX_BATCH_LATENCY: Duration = Duration::from_millis(250);

mod inputs;

pub(crate) fn run(
    args: &QueryArgs,
    terms: &[String],
    _query: &Query,
    path_arg: &str,
) -> Result<()> {
    validate(args, terms)?;
    let (watch_args, _temporary_cache) = watch_args(args)?;
    let cache_dir = watch_args
        .cache_dir
        .clone()
        .context("watch session requires a cache directory")?;
    let roots = watch_root_paths(&watch_args);
    let (receiver, mut watcher) = watch_roots(&roots)?;
    let mut watched = roots.into_iter().collect::<BTreeSet<_>>();
    let mut input_files = inputs::register(&watch_args, &mut watcher, &mut watched)?;
    let refs = paths_as_refs(&watch_args.paths);

    // Seed or validate the ordinary transactional cache before the long-lived session takes
    // ownership of its generation. The session-derived snapshot below closes the startup race:
    // an edit during this seed pass cannot pair an old snapshot with a new source digest.
    let (mut session, initial_dataset) = open_session(&watch_args, &refs)?;
    let initial_invalidation = session.take_initial_invalidation();
    let mut source_set_digest = session.source_set_digest();
    let initial_snapshot = dashboard_snapshot(&watch_args, path_arg, initial_dataset)?;
    let mut previous_snapshot = initial_snapshot.clone();
    emit(WatchEmission {
        sequence: 0,
        source_set_digest: &source_set_digest,
        changed_paths: &[],
        reconciliation: "initial",
        invalidation: initial_invalidation.as_ref(),
        latency: Duration::ZERO,
        snapshot: initial_snapshot,
    })?;

    let mut session = Some(session);
    let mut sequence = 0_u64;
    let mut last_good_sequence = 0_u64;
    loop {
        let Some(batch) = receive_batch(&receiver) else {
            return Ok(());
        };
        let changed_paths = display_paths(&batch.paths);
        let recovering = session.is_none();
        let action = classify(&batch, session.as_ref(), &cache_dir, &input_files);
        let Some(action) = action else { continue };
        let result: Result<_> = (|| {
            input_files = inputs::register(&watch_args, &mut watcher, &mut watched)?;
            let update = match (action, session.as_mut()) {
                (WatchAction::Leaf(path), Some(session)) => {
                    session.refresh_leaf(&watch_args, &refs, &path)?
                }
                _ => None,
            };
            let (dataset, invalidation, digest, reconciliation) = match update {
                Some(update) => (
                    update.dataset,
                    update.invalidation,
                    update.source_set_digest,
                    "incremental-leaf",
                ),
                None => {
                    drop(session.take());
                    let (mut replacement, dataset) = open_session(&watch_args, &refs)?;
                    let invalidation = replacement
                        .take_initial_invalidation()
                        .context("full reconciliation omitted invalidation evidence")?;
                    let digest = replacement.source_set_digest();
                    session = Some(replacement);
                    (dataset, invalidation, digest, "full-reconciliation")
                }
            };
            let snapshot = dashboard_snapshot(&watch_args, path_arg, dataset)?;
            Ok((snapshot, invalidation, digest, reconciliation))
        })();
        let (snapshot, invalidation, digest, reconciliation) = match result {
            Ok(result) => result,
            Err(error) => {
                // A failed refresh can have mutated intermediate state. Release it
                // and retry from a complete generation on the next filesystem hint.
                drop(session.take());
                sequence = sequence.saturating_add(1);
                emit_value(&json!({
                    "schema": QUERY_WATCH_JSONL_SCHEMA, "kind": "error",
                    "sequence": sequence, "last_good_sequence": last_good_sequence,
                    "snapshot_valid": false, "changed_paths": changed_paths,
                    "error": {"message": format!("{error:#}")}
                }))?;
                continue;
            }
        };
        if !recovering && digest == source_set_digest && snapshot == previous_snapshot {
            continue;
        }
        previous_snapshot = snapshot.clone();
        sequence = sequence.saturating_add(1);
        emit(WatchEmission {
            sequence,
            source_set_digest: &digest,
            changed_paths: &changed_paths,
            reconciliation,
            invalidation: Some(&invalidation),
            latency: batch.first_seen.elapsed(),
            snapshot,
        })?;
        source_set_digest = digest;
        last_good_sequence = sequence;
    }
}

fn watch_root_paths(args: &QueryArgs) -> Vec<PathBuf> {
    args.paths
        .iter()
        .map(|path| {
            let path = absolute(path);
            if path.is_dir() {
                path
            } else {
                path.parent().unwrap().to_path_buf()
            }
        })
        .collect()
}

fn open_session(args: &QueryArgs, refs: &[&Path]) -> Result<(QueryAnalysisSession, QueryDataset)> {
    build_query_dataset(args, refs)?;
    let mut session = QueryAnalysisSession::open(args, refs)?
        .context("watch session could not open the incremental cache; external semantic-pack influence is not supported in watch mode")?;
    let dataset = session.current_dataset(args, refs)?;
    Ok((session, dataset))
}

fn validate(args: &QueryArgs, terms: &[String]) -> Result<()> {
    if !matches!(args.format, ReportFormat::Jsonl) {
        anyhow::bail!("--watch requires --format jsonl");
    }
    if !terms.is_empty() {
        anyhow::bail!(
            "--watch currently emits the dashboard snapshot; query terms are not supported"
        );
    }
    if args.fail_on.is_some() || args.baseline.is_some() || args.write_baseline {
        anyhow::bail!("--watch is an observation stream and does not support baseline writes or CI fail gates");
    }
    Ok(())
}

fn dashboard_snapshot(
    args: &QueryArgs,
    path_arg: &str,
    mut dataset: QueryDataset,
) -> Result<Value> {
    let baseline = activate_query_families(args, &mut dataset)?;
    let overrides = query_surface_overrides(&mut dataset);
    let opportunities = query_opportunities(&dataset.families, &overrides);
    discard_accepted_coverage(&mut dataset.families);
    let semantic_packs = semantic_packs_for_output(args.format, &dataset);
    let reinvented = dataset
        .reinvented
        .iter()
        .filter(|finding| !finding.container_in_test && !finding.helper_in_test)
        .count();
    let markdown = crate::markdown::QueryMarkdownReport::detect_under(
        &args.paths,
        &dataset.settings.exclude,
        args.cache_dir.as_deref(),
        dataset.settings.cache_max_bytes,
    )?;
    Ok(query_dashboard_json(
        &dataset.families,
        &overrides,
        &opportunities,
        &dataset.scope,
        path_arg,
        &crate::query_navigation::path(args, path_arg),
        reinvented,
        baseline.as_ref(),
        None,
        &markdown,
        &semantic_packs,
    ))
}

struct WatchEmission<'a> {
    sequence: u64,
    source_set_digest: &'a str,
    changed_paths: &'a [String],
    reconciliation: &'a str,
    invalidation: Option<&'a crate::cache::InvalidationReport>,
    latency: Duration,
    snapshot: Value,
}

fn emit(emission: WatchEmission<'_>) -> Result<()> {
    let value = json!({
        "schema": QUERY_WATCH_JSONL_SCHEMA,
        "kind": "snapshot",
        "sequence": emission.sequence,
        "source_set_digest": emission.source_set_digest,
        "changed_paths": emission.changed_paths,
        "reconciliation": emission.reconciliation,
        "invalidation": emission.invalidation,
        "latency_ms": emission.latency.as_secs_f64() * 1000.0,
        "snapshot": emission.snapshot,
    });
    emit_value(&value)
}

fn emit_value(value: &Value) -> Result<()> {
    let stdout = std::io::stdout();
    let mut output = stdout.lock();
    serde_json::to_writer(&mut output, value)?;
    output.write_all(b"\n")?;
    output.flush()?;
    Ok(())
}

struct WatchBatch {
    paths: BTreeSet<PathBuf>,
    reconcile: bool,
    hierarchy_paths: BTreeSet<PathBuf>,
    first_seen: Instant,
}

impl WatchBatch {
    fn add(&mut self, result: notify::Result<Event>) {
        match result {
            Ok(event) => {
                if matches!(event.kind, EventKind::Access(_)) {
                    return;
                }
                self.reconcile |= event.need_rescan();
                if matches!(
                    event.kind,
                    EventKind::Create(notify::event::CreateKind::Folder)
                        | EventKind::Remove(notify::event::RemoveKind::Folder)
                        | EventKind::Modify(notify::event::ModifyKind::Name(_))
                ) {
                    self.hierarchy_paths.extend(event.paths.iter().cloned());
                }
                self.paths.extend(event.paths);
            }
            Err(_) => self.reconcile = true,
        }
    }
}

fn watch_roots(
    roots: &[PathBuf],
) -> Result<(mpsc::Receiver<notify::Result<Event>>, RecommendedWatcher)> {
    let (sender, receiver) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |event| {
        let _ = sender.send(event);
    })?;
    for root in roots {
        let mode = if root.is_dir() {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };
        watcher.watch(root, mode)?;
    }
    Ok((receiver, watcher))
}

fn receive_batch(receiver: &mpsc::Receiver<notify::Result<Event>>) -> Option<WatchBatch> {
    let first = receiver.recv().ok()?;
    let mut batch = WatchBatch {
        paths: BTreeSet::new(),
        reconcile: false,
        hierarchy_paths: BTreeSet::new(),
        first_seen: Instant::now(),
    };
    batch.add(first);
    let mut deadline = Instant::now() + DEBOUNCE;
    let latest = batch.first_seen + MAX_BATCH_LATENCY;
    loop {
        let remaining = deadline
            .min(latest)
            .saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Some(batch);
        }
        match receiver.recv_timeout(remaining) {
            Ok(event) => {
                batch.add(event);
                deadline = Instant::now() + DEBOUNCE;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => return Some(batch),
            Err(mpsc::RecvTimeoutError::Disconnected) => return Some(batch),
        }
    }
}

enum WatchAction {
    Leaf(PathBuf),
    Full,
}

fn classify(
    batch: &WatchBatch,
    session: Option<&QueryAnalysisSession>,
    cache_dir: &Path,
    input_files: &BTreeSet<PathBuf>,
) -> Option<WatchAction> {
    if batch.reconcile {
        return Some(WatchAction::Full);
    }
    let cache = absolute(cache_dir);
    let mut sources = BTreeSet::new();
    let mut full = false;
    for path in &batch.paths {
        if absolute(path).starts_with(&cache) {
            continue;
        }
        if input_files.contains(&absolute(path)) || batch.hierarchy_paths.contains(path) {
            full = true;
        } else if let Some(source) = session.and_then(|s| s.source_path_for_event(path)) {
            sources.insert(source);
        } else if affects_query_inputs(path) {
            full = true;
        }
    }
    if full || sources.len() > 1 {
        Some(WatchAction::Full)
    } else {
        sources.into_iter().next().map(WatchAction::Leaf)
    }
}

fn affects_query_inputs(path: &Path) -> bool {
    if nose_il::Lang::from_file_path(path).is_some() {
        return true;
    }
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    matches!(
        name,
        ".gitignore" | ".ignore" | "nose.toml" | ".nose.toml" | "nose.ignore.json"
    ) || path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| matches!(extension, "md" | "markdown" | "json" | "toml"))
}

fn display_paths(paths: &BTreeSet<PathBuf>) -> Vec<String> {
    let cwd = std::env::current_dir().ok();
    paths
        .iter()
        .map(|path| {
            let value = absolute(path).to_string_lossy().into_owned();
            cwd.as_ref()
                .map_or_else(|| value.clone(), |cwd| relativize(&value, cwd))
        })
        .collect()
}

fn absolute(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()
                .map(|cwd| cwd.join(path))
                .unwrap_or_else(|_| path.to_path_buf())
        }
    })
}

fn watch_args(args: &QueryArgs) -> Result<(QueryArgs, Option<TemporaryCache>)> {
    let mut args = args.clone();
    if args.cache_dir.is_some() {
        return Ok((args, None));
    }
    let cache = TemporaryCache::new()?;
    args.cache_dir = Some(cache.path.clone());
    Ok((args, Some(cache)))
}

struct TemporaryCache {
    path: PathBuf,
}

impl TemporaryCache {
    fn new() -> Result<Self> {
        static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let sequence = SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "nose-watch-cache-{}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path)
            .with_context(|| format!("create temporary watch cache {}", path.display()))?;
        Ok(Self { path })
    }
}

impl Drop for TemporaryCache {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_session_ignores_access_and_cache_events_but_retries_source_edits() {
        let cache = absolute(Path::new(".nose-test-cache"));
        let inputs = BTreeSet::new();
        let mut batch = WatchBatch {
            paths: BTreeSet::new(),
            reconcile: false,
            hierarchy_paths: BTreeSet::new(),
            first_seen: Instant::now(),
        };
        batch.add(Ok(Event::new(EventKind::Access(
            notify::event::AccessKind::Any,
        ))
        .add_path(PathBuf::from("source.ts"))));
        assert!(classify(&batch, None, &cache, &inputs).is_none());
        batch.paths.insert(cache.join("state.json"));
        assert!(classify(&batch, None, &cache, &inputs).is_none());
        batch.paths.insert(PathBuf::from("source.ts"));
        assert!(matches!(
            classify(&batch, None, &cache, &inputs),
            Some(WatchAction::Full)
        ));
    }
}
