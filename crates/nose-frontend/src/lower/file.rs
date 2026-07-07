use super::library_api_post_lower::record_post_lower_library_api_evidence;
use super::post_lower_evidence::record_post_lower_bound_order_guard_evidence;
use super::*;

/// The shared parse → lower-root → finish pipeline every frontend's `lower` entry
/// point repeats. The frontend supplies only what is language-specific: the grammar
/// (`key` + `lang_fn`), its [`Lang`] tag, and `lower_root`, which turns the parsed
/// CST root into the file's `Module` node.
// The arguments are irreducible: the four file-context values (which mirror every
// frontend's `lower` signature) plus the three grammar/lang specifics and the root
// lowering. Bundling them into a struct used by this one function would add
// indirection without clarifying anything.
#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_file(
    file: FileId,
    path: &str,
    src: &[u8],
    interner: &Interner,
    key: u16,
    lang_fn: impl FnOnce() -> tree_sitter::Language,
    lang: Lang,
    lower_root: impl FnOnce(&mut Lowering, TsNode) -> NodeId,
) -> anyhow::Result<Il> {
    lower_file_with_setup(
        file,
        path,
        src,
        interner,
        key,
        lang_fn,
        lang,
        |_| {},
        lower_root,
    )
}

/// Like [`lower_file`], but lets a frontend seed file-local proof facts after
/// parsing and before walking the root. This keeps language-specific facts in the
/// frontend while preserving the shared IL construction path.
#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_file_with_setup(
    file: FileId,
    path: &str,
    src: &[u8],
    interner: &Interner,
    key: u16,
    lang_fn: impl FnOnce() -> tree_sitter::Language,
    lang: Lang,
    setup: impl FnOnce(&mut Lowering),
    lower_root: impl FnOnce(&mut Lowering, TsNode) -> NodeId,
) -> anyhow::Result<Il> {
    let tree = parse(key, lang_fn, src)?;
    let mut lo = Lowering::new(file, src, lang, interner);
    setup(&mut lo);
    let module = lower_root(&mut lo, tree.root_node());
    let meta = FileMeta {
        path: path.to_string(),
        lang,
    };
    let units = std::mem::take(&mut lo.units);
    let evidence = std::mem::take(&mut lo.evidence);
    let mut il = lo.b.finish(module, meta, units, Vec::new());
    il.evidence = evidence;
    record_post_lower_bound_order_guard_evidence(&mut il, interner);
    record_post_lower_library_api_evidence(&mut il, interner);
    drop_suppressed_units(&mut il, src);
    Ok(il)
}

/// Inline suppression: drop any unit whose source carries a `nose-ignore` marker
/// on its first line or the line just above it (in a comment, any language). Lets a
/// maintainer mark a clone as intentionally-kept so it never shows up as a candidate.
fn drop_suppressed_units(il: &mut Il, src: &[u8]) {
    if il.units.is_empty() || !contains_marker(src) {
        return; // fast path: nothing to suppress
    }
    let keep: Vec<bool> = il
        .units
        .iter()
        .map(|u| !unit_suppressed(src, il.node(u.root).span.start_byte as usize))
        .collect();
    // Record suppressed units' byte spans so the contiguous channel excludes them too.
    for (u, &kept) in il.units.iter().zip(&keep) {
        if !kept {
            let sp = il.node(u.root).span;
            il.suppressed.push((sp.start_byte, sp.end_byte));
        }
    }
    let mut it = keep.iter();
    il.units.retain(|_| *it.next().unwrap());
}

const SUPPRESS_MARKER: &str = "nose-ignore";

fn contains_marker(src: &[u8]) -> bool {
    // cheap whole-file prescreen so the per-unit work only runs when relevant
    let marker = SUPPRESS_MARKER.as_bytes();
    if src.len() < marker.len() {
        return false;
    }

    for i in memchr::memchr2_iter(b'n', b'N', src) {
        if i + marker.len() <= src.len() && src[i..i + marker.len()].eq_ignore_ascii_case(marker) {
            return true;
        }
    }
    false
}

