//! Explicit source lookup: captured addresses must verify before text is exposed.
#[cfg(test)]
mod tests;
use anyhow::{ensure, Context, Result};
use nose_detect::regions::evolution::{AnalysisSnapshot, MemberObservation};
use nose_il::ContentDigest;
use std::{
    collections::BTreeMap,
    io::Read,
    path::{Component, Path, PathBuf},
};

pub(super) const MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;
pub(super) const MAX_TOTAL_BYTES: usize = 64 * 1024 * 1024;
pub(super) const MAX_REGION_BYTES: usize = 64 * 1024;

/// Keep the original cwd layout when it contains the roots; external roots need
/// their own common directory so the explicit source action can reopen them.
pub(super) fn base(snapshot: &AnalysisSnapshot) -> PathBuf {
    let original = PathBuf::from(&snapshot.path_base);
    let roots: Vec<_> = snapshot
        .roots
        .iter()
        .map(|root| original.join(root))
        .collect();
    if roots.iter().all(|root| root.starts_with(&original)) {
        return original;
    }
    let mut common = roots[0].clone();
    for root in &roots[1..] {
        while !root.starts_with(&common) {
            if !common.pop() {
                return original;
            }
        }
    }
    if snapshot
        .families
        .iter()
        .flat_map(|f| &f.members)
        .any(|m| original.join(&m.file) == common)
    {
        common.pop();
    }
    common
}

pub(super) struct Sources {
    root: PathBuf,
    path_base: PathBuf,
    captured_base: PathBuf,
    remaining_bytes: usize,
    files: BTreeMap<PathBuf, Result<SourceBytes, String>>,
}
impl Sources {
    pub(super) fn new(root: &Path, snapshot: &AnalysisSnapshot) -> Result<Self> {
        let root = root
            .canonicalize()
            .context("opening explicit source directory")?;
        ensure!(root.is_dir(), "source base must be a directory");
        Ok(Self {
            root,
            path_base: PathBuf::from(&snapshot.path_base),
            captured_base: base(snapshot),
            remaining_bytes: MAX_TOTAL_BYTES,
            files: BTreeMap::new(),
        })
    }
    fn relative(&self, file: &str) -> Result<PathBuf> {
        let absolute = self.path_base.join(file);
        let relative = absolute
            .strip_prefix(&self.captured_base)
            .context("member is outside the captured source base")?;
        ensure!(
            relative
                .components()
                .all(|c| matches!(c, Component::Normal(_) | Component::CurDir)),
            "member path escapes the explicit source base"
        );
        Ok(relative.to_owned())
    }
    pub(super) fn text(&mut self, member: &MemberObservation) -> Result<String> {
        let source = member
            .source
            .as_ref()
            .context("captured source address unavailable")?;
        let relative = self.relative(&member.file)?;
        let root = &self.root;
        let remaining = &mut self.remaining_bytes;
        let bytes = self
            .files
            .entry(relative.clone())
            .or_insert_with(|| read_file(root, &relative, remaining).map_err(|e| e.to_string()))
            .as_ref()
            .map_err(|e| anyhow::anyhow!(e.clone()))?;
        ensure!(
            bytes.digest == source.source_digest,
            "source snapshot mismatch; supply the directory from this capture's revision"
        );
        let selected = bytes
            .bytes
            .get(source.start_byte as usize..source.end_byte as usize)
            .context("captured byte range is unavailable")?;
        ensure!(
            ContentDigest::sha256(selected) == source.content_digest,
            "selected content digest mismatch"
        );
        ensure!(
            selected.len() <= MAX_REGION_BYTES,
            "selected region exceeds 64 KiB display budget"
        );
        Ok(std::str::from_utf8(selected)
            .context("selected source is not UTF-8")?
            .to_owned())
    }
}

struct SourceBytes {
    bytes: Vec<u8>,
    digest: ContentDigest,
}
fn read_file(root: &Path, relative: &Path, remaining: &mut usize) -> Result<SourceBytes> {
    let path = root
        .join(relative)
        .canonicalize()
        .context("source file unavailable")?;
    ensure!(
        path.starts_with(root),
        "source symlink escapes the explicit source base"
    );
    ensure!(
        *remaining > 0,
        "source inspection exhausted its 64 MiB read budget"
    );
    ensure!(
        std::fs::metadata(&path)?.is_file(),
        "source is not a regular file"
    );
    let file = std::fs::File::open(path)?;
    ensure!(file.metadata()?.is_file(), "source is not a regular file");
    ensure!(
        file.metadata()?.len() <= *remaining as u64,
        "source inspection exceeds its remaining read budget"
    );
    let mut bytes = Vec::new();
    let limit = (MAX_FILE_BYTES + 1).min(*remaining as u64);
    file.take(limit).read_to_end(&mut bytes)?;
    *remaining -= bytes.len();
    ensure!(
        bytes.len() as u64 <= MAX_FILE_BYTES,
        "source file exceeds 16 MiB read budget"
    );
    let digest = ContentDigest::sha256(&bytes);
    Ok(SourceBytes { bytes, digest })
}
