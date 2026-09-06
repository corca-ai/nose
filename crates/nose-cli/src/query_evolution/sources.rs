//! Explicit source lookup: captured addresses must verify before text is exposed.
use anyhow::{ensure, Context, Result};
use nose_detect::regions::evolution::{AnalysisSnapshot, MemberObservation};
use nose_il::ContentDigest;
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    io::Read,
    path::{Component, Path, PathBuf},
};

const MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_REGION_BYTES: usize = 64 * 1024;

pub(super) struct Sources {
    root: PathBuf,
    path_base: PathBuf,
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
            remaining_bytes: 64 * 1024 * 1024,
            files: BTreeMap::new(),
        })
    }
    fn relative(&self, file: &str) -> Result<PathBuf> {
        let path = Path::new(file);
        let relative = if path.is_absolute() {
            path.strip_prefix(&self.path_base)
                .context("member is outside the captured path base")?
        } else {
            path
        };
        ensure!(
            relative
                .components()
                .all(|c| matches!(c, Component::Normal(_) | Component::CurDir)),
            "member path escapes the explicit source base"
        );
        Ok(relative.to_owned())
    }
    fn text(&mut self, member: &MemberObservation) -> Result<String> {
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
    pub(super) fn member(&mut self, member: &MemberObservation) -> Value {
        match self.text(member) {
            Ok(text) => {
                json!({"file":member.file,"region":member.source,"status":"verified", "text":text})
            }
            Err(error) => {
                json!({"file":member.file,"region":member.source,"status":"unavailable","reason":error.to_string()})
            }
        }
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

pub(super) fn attach(item: &mut Value, before: &mut Option<Sources>, after: &mut Option<Sources>) {
    for (key, sources) in [
        ("before_observation", before),
        ("after_observations", after),
    ] {
        let Some(sources) = sources else { continue };
        if key == "before_observation" {
            attach_family(&mut item[key], sources);
        } else if let Some(families) = item[key].as_array_mut() {
            for family in families {
                attach_family(family, sources);
            }
        }
    }
    let diffs = item["member_changes"]["members"].as_array().into_iter().flatten().filter_map(|row| {
        let before = body_at(&item["before_observation"], &row["before"])?;
        let after_locations = row["after"].as_array()?;
        if after_locations.len() != 1 { return None }
        let after = item["after_observations"].as_array()?.iter().find_map(|f| body_at(f, &after_locations[0]))?;
        let a: Vec<_> = before.lines().collect();
        let b: Vec<_> = after.lines().collect();
        let lines: Vec<_> = crate::source_lines::line_diff(&a, &b).into_iter().map(|(tag, text)| json!({"tag":tag.to_string(),"text":text})).collect();
        Some(json!({"before":row["before"],"after":after_locations[0],"correspondence":row["status"],
            "same_content":before == after,"lines":lines,"truncated":a.len() > 120 || b.len() > 120,"line_limit_per_side":120,
            "meaning":"Text alignment of verified selected regions; correspondence remains advisory where labeled candidate."}))
    }).collect::<Vec<_>>();
    item["source_diffs"] = json!(diffs);
    item["source_body_status"] = json!("explicit-verified-lookup");
}
fn attach_family(family: &mut Value, sources: &mut Sources) {
    let Some(members) = family["members"].as_array_mut() else {
        return;
    };
    for member in members {
        let observation: MemberObservation =
            serde_json::from_value(member.clone()).expect("captured member serializes");
        member["observation_id"] = json!(observation.observation_id());
        member["source_body"] = sources.member(&observation);
    }
}

fn body_at<'a>(family: &'a Value, location: &Value) -> Option<&'a str> {
    let id = location["observation_id"].as_str()?;
    family["members"]
        .as_array()?
        .iter()
        .find(|m| m["observation_id"].as_str() == Some(id))?["source_body"]["text"]
        .as_str()
}
