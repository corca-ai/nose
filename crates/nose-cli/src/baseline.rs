//! Baseline support for incremental adoption: record the duplication a codebase
//! already has, so later runs (and the `--fail-on` gate) flag only *new* families.
//!
//! A family's identity is the sorted set of its reported locations. Fragment families
//! can repeat in the same file and enclosing symbol, so the key includes the displayed
//! path, language, source span, syntactic kind, symbol name, and fragment proof metadata.
//! Baseline files also keep those member identities for changed-family matching; older
//! member-only baselines are still accepted as coarse `(file, name)` overlap hints.

use anyhow::{Context, Result};
use nose_detect::{Loc, RefactorFamily};
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
    for member in members {
        member.mix_into(&mut mix);
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
    let hex = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    if hex.len() != 16 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    u64::from_str_radix(hex, 16).ok()
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct MemberKey {
    pub file: String,
    pub lang: Option<String>,
    pub start_line: Option<u32>,
    pub end_line: Option<u32>,
    pub kind: Option<String>,
    pub name: String,
    pub is_fragment: Option<bool>,
    pub fragment_kind: Option<String>,
    pub reason_code: Option<String>,
}

impl MemberKey {
    fn mix_into(&self, mix: &mut impl FnMut(&[u8])) {
        mix(self.file.as_bytes());
        mix_opt_str(mix, self.lang.as_deref());
        mix_opt_u32(mix, self.start_line);
        mix_opt_u32(mix, self.end_line);
        mix_opt_str(mix, self.kind.as_deref());
        mix(self.name.as_bytes());
        mix_opt_bool(mix, self.is_fragment);
        mix_opt_str(mix, self.fragment_kind.as_deref());
        mix_opt_str(mix, self.reason_code.as_deref());
    }

    fn has_location_identity(&self) -> bool {
        self.lang.is_some()
            && self.start_line.is_some()
            && self.end_line.is_some()
            && self.kind.is_some()
            && self.is_fragment.is_some()
    }

    fn overlaps(&self, other: &Self) -> bool {
        if self.file != other.file || self.name != other.name {
            return false;
        }
        if self.has_location_identity() && other.has_location_identity() {
            return self == other;
        }
        true
    }
}

fn mix_opt_str(mix: &mut impl FnMut(&[u8]), value: Option<&str>) {
    match value {
        Some(value) => {
            mix(b"some");
            mix(value.as_bytes());
        }
        None => mix(b"none"),
    }
}

fn mix_opt_u32(mix: &mut impl FnMut(&[u8]), value: Option<u32>) {
    match value {
        Some(value) => {
            mix(b"some");
            mix(&value.to_le_bytes());
        }
        None => mix(b"none"),
    }
}

fn mix_opt_bool(mix: &mut impl FnMut(&[u8]), value: Option<bool>) {
    match value {
        Some(true) => mix(b"true"),
        Some(false) => mix(b"false"),
        None => mix(b"none"),
    }
}

fn serialized_name<T>(value: T) -> String
where
    T: serde::Serialize + std::fmt::Debug,
{
    serde_json::to_value(&value)
        .ok()
        .and_then(|value| value.as_str().map(ToOwned::to_owned))
        .unwrap_or_else(|| format!("{value:?}"))
}

fn member_key_from_location(loc: &Loc) -> MemberKey {
    MemberKey {
        file: loc.file.clone(),
        lang: Some(loc.lang.clone()),
        start_line: Some(loc.start_line),
        end_line: Some(loc.end_line),
        kind: Some(serialized_name(loc.kind)),
        name: loc.name.clone().unwrap_or_default(),
        is_fragment: Some(loc.is_fragment),
        fragment_kind: loc.fragment_kind.map(serialized_name),
        reason_code: loc.reason_code.map(ToOwned::to_owned),
    }
}

pub(crate) fn member_keys(f: &RefactorFamily) -> Vec<MemberKey> {
    f.locations.iter().map(member_key_from_location).collect()
}

pub(crate) fn member_sets_overlap(left: &[MemberKey], right: &[MemberKey]) -> bool {
    left.iter()
        .any(|left| right.iter().any(|right| left.overlaps(right)))
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

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct MemberEntry {
    file: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    lang: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    start_line: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    end_line: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    is_fragment: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    fragment_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reason_code: Option<String>,
}

impl MemberEntry {
    fn into_member_key(self) -> MemberKey {
        MemberKey {
            file: self.file,
            lang: self.lang,
            start_line: self.start_line,
            end_line: self.end_line,
            kind: self.kind,
            name: self.name.unwrap_or_default(),
            is_fragment: self.is_fragment,
            fragment_kind: self.fragment_kind,
            reason_code: self.reason_code,
        }
    }

    fn from_member_key(member: MemberKey) -> Self {
        Self {
            file: member.file,
            lang: member.lang,
            start_line: member.start_line,
            end_line: member.end_line,
            kind: member.kind,
            name: (!member.name.is_empty()).then_some(member.name),
            is_fragment: member.is_fragment,
            fragment_kind: member.fragment_kind,
            reason_code: member.reason_code,
        }
    }
}

/// Load the set of accepted family keys. A missing or malformed baseline is a hard
/// error because `--baseline` is a CI ratchet artifact, not an optional hint.
pub(crate) fn load(path: &Path) -> Result<Baseline> {
    let bytes =
        std::fs::read(path).with_context(|| format!("reading baseline {}", path.display()))?;
    let entries: Vec<Entry> = serde_json::from_slice(&bytes)
        .with_context(|| format!("parsing baseline {}", path.display()))?;
    let entries: Vec<BaselineEntry> = entries
        .iter()
        .enumerate()
        .map(|(index, e)| {
            let key = parse_key(&e.key).ok_or_else(|| {
                anyhow::anyhow!(
                    "baseline {} entry[{index}].key must be 16 hex digits, optionally prefixed with 0x",
                    path.display()
                )
            })?;
            let members = e
                .members
                .iter()
                .cloned()
                .map(MemberEntry::into_member_key)
                .collect();
            Ok(BaselineEntry { key, members })
        })
        .collect::<Result<Vec<_>>>()?;
    let keys = entries.iter().map(|e| e.key).collect();
    Ok(Baseline { keys, entries })
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
                .map(MemberEntry::from_member_key)
                .collect(),
        })
        .collect();
    entries.sort_by(|a, b| a.key.cmp(&b.key));
    let mut json = serde_json::to_string_pretty(&entries).unwrap_or_default();
    json.push('\n');
    std::fs::write(path, json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nose_detect::{FragmentKind, Loc};
    use nose_il::UnitKind;

    fn fragment_family(starts: &[u32]) -> RefactorFamily {
        RefactorFamily {
            value: 1.0,
            members: starts.len(),
            files: 1,
            modules: 1,
            languages: 1,
            mean_score: 1.0,
            mean_lines: 1,
            dup_lines: starts.len().saturating_sub(1) as u32,
            shared_lines: 1,
            params: 0,
            shared_weight: 1.0,
            locations: starts.iter().copied().map(fragment_loc).collect(),
            mean_sem: 1.0,
            scope: "prod",
            discount: 1.0,
            abstraction_witness: None,
            witness: None,
            semantic_laws: Vec::new(),
        }
    }

    fn fragment_loc(start_line: u32) -> Loc {
        Loc {
            file: "src/main/java/example/DateUtils.java".to_owned(),
            start_line,
            end_line: start_line,
            lang: "java".to_owned(),
            kind: UnitKind::Block,
            name: None,
            sem: 1,
            span_lines: 1,
            span_tokens: 4,
            is_fragment: true,
            fragment_kind: Some(FragmentKind::ExprEffect),
            reason_code: Some(FragmentKind::ExprEffect.reason_code()),
            enclosing_unit: None,
            shared_subdag: None,
        }
    }

    #[test]
    fn family_key_distinguishes_same_file_fragment_families_by_span() {
        let first = fragment_family(&[866, 902, 937]);
        let second = fragment_family(&[867, 903, 938]);

        assert_ne!(
            family_id(&first),
            family_id(&second),
            "families with the same file/name members but different reported spans need unique ids"
        );
    }

    #[test]
    fn legacy_member_keys_overlap_location_identities_by_file_and_name() {
        let current = member_keys(&fragment_family(&[866, 902, 937]));
        let legacy = vec![MemberKey {
            file: "src/main/java/example/DateUtils.java".to_owned(),
            lang: None,
            start_line: None,
            end_line: None,
            kind: None,
            name: String::new(),
            is_fragment: None,
            fragment_kind: None,
            reason_code: None,
        }];

        assert!(
            member_sets_overlap(&legacy, &current),
            "member-only baselines should still classify re-keyed families as changed"
        );
    }
}
