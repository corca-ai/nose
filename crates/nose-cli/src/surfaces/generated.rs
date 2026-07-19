use rayon::prelude::*;
use rustc_hash::FxHashSet;

use super::generated_paths::GeneratedPathAssertions;
use crate::path_utils::relativize;

#[derive(Default)]
pub(super) struct GeneratedSourceIndexes {
    pub(super) sources: FxHashSet<String>,
    pub(super) additional_surface_sources: FxHashSet<String>,
    pub(super) caller_surface_sources: FxHashSet<String>,
}

#[derive(Clone, Copy)]
enum GeneratedSourceKind {
    Established,
    AdditionalSurface,
}

pub(super) fn generated_source_indexes(
    families: &[nose_detect::RefactorFamily],
    caller_generated_paths: &GeneratedPathAssertions,
) -> GeneratedSourceIndexes {
    let cwd = std::env::current_dir().ok();
    let mut generated = GeneratedSourceIndexes::default();
    let files = families
        .iter()
        .flat_map(|family| {
            family
                .locations
                .iter()
                .map(|location| location.file.as_str())
        })
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let generated_files = files
        .into_par_iter()
        .map(|path| {
            let kind = generated_source_kind(&path);
            let caller = caller_generated_paths.matches(&path);
            (path, kind, caller)
        })
        .collect::<Vec<_>>();
    for (path, kind, caller) in generated_files {
        if let Some(kind) = kind {
            let index = match kind {
                GeneratedSourceKind::Established => &mut generated.sources,
                GeneratedSourceKind::AdditionalSurface => &mut generated.additional_surface_sources,
            };
            insert_path_aliases(index, &path, cwd.as_deref());
        }
        if caller {
            insert_path_aliases(&mut generated.caller_surface_sources, &path, cwd.as_deref());
        }
    }
    generated
}

fn insert_path_aliases(index: &mut FxHashSet<String>, path: &str, cwd: Option<&std::path::Path>) {
    index.insert(path.to_string());
    if let Some(cwd) = cwd {
        index.insert(relativize(path, cwd));
    }
}

fn generated_source_kind(file: &str) -> Option<GeneratedSourceKind> {
    if file.ends_with(".css") {
        let text = std::fs::read_to_string(file).ok()?;
        return (text.lines().take(8).any(is_generated_header_line)
            || looks_compiled_css(file, &text))
        .then_some(GeneratedSourceKind::Established);
    }
    let head = bounded_source_head(file)?;
    if source_head_has_generated_header(&head) {
        Some(GeneratedSourceKind::Established)
    } else if head_has_declared_generator_provenance(&head)
        || head_has_jazzy_generated_provenance(file, &head)
    {
        Some(GeneratedSourceKind::AdditionalSurface)
    } else {
        None
    }
}

const GENERATED_HEADER_READ_BYTES: u64 = 64 * 1024;

fn bounded_source_head(file: &str) -> Option<Vec<u8>> {
    let Ok(mut file) = std::fs::File::open(file) else {
        return None;
    };
    let mut head = Vec::new();
    let mut limited = std::io::Read::take(&mut file, GENERATED_HEADER_READ_BYTES);
    std::io::Read::read_to_end(&mut limited, &mut head)
        .ok()
        .map(|_| head)
}

fn source_head_has_generated_header(head: &[u8]) -> bool {
    std::str::from_utf8(head)
        .ok()
        .is_some_and(|text| text.lines().take(8).any(is_generated_header_line))
}

