//! Contiguous copy-paste channel — the Type-1/2 floor.
//!
//! The structural detector works at *unit* granularity (function/class/loop/if/
//! try). That misses the bulk of what a token-based copy-paste detector (jscpd,
//! CCFinder) catches: duplicated runs that start and end mid-unit, or span unit
//! boundaries — repeated test tables, assertion blocks, switch arms, boilerplate.
//!
//! This channel finds them directly. Each file's *normalized* IL is flattened to a
//! pre-order token stream (`node_tag`, which content-hashes symbols, so it is
//! interner-independent and — being post-alpha-rename — catches identifier-renamed
//! Type-2 copies that jscpd's raw-token matching misses). A single left-to-right
//! Rabin-Karp pass over all streams finds maximal duplicated runs ≥ `MIN_TOKENS`,
//! mapped back to source line spans via per-token provenance, then clustered into
//! families like the structural channel.

use crate::cluster::UnionFind;
use crate::Loc;
use rustc_hash::FxHashMap;
use nose_il::{Il, Interner, NodeId, UnitKind};
use nose_normalize::node_tag_valued;

/// One file's normalized-IL token stream, in source (pre-order) order.
pub(crate) struct Stream {
    path: String,
    lang: nose_il::Lang,
    tags: Vec<u64>,
    start: Vec<u32>,
    end: Vec<u32>,
    /// Per-token: does this node perform a computation (a call, operator, branch,
    /// loop, assignment…)? A duplicated run with *no* operations is a flat list of
    /// names / fields / literals — a prop list, a destructuring pattern, an import or
    /// enum list, a type-member list — not extractable logic. Such runs are skipped, so
    /// e.g. a hook call site `useX({ a, b, c })` no longer "clones" the hook's own
    /// parameter destructure `const { a, b, c } = input` (both are `Seq` of `Var`).
    op: Vec<bool>,
}

/// Does this node kind perform a computation (vs. just name/structure a value)?
fn is_operation(kind: nose_il::NodeKind) -> bool {
    use nose_il::NodeKind::*;
    matches!(
        kind,
        Assign | Return | If | Loop | Break | Continue | Throw | Try | Call | BinOp | UnOp | HoF
    )
}

/// Flatten an `Il` to its full pre-order token stream (with per-token spans), like a
/// token-based copy-paste detector — boundary-agnostic, so runs may span functions.
/// Nodes inside an inline-`// nose-ignore` byte range (`il.suppressed`) are skipped,
/// so the contiguous channel honours suppression just like the structural one does
/// by dropping the unit.
pub(crate) fn stream(il: &Il, interner: &Interner) -> Stream {
    let mut s = Stream {
        path: il.meta.path.clone(),
        lang: il.meta.lang,
        tags: Vec::new(),
        start: Vec::new(),
        end: Vec::new(),
        op: Vec::new(),
    };
    let suppressed = &il.suppressed;
    let is_suppressed = |b: u32| suppressed.iter().any(|&(lo, hi)| b >= lo && b < hi);
    // Iterative pre-order DFS (files nest deeply enough to overflow a recursive walk).
    let mut stack: Vec<NodeId> = vec![il.root];
    while let Some(nid) = stack.pop() {
        let n = il.node(nid);
        if suppressed.is_empty() || !is_suppressed(n.span.start_byte) {
            s.tags.push(node_tag_valued(n.kind, n.payload, interner));
            s.start.push(n.span.start_line);
            s.end.push(n.span.end_line);
            s.op.push(is_operation(n.kind));
        }
        for &c in il.children(nid).iter().rev() {
            stack.push(c);
        }
    }
    s
}

const BASE: u64 = 0x100_0000_01b3; // FNV prime, used as the rolling-hash base

// Tunable during the jscpd-superset sweep via env; see the chosen defaults below.
fn envu(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}
/// k-gram window for the rolling hash. Consecutive equal `node_tag`s are a strong
/// signal (each tag hashes kind+payload), so a modest window keeps false seeds rare.
fn k() -> usize {
    envu("NOSE_CONTIG_K", 10)
}
/// Minimum matched run, in IL tokens (coarser than source tokens, so well below
/// jscpd's source-token default). Set to match jscpd-weak's granularity as the
/// Type-1/2 floor. Small/high-fanout runs no longer dominate the user-facing top-N:
/// the refactoring-value ranking dampens fanout (see `report::refactor_value`), so a
/// fragment repeated in hundreds of sites (boilerplate / generated / test scaffolding)
/// ranks low, while genuine few-site copy-paste surfaces.
fn min_tokens() -> usize {
    envu("NOSE_CONTIG_MIN_TOKENS", 10)
}
/// Minimum matched run, in source lines.
fn min_lines() -> u32 {
    envu("NOSE_CONTIG_MIN_LINES", 3) as u32
}

