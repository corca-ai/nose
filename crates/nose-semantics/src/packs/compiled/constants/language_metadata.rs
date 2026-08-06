use super::*;

pub(in crate::packs::compiled) const C_LANGUAGE: &[&str] = &["c"];
pub(in crate::packs::compiled) const C_LANGUAGE_FILE_EXTENSIONS: &[&str] = &["c", "h"];
pub(in crate::packs::compiled) const PYTHON_BINDING_LANGS: &[Lang] = &[Lang::Python];
pub(in crate::packs::compiled) const JS_TS_BINDING_LANGS: &[Lang] =
    &[Lang::JavaScript, Lang::TypeScript];
pub(in crate::packs::compiled) const GO_BINDING_LANGS: &[Lang] = &[Lang::Go];
pub(in crate::packs::compiled) const RUST_BINDING_LANGS: &[Lang] = &[Lang::Rust];
pub(in crate::packs::compiled) const JAVA_BINDING_LANGS: &[Lang] = &[Lang::Java];
pub(in crate::packs::compiled) const C_BINDING_LANGS: &[Lang] = &[Lang::C];
pub(in crate::packs::compiled) const RUBY_BINDING_LANGS: &[Lang] = &[Lang::Ruby];
pub(in crate::packs::compiled) const SWIFT_BINDING_LANGS: &[Lang] = &[Lang::Swift];
pub(in crate::packs::compiled) const CSS_BINDING_LANGS: &[Lang] = &[Lang::Css];
pub(in crate::packs::compiled) const HTML_EMBEDDED_BINDING_LANGS: &[Lang] =
    &[Lang::Html, Lang::Vue, Lang::Svelte];
pub(in crate::packs::compiled) const PYTHON_LANGUAGE_PRODUCER_IDS: &[&str] = &[
    PYTHON_LANGUAGE_CORE_PRODUCER_ID,
    PYTHON_SOURCE_FACT_PRODUCER_ID,
];
pub(in crate::packs::compiled) const PYTHON_LANGUAGE_SOURCE_FACT_PRODUCER_IDS: &[&str] =
    &[PYTHON_SOURCE_FACT_PRODUCER_ID];
pub(in crate::packs::compiled) const JS_TS_LANGUAGE_PRODUCER_IDS: &[&str] = &[
    JS_TS_LANGUAGE_CORE_PRODUCER_ID,
    JS_TS_SOURCE_FACT_PRODUCER_ID,
];
pub(in crate::packs::compiled) const JS_TS_LANGUAGE_SOURCE_FACT_PRODUCER_IDS: &[&str] =
    &[JS_TS_SOURCE_FACT_PRODUCER_ID];
pub(in crate::packs::compiled) const GO_LANGUAGE_PRODUCER_IDS: &[&str] =
    &[GO_LANGUAGE_CORE_PRODUCER_ID, GO_SOURCE_FACT_PRODUCER_ID];
pub(in crate::packs::compiled) const GO_LANGUAGE_SOURCE_FACT_PRODUCER_IDS: &[&str] =
    &[GO_SOURCE_FACT_PRODUCER_ID];
pub(in crate::packs::compiled) const RUST_LANGUAGE_PRODUCER_IDS: &[&str] =
    &[RUST_LANGUAGE_CORE_PRODUCER_ID, RUST_SOURCE_FACT_PRODUCER_ID];
pub(in crate::packs::compiled) const RUST_LANGUAGE_SOURCE_FACT_PRODUCER_IDS: &[&str] =
    &[RUST_SOURCE_FACT_PRODUCER_ID];
pub(in crate::packs::compiled) const JAVA_LANGUAGE_PRODUCER_IDS: &[&str] =
    &[JAVA_LANGUAGE_CORE_PRODUCER_ID, JAVA_SOURCE_FACT_PRODUCER_ID];
pub(in crate::packs::compiled) const JAVA_LANGUAGE_SOURCE_FACT_PRODUCER_IDS: &[&str] =
    &[JAVA_SOURCE_FACT_PRODUCER_ID];
pub(in crate::packs::compiled) const C_LANGUAGE_PRODUCER_IDS: &[&str] = &[
    C_LANGUAGE_CORE_PRODUCER_ID,
    C_SOURCE_FACT_PRODUCER_ID,
    C_UNSIGNED_32_CAST_SOURCE_PRODUCER_ID,
];
pub(in crate::packs::compiled) const C_LANGUAGE_SOURCE_FACT_PRODUCER_IDS: &[&str] = &[
    C_SOURCE_FACT_PRODUCER_ID,
    C_UNSIGNED_32_CAST_SOURCE_PRODUCER_ID,
];
pub(in crate::packs::compiled) const RUBY_LANGUAGE_PRODUCER_IDS: &[&str] =
    &[RUBY_LANGUAGE_CORE_PRODUCER_ID, RUBY_SOURCE_FACT_PRODUCER_ID];
