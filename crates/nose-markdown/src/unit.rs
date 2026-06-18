//! Markdown document → detection units.
//!
//! A unit is a heading-rooted **section** (a heading plus the block content under it, up to the
//! next heading of any level), or the whole document when there are no headings. Section
//! granularity is the survey default; block/sliding-window granularity is a measured follow-up
//! (#437 unit-granularity experiment). Parsing is a deterministic, dependency-free line scan —
//! no tree-sitter grammar — because the substrate is normalized text, not a fine AST.

use crate::norm;
use regex::Regex;
use std::sync::LazyLock;

static HEADING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^#{1,6}\s+").unwrap());

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum UnitKind {
    Document,
    Section,
}

/// One detection unit: a span of one Markdown file with its normalized prose substrate.
#[derive(Clone, Debug)]
pub struct Unit {
    pub path: String,
    /// 1-based inclusive line range in the source file.
    pub start_line: u32,
    pub end_line: u32,
    pub kind: UnitKind,
    pub heading: Option<String>,
    /// Original source slice (for the witness / diff).
    pub raw: String,
    /// Normalized prose (markup stripped, width-folded, lowercased, whitespace-collapsed).
    pub norm: String,
    /// Concatenated fenced-code-block contents, kept separate from prose.
    pub code: String,
    /// Chosen char-gram size for this unit's script.
    pub gram: usize,
}

impl Unit {
    /// The character-n-gram shingle multiset over normalized prose — the substrate Stage-1
    /// fingerprints (MinHash / winnowing / containment) operate on.
    pub fn shingles(&self) -> Vec<String> {
        norm::char_ngrams(&self.norm, self.gram)
    }

    /// Word count of the normalized prose (used by length floors).
    pub fn prose_words(&self) -> usize {
        if self.norm.is_empty() {
            0
        } else {
            self.norm.split(' ').filter(|w| !w.is_empty()).count()
        }
    }
}

fn make_unit(path: &str, kind: UnitKind, start_line: u32, end_line: u32, raw: &str) -> Unit {
    let n = norm::normalize_text(raw, true);
    let gram = norm::gram_size_for(&n);
    let heading = raw
        .lines()
        .next()
        .filter(|l| HEADING.is_match(l))
        .map(|l| HEADING.replace(l, "").trim().to_string());
    Unit {
        path: path.to_string(),
        start_line,
        end_line,
        kind,
        heading,
        raw: raw.to_string(),
        norm: n,
        code: norm::extract_code(raw),
        gram,
    }
}

/// Split a Markdown document into section units (or one Document unit if heading-free).
pub fn split_units(path: &str, src: &str) -> Vec<Unit> {
    let lines: Vec<&str> = src.lines().collect();
    if lines.is_empty() {
        return Vec::new();
    }
    let heading_idx: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| HEADING.is_match(l))
        .map(|(i, _)| i)
        .collect();

    if heading_idx.is_empty() {
        let raw = src;
        return vec![make_unit(
            path,
            UnitKind::Document,
            1,
            lines.len() as u32,
            raw,
        )];
    }

    let mut units = Vec::new();
    // Preamble before the first heading (if any non-blank content) becomes its own section.
    if heading_idx[0] > 0 {
        let raw = lines[..heading_idx[0]].join("\n");
        if !raw.trim().is_empty() {
            units.push(make_unit(
                path,
                UnitKind::Section,
                1,
                heading_idx[0] as u32,
                &raw,
            ));
        }
    }
    for (k, &start) in heading_idx.iter().enumerate() {
        let end = heading_idx.get(k + 1).copied().unwrap_or(lines.len());
        let raw = lines[start..end].join("\n");
        units.push(make_unit(
            path,
            UnitKind::Section,
            (start + 1) as u32,
            end as u32,
            &raw,
        ));
    }
    units
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_into_heading_sections() {
        let src = "intro line\n\n# A\ntext a\n\n## B\ntext b\nmore b\n";
        let units = split_units("x.md", src);
        // preamble + 2 headings = 3 sections
        assert_eq!(units.len(), 3);
        assert_eq!(units[1].heading.as_deref(), Some("A"));
        assert_eq!(units[2].heading.as_deref(), Some("B"));
        assert_eq!(units[1].kind, UnitKind::Section);
        // line spans are 1-based inclusive and contiguous
        assert_eq!(units[1].start_line, 3);
    }

    #[test]
    fn heading_free_doc_is_one_document_unit() {
        let src = "just some prose\nwith two lines";
        let units = split_units("y.md", src);
        assert_eq!(units.len(), 1);
        assert_eq!(units[0].kind, UnitKind::Document);
        assert_eq!(units[0].end_line, 2);
    }

    #[test]
    fn unit_substrate_is_char_ngrams() {
        let src = "# Title\n\nThe quick brown fox jumps over the lazy dog.";
        let units = split_units("z.md", src);
        let s = &units[units.len() - 1];
        assert!(s.prose_words() > 5);
        assert_eq!(s.gram, 5); // latin
        let sh = s.shingles();
        assert!(!sh.is_empty());
        assert_eq!(sh[0].chars().count(), 5);
    }
}
