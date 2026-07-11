//! Baseline support for incremental adoption: record the duplication a codebase
//! already accepts, so later runs (and the `--fail-on` gate) flag only new or
//! changed families.
//!
//! A family's identity is the sorted set of its reported locations. Fragment families
//! can repeat in the same file and enclosing symbol, so the key includes the displayed
//! path, language, source span, syntactic kind, symbol name, and fragment proof metadata.
//! Baseline files also keep each accepted member's source digest, so already-accepted
//! members stay accepted after a family reshapes, while edited source is reported again.

use anyhow::{Context, Result};
use nose_detect::{Loc, RefactorFamily};
use std::collections::{HashMap, HashSet};
use std::path::Path;

pub(crate) const BASELINE_SCHEMA_VERSION: u32 = 2;
const BASELINE_KIND: &str = "accepted-duplication";
const TOOL: &str = "nose";
const DIGEST_PREFIX: &str = "fnv1a64";

/// Stable cross-run identity of a family.
pub(crate) fn family_key(f: &RefactorFamily) -> u64 {
    let mut members: Vec<_> = f
        .locations
        .iter()
        .map(BorrowedMemberKey::from_location)
        .collect();
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

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
struct BorrowedMemberKey<'a> {
    file: &'a str,
    lang: Option<&'a str>,
    start_line: Option<u32>,
    end_line: Option<u32>,
    kind: Option<&'a str>,
    name: &'a str,
    is_fragment: Option<bool>,
    fragment_kind: Option<&'a str>,
    reason_code: Option<&'a str>,
}

impl<'a> BorrowedMemberKey<'a> {
    fn from_location(loc: &'a Loc) -> Self {
        Self {
            file: &loc.file,
            lang: Some(&loc.lang),
            start_line: Some(loc.start_line),
            end_line: Some(loc.end_line),
            kind: Some(unit_kind_name(loc.kind)),
            name: loc.name.as_deref().unwrap_or_default(),
            is_fragment: Some(loc.is_fragment),
            fragment_kind: loc.fragment_kind.map(fragment_kind_name),
            reason_code: loc.reason_code,
        }
    }

    fn from_member_key(member: &'a MemberKey) -> Self {
        Self {
            file: &member.file,
            lang: member.lang.as_deref(),
            start_line: member.start_line,
            end_line: member.end_line,
            kind: member.kind.as_deref(),
            name: &member.name,
            is_fragment: member.is_fragment,
            fragment_kind: member.fragment_kind.as_deref(),
            reason_code: member.reason_code.as_deref(),
        }
    }

