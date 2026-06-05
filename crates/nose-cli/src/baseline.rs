//! Baseline support for incremental adoption: record the duplication a codebase
//! already has, so later runs (and the `--fail` gate) flag only *new* families.
//!
//! A family's identity must survive ordinary edits. Line numbers shift constantly,
//! so the key is a hash over the family's members' `(file, symbol-name)` pairs,
//! sorted — invariant to line moves and member order, but sensitive to *which*
//! sites form the family (adding/removing a copy is legitimately a new family).

use nose_detect::RefactorFamily;
use std::collections::HashSet;
use std::path::Path;

/// Stable cross-run identity of a family.
pub(crate) fn family_key(f: &RefactorFamily) -> u64 {
    let mut members = member_keys(f);
    members.sort_unstable();
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut mix = |bytes: &[u8]| {
        for &b in bytes {
            h = (h ^ b as u64).wrapping_mul(0x0000_0100_0000_01b3);
        }
        h = (h ^ 0xff).wrapping_mul(0x0000_0100_0000_01b3); // field separator
    };
    for MemberKey { file, name } in members {
        mix(file.as_bytes());
        mix(name.as_bytes());
    }
    h
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct MemberKey {
    pub file: String,
    pub name: String,
}

pub(crate) fn member_keys(f: &RefactorFamily) -> Vec<MemberKey> {
    f.locations
        .iter()
        .map(|l| MemberKey {
            file: l.file.clone(),
            name: l.name.clone().unwrap_or_default(),
        })
        .collect()
}

pub(crate) struct Baseline {
    pub keys: HashSet<u64>,
    pub entries: Vec<BaselineEntry>,
}

pub(crate) struct BaselineEntry {
    pub key: u64,
    pub members: Vec<MemberKey>,
}

/// One recorded family: the matching `key` plus a human note (so the baseline file
/// is reviewable in a diff). Only `key` is used for matching.
#[derive(serde::Serialize, serde::Deserialize)]
struct Entry {
    key: String,
    note: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    members: Vec<MemberEntry>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct MemberEntry {
    file: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

/// Load the set of accepted family keys (empty if the file is absent/unreadable).
pub(crate) fn load(path: &Path) -> Baseline {
    let Ok(bytes) = std::fs::read(path) else {
        return Baseline {
            keys: HashSet::new(),
            entries: Vec::new(),
        };
    };
    let entries: Vec<Entry> = serde_json::from_slice(&bytes).unwrap_or_default();
    let entries: Vec<BaselineEntry> = entries
        .iter()
        .filter_map(|e| {
            let key = u64::from_str_radix(&e.key, 16).ok()?;
            let members = e
                .members
                .iter()
                .map(|m| MemberKey {
                    file: m.file.clone(),
                    name: m.name.clone().unwrap_or_default(),
                })
                .collect();
            Some(BaselineEntry { key, members })
        })
        .collect();
    let keys = entries.iter().map(|e| e.key).collect();
    Baseline { keys, entries }
}

/// Write `families` as the accepted baseline, sorted by key for stable git diffs.
pub(crate) fn write(
    path: &Path,
    families: &[RefactorFamily],
    note_of: impl Fn(&RefactorFamily) -> String,
) -> std::io::Result<()> {
    let mut entries: Vec<Entry> = families
        .iter()
        .map(|f| Entry {
            key: format!("{:016x}", family_key(f)),
            note: note_of(f),
            members: member_keys(f)
                .into_iter()
                .map(|m| MemberEntry {
                    file: m.file,
                    name: (!m.name.is_empty()).then_some(m.name),
                })
                .collect(),
        })
        .collect();
    entries.sort_by(|a, b| a.key.cmp(&b.key));
    let mut json = serde_json::to_string_pretty(&entries).unwrap_or_default();
    json.push('\n');
    std::fs::write(path, json)
}
