//! Normalization + tokenization for the character-n-gram substrate.
//!
//! Ported from the validated survey prototype (docs/markdown-dup-detection-algorithm-survey
//! -2026-06-18.md, harness `tmp/md-dup/src/lib.py`). The substrate is character n-grams of
//! NORMALIZED prose, which is what makes detection script-agnostic with no per-language
//! segmenter (word-token IR collapses on no-space scripts; char-grams do not).
//!
//! Normalization steps (each preserves rendered meaning for the fuzzy channel; none of these
//! folds is ever used to claim two texts are *identical* — see the honesty contract):
//!   strip Markdown markup → fullwidth→halfwidth fold → lowercase → collapse whitespace.
//! NOTE: full NFC/NFKC is a measured follow-up (would add `unicode-normalization`); the
//! fullwidth fold covers the dominant CJK width-variant case found in the corpus.

use regex::Regex;
use std::sync::LazyLock;

static CODE_FENCE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)```.*?```").unwrap());
static CODE_FENCE_CAP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)```(.*?)```").unwrap());
static INLINE_CODE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"`[^`]*`").unwrap());
static IMG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"!\[([^\]]*)\]\([^)]*\)").unwrap());
static LINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[([^\]]*)\]\([^)]*\)").unwrap());
static HEADING_HASH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^#{1,6}\s+").unwrap());
static EMPH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\*\*|__|\*|_|~~)").unwrap());
static LIST_MARK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^\s*([-*+]|\d+\.)\s+").unwrap());
static BLOCKQUOTE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^\s*>\s?").unwrap());
// GFM table separator row (`|---|:--:|`): pure scaffolding, no content.
static TABLE_SEP: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^[ \t|:-]*-[ \t|:-]*$").unwrap());
static WS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());

/// True for characters from no-space scripts (CJK, Hangul, Thai) where there are no word
/// boundaries — these are tokenized per-character so char-grams capture local context.
pub fn is_nospace_char(c: char) -> bool {
    let o = c as u32;
    (0x3040..=0x30FF).contains(&o)   // Hiragana, Katakana
        || (0x3400..=0x9FFF).contains(&o) // CJK Unified Ideographs (+ ext A)
        || (0xF900..=0xFAFF).contains(&o) // CJK compat
        || (0xAC00..=0xD7A3).contains(&o) // Hangul syllables
        || (0x0E00..=0x0E7F).contains(&o) // Thai
}

/// Fold fullwidth ASCII forms (U+FF01..U+FF5E) and the ideographic space to their
/// halfwidth equivalents, so e.g. fullwidth `！` and `!` hash identically.
fn fold_width(s: &str) -> String {
    s.chars()
        .map(|c| {
            let o = c as u32;
            if (0xFF01..=0xFF5E).contains(&o) {
                char::from_u32(o - 0xFEE0).unwrap_or(c)
            } else if o == 0x3000 {
                ' '
            } else {
                c
            }
        })
        .collect()
}

/// Normalize a chunk of Markdown into the prose token stream the substrate operates on.
/// When `strip_md` is false, only width-fold + lowercase + whitespace-collapse is applied.
pub fn normalize_text(raw: &str, strip_md: bool) -> String {
    let mut t = raw.to_string();
    if strip_md {
        t = CODE_FENCE.replace_all(&t, " ").into_owned();
        t = IMG.replace_all(&t, " ${1} ").into_owned();
        t = LINK.replace_all(&t, " ${1} ").into_owned();
        t = INLINE_CODE.replace_all(&t, " ").into_owned();
        t = HEADING_HASH.replace_all(&t, "").into_owned();
        t = BLOCKQUOTE.replace_all(&t, "").into_owned();
        t = LIST_MARK.replace_all(&t, "").into_owned();
        t = EMPH.replace_all(&t, "").into_owned();
        // Strip GFM table scaffolding: separator rows entirely, and cell-delimiter pipes
        // to spaces. Table pipes/separators are format, not content — keeping them makes
        // every table near-identical and drives over-merge across templated docs.
        t = TABLE_SEP.replace_all(&t, " ").into_owned();
        t = t.replace('|', " ");
    }
    let t = fold_width(&t).to_lowercase();
    WS.replace_all(t.trim(), " ").into_owned()
}

/// Concatenate the verbatim contents of fenced code blocks (kept separate from prose
/// because code and prose have different n-gram distributions — code/prose split).
pub fn extract_code(raw: &str) -> String {
    let mut out = String::new();
    for cap in CODE_FENCE_CAP.captures_iter(raw) {
        if let Some(m) = cap.get(1) {
            out.push_str(m.as_str().trim());
            out.push('\n');
        }
    }
    out
}