    fn mix_into(&self, mix: &mut impl FnMut(&[u8])) {
        mix(self.file.as_bytes());
        mix_opt_str(mix, self.lang);
        mix_opt_u32(mix, self.start_line);
        mix_opt_u32(mix, self.end_line);
        mix_opt_str(mix, self.kind);
        mix(self.name.as_bytes());
        mix_opt_bool(mix, self.is_fragment);
        mix_opt_str(mix, self.fragment_kind);
        mix_opt_str(mix, self.reason_code);
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct AcceptedMember {
    pub key: MemberKey,
    pub source_digest: String,
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

fn member_key_from_location(loc: &Loc) -> MemberKey {
    MemberKey {
        file: loc.file.clone(),
        lang: Some(loc.lang.clone()),
        start_line: Some(loc.start_line),
        end_line: Some(loc.end_line),
        kind: Some(unit_kind_name(loc.kind).to_string()),
        name: loc.name.clone().unwrap_or_default(),
        is_fragment: Some(loc.is_fragment),
        fragment_kind: loc
            .fragment_kind
            .map(fragment_kind_name)
            .map(str::to_string),
        reason_code: loc.reason_code.map(ToOwned::to_owned),
    }
}

fn unit_kind_name(kind: nose_il::UnitKind) -> &'static str {
    match kind {
        nose_il::UnitKind::Function => "Function",
        nose_il::UnitKind::Method => "Method",
        nose_il::UnitKind::Class => "Class",
        nose_il::UnitKind::Block => "Block",
    }
}

fn fragment_kind_name(kind: nose_detect::FragmentKind) -> &'static str {
    match kind {
        nose_detect::FragmentKind::DirectReturn => "direct-return",
        nose_detect::FragmentKind::DirectThrow => "direct-throw",
        nose_detect::FragmentKind::IndexAssignEffect => "index-assign-effect",
        nose_detect::FragmentKind::SelfFieldAssign => "self-field-assign",
        nose_detect::FragmentKind::ExprEffect => "expr-effect",
        nose_detect::FragmentKind::ConditionalGuard => "conditional-guard",
        nose_detect::FragmentKind::LoopEffect => "loop-effect",
        nose_detect::FragmentKind::SelfFieldBody => "self-field-body",
    }
}

pub(crate) fn member_id(loc: &Loc) -> String {
    format_key(member_key_hash_borrowed(&BorrowedMemberKey::from_location(
        loc,
    )))
}

fn member_key_hash(member: &MemberKey) -> u64 {
    member_key_hash_borrowed(&BorrowedMemberKey::from_member_key(member))
}

fn member_key_hash_borrowed(member: &BorrowedMemberKey<'_>) -> u64 {
    let mut h = crate::fnv::OFFSET_BASIS;
    let mut mix = |bytes: &[u8]| {
        for &b in bytes {
            h = crate::fnv::mix(h, b as u64);
        }
        h = crate::fnv::mix(h, 0xff);
    };
    member.mix_into(&mut mix);
    h
}

#[cfg(test)]
fn member_keys(f: &RefactorFamily) -> Vec<MemberKey> {
    f.locations.iter().map(member_key_from_location).collect()
}

pub(crate) fn accepted_members(f: &RefactorFamily) -> Result<Vec<AcceptedMember>> {
    f.locations
        .iter()
        .map(|loc| {
            Ok(AcceptedMember {
                key: member_key_from_location(loc),
                source_digest: source_digest(loc)?,
            })
        })
        .collect()
}

fn source_digest(loc: &Loc) -> Result<String> {
    let lines = crate::source_lines::read_lines(&loc.file, loc.start_line, loc.end_line)
        .with_context(|| {
            format!(
                "reading source for baseline member {}:{}-{}",
                loc.file, loc.start_line, loc.end_line
            )
        })?;
    Ok(format!(
        "{DIGEST_PREFIX}:{:016x}",
        source_digest_hash(&lines)
    ))
}

fn source_digest_hash(lines: &[String]) -> u64 {
    let mut h = crate::fnv::OFFSET_BASIS;
    for &b in b"nose-baseline-member-source-v1" {
        h = crate::fnv::mix(h, b as u64);
    }
    h = crate::fnv::mix(h, 0xff);
    for line in lines {
        for &b in line.as_bytes() {
            h = crate::fnv::mix(h, b as u64);
        }
        h = crate::fnv::mix(h, b'\n' as u64);
    }
    h
}

pub(crate) struct Baseline {
    pub keys: HashSet<u64>,
    pub entries: Vec<BaselineEntry>,
    pub entries_by_key: HashMap<u64, Vec<usize>>,
}

pub(crate) struct BaselineEntry {
    pub key: u64,
    pub members: Vec<AcceptedMember>,
}

/// The baseline file is an auditable envelope around accepted duplicated members.
#[derive(serde::Serialize, serde::Deserialize)]
struct BaselineFile {
    schema_version: u32,
    tool: String,
    baseline_kind: String,
    families: Vec<Entry>,
}

/// One recorded family: the current family id plus a human note. Matching uses the
/// member identities and source digests, not just the family id.
#[derive(serde::Serialize, serde::Deserialize)]
struct Entry {
    id: String,
    note: String,
    members: Vec<MemberEntry>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct MemberEntry {
    id: String,
    source_digest: String,
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
    fn into_accepted_member(self) -> AcceptedMember {
        AcceptedMember {
            key: MemberKey {
                file: self.file,
                lang: self.lang,
                start_line: self.start_line,
                end_line: self.end_line,
                kind: self.kind,
                name: self.name.unwrap_or_default(),
                is_fragment: self.is_fragment,
                fragment_kind: self.fragment_kind,
                reason_code: self.reason_code,
            },
            source_digest: self.source_digest,
        }
    }

    fn from_accepted_member(member: AcceptedMember) -> Self {
        let id = format_key(member_key_hash(&member.key));
        Self {
            id,
            source_digest: member.source_digest,
            file: member.key.file,
            lang: member.key.lang,
            start_line: member.key.start_line,
            end_line: member.key.end_line,
            kind: member.key.kind,
            name: (!member.key.name.is_empty()).then_some(member.key.name),
            is_fragment: member.key.is_fragment,
            fragment_kind: member.key.fragment_kind,
            reason_code: member.key.reason_code,
        }
    }
}

/// Load accepted duplicated members. A missing or malformed baseline is a hard
/// error because `--baseline` is a CI ratchet artifact, not an optional hint.
pub(crate) fn load(path: &Path) -> Result<Baseline> {
    let bytes =
        std::fs::read(path).with_context(|| format!("reading baseline {}", path.display()))?;
    let raw: serde_json::Value = serde_json::from_slice(&bytes)
        .with_context(|| format!("parsing baseline {}", path.display()))?;
    if raw.is_array() {
        anyhow::bail!(
            "baseline {} uses the pre-v2 array format, which is no longer supported; regenerate it with `nose query <path> --baseline {} --write-baseline`",
            path.display(),
            path.display()
        );
    }
    let file: BaselineFile = serde_json::from_value(raw)
        .with_context(|| format!("parsing baseline {}", path.display()))?;
    if file.schema_version != BASELINE_SCHEMA_VERSION {
        anyhow::bail!(
            "baseline {} schema_version must be {BASELINE_SCHEMA_VERSION}, got {}",
            path.display(),
            file.schema_version
        );
    }
    if file.tool != TOOL {
        anyhow::bail!(
            "baseline {} tool must be `{TOOL}`, got `{}`",
            path.display(),
            file.tool
        );
    }
    if file.baseline_kind != BASELINE_KIND {
        anyhow::bail!(
            "baseline {} baseline_kind must be `{BASELINE_KIND}`, got `{}`",
            path.display(),
            file.baseline_kind
        );
    }
    let entries: Vec<BaselineEntry> = file
        .families
        .iter()
        .enumerate()
        .map(|(index, e)| {
            let key = parse_key(&e.id).ok_or_else(|| {
                anyhow::anyhow!(
                    "baseline {} families[{index}].id must be 16 hex digits, optionally prefixed with 0x",
                    path.display()
                )
            })?;
            let members = e
                .members
                .iter()
                .cloned()
                .map(MemberEntry::into_accepted_member)
                .collect();
            Ok(BaselineEntry { key, members })
        })
        .collect::<Result<Vec<_>>>()?;
    let keys = entries.iter().map(|e| e.key).collect();
    let mut entries_by_key: HashMap<u64, Vec<usize>> = HashMap::new();
    for (index, entry) in entries.iter().enumerate() {
        entries_by_key.entry(entry.key).or_default().push(index);
    }
    Ok(Baseline {
        keys,
        entries,
        entries_by_key,
    })
}

/// Write `families` as the accepted baseline, sorted by id for stable git diffs.
pub(crate) fn write(
    path: &Path,
    families: &[RefactorFamily],
    note_of: impl Fn(&RefactorFamily) -> String,
) -> Result<()> {
    let mut entries: Vec<Entry> = families
        .iter()
        .map(|f| {
            Ok(Entry {
                id: family_id(f),
                note: note_of(f),
                members: accepted_members(f)?
                    .into_iter()
                    .map(MemberEntry::from_accepted_member)
                    .collect(),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    entries.sort_by(|a, b| a.id.cmp(&b.id));
    let file = BaselineFile {
        schema_version: BASELINE_SCHEMA_VERSION,
        tool: TOOL.to_owned(),
        baseline_kind: BASELINE_KIND.to_owned(),
        families: entries,
    };
    let mut json = serde_json::to_string_pretty(&file)?;
    json.push('\n');
    std::fs::write(path, json).with_context(|| format!("writing baseline {}", path.display()))
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
            accepted_coverage: Vec::new(),
            mean_sem: 1.0,
            scope: "prod",
            discount: 1.0,
            abstraction_witness: None,
            witness: None,
            varying_spots: Vec::new(),
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
            origin: Default::default(),
            name: None,
            sem: 1,
            span_lines: 1,
            span_tokens: 4,
            is_fragment: true,
            fragment_kind: Some(FragmentKind::ExprEffect),
            reason_code: Some(FragmentKind::ExprEffect.reason_code()),
            enclosing_unit: None,
            in_test_module: false,
            looks_generated: false,
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
    fn baseline_member_kind_names_match_wire_serialization() {
        for kind in [
            UnitKind::Function,
            UnitKind::Method,
            UnitKind::Class,
            UnitKind::Block,
        ] {
            assert_eq!(
                unit_kind_name(kind),
                serde_json::to_value(kind).unwrap().as_str().unwrap()
            );
        }
        for kind in [
            FragmentKind::DirectReturn,
            FragmentKind::DirectThrow,
            FragmentKind::IndexAssignEffect,
            FragmentKind::SelfFieldAssign,
            FragmentKind::ExprEffect,
            FragmentKind::ConditionalGuard,
            FragmentKind::LoopEffect,
            FragmentKind::SelfFieldBody,
        ] {
            assert_eq!(
                fragment_kind_name(kind),
                serde_json::to_value(kind).unwrap().as_str().unwrap()
            );
        }
    }

    #[test]
    fn borrowed_member_keys_match_owned_baseline_identity() {
        let family = fragment_family(&[937, 866, 902]);
        let loc = &family.locations[0];
        let owned = member_key_from_location(loc);

        assert_eq!(
            member_id(loc),
            format_key(member_key_hash(&owned)),
            "fast member ids must stay identical to the owned baseline key"
        );

        let mut members = member_keys(&family);
        members.sort_unstable();
        let mut h = crate::fnv::OFFSET_BASIS;
        let mut mix = |bytes: &[u8]| {
            for &b in bytes {
                h = crate::fnv::mix(h, b as u64);
            }
            h = crate::fnv::mix(h, 0xff);
        };
        for member in &members {
            BorrowedMemberKey::from_member_key(member).mix_into(&mut mix);
        }
        assert_eq!(
            family_key(&family),
            h,
            "fast family ids must preserve owned member ordering and hashing"
        );
    }
}
