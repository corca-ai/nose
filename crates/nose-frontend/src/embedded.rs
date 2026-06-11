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
/// JS/TS in place, and tag the IL with the script language. The path still preserves
/// the container file provenance for reporting.
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
///
/// A small stateful scanner rather than naive substring search, because five real
/// shapes (coevo §CE / #280) defeat `find_ci`: a `<script>` *inside an HTML
/// comment* must be skipped; the opening tag's `>` must not be taken from inside a
/// quoted attribute value (Vue 3.3 `generic="T extends Record<string, number>"`);
/// the closing `</script>` must not be taken from inside a JS string or comment in
/// the script body; and an unclosed `<script>` (valid HTML — the browser runs it to
/// EOF) must extract to EOF, not vanish; and trailing markup must not extend the block span (see blank_except).
fn extract_scripts(src: &[u8]) -> (Vec<(usize, usize)>, bool) {
    let mut ranges = Vec::new();
    let mut is_ts = false;
    let mut pos = 0;
    while pos < src.len() {
        // Skip HTML comments so a commented-out `<script>` is never extracted.
        if src[pos..].starts_with(b"<!--") {
            pos = find_ci(src, b"-->", pos + 4)
                .map(|c| c + 3)
                .unwrap_or(src.len());
            continue;
        }
        let Some(open) = find_script_open(src, pos) else {
            break;
        };
        // End of the opening tag: the first `>` NOT inside a quoted attribute.
        let Some(tag_end) = tag_end(src, open) else {
            break;
        };
        if open_tag_is_ts(&src[open..tag_end]) {
            is_ts = true;
        }
        let content_start = tag_end + 1;
        // Closing `</script>`, ignoring matches inside JS strings/comments; an
        // unclosed block runs to EOF.
        let close = script_close(src, content_start).unwrap_or(src.len());
        if close > content_start {
            ranges.push((content_start, close));
        }
        pos = (close + b"</script".len())
            .min(src.len())
            .max(content_start + 1);
    }
    (ranges, is_ts)
}

/// The next `<script` that starts a tag (followed by whitespace, `>`, or EOF),
/// skipping HTML comments encountered along the way.
fn find_script_open(src: &[u8], mut pos: usize) -> Option<usize> {
    loop {
        let open = find_ci(src, b"<script", pos)?;
        // Skip if it sits inside a comment that began before `pos`.
        if let Some(c) = find_ci(src, b"<!--", pos) {
            if c < open {
                let end = find_ci(src, b"-->", c + 4)
                    .map(|e| e + 3)
                    .unwrap_or(src.len());
                if open < end {
                    pos = end;
                    continue;
                }
            }
        }
        // `<scripts>` / `<scripting` are not a script tag.
        match src.get(open + b"<script".len()) {
            Some(b) if b.is_ascii_whitespace() || *b == b'>' => return Some(open),
            None => return Some(open),
            _ => pos = open + b"<script".len(),
        }
    }
}

/// The index of the `>` that closes the opening tag at `open`, skipping any `>`
/// inside a quoted attribute value.
fn tag_end(src: &[u8], open: usize) -> Option<usize> {
    let mut i = open + b"<script".len();
    let mut quote: Option<u8> = None;
    while i < src.len() {
        let b = src[i];
        match quote {
            Some(q) if b == q => quote = None,
            Some(_) => {}
            None if b == b'"' || b == b'\'' => quote = Some(b),
            None if b == b'>' => return Some(i),
            None => {}
        }
        i += 1;
    }
    None
}

