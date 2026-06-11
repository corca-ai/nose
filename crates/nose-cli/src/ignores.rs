//! Structured suppressions for scan findings.
//!
//! Inline `nose-ignore` removes a unit before detection. This module handles the
//! later, auditable case: a family was found, reviewed, and intentionally hidden
//! with a reason, owner, and optional expiry.

use crate::baseline;
use anyhow::{Context, Result};
use ignore::overrides::Override;
use nose_detect::RefactorFamily;
use serde::Deserialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub(crate) const DEFAULT_IGNORE_FILE: &str = "nose.ignore.json";

pub(crate) struct IgnoreSet {
    path: PathBuf,
    entries: Vec<Entry>,
    expired: Vec<ExpiredEntry>,
}

#[derive(Clone, serde::Serialize)]
pub(crate) struct IgnoreSelectors {
    #[serde(skip_serializing_if = "Option::is_none")]
    family_id: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    paths: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    languages: Vec<String>,
}

#[derive(Clone, serde::Serialize)]
pub(crate) struct IgnoreMatch {
    entry: usize,
    reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    owner: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expires_at: Option<String>,
    selectors: IgnoreSelectors,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    matched_paths: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    matched_languages: Vec<String>,
}

#[derive(Clone, serde::Serialize)]
pub(crate) struct ExpiredEntry {
    entry: usize,
    reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    owner: Option<String>,
    expires_at: String,
}

#[derive(serde::Serialize)]
pub(crate) struct IgnoreSummary<'a> {
    path: String,
    active_entries: usize,
    expired_entries: usize,
    ignored_families: usize,
    expired: &'a [ExpiredEntry],
}

impl<'a> IgnoreSummary<'a> {
    pub(crate) fn line(&self) -> String {
        format!(
            "structured ignores: {} ignored {} · {} active {} · {} expired {} ({})",
            self.ignored_families,
            plural(self.ignored_families, "family", "families"),
            self.active_entries,
            plural(self.active_entries, "entry", "entries"),
            self.expired_entries,
            plural(self.expired_entries, "entry", "entries"),
            self.path
        )
    }
}