/// Script-aware word tokens: spaced scripts → words; no-space scripts → per-character.
pub fn word_tokens(norm: &str) -> Vec<String> {
    static WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\w+").unwrap());
    let mut toks = Vec::new();
    for m in WORD.find_iter(norm) {
        let w = m.as_str();
        if w.chars().any(is_nospace_char) {
            toks.extend(w.chars().map(|c| c.to_string()));
        } else {
            toks.push(w.to_string());
        }
    }
    toks
}

/// Character n-grams over the normalized string (the universal substrate). Operates on
/// Unicode scalar values, not bytes, so multibyte scripts are not split mid-character.
pub fn char_ngrams(norm: &str, n: usize) -> Vec<String> {
    let chars: Vec<char> = norm.chars().collect();
    if chars.len() < n {
        return if chars.is_empty() {
            Vec::new()
        } else {
            vec![chars.iter().collect()]
        };
    }
    (0..=chars.len() - n)
        .map(|i| chars[i..i + n].iter().collect())
        .collect()
}

/// Recommended char-gram size for a normalized string: 3 if it is predominantly a
/// no-space script (CJK/Thai), else 5 (the measured Latin/CJK optima from the survey).
pub fn gram_size_for(norm: &str) -> usize {
    let letters = norm.chars().filter(|c| c.is_alphabetic()).count();
    if letters == 0 {
        return 5;
    }
    let nospace = norm.chars().filter(|&c| is_nospace_char(c)).count();
    if nospace * 100 / letters.max(1) > 30 {
        3
    } else {
        5
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_markdown_and_lowercases() {
        let s = "# Hello World\n\nSome **bold** and `code` and [a link](http://x).";
        let n = normalize_text(s, true);
        assert!(n.contains("hello world"));
        assert!(n.contains("bold")); // emphasis markers gone, word kept
        assert!(n.contains("a link")); // link text kept, url dropped
        assert!(!n.contains("http")); // url dropped
        assert!(!n.contains('#') && !n.contains('*') && !n.contains('`'));
    }

    #[test]
    fn formatting_only_change_is_invariant() {
        let a = normalize_text("- item one\n- item two", true);
        let b = normalize_text("* item one\n* item two", true); // bullet marker swap
        assert_eq!(a, b);
        let c = normalize_text("**x** and _y_", true);
        let d = normalize_text("__x__ and *y*", true); // emphasis style swap
        assert_eq!(c, d);
    }

    #[test]
    fn fullwidth_folds_to_halfwidth() {
        // fullwidth "ABC!" → "abc!"
        let n = normalize_text("ＡＢＣ！", true);
        assert_eq!(n, "abc!");
    }

    #[test]
    fn cjk_tokenizes_per_character() {
        let toks = word_tokens("안녕하세요");
        assert_eq!(toks.len(), 5); // each Hangul syllable a token
        assert!(gram_size_for("안녕하세요 세계") == 3);
        assert!(gram_size_for("hello world this is latin prose") == 5);
    }

    #[test]
    fn char_ngrams_are_unicode_safe() {
        let g = char_ngrams("héllo", 3);
        assert_eq!(g[0], "hél"); // 3 scalar values, not bytes
        assert_eq!(g.len(), 3);
    }

    #[test]
    fn extract_code_keeps_code_separate() {
        let s = "intro\n```rust\nlet x = 1;\n```\noutro";
        let code = extract_code(s);
        assert!(code.contains("let x = 1;"));
        let prose = normalize_text(s, true);
        assert!(prose.contains("intro") && prose.contains("outro"));
        assert!(!prose.contains("let x")); // code dropped from prose
    }

    #[test]
    fn strips_table_scaffolding() {
        let n = normalize_text("| Method | GET |\n|---|---|\n| Path | /x |", true);
        assert!(!n.contains('|'), "pipes removed: {n}");
        assert!(!n.contains("---"), "separator removed: {n}");
        assert!(n.contains("method") && n.contains("get") && n.contains("path"));
        // Two tables with identical scaffolding but different content do NOT collapse to equal.
        let a = normalize_text("| k | v |\n|---|---|\n| Method | GET |", true);
        let b = normalize_text("| k | v |\n|---|---|\n| Method | POST |", true);
        assert_ne!(a, b);
    }
}
