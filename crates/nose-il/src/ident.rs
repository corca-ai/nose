//! Identifier-boundary helpers shared by frontends and semantic consumers.
//!
//! These helpers only answer whether source text contains a bounded identifier
//! token under a language family spelling rule. They are not semantic proof that
//! the identifier denotes a particular binding.

/// Return true when `text` contains `ident` bounded by C identifier characters.
pub fn contains_c_identifier(text: &str, ident: &str) -> bool {
    contains_identifier(text, ident, is_c_identifier_continue)
}

/// Return true when `text` contains `ident` bounded by JavaScript identifier
/// characters supported by nose's current lightweight source scans.
pub fn contains_js_identifier(text: &str, ident: &str) -> bool {
    contains_identifier(text, ident, is_js_identifier_continue)
}

/// Current C identifier-continuation approximation used by source scans.
pub fn is_c_identifier_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

/// Current JavaScript identifier-continuation approximation used by source scans.
pub fn is_js_identifier_continue(ch: char) -> bool {
    ch == '_' || ch == '$' || ch.is_ascii_alphanumeric()
}

fn contains_identifier(text: &str, ident: &str, is_continue: fn(char) -> bool) -> bool {
    if ident.is_empty() {
        return false;
    }
    text.match_indices(ident).any(|(idx, _)| {
        let before = text[..idx].chars().next_back();
        let after = text[idx + ident.len()..].chars().next();
        !before.is_some_and(is_continue) && !after.is_some_and(is_continue)
    })
}

#[cfg(test)]
mod tests {
    use super::{contains_c_identifier, contains_js_identifier};

    #[test]
    fn c_identifier_matches_only_token_boundaries() {
        assert!(contains_c_identifier("typedef unsigned char u8;", "u8"));
        assert!(!contains_c_identifier("u8_buf", "u8"));
        assert!(!contains_c_identifier("xu8", "u8"));
    }

    #[test]
    fn js_identifier_treats_dollar_as_identifier_character() {
        assert!(contains_js_identifier("const map = require('x');", "map"));
        assert!(!contains_js_identifier("map$impl", "map"));
        assert!(!contains_js_identifier("remap", "map"));
    }
}
