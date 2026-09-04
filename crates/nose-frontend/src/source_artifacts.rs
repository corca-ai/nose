use nose_il::Lang;
use std::path::Path;

const SNIFF_BYTES: usize = 256 * 1024;

pub(crate) fn skip_reason(path: &Path, lang: Lang, src: &[u8]) -> Option<&'static str> {
    if looks_like_binary_source_artifact(src) {
        return Some("binary-source-artifact");
    }
    if looks_like_ansi_highlight_output(src) {
        return Some("ansi-highlight-output");
    }
    if lang == Lang::C && is_header_path(path) && looks_like_cpp_header(src) {
        return Some("unsupported-cpp-header");
    }
    None
}

fn sniff_bytes(src: &[u8]) -> &[u8] {
    &src[..src.len().min(SNIFF_BYTES)]
}

fn looks_like_binary_source_artifact(src: &[u8]) -> bool {
    let sample = sniff_bytes(src);
    sample.contains(&0)
        || sample.starts_with(b"\x89PNG\r\n\x1a\n")
        || sample.starts_with(b"\xff\xd8\xff")
        || sample.starts_with(b"GIF87a")
        || sample.starts_with(b"GIF89a")
        || sample.starts_with(b"%PDF-")
        || sample.starts_with(b"PK\x03\x04")
}

fn looks_like_ansi_highlight_output(src: &[u8]) -> bool {
    // Plain source can mention "\x1b[" textually; syntax-highlight output contains
    // repeated raw CSI escapes throughout the file.
    let sample = sniff_bytes(src);
    memchr::memchr_iter(b'\x1b', sample)
        .filter(|&offset| sample.get(offset + 1) == Some(&b'['))
        .take(3)
        .count()
        >= 3
}

fn is_header_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("h"))
}

fn looks_like_cpp_header(src: &[u8]) -> bool {
    let code = c_like_code_without_comments_and_literals(sniff_bytes(src));
    // The C grammar can recover C++ declarations as macro-like constructs
    // without ERROR nodes. Require declaration syntax, not a keyword alone.
    if has_cpp_declaration(&code, "namespace") || has_cpp_declaration(&code, "class") {
        return true;
    }
    // These words are legal C identifiers. Prefer a clean C parse over lexical
    // hints; only error-recovered headers need the unsupported-language filter.
    if crate::lower::parse(
        crate::lower::grammar::C,
        || tree_sitter_c::LANGUAGE.into(),
        src,
    )
    .is_ok_and(|tree| !tree.root_node().has_error())
    {
        return false;
    }
    contains_word(&code, "namespace")
        || code.contains("template <")
        || code.contains("template<")
        || code.contains("using namespace")
        || code.contains("std::")
        || contains_word_followed_by_ident(&code, "class")
        || ["public:", "private:", "protected:"]
            .iter()
            .any(|marker| code.contains(marker))
}

fn has_cpp_declaration(code: &str, keyword: &str) -> bool {
    code.match_indices(keyword).any(|(offset, _)| {
        let bytes = code.as_bytes();
        if offset > 0 && is_ident_byte(bytes[offset - 1]) {
            return false;
        }
        let before = code[..offset].trim_end();
        let preceding_word = before
            .rsplit(|c: char| !c.is_ascii_alphanumeric() && c != '_')
            .next()
            .unwrap_or("");
        if matches!(preceding_word, "struct" | "union" | "enum") {
            return false;
        }
        let tail = &code[offset + keyword.len()..];
        if tail.as_bytes().first().is_some_and(|b| is_ident_byte(*b)) {
            return false;
        }
        let tail = tail.trim_start();
        if keyword == "namespace" && tail.starts_with('{') {
            return true;
        }
        let name_end = tail
            .bytes()
            .take_while(|b| is_ident_byte(*b) || *b == b':')
            .count();
        if name_end == 0 {
            return false;
        }
        let rest = tail[name_end..].trim_start();
        rest.starts_with('{')
            || (keyword == "class"
                && rest.starts_with(':')
                && rest
                    .find('{')
                    .is_some_and(|brace| !rest[..brace].contains(';')))
    })
}

fn c_like_code_without_comments_and_literals(src: &[u8]) -> String {
    let mut out = Vec::with_capacity(src.len());
    let mut i = 0;
    while i < src.len() {
        if i + 1 < src.len() && src[i] == b'/' && src[i + 1] == b'/' {
            mask_byte(&mut out, src[i]);
            mask_byte(&mut out, src[i + 1]);
            i += 2;
            while i < src.len() {
                let b = src[i];
                mask_byte(&mut out, b);
                i += 1;
                if b == b'\n' {
                    break;
                }
            }
        } else if i + 1 < src.len() && src[i] == b'/' && src[i + 1] == b'*' {
            mask_byte(&mut out, src[i]);
            mask_byte(&mut out, src[i + 1]);
            i += 2;
            while i < src.len() {
                let b = src[i];
                mask_byte(&mut out, b);
                i += 1;
                if b == b'*' && i < src.len() && src[i] == b'/' {
                    mask_byte(&mut out, src[i]);
                    i += 1;
                    break;
                }
            }
        } else if src[i] == b'"' || src[i] == b'\'' {
            let quote = src[i];
            mask_byte(&mut out, src[i]);
            i += 1;
            while i < src.len() {
                let b = src[i];
                mask_byte(&mut out, b);
                i += 1;
                if b == b'\\' && i < src.len() {
                    mask_byte(&mut out, src[i]);
                    i += 1;
                } else if b == quote {
                    break;
                }
            }
        } else {
            out.push(src[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn mask_byte(out: &mut Vec<u8>, b: u8) {
    out.push(if b == b'\n' { b'\n' } else { b' ' });
}

fn contains_word(text: &str, word: &str) -> bool {
    text.match_indices(word).any(|(idx, _)| {
        let bytes = text.as_bytes();
        let before = idx == 0 || !is_ident_byte(bytes[idx - 1]);
        let after_idx = idx + word.len();
        let after = after_idx >= bytes.len() || !is_ident_byte(bytes[after_idx]);
        before && after
    })
}

fn contains_word_followed_by_ident(text: &str, word: &str) -> bool {
    text.match_indices(word).any(|(idx, _)| {
        let bytes = text.as_bytes();
        let before = idx == 0 || !is_ident_byte(bytes[idx - 1]);
        let mut j = idx + word.len();
        if !before || (j < bytes.len() && is_ident_byte(bytes[j])) {
            return false;
        }
        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
            j += 1;
        }
        j < bytes.len() && (bytes[j].is_ascii_alphabetic() || bytes[j] == b'_')
    })
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

#[cfg(test)]
mod tests {
    #[test]
    fn c_tag_names_may_be_cpp_keywords() {
        for kind in ["struct", "union", "enum"] {
            let body = if kind == "enum" {
                "VALUE"
            } else {
                "int value;"
            };
            let source = format!("{kind} namespace {{ {body} }};");
            assert_eq!(
                super::skip_reason(
                    std::path::Path::new("a.h"),
                    nose_il::Lang::C,
                    source.as_bytes()
                ),
                None,
                "{source}"
            );
        }
    }
}