/// A complete HTML document can declare the program that produced it with the standard
/// `<meta name="generator" content="…">` element. This is producer-independent, unlike a
/// stylesheet or directory name, and the non-empty content is positive provenance rather
/// than a guess from document shape.
///
/// Only a real element in an explicit document head qualifies. Comments and raw-text or
/// template elements are skipped so documentation examples and embedded markup fail open.
/// The caller supplies the bounded source head and the all-family-members quantifier.
pub(crate) fn head_has_declared_generator_provenance(head: &[u8]) -> bool {
    let mut lower = head.to_vec();
    lower.make_ascii_lowercase();

    let mut cursor = 0;
    let mut saw_doctype = false;
    let mut saw_html = false;
    let mut in_head = false;
    while let Some(relative) = lower[cursor..].iter().position(|byte| *byte == b'<') {
        let start = cursor + relative;
        if lower[start..].starts_with(b"<!--") {
            let Some(end) = find_bytes(&lower, start + 4, b"-->") else {
                return false;
            };
            cursor = end + 3;
            continue;
        }

        let Some(end) = html_tag_end(&lower, start + 1) else {
            return false;
        };
        let tag = trim_ascii(&lower[start + 1..end]);
        if tag
            .strip_prefix(b"!doctype")
            .is_some_and(|rest| rest.first().is_some_and(u8::is_ascii_whitespace))
        {
            saw_doctype = true;
            cursor = end + 1;
            continue;
        }

        let (closing, tag) = if let Some(rest) = tag.strip_prefix(b"/") {
            (true, trim_ascii_start(rest))
        } else {
            (false, tag)
        };
        let name_end = tag
            .iter()
            .position(|byte| byte.is_ascii_whitespace() || matches!(*byte, b'/' | b'>'))
            .unwrap_or(tag.len());
        let name = &tag[..name_end];

        if closing {
            if name == b"head" {
                in_head = false;
            }
            cursor = end + 1;
            continue;
        }
        if name == b"html" && saw_doctype {
            saw_html = true;
        } else if name == b"head" && saw_doctype && saw_html {
            in_head = true;
        } else if name == b"meta" && in_head && meta_declares_nonempty_generator(&tag[name_end..]) {
            return true;
        }

        if in_head && matches!(name, b"script" | b"style" | b"template") {
            let mut closing_tag = Vec::with_capacity(name.len() + 2);
            closing_tag.extend_from_slice(b"</");
            closing_tag.extend_from_slice(name);
            let Some(raw_end) = find_bytes(&lower, end + 1, &closing_tag) else {
                return false;
            };
            cursor = raw_end;
        } else {
            cursor = end + 1;
        }
    }
    false
}

fn find_bytes(haystack: &[u8], start: usize, needle: &[u8]) -> Option<usize> {
    haystack[start..]
        .windows(needle.len())
        .position(|window| window == needle)
        .map(|offset| start + offset)
}

fn html_tag_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut quote = None;
    for (offset, byte) in bytes[start..].iter().copied().enumerate() {
        match (quote, byte) {
            (Some(open), close) if open == close => quote = None,
            (None, b'\'' | b'"') => quote = Some(byte),
            (None, b'>') => return Some(start + offset),
            _ => {}
        }
    }
    None
}

fn trim_ascii(mut value: &[u8]) -> &[u8] {
    value = trim_ascii_start(value);
    while value.last().is_some_and(u8::is_ascii_whitespace) {
        value = &value[..value.len() - 1];
    }
    value
}

fn trim_ascii_start(mut value: &[u8]) -> &[u8] {
    while value.first().is_some_and(u8::is_ascii_whitespace) {
        value = &value[1..];
    }
    value
}

