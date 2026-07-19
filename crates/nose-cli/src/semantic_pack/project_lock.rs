use anyhow::Result;
use std::path::PathBuf;

pub(crate) const LOCK_STATUS_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, PartialEq, clap::ValueEnum)]
pub(crate) enum LockStatusFormat {
    Human,
    Json,
}

#[derive(Clone, Copy, PartialEq, clap::ValueEnum)]
pub(crate) enum LockChannel {
    Near,
    ExternalExact,
}

impl From<LockChannel> for nose_semantics::SemanticPackV1Channel {
    fn from(value: LockChannel) -> Self {
        match value {
            LockChannel::Near => Self::Near,
            LockChannel::ExternalExact => Self::ExternalExact,
        }
    }
}

#[derive(serde::Serialize)]
struct LockStatusReport {
    schema_version: u32,
    status: &'static str,
    lock_api_version: &'static str,
    lock_path: String,
    decision_digest: String,
    influence: &'static str,
    totals: LockStatusTotals,
    dependencies: Vec<LockedFileReport>,
    packs: Vec<LockedPackReport>,
}

#[derive(serde::Serialize)]
struct LockStatusTotals {
    packs: usize,
    selected_rows: usize,
    dependencies: usize,
    exact_receipts: usize,
    conflicts: usize,
}

#[derive(serde::Serialize)]
struct LockedPackReport {
    pack_id: String,
    manifest_api_version: &'static str,
    pack_version: String,
    semantic_digest: String,
    allowed_channels: Vec<&'static str>,
    selected_rows: Vec<String>,
    exact_receipt: Option<LockedFileReport>,
}

#[derive(Clone, serde::Serialize)]
struct LockedFileReport {
    path: String,
    content_digest: String,
}

impl LockStatusReport {
    fn new(lock: &nose_semantics::ValidatedSemanticPackProjectLock) -> Self {
        let dependencies = lock
            .authorizations()
            .first()
            .map(|authorization| {
                authorization
                    .dependencies()
                    .iter()
                    .map(LockedFileReport::new)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let packs = lock
            .authorizations()
            .iter()
            .map(|authorization| {
                let compiled = lock
                    .semantic_packs()
                    .compiled_external_v1_packs()
                    .iter()
                    .find(|pack| pack.pack_id() == authorization.pack_id())
                    .expect("validated authorization has a compiled pack");
                LockedPackReport {
                    pack_id: authorization.pack_id().to_string(),
                    manifest_api_version: nose_semantics::SEMANTIC_PACK_API_VERSION_V1,
                    pack_version: compiled.pack_version().to_string(),
                    semantic_digest: compiled.semantic_digest().to_string(),
                    allowed_channels: authorization
                        .allowed_channels()
                        .iter()
                        .map(|channel| channel.as_str())
                        .collect(),
                    selected_rows: authorization.selected_rows().to_vec(),
                    exact_receipt: authorization.exact_receipt().map(LockedFileReport::new),
                }
            })
            .collect::<Vec<_>>();
        Self {
            schema_version: LOCK_STATUS_SCHEMA_VERSION,
            status: "ok",
            lock_api_version: lock.summary().api_version(),
            lock_path: lock.summary().lock_path().display().to_string(),
            decision_digest: lock.summary().decision_digest().to_string(),
            influence: if lock.authorizations().iter().any(|authorization| {
                authorization
                    .allowed_channels()
                    .contains(&nose_semantics::SemanticPackV1Channel::ExternalExact)
            }) {
                "external-claim-exact"
            } else if lock.authorizations().iter().any(|authorization| {
                authorization
                    .allowed_channels()
                    .contains(&nose_semantics::SemanticPackV1Channel::Near)
            }) {
                "near-only"
            } else {
                "metadata-only"
            },
            totals: LockStatusTotals {
                packs: packs.len(),
                selected_rows: packs.iter().map(|pack| pack.selected_rows.len()).sum(),
                dependencies: dependencies.len(),
                exact_receipts: packs
                    .iter()
                    .filter(|pack| pack.exact_receipt.is_some())
                    .count(),
                conflicts: 0,
            },
            dependencies,
            packs,
        }
    }
}

impl LockedFileReport {
    fn new(file: &nose_semantics::SemanticPackLockedFile) -> Self {
        Self {
            path: file.declared_path().to_string(),
            content_digest: file.content_digest().to_string(),
        }
    }
}

pub(crate) struct LockCommand {
    pub manifests: Vec<PathBuf>,
    pub output: PathBuf,
    pub channels: Vec<LockChannel>,
    pub selected_rows: Vec<String>,
    pub dependencies: Vec<PathBuf>,
    pub exact_receipt: Option<PathBuf>,
    pub format: LockStatusFormat,
}

pub(crate) fn cmd_lock(command: LockCommand) -> Result<()> {
    let lock = nose_semantics::create_project_lock(
        &command.output,
        &command.manifests,
        nose_semantics::SemanticPackLockOptions {
            allowed_channels: command.channels.into_iter().map(Into::into).collect(),
            selected_rows: command.selected_rows,
            dependency_paths: command.dependencies,
            exact_receipt: command.exact_receipt,
        },
    )?;
    print_report(&LockStatusReport::new(&lock), command.format)
}

pub(crate) fn cmd_status(lock_path: PathBuf, format: LockStatusFormat) -> Result<()> {
    let lock = nose_semantics::validate_project_lock(&lock_path)?;
    print_report(&LockStatusReport::new(&lock), format)
}

fn print_report(report: &LockStatusReport, format: LockStatusFormat) -> Result<()> {
    match format {
        LockStatusFormat::Json => println!("{}", serde_json::to_string_pretty(report)?),
        LockStatusFormat::Human => {
            println!("semantic-pack project lock: {}", report.status);
            println!("lock: {}", report.lock_path);
            println!("decision digest: {}", report.decision_digest);
            println!(
                "packs: {}; selected rows: {}; dependencies: {}; conflicts: {}",
                report.totals.packs,
                report.totals.selected_rows,
                report.totals.dependencies,
                report.totals.conflicts
            );
            for pack in &report.packs {
                println!(
                    "  {}@{}: {} row(s), channels {}",
                    pack.pack_id,
                    pack.pack_version,
                    pack.selected_rows.len(),
                    pack.allowed_channels.join(", ")
                );
            }
            println!("influence: {}", report.influence);
        }
    }
    Ok(())
}