/// Rolling k-gram hashes for one stream (`tags.len() - k + 1` entries, or empty).
fn kgrams(tags: &[u64], k: usize) -> Vec<u64> {
    if tags.len() < k {
        return Vec::new();
    }
    let mut pow = 1u64;
    for _ in 0..k - 1 {
        pow = pow.wrapping_mul(BASE);
    }
    let mut out = Vec::with_capacity(tags.len() - k + 1);
    let mut h = 0u64;
    for &t in &tags[..k] {
        h = h.wrapping_mul(BASE).wrapping_add(t);
    }
    out.push(h);
    for i in k..tags.len() {
        // drop tags[i-k], add tags[i]
        h = h
            .wrapping_sub(tags[i - k].wrapping_mul(pow))
            .wrapping_mul(BASE)
            .wrapping_add(tags[i]);
        out.push(h);
    }
    out
}

/// How far two streams match forward from `(a, b)` (they share ≥ K tokens already).
fn extend(sa: &Stream, a: usize, sb: &Stream, b: usize) -> usize {
    let mut len = 0;
    while a + len < sa.tags.len() && b + len < sb.tags.len() && sa.tags[a + len] == sb.tags[b + len]
    {
        len += 1;
    }
    len
}

fn loc(s: &Stream, lo: usize, hi: usize) -> Loc {
    let start = s.start[lo..hi].iter().copied().min().unwrap_or(0);
    let end = s.end[lo..hi].iter().copied().max().unwrap_or(0);
    Loc {
        file: s.path.clone(),
        start_line: start,
        end_line: end,
        lang: s.lang.name().to_string(),
        kind: UnitKind::Block,
        name: None,
        sem: hi - lo,
    }
}

