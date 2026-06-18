//! Stage 3 — witness / span extraction.
//!
//! On a confirmed pair only (never corpus-wide), recover the exact duplicated span via a
//! line-level local alignment (Smith-Waterman) over normalized lines, reported in the units'
//! original 1-based line coordinates. This is what a refactoring tool must show: *where* the
//! duplication is, not just a score. Line granularity is the Markdown-natural unit and doubles as
//! the Myers-diff-style render.

use crate::fingerprint::fnv1a;
use crate::norm;
use crate::unit::Unit;

/// A duplicated span, in original 1-based inclusive line numbers of each unit's file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Span {
    pub a_start: u32,
    pub a_end: u32,
    pub b_start: u32,
    pub b_end: u32,
    /// Number of byte-identical (after normalization) lines in the aligned block.
    pub matched_lines: u32,
}

const MAX_LINES: usize = 400; // bound the O(n*m) alignment; runs on confirmed pairs only

/// Non-empty normalized lines of a unit with their original 1-based line numbers.
fn line_seq(u: &Unit) -> Vec<(u32, u64)> {
    let mut out = Vec::new();
    for (idx, line) in u.raw.lines().enumerate() {
        let n = norm::normalize_text(line, true);
        if n.is_empty() {
            continue;
        }
        out.push((u.start_line + idx as u32, fnv1a(&n)));
        if out.len() >= MAX_LINES {
            break;
        }
    }
    out
}

/// Local-alignment witness between two units. Returns the matched span in each unit, or `None`
/// if there is no shared line block.
pub fn witness(a: &Unit, b: &Unit) -> Option<Span> {
    let sa = line_seq(a);
    let sb = line_seq(b);
    if sa.is_empty() || sb.is_empty() {
        return None;
    }
    let (m, n) = (sa.len(), sb.len());
    // dp + traceback direction: 0=stop, 1=diag, 2=up, 3=left
    let mut dp = vec![0i32; (m + 1) * (n + 1)];
    let mut dir = vec![0u8; (m + 1) * (n + 1)];
    let w = n + 1;
    let (mut best, mut bi, mut bj) = (0i32, 0usize, 0usize);
    for i in 1..=m {
        for j in 1..=n {
            let eq = sa[i - 1].1 == sb[j - 1].1;
            let diag = dp[(i - 1) * w + (j - 1)] + if eq { 2 } else { -1 };
            let up = dp[(i - 1) * w + j] - 1;
            let left = dp[i * w + (j - 1)] - 1;
            let mut v = 0;
            let mut d = 0u8;
            if diag > v {
                v = diag;
                d = 1;
            }
            if up > v {
                v = up;
                d = 2;
            }
            if left > v {
                v = left;
                d = 3;
            }
            dp[i * w + j] = v;
            dir[i * w + j] = d;
            if v > best {
                best = v;
                bi = i;
                bj = j;
            }
        }
    }
    if best <= 0 {
        return None;
    }
    // Traceback to recover the aligned block extent + count of equal-line matches.
    let (mut i, mut j) = (bi, bj);
    let (mut a_lo, mut a_hi, mut b_lo, mut b_hi) = (u32::MAX, 0u32, u32::MAX, 0u32);
    let mut matched = 0u32;
    while i > 0 && j > 0 && dp[i * w + j] > 0 {
        match dir[i * w + j] {
            1 => {
                let (la, lb) = (sa[i - 1].0, sb[j - 1].0);
                if sa[i - 1].1 == sb[j - 1].1 {
                    matched += 1;
                    a_lo = a_lo.min(la);
                    a_hi = a_hi.max(la);
                    b_lo = b_lo.min(lb);
                    b_hi = b_hi.max(lb);
                }
                i -= 1;
                j -= 1;
            }
            2 => i -= 1,
            3 => j -= 1,
            _ => break,
        }
    }
    if matched == 0 {
        return None;
    }
    Some(Span {
        a_start: a_lo,
        a_end: a_hi,
        b_start: b_lo,
        b_end: b_hi,
        matched_lines: matched,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unit::split_units;

    #[test]
    fn witnesses_shared_line_block() {
        let a = "# A\nintro alpha\nshared line one here\nshared line two here\nshared line three\ntail a";
        let b = "# B\ndifferent preamble\nanother\nshared line one here\nshared line two here\nshared line three\nepilogue b";
        let ua = &split_units("a.md", a)[0];
        let ub = &split_units("b.md", b)[0];
        let s = witness(ua, ub).expect("should witness a shared block");
        assert_eq!(s.matched_lines, 3);
        // span covers the three shared lines in each file's own coordinates
        assert!(s.a_start <= s.a_end && s.b_start <= s.b_end);
        assert_eq!(s.a_end - s.a_start, 2); // 3 consecutive lines
    }

    #[test]
    fn no_witness_for_unrelated() {
        let a = &split_units("a.md", "the quick brown fox")[0];
        let b = &split_units("b.md", "database indexes and tables")[0];
        assert!(witness(a, b).is_none());
    }

    #[test]
    fn deterministic() {
        let a = &split_units("a.md", "shared one\nshared two\nshared three")[0];
        let b = &split_units("b.md", "shared one\nshared two\nshared three")[0];
        assert_eq!(witness(a, b), witness(a, b));
    }
}