fn meta_declares_nonempty_generator(mut attributes: &[u8]) -> bool {
    let mut name_is_generator = false;
    let mut content_is_nonempty = false;
    while !attributes.is_empty() {
        attributes = trim_ascii_start(attributes);
        if attributes.is_empty() || attributes[0] == b'/' {
            break;
        }
        let name_end = attributes
            .iter()
            .position(|byte| byte.is_ascii_whitespace() || matches!(*byte, b'=' | b'/'))
            .unwrap_or(attributes.len());
        if name_end == 0 {
            return false;
        }
        let attribute_name = &attributes[..name_end];
        attributes = trim_ascii_start(&attributes[name_end..]);
        if !attributes.starts_with(b"=") {
            continue;
        }
        attributes = trim_ascii_start(&attributes[1..]);
        let (value, rest) = match attributes.first().copied() {
            Some(quote @ (b'\'' | b'"')) => {
                let quoted = &attributes[1..];
                let Some(end) = quoted.iter().position(|byte| *byte == quote) else {
                    return false;
                };
                (&quoted[..end], &quoted[end + 1..])
            }
            Some(_) => {
                let end = attributes
                    .iter()
                    .position(|byte| byte.is_ascii_whitespace() || *byte == b'/')
                    .unwrap_or(attributes.len());
                (&attributes[..end], &attributes[end..])
            }
            None => return false,
        };
        if attribute_name == b"name" && trim_ascii(value) == b"generator" {
            name_is_generator = true;
        } else if attribute_name == b"content" && !trim_ascii(value).is_empty() {
            content_is_nonempty = true;
        }
        attributes = rest;
    }
    name_is_generator && content_is_nonempty
}

/// Jazzy output carries two independent provenance classes in the bounded file head: its
/// asset and an Apple/Dash symbol anchor. Requiring both classes in every HTML member keeps
/// partial generation and ordinary hand-written HTML fail-open; the caller supplies the
/// all-members quantifier.
pub(crate) fn head_has_jazzy_generated_provenance(file: &str, head: &[u8]) -> bool {
    if !std::path::Path::new(file)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("html"))
    {
        return false;
    }
    let mut lower = head.to_vec();
    lower.make_ascii_lowercase();
    let has = |token: &[u8]| lower.windows(token.len()).any(|window| window == token);
    (has(b"jazzy.css") || has(b"jazzy.js"))
        && (has(b"class=\"dashanchor\"") || has(b"//apple_ref/"))
}

fn is_generated_header_line(line: &str) -> bool {
    let line = line.trim().to_ascii_lowercase();
    line.contains("@generated")
        || line.contains("generated by")
        || line.contains("code generated")
        || line.contains("automatically generated")
        || line.contains("auto-generated")
        || line.contains("autogenerated")
        || (line.contains("generated") && line.contains("do not edit"))
}

/// A compiled/distributed stylesheet is a build artifact, not hand-edited source. Detect
/// preserved license/version banners, source maps, and minified paths. The frontend gold
/// set measured 147 generated demotions with no worthy family.
pub(crate) fn looks_compiled_css(file: &str, text: &str) -> bool {
    if !file.ends_with(".css") {
        return false;
    }
    if file
        .split('/')
        .any(|segment| matches!(segment, "scss" | "sass" | "less" | "styl"))
    {
        return false;
    }
    if file.ends_with(".min.css") || std::path::Path::new(&format!("{file}.map")).exists() {
        return true;
    }
    for line in text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .take(8)
    {
        if line.starts_with("/*!")
            || (line.starts_with("@charset") && line.contains("/*!"))
            || (line.starts_with("/*") && has_version_tag(line))
        {
            return true;
        }
    }
    text.lines()
        .rev()
        .filter(|line| !line.trim().is_empty())
        .take(3)
        .any(|line| line.contains("sourceMappingURL"))
}

/// A `vN.N`(.N) version token: a release marker of a distributed stylesheet.
pub(crate) fn has_version_tag(value: &str) -> bool {
    let bytes = value.as_bytes();
    for index in 0..bytes.len().saturating_sub(2) {
        if (bytes[index] | 0x20) == b'v' && bytes[index + 1].is_ascii_digit() {
            let mut end = index + 1;
            while end < bytes.len() && bytes[end].is_ascii_digit() {
                end += 1;
            }
            if end + 1 < bytes.len() && bytes[end] == b'.' && bytes[end + 1].is_ascii_digit() {
                return true;
            }
        }
    }
    false
}