struct Entry {
    index: usize,
    family_id: Option<u64>,
    path_matcher: Option<Override>,
    language_set: BTreeSet<String>,
    selectors: IgnoreSelectors,
    reason: String,
    note: Option<String>,
    owner: Option<String>,
    expires_at: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Document {
    Object(DocumentObject),
    Entries(Vec<RawEntry>),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DocumentObject {
    ignores: Vec<RawEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEntry {
    family_id: Option<String>,
    #[serde(default)]
    paths: Vec<String>,
    #[serde(default)]
    languages: Vec<String>,
    reason: String,
    note: Option<String>,
    owner: Option<String>,
    expires_at: Option<String>,
}

pub(crate) fn load_for_scan(path: Option<&Path>) -> Result<Option<IgnoreSet>> {
    let path = match path {
        Some(path) => Some(path.to_path_buf()),
        None => {
            let default = PathBuf::from(DEFAULT_IGNORE_FILE);
            default.is_file().then_some(default)
        }
    };
    path.map(|path| load(&path)).transpose()
}

pub(crate) fn load(path: &Path) -> Result<IgnoreSet> {
    let bytes =
        std::fs::read(path).with_context(|| format!("reading ignore file {}", path.display()))?;
    let document: Document = serde_json::from_slice(&bytes)
        .with_context(|| format!("parsing ignore file {}", path.display()))?;
    let raw_entries = match document {
        Document::Object(document) => document.ignores,
        Document::Entries(entries) => entries,
    };

    let today = Date::today();
    let mut entries = Vec::new();
    let mut expired = Vec::new();
    for (index, raw) in raw_entries.into_iter().enumerate() {
        let entry = Entry::from_raw(index, raw)
            .with_context(|| format!("validating ignore file {}", path.display()))?;
        if entry.is_expired(today) {
            expired.push(ExpiredEntry {
                entry: index,
                reason: entry.reason,
                owner: entry.owner,
                expires_at: entry.expires_at.expect("expired entries have expires_at"),
            });
        } else {
            entries.push(entry);
        }
    }
    Ok(IgnoreSet {
        path: path.to_path_buf(),
        entries,
        expired,
    })
}

impl IgnoreSet {
    pub(crate) fn match_family(&self, family: &RefactorFamily) -> Option<IgnoreMatch> {
        self.entries
            .iter()
            .find_map(|entry| entry.match_family(family))
    }

    pub(crate) fn summary(&self, ignored_families: usize) -> IgnoreSummary<'_> {
        IgnoreSummary {
            path: self.path.display().to_string(),
            active_entries: self.entries.len(),
            expired_entries: self.expired.len(),
            ignored_families,
            expired: &self.expired,
        }
    }

    pub(crate) fn warn_expired(&self) {
        for entry in &self.expired {
            eprintln!(
                "warning: ignore file {} entry ignores[{}] expired on {} and was not applied ({})",
                self.path.display(),
                entry.entry,
                entry.expires_at,
                entry.reason
            );
        }
    }
}

impl Entry {
    fn from_raw(index: usize, raw: RawEntry) -> Result<Self> {
        if raw.reason.trim().is_empty() {
            anyhow::bail!("ignores[{index}].reason must be a non-empty string");
        }
        let family_id = raw
            .family_id
            .as_deref()
            .map(parse_family_id)
            .transpose()
            .with_context(|| format!("ignores[{index}].family_id is not a hex family id"))?;
        let path_matcher = build_path_matcher(index, &raw.paths)?;
        let language_set = raw
            .languages
            .iter()
            .map(|language| {
                let language = language.trim().to_ascii_lowercase();
                if language.is_empty() {
                    anyhow::bail!("ignores[{index}].languages contains an empty language");
                }
                Ok(language)
            })
            .collect::<Result<BTreeSet<_>>>()?;
        if family_id.is_none() && raw.paths.is_empty() && language_set.is_empty() {
            anyhow::bail!(
                "ignores[{index}] must set at least one selector: family_id, paths, or languages"
            );
        }
        let expires_at = raw
            .expires_at
            .as_deref()
            .map(parse_date_for_entry(index))
            .transpose()?
            .map(|date| date.to_string());

        Ok(Entry {
            index,
            family_id,
            path_matcher,
            language_set,
            selectors: IgnoreSelectors {
                family_id: raw.family_id,
                paths: raw.paths,
                languages: raw.languages,
            },
            reason: raw.reason,
            note: raw.note,
            owner: raw.owner,
            expires_at,
        })
    }

    fn is_expired(&self, today: Date) -> bool {
        self.expires_at
            .as_deref()
            .and_then(|date| Date::parse(date).ok())
            .is_some_and(|date| date < today)
    }

    fn match_family(&self, family: &RefactorFamily) -> Option<IgnoreMatch> {
        if self
            .family_id
            .is_some_and(|family_id| family_id != baseline::family_key(family))
        {
            return None;
        }

        // Selector semantics are ALL-members (coevo series 3, packet
        // c4-path-oversuppress): an entry describes families wholly inside
        // its selectors. Any-member matching let `paths: ["vendor/**"]`
        // swallow a family whose OTHER copy lives in `src/` — the first-party
        // duplication silently passed `--fail-on any`. Suppression must never
        // hide a member the selector does not cover.
        let matched_paths = match &self.path_matcher {
            Some(matcher) => {
                if !family
                    .locations
                    .iter()
                    .all(|location| matcher.matched(&location.file, false).is_whitelist())
                {
                    return None;
                }
                family
                    .locations
                    .iter()
                    .map(|location| location.file.clone())
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect()
            }
            None => Vec::new(),
        };

        let matched_languages = if self.language_set.is_empty() {
            Vec::new()
        } else {
            if !family.locations.iter().all(|location| {
                self.language_set
                    .contains(&location.lang.to_ascii_lowercase())
            }) {
                return None;
            }
            family
                .locations
                .iter()
                .map(|location| location.lang.to_ascii_lowercase())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect()
        };

        Some(IgnoreMatch {
            entry: self.index,
            reason: self.reason.clone(),
            note: self.note.clone(),
            owner: self.owner.clone(),
            expires_at: self.expires_at.clone(),
            selectors: self.selectors.clone(),
            matched_paths,
            matched_languages,
        })
    }
}

fn build_path_matcher(index: usize, paths: &[String]) -> Result<Option<Override>> {
    if paths.is_empty() {
        return Ok(None);
    }
    let mut builder = ignore::overrides::OverrideBuilder::new(".");
    for pattern in paths {
        let pattern = pattern.trim();
        if pattern.is_empty() {
            anyhow::bail!("ignores[{index}].paths contains an empty pattern");
        }
        if pattern.starts_with('!') {
            anyhow::bail!("ignores[{index}].paths does not support negative pattern {pattern:?}");
        }
        builder
            .add(pattern)
            .with_context(|| format!("ignores[{index}].paths has invalid glob {pattern:?}"))?;
    }
    builder
        .build()
        .with_context(|| format!("building path matcher for ignores[{index}]"))
        .map(Some)
}

fn parse_family_id(s: &str) -> Result<u64> {
    let s = s.trim();
    let hex = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    // Require exactly 16 hex digits: this rejects a sign prefix (`+`/`-`) that
    // u64::from_str_radix would otherwise accept, and any other non-hex character.
    if hex.len() != 16 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        anyhow::bail!("family ids must be 16 hex digits, optionally prefixed with 0x");
    }
    u64::from_str_radix(hex, 16).map_err(|_| {
        anyhow::anyhow!("family ids must be 16 hex digits, optionally prefixed with 0x")
    })
}

fn parse_date_for_entry(index: usize) -> impl FnOnce(&str) -> Result<Date> {
    move |date| {
        Date::parse(date).with_context(|| format!("ignores[{index}].expires_at must be YYYY-MM-DD"))
    }
}

fn plural<'a>(n: usize, singular: &'a str, plural: &'a str) -> &'a str {
    if n == 1 {
        singular
    } else {
        plural
    }
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
struct Date {
    year: i32,
    month: u32,
    day: u32,
}

impl Date {
    fn parse(s: &str) -> Result<Self> {
        let bytes = s.as_bytes();
        if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
            anyhow::bail!("expected YYYY-MM-DD");
        }
        let year = parse_digits(&bytes[0..4]).context("invalid year")? as i32;
        let month = parse_digits(&bytes[5..7]).context("invalid month")?;
        let day = parse_digits(&bytes[8..10]).context("invalid day")?;
        if !(1..=12).contains(&month) {
            anyhow::bail!("month must be 01 through 12");
        }
        let max_day = days_in_month(year, month);
        if day == 0 || day > max_day {
            anyhow::bail!("day must be valid for the month");
        }
        Ok(Date { year, month, day })
    }

    fn today() -> Self {
        let days = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            / 86_400;
        civil_from_days(days as i64)
    }
}

impl std::fmt::Display for Date {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

fn parse_digits(bytes: &[u8]) -> Result<u32> {
    let mut value = 0u32;
    for &byte in bytes {
        if !byte.is_ascii_digit() {
            anyhow::bail!("not a decimal digit");
        }
        value = value * 10 + u32::from(byte - b'0');
    }
    Ok(value)
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn civil_from_days(days_since_epoch: i64) -> Date {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = y + i64::from(month <= 2);
    Date {
        year: year as i32,
        month: month as u32,
        day: day as u32,
    }
}

#[cfg(test)]
mod parse_id_tests {
    use super::parse_family_id;

    #[test]
    fn rejects_non_hex_sign_prefix() {
        // '+' + 15 hex digits = 16 chars; u64::from_str_radix would accept the '+'.
        assert!(parse_family_id("+aaaaaaaaaaaaaaa").is_err());
        assert!(parse_family_id("-aaaaaaaaaaaaaaa").is_err());
    }

    #[test]
    fn accepts_uppercase_0x_prefix() {
        let v = parse_family_id("0XAAAAAAAAAAAAAAAA").expect("uppercase 0X prefix is valid");
        assert_eq!(v, 0xAAAA_AAAA_AAAA_AAAA);
    }

    #[test]
    fn accepts_plain_and_lowercase_prefixed_ids() {
        assert_eq!(
            parse_family_id("aaaaaaaaaaaaaaaa").unwrap(),
            0xAAAA_AAAA_AAAA_AAAA
        );
        assert_eq!(
            parse_family_id("0xaaaaaaaaaaaaaaaa").unwrap(),
            0xAAAA_AAAA_AAAA_AAAA
        );
    }
}