/// The index of the `</script` that closes the block whose content starts at
/// `content_start`, ignoring `</script` that falls inside a JS string literal,
/// template literal, or `//` / `/* */` comment.
fn script_close(src: &[u8], content_start: usize) -> Option<usize> {
    let mut i = content_start;
    let mut quote: Option<u8> = None;
    while i < src.len() {
        if let Some(q) = quote {
            if src[i] == b'\\' {
                i += 2;
                continue;
            }
            if src[i] == q {
                quote = None;
            }
            i += 1;
            continue;
        }
        if src[i..].starts_with(b"//") {
            i = src[i..]
                .iter()
                .position(|&b| b == b'\n')
                .map_or(src.len(), |n| i + n);
            continue;
        }
        if src[i..].starts_with(b"/*") {
            i = find_ci(src, b"*/", i + 2)
                .map(|e| e + 2)
                .unwrap_or(src.len());
            continue;
        }
        if matches!(src[i], b'"' | b'\'' | b'`') {
            quote = Some(src[i]);
            i += 1;
            continue;
        }
        if src[i..].starts_with(b"</script") {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Does a `<script …>` opening tag select TypeScript?
fn open_tag_is_ts(tag: &[u8]) -> bool {
    contains_ci(tag, b"lang=\"ts\"")
        || contains_ci(tag, b"lang='ts'")
        || contains_ci(tag, b"lang=ts")
        || contains_ci(tag, b"lang=\"tsx\"")
        || contains_ci(tag, b"typescript")
}

/// Replace every byte outside `keep` with a space, preserving `\n`/`\r` so line
/// numbers stay aligned with the original file, and TRUNCATE the buffer at the
/// last kept byte. Truncation matters: trailing markup left as blanked lines made
/// the top-level (whole-script-block) unit's span bleed past `</script>` onto the
/// closing tag and following markup (coevo §CE / #280 defect 4). Markup BETWEEN
/// blocks stays blanked (offsets preserved); only the tail after the final block
/// is dropped.
fn blank_except(src: &[u8], keep: &[(usize, usize)]) -> Vec<u8> {
    let end = keep.iter().map(|&(_, e)| e).max().unwrap_or(0);
    let mut out: Vec<u8> = src[..end]
        .iter()
        .map(|&b| if b == b'\n' || b == b'\r' { b } else { b' ' })
        .collect();
    for &(s, e) in keep {
        if e <= end {
            out[s..e].copy_from_slice(&src[s..e]);
        }
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

fn contains_ci(hay: &[u8], needle: &[u8]) -> bool {
    find_ci(hay, needle, 0).is_some()
}

#[cfg(test)]
mod tests {
    use super::extract_scripts;

    /// Content of every extracted `<script>` block, as text.
    fn scripts(src: &str) -> Vec<String> {
        let (ranges, _) = extract_scripts(src.as_bytes());
        ranges
            .into_iter()
            .map(|(s, e)| String::from_utf8(src.as_bytes()[s..e].to_vec()).unwrap())
            .collect()
    }

    #[test]
    fn closing_script_inside_a_js_string_does_not_truncate() {
        // #280 defect 1.
        let s = scripts("<script>\nconst x = \"</script>\";\nfn();\n</script>");
        assert_eq!(s.len(), 1);
        assert!(
            s[0].contains("fn();"),
            "block ran past the string literal: {s:?}"
        );
    }

    #[test]
    fn script_inside_an_html_comment_is_skipped() {
        // #280 defect 2.
        let s = scripts("<!--\n<script>dead();</script>\n-->\n<script>live();</script>");
        assert_eq!(s.len(), 1, "only the live block: {s:?}");
        assert!(s[0].contains("live()") && !s[0].contains("dead()"));
    }

    #[test]
    fn tag_end_skips_a_greater_than_inside_an_attribute() {
        // #280 defect 3 (Vue 3.3 `generic="T extends Record<string, number>"`).
        let s = scripts(
            "<script setup generic=\"T extends Record<string, number>\">\nbody();\n</script>",
        );
        assert_eq!(s.len(), 1);
        assert!(
            s[0].trim_start().starts_with("body()"),
            "content started mid-tag: {s:?}"
        );
    }

    #[test]
    fn unclosed_script_extracts_to_eof() {
        // #280 defect 5 (valid HTML — the browser runs it to EOF).
        let s = scripts("<body>\n<script>\nfn();\nmore();\n");
        assert_eq!(s.len(), 1);
        assert!(s[0].contains("fn();") && s[0].contains("more();"), "{s:?}");
    }

    #[test]
    fn plain_blocks_and_multi_block_still_extract() {
        // Regression guard: the common shapes are unchanged.
        assert_eq!(scripts("<script>a();</script>").len(), 1);
        let two = scripts("<script setup>a();</script>\n<script>b();</script>");
        assert_eq!(two.len(), 2);
        // Import-only / plain content unaffected; markup between blocks ignored.
        assert!(scripts("<template><div/></template>\n<script>x();</script>")[0].contains("x()"));
    }
}