/// Find maximal duplicated runs across all streams and cluster them into groups.
/// A single forward pass keyed by k-gram hash: the first time a k-gram is seen it is
/// recorded; a later identical k-gram seeds a match against that first occurrence,
/// which is extended to its maximal length and (if large enough) emitted. After a
/// match the scan skips past it, so each duplicated region is reported once and the
/// pass stays linear even on highly repetitive code.
pub(crate) fn detect(streams: &[Stream]) -> Vec<crate::Group> {
    let (k, mint, minl) = (k(), min_tokens(), min_lines());
    // First occurrence of each k-gram, keyed by (hash, language): `(hash, lang) ->
    // (stream, pos)`. Keying on language makes the contiguous channel **same-language
    // by construction** — literal copy-paste (Type-1/2) doesn't cross languages, so a
    // cross-language "duplicated run" here is a false merge (unrelated code sharing a
    // normalized-IL token run). Cross-language equivalence is Type-4 and is recovered
    // by the value-graph channel instead. Per-language keying also stops an unrelated
    // collision in one language from masking a real same-language match in another.
    let mut seen: FxHashMap<(u64, nose_il::Lang), (usize, usize)> = FxHashMap::default();
    let grams: Vec<Vec<u64>> = streams.iter().map(|s| kgrams(&s.tags, k)).collect();

    // Emitted clone instances and the pairs linking them (for union-find clustering).
    let mut locs: Vec<Loc> = Vec::new();
    let mut pairs: Vec<(usize, usize)> = Vec::new();
    // Dedup identical (file,start,end) instances to one node id.
    let mut loc_id: FxHashMap<(String, u32, u32), usize> = FxHashMap::default();
    let mut intern_loc = |locs: &mut Vec<Loc>, l: Loc| -> usize {
        let key = (l.file.clone(), l.start_line, l.end_line);
        *loc_id.entry(key).or_insert_with(|| {
            locs.push(l);
            locs.len() - 1
        })
    };

    for (si, g) in grams.iter().enumerate() {
        let lang = streams[si].lang;
        let mut i = 0;
        while i < g.len() {
            let h = g[i];
            if let Some(&(sj, j)) = seen.get(&(h, lang)) {
                // `sj` is the same language as `si` by construction (the key includes
                // `lang`). Don't match a stream against an overlapping window of itself.
                let self_overlap = sj == si && i.abs_diff(j) < k;
                if !self_overlap {
                    let len = extend(&streams[sj], j, &streams[si], i);
                    let la = loc(&streams[sj], j, j + len);
                    let lb = loc(&streams[si], i, i + len);
                    let lines = lb.end_line.saturating_sub(lb.start_line) + 1;
                    // Require the run to contain at least one operation — a flat
                    // name/field/literal list (prop list, destructure, import/enum list)
                    // is not extractable logic, however literally it repeats.
                    let has_op = streams[si].op[i..(i + len).min(streams[si].op.len())]
                        .iter()
                        .any(|&o| o);
                    if has_op && len >= mint && lines >= minl {
                        let a = intern_loc(&mut locs, la);
                        let b = intern_loc(&mut locs, lb);
                        if a != b {
                            pairs.push((a, b));
                        }
                        // Skip past the matched run in this stream.
                        i += len.max(1);
                        continue;
                    }
                }
            } else {
                seen.insert((h, lang), (si, i));
            }
            i += 1;
        }
    }

    if locs.is_empty() {
        return Vec::new();
    }
    // Cluster instances into families (transitively: copy C of a block joins A,B).
    let mut uf = UnionFind::new(locs.len());
    for &(a, b) in &pairs {
        uf.union(a, b);
    }
    let mut groups = Vec::new();
    for members in uf.groups(locs.len()) {
        // The contiguous channel can't score similarity per-pair cheaply; these are
        // exact-token-run clones, so report them at sim 1.0.
        groups.push(crate::Group {
            score: 1.0,
            members: members.iter().map(|&m| locs[m].clone()).collect(),
        });
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk(path: &str, tags: Vec<u64>) -> Stream {
        let n = tags.len() as u32;
        Stream {
            path: path.into(),
            lang: nose_il::Lang::Python,
            // These tests exercise the run-matching mechanism, not the operation gate,
            // so every token counts as an operation.
            op: vec![true; tags.len()],
            tags,
            start: (1..=n).collect(),
            end: (1..=n).collect(),
        }
    }

    #[test]
    fn finds_a_shared_sub_unit_run() {
        // A 25-token run shared by two files, wrapped in differing tokens — the kind
        // of mid-function copy-paste the unit-level detector misses.
        let shared: Vec<u64> = (100..125).collect();
        let mut a = vec![1, 2, 3];
        a.extend(&shared);
        a.extend([4, 5]);
        let mut b = vec![9, 8];
        b.extend(&shared);
        b.extend([7, 6, 5, 4]);
        let groups = detect(&[mk("a.py", a), mk("b.py", b)]);
        assert_eq!(groups.len(), 1, "the shared run is one family");
        assert_eq!(groups[0].members.len(), 2, "one site per file");
    }

    #[test]
    fn ignores_runs_below_min_tokens() {
        // An 8-token shared run is below MIN_TOKENS (10) → not a clone.
        let shared: Vec<u64> = (200..208).collect();
        let mut a = vec![1, 2];
        a.extend(&shared);
        let mut b = vec![9, 8, 7];
        b.extend(&shared);
        assert!(detect(&[mk("a.py", a), mk("b.py", b)]).is_empty());
    }

    /// A run with no operation tokens (a flat name/field/literal list — a prop list, a
    /// destructuring pattern, an import/enum list) is not extractable logic and is
    /// dropped, even when it repeats verbatim and is long enough.
    #[test]
    fn ignores_operationless_runs() {
        let shared: Vec<u64> = (100..125).collect(); // 25 tokens, well over MIN_TOKENS
        let stream = |path: &str| {
            let n = shared.len() as u32;
            Stream {
                path: path.into(),
                lang: nose_il::Lang::Python,
                op: vec![false; shared.len()], // no operations anywhere in the run
                tags: shared.clone(),
                start: (1..=n).collect(),
                end: (1..=n).collect(),
            }
        };
        assert!(
            detect(&[stream("a.py"), stream("b.py")]).is_empty(),
            "an operation-free run is not a refactor candidate"
        );
        // The same run with one operation token IS reported.
        let with_op = |path: &str| {
            let mut s = stream(path);
            s.op[5] = true;
            s
        };
        assert_eq!(
            detect(&[with_op("a.py"), with_op("b.py")]).len(),
            1,
            "a run containing an operation is a clone"
        );
    }
}
