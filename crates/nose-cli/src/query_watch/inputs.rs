use crate::cli_args::QueryArgs;
use anyhow::Result;
use std::collections::BTreeSet;
use std::path::PathBuf;

/// Watch containing directories so atomic file replacement keeps being observed.
fn input_files(args: &QueryArgs) -> Result<BTreeSet<PathBuf>> {
    let cfg = crate::config::load_query(args.config.as_deref())?;
    let mut files = vec![PathBuf::from("nose.toml"), PathBuf::from(".nose.toml")];
    files.extend(args.config.iter().cloned());
    files.push(
        args.ignore_file
            .clone()
            .or(cfg.ignore_file)
            .unwrap_or_else(|| PathBuf::from(crate::ignores::DEFAULT_IGNORE_FILE)),
    );
    Ok(files.iter().map(|file| super::absolute(file)).collect())
}

pub(super) fn register(
    args: &QueryArgs,
    watcher: &mut notify::RecommendedWatcher,
    watched: &mut BTreeSet<PathBuf>,
) -> Result<BTreeSet<PathBuf>> {
    use notify::Watcher;
    let files = input_files(args)?;
    for file in &files {
        let mut path = file.parent().unwrap().to_path_buf();
        while !path.is_dir() {
            if !path.pop() {
                break;
            }
        }
        if watched.insert(path.clone()) {
            watcher.watch(&path, notify::RecursiveMode::NonRecursive)?;
        }
    }
    Ok(files)
}
