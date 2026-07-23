use super::*;
use crate::detect_pipeline::validate_exclude_globs;
use crate::query_options::{validate_min_value, DetectionChannels, SortKey};
use crate::surfaces::GeneratedPathAssertions;
use crate::{config, ignores};

/// The query settings after layering: CLI flag wins, else config file, else built-in
/// default.
pub(crate) struct QuerySettings {
    pub(crate) min_members: usize,
    pub(crate) min_value: f64,
    pub(crate) sort: SortKey,
    pub(crate) channels: DetectionChannels,
    pub(crate) min_lines: u32,
    pub(crate) min_tokens: usize,
    pub(crate) exclude: Vec<String>,
    pub(crate) generated_paths: GeneratedPathAssertions,
    pub(crate) ignore_set: Option<ignores::IgnoreSet>,
    pub(crate) cache_max_bytes: u64,
}

pub(crate) fn resolve_query_semantic_packs(
    args: &QueryArgs,
) -> Result<nose_semantics::SemanticPackSet> {
    let cfg = config::load_query(args.config.as_deref())?;
    semantic_pack_set_from_inputs(
        cfg.semantic_packs,
        &args.semantic_pack,
        cfg.semantic_pack_lock,
        args.semantic_pack_lock.as_ref(),
    )
}

fn semantic_pack_set_from_inputs(
    mut semantic_pack_paths: Vec<std::path::PathBuf>,
    cli_semantic_pack_paths: &[std::path::PathBuf],
    config_lock: Option<std::path::PathBuf>,
    cli_lock: Option<&std::path::PathBuf>,
) -> Result<nose_semantics::SemanticPackSet> {
    semantic_pack_paths.extend(cli_semantic_pack_paths.iter().cloned());
    let lock = cli_lock.cloned().or(config_lock);
    if let Some(lock) = lock {
        if !semantic_pack_paths.is_empty() {
            anyhow::bail!(
                "a semantic-pack project lock is mutually exclusive with `--semantic-pack` and `[query].semantic-packs`; the lock owns the complete manifest set"
            );
        }
        return Ok(nose_semantics::SemanticPackSet::new_locked(&lock)?);
    }
    Ok(nose_semantics::SemanticPackSet::new_local(
        &semantic_pack_paths,
    )?)
}

pub(super) fn resolve_query_settings(
    args: &QueryArgs,
    default_modes: &[crate::query_options::DetectionMode],
) -> Result<(QuerySettings, nose_semantics::SemanticPackSet)> {
    let cfg = config::load_query(args.config.as_deref())?;
    let min_members = args.min_members.or(cfg.min_members).unwrap_or(2);
    let min_value = validate_min_value(args.min_value.or(cfg.min_value).unwrap_or(0.0))?;
    let sort = args.sort.or(cfg.sort).unwrap_or(SortKey::Extractability);
    let channels = DetectionChannels::resolve(args.mode.clone(), cfg.mode, default_modes)?;
    let min_lines = args.min_lines.or(cfg.min_lines).unwrap_or(5);
    let min_tokens = args.min_size.or(cfg.min_size).unwrap_or(24);
    let cache_max_bytes = args
        .cache_max_bytes
        .or(cfg.cache_max_bytes)
        .unwrap_or(cache::DEFAULT_MAX_BYTES);
    let ignore_file = args.ignore_file.clone().or(cfg.ignore_file);
    let semantic_packs = semantic_pack_set_from_inputs(
        cfg.semantic_packs,
        &args.semantic_pack,
        cfg.semantic_pack_lock,
        args.semantic_pack_lock.as_ref(),
    )?;
    let mut exclude = cfg.exclude;
    exclude.extend(args.exclude.iter().cloned());
    validate_exclude_globs(&exclude)?;
    let mut generated_path_patterns = cfg.generated_paths;
    generated_path_patterns.extend(args.generated_path.iter().cloned());
    let generated_paths = GeneratedPathAssertions::new(&args.paths, generated_path_patterns)?;
    let ignore_set = ignores::load_for_query(ignore_file.as_deref())?;
    if let Some(ignore_set) = &ignore_set {
        ignore_set.warn_expired();
    }
    Ok((
        QuerySettings {
            min_members,
            min_value,
            sort,
            channels,
            min_lines,
            min_tokens,
            exclude,
            generated_paths,
            ignore_set,
            cache_max_bytes,
        },
        semantic_packs,
    ))
}
