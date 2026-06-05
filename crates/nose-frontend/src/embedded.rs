//! Frontends for file types that *embed* JavaScript/TypeScript in `<script>`
//! blocks: Vue single-file components, Svelte components, and HTML.
//!
//! Rather than model the template/markup grammar, we analyze the script logic —
//! which is what clone detection cares about. The trick that keeps provenance exact:
//! every byte outside a `<script>` block is blanked to a space (newlines kept), so
//! the script content stays at its *original* byte/line offsets and the whole
//! buffer is valid JS/TS (the markup becomes whitespace). Reported spans therefore
//! point at the right lines in the original `.vue`/`.svelte`/`.html` file.

use nose_il::{FileId, Il, Interner, Lang};

/// Lower an embedded-script file: extract its `<script>` blocks, parse them as
/// JS/TS in place, and tag the result with the container language for reporting.
pub(crate) fn lower(
    file: FileId,
    path: &str,
    src: &[u8],
    container: Lang,
    interner: &Interner,
) -> anyhow::Result<Il> {
    let (scripts, is_ts) = extract_scripts(src);
    let blanked = blank_except(src, &scripts);
    let script_lang = if is_ts {
        Lang::TypeScript
    } else {
        Lang::JavaScript
    };
    // Tag the IL with the *script* language it's actually analyzed as (TS/JS), not the
    // container (`vue`/`svelte`/`html`). The file path already shows it's a component;
    // tagging by container made a `<script lang="ts">` block and a plain `.ts` file
    // look like a *cross-language* clone ("2 languages: svelte, typescript") when they
    // are both TypeScript — which mislabeled honest same-language type/code duplication
    // and sent it down the cross-language (no line-diff) path. They're still a
    // cross-*container* family (different files); the language count is just honest now.
    let _ = container;
    crate::js_ts::lower(file, path, &blanked, script_lang, interner)
}

/// Byte ranges of every `<script>…</script>` block's *content*, plus whether any
/// block declares TypeScript (`lang="ts"`/`tsx` or a TypeScript `type`).
fn extract_scripts(src: &[u8]) -> (Vec<(usize, usize)>, bool) {
    let mut ranges = Vec::new();
    let mut is_ts = false;
    let mut pos = 0;
    while let Some(open) = find_ci(src, b"<script", pos) {
        // End of the opening tag (best-effort: first `>` after `<script`).
        let Some(rel) = src[open..].iter().position(|&b| b == b'>') else {
            break;
        };
        let tag_end = open + rel;
        if open_tag_is_ts(&src[open..tag_end]) {
            is_ts = true;
        }
        let content_start = tag_end + 1;
        let Some(close) = find_ci(src, b"</script", content_start) else {
            break;
        };
        if close > content_start {
            ranges.push((content_start, close));
        }
        pos = close + b"</script".len();
    }
    (ranges, is_ts)
}

/// Does a `<script …>` opening tag select TypeScript?
fn open_tag_is_ts(tag: &[u8]) -> bool {
    let lower: String = tag
        .iter()
        .map(|&b| b.to_ascii_lowercase() as char)
        .collect();
    lower.contains("lang=\"ts\"")
        || lower.contains("lang='ts'")
        || lower.contains("lang=ts")
        || lower.contains("lang=\"tsx\"")
        || lower.contains("typescript")
}

/// Replace every byte outside `keep` with a space, preserving `\n`/`\r` so line
/// numbers stay aligned with the original file.
fn blank_except(src: &[u8], keep: &[(usize, usize)]) -> Vec<u8> {
    let mut out: Vec<u8> = src
        .iter()
        .map(|&b| if b == b'\n' || b == b'\r' { b } else { b' ' })
        .collect();
    for &(s, e) in keep {
        out[s..e].copy_from_slice(&src[s..e]);
    }
    out
}

/// Case-insensitive ASCII substring search starting at `from`.
fn find_ci(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if needle.is_empty() || hay.len() < needle.len() || from > hay.len() - needle.len() {
        return None;
    }
    (from..=hay.len() - needle.len())
        .find(|&i| hay[i..i + needle.len()].eq_ignore_ascii_case(needle))
}