/// Is the unit starting at `start_byte` suppressed — i.e. does its first line, the
/// line immediately above, or the line immediately above a contiguous decorator /
/// annotation block contain the marker (typically in a trailing/preceding comment)?
fn unit_suppressed(src: &[u8], start_byte: usize) -> bool {
    let start = start_byte.min(src.len());
    let cur_begin = src[..start]
        .iter()
        .rposition(|&b| b == b'\n')
        .map_or(0, |p| p + 1);
    let prev_begin = if cur_begin == 0 {
        0
    } else {
        src[..cur_begin - 1]
            .iter()
            .rposition(|&b| b == b'\n')
            .map_or(0, |p| p + 1)
    };
    let cur_end = src[start..]
        .iter()
        .position(|&b| b == b'\n')
        .map_or(src.len(), |p| start + p);
    let window_begin = suppression_window_begin(src, prev_begin);
    contains_marker(&src[window_begin..cur_end])
}

fn suppression_window_begin(src: &[u8], prev_begin: usize) -> usize {
    let mut window_begin = prev_begin;
    let mut line_begin = prev_begin;
    while line_starts_annotation(src, line_begin) {
        let above_begin = previous_line_begin(src, line_begin);
        window_begin = above_begin;
        if above_begin == line_begin {
            break;
        }
        line_begin = above_begin;
    }
    window_begin
}

fn previous_line_begin(src: &[u8], line_begin: usize) -> usize {
    if line_begin == 0 {
        return 0;
    }
    src[..line_begin - 1]
        .iter()
        .rposition(|&b| b == b'\n')
        .map_or(0, |p| p + 1)
}

fn line_starts_annotation(src: &[u8], line_begin: usize) -> bool {
    let line_end = src[line_begin..]
        .iter()
        .position(|&b| b == b'\n')
        .map_or(src.len(), |p| line_begin + p);
    src[line_begin..line_end]
        .iter()
        .copied()
        .find(|b| !matches!(b, b' ' | b'\t'))
        == Some(b'@')
}

#[cfg(test)]
mod tests {
    use super::{contains_marker, unit_suppressed};

    #[test]
    fn contains_marker_matches_ascii_case_insensitively() {
        assert!(contains_marker(b"// nose-ignore\nfn clone() {}"));
        assert!(contains_marker(b"# NOSE-IGNORE\ndef clone(): pass"));
        assert!(contains_marker(b"prefix NoSe-IgNoRe suffix"));
    }

    #[test]
    fn contains_marker_requires_exact_marker() {
        assert!(!contains_marker(b""));
        assert!(!contains_marker(b"nose-ignor"));
        assert!(!contains_marker(b"noise-ignore"));
        assert!(!contains_marker(b"nose_ignore"));
    }

    #[test]
    fn unit_suppressed_matches_marker_case_insensitively() {
        let src = b"# NOSE-IGNORE\ndef clone():\n    return 1\n";
        let start = src
            .windows(b"def clone".len())
            .position(|window| window == b"def clone")
            .expect("function start");

        assert!(unit_suppressed(src, start));
    }

    #[test]
    fn unit_suppressed_only_looks_at_current_or_previous_line() {
        let src = b"# nose-ignore\n\n\ndef clone():\n    return 1\n";
        let start = src
            .windows(b"def clone".len())
            .position(|window| window == b"def clone")
            .expect("function start");

        assert!(!unit_suppressed(src, start));
    }

    #[test]
    fn unit_suppressed_checks_line_above_contiguous_decorators() {
        let src = b"# nose-ignore\n@memoize\n@trace\ndef clone():\n    return 1\n";
        let start = src
            .windows(b"def clone".len())
            .position(|window| window == b"def clone")
            .expect("function start");

        assert!(unit_suppressed(src, start));
    }

    #[test]
    fn unit_suppressed_does_not_cross_blank_line_before_decorator() {
        let src = b"# nose-ignore\n\n@memoize\ndef clone():\n    return 1\n";
        let start = src
            .windows(b"def clone".len())
            .position(|window| window == b"def clone")
            .expect("function start");

        assert!(!unit_suppressed(src, start));
    }
}
