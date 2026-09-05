use crate::cli_args::RegionCmd;
use anyhow::{Context, Result};
use nose_detect::regions::RegionSnapshot;
use std::path::Path;

pub(crate) fn run(command: RegionCmd) -> Result<()> {
    match command {
        RegionCmd::Snapshot { path } => {
            let snapshot = capture(&path)?;
            println!("{}", serde_json::to_string(&snapshot)?);
        }
        RegionCmd::Compare {
            before,
            after,
            max_candidates,
        } => {
            let before = read_snapshot(&before)?;
            let after = read_snapshot(&after)?;
            let result = nose_detect::regions::reconcile(&before, &after, max_candidates)
                .map_err(anyhow::Error::msg)?;
            println!("{}", serde_json::to_string(&result)?);
        }
    }
    Ok(())
}

fn read_snapshot(path: &Path) -> Result<RegionSnapshot> {
    // Explicit interchange files have a bounded read; cache files are not inputs.
    const MAX_SNAPSHOT_BYTES: u64 = 128 * 1024 * 1024;
    use std::io::Read;
    let file = std::fs::File::open(path).with_context(|| format!("reading {}", path.display()))?;
    let mut bytes = Vec::new();
    file.take(MAX_SNAPSHOT_BYTES + 1).read_to_end(&mut bytes)?;
    anyhow::ensure!(
        bytes.len() as u64 <= MAX_SNAPSHOT_BYTES,
        "region snapshot exceeds 128 MiB"
    );
    serde_json::from_slice(&bytes).with_context(|| format!("decoding {}", path.display()))
}

fn capture(path: &Path) -> Result<RegionSnapshot> {
    let root =
        std::fs::canonicalize(path).with_context(|| format!("opening {}", path.display()))?;
    let corpus = nose_frontend::lower_corpus(&root);
    corpus.ensure_complete()?;
    let options = nose_detect::DetectOptions {
        min_lines: 1,
        min_tokens: 1,
        ..Default::default()
    };
    let base = if root.is_dir() {
        root.as_path()
    } else {
        root.parent().unwrap_or(&root)
    };
    let mut units = Vec::new();
    for il in &corpus.files {
        let mut extracted = nose_detect::units_of_file(il, &corpus.interner, &options);
        for unit in &mut extracted {
            // Embedded paths carry a region suffix; strip_prefix operates on the
            // shared containing directory and retains that suffix verbatim.
            unit.path = Path::new(&unit.path)
                .strip_prefix(base)
                .context("source region lies outside snapshot root")?
                .to_string_lossy()
                .replace('\\', "/");
        }
        units.extend(extracted);
    }
    Ok(RegionSnapshot::from_units(
        &units,
        format!(
            "nose/{}/regions-v1/default-normalization/min-1",
            env!("CARGO_PKG_VERSION")
        ),
    ))
}
