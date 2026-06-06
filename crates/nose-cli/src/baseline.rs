//! Baseline support for incremental adoption: record the duplication a codebase
//! already has, so later runs (and the `--fail-on` gate) flag only *new* families.
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
    let mut h = crate::fnv::OFFSET_BASIS;
    let mut mix = |bytes: &[u8]| {
        for &b in bytes {
            h = crate::fnv::mix(h, b as u64);
        }
        h = crate::fnv::mix(h, 0xff); // field separator
    };
    for MemberKey { file, name } in members {
        mix(file.as_bytes());
        mix(name.as_bytes());
    }
    h
}

pub(crate) fn family_id(f: &RefactorFamily) -> String {
    format_key(family_key(f))
}

pub(crate) fn format_key(key: u64) -> String {
    format!("{key:016x}")
}

pub(crate) fn parse_key(s: &str) -> Option<u64> {
    let s = s.strip_prefix("0x").unwrap_or(s);
    u64::from_str_radix(s, 16).ok()
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct MemberKey {
    pub file: String,
    pub name: String,
}

/// Build a [`MemberKey`] from a `(file, optional name)` pair, applying the canonical
/// missing-name default (`""`). The single place that decides how an absent member name
/// maps into a key, shared by `member_keys` and baseline loading.
pub(crate) fn member_key(file: &str, name: &Option<String>) -> MemberKey {
    MemberKey {
        file: file.to_owned(),
        name: name.clone().unwrap_or_default(),
    }
}

pub(crate) fn member_keys(f: &RefactorFamily) -> Vec<MemberKey> {
    f.locations
        .iter()
        .map(|l| member_key(&l.file, &l.name))
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
            let key = parse_key(&e.key)?;
            let members = e
                .members
                .iter()
                .map(|m| member_key(&m.file, &m.name))
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
            key: family_id(f),
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