pub(in crate::packs::compiled) const RUBY_LANGUAGE_SOURCE_FACT_PRODUCER_IDS: &[&str] =
    &[RUBY_SOURCE_FACT_PRODUCER_ID];
pub(in crate::packs::compiled) const SWIFT_LANGUAGE_PRODUCER_IDS: &[&str] = &[
    SWIFT_LANGUAGE_CORE_PRODUCER_ID,
    SWIFT_SOURCE_FACT_PRODUCER_ID,
];
pub(in crate::packs::compiled) const SWIFT_LANGUAGE_SOURCE_FACT_PRODUCER_IDS: &[&str] =
    &[SWIFT_SOURCE_FACT_PRODUCER_ID];
pub(in crate::packs::compiled) const CSS_LANGUAGE_PRODUCER_IDS: &[&str] =
    &[CSS_LANGUAGE_CORE_PRODUCER_ID, CSS_SOURCE_FACT_PRODUCER_ID];
pub(in crate::packs::compiled) const CSS_LANGUAGE_SOURCE_FACT_PRODUCER_IDS: &[&str] =
    &[CSS_SOURCE_FACT_PRODUCER_ID];
pub(in crate::packs::compiled) const HTML_EMBEDDED_LANGUAGE_PRODUCER_IDS: &[&str] = &[
    HTML_EMBEDDED_LANGUAGE_CORE_PRODUCER_ID,
    HTML_EMBEDDED_SOURCE_FACT_PRODUCER_ID,
];
pub(in crate::packs::compiled) const HTML_EMBEDDED_LANGUAGE_SOURCE_FACT_PRODUCER_IDS: &[&str] =
    &[HTML_EMBEDDED_SOURCE_FACT_PRODUCER_ID];
pub(in crate::packs::compiled) const C_LANGUAGE_CONFORMANCE_REFS: &[&str] = &[
    "c-unsigned32-byte-lane-cast-positive",
    "c-unsigned32-alias-cast-positive",
    "c-unsigned32-signed-cast-hard-negative",
    "c-unsigned32-non-byte-lane-hard-negative",
];
pub(in crate::packs::compiled) const PYTHON_LANGUAGE_FILE_EXTENSIONS: &[&str] = &["py", "pyi"];
pub(in crate::packs::compiled) const JS_TS_LANGUAGE_FILE_EXTENSIONS: &[&str] =
    &["js", "jsx", "mjs", "cjs", "ts", "tsx", "mts", "cts"];
pub(in crate::packs::compiled) const GO_LANGUAGE: &[&str] = &["go"];
pub(in crate::packs::compiled) const GO_STDLIB_NAMESPACE_CALL_LANGUAGE: &[&str] = &["go"];
pub(in crate::packs::compiled) const GO_LANGUAGE_FILE_EXTENSIONS: &[&str] = &["go"];
pub(in crate::packs::compiled) const RUST_LANGUAGE_FILE_EXTENSIONS: &[&str] = &["rs"];
pub(in crate::packs::compiled) const JAVA_LANGUAGE_FILE_EXTENSIONS: &[&str] = &["java"];
pub(in crate::packs::compiled) const RUBY_LANGUAGE_FILE_EXTENSIONS: &[&str] = &["rb"];
pub(in crate::packs::compiled) const SWIFT_LANGUAGE: &[&str] = &["swift"];
pub(in crate::packs::compiled) const SWIFT_LANGUAGE_FILE_EXTENSIONS: &[&str] = &["swift"];
pub(in crate::packs::compiled) const CSS_LANGUAGE: &[&str] = &["css"];
pub(in crate::packs::compiled) const CSS_LANGUAGE_FILE_EXTENSIONS: &[&str] = &["css"];
pub(in crate::packs::compiled) const HTML_EMBEDDED_LANGUAGES: &[&str] = &["html", "vue", "svelte"];
pub(in crate::packs::compiled) const HTML_EMBEDDED_LANGUAGE_FILE_EXTENSIONS: &[&str] =
    &["html", "htm", "vue", "svelte"];
pub(in crate::packs::compiled) const JS_LIKE_LANGUAGE: &[&str] = &["javascript", "typescript"];
pub(in crate::packs::compiled) const JAVA_LANGUAGE: &[&str] = &["java"];
pub(in crate::packs::compiled) const JAVA_RUST_LANGUAGE: &[&str] = &["java", "rust"];
