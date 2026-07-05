//! First-party language profile facade.

use super::*;

pub const C_LANGUAGE_PACK_ID: &str = "nose.lang.c";
pub const PYTHON_LANGUAGE_PACK_ID: &str = "nose.lang.python";
pub const JS_TS_LANGUAGE_PACK_ID: &str = "nose.lang.javascript-typescript";
pub const GO_LANGUAGE_PACK_ID: &str = "nose.lang.go";
pub const RUST_LANGUAGE_PACK_ID: &str = "nose.lang.rust";
pub const JAVA_LANGUAGE_PACK_ID: &str = "nose.lang.java";
pub const RUBY_LANGUAGE_PACK_ID: &str = "nose.lang.ruby";
pub const SWIFT_LANGUAGE_PACK_ID: &str = "nose.lang.swift";
pub const CSS_LANGUAGE_PACK_ID: &str = "nose.lang.css";
pub const HTML_EMBEDDED_LANGUAGE_PACK_ID: &str = "nose.lang.html";
pub const PYTHON_SOURCE_FACT_PRODUCER_ID: &str = "python.source.fact";
pub const JS_TS_SOURCE_FACT_PRODUCER_ID: &str = "javascript-typescript.source.fact";
pub const GO_SOURCE_FACT_PRODUCER_ID: &str = "go.source.fact";
pub const RUST_SOURCE_FACT_PRODUCER_ID: &str = "rust.source.fact";
pub const JAVA_SOURCE_FACT_PRODUCER_ID: &str = "java.source.fact";
pub const C_SOURCE_FACT_PRODUCER_ID: &str = "c.source.fact";
pub const RUBY_SOURCE_FACT_PRODUCER_ID: &str = "ruby.source.fact";
pub const SWIFT_SOURCE_FACT_PRODUCER_ID: &str = "swift.source.fact";
pub const CSS_SOURCE_FACT_PRODUCER_ID: &str = "css.source.fact";
pub const HTML_EMBEDDED_SOURCE_FACT_PRODUCER_ID: &str = "html-embedded.source.fact";
pub const C_UNSIGNED_32_CAST_SOURCE_PRODUCER_ID: &str = "c.source.cast.unsigned32";
pub const PYTHON_LANGUAGE_CORE_PRODUCER_ID: &str = "python.language.core";
pub const JS_TS_LANGUAGE_CORE_PRODUCER_ID: &str = "javascript-typescript.language.core";
pub const GO_LANGUAGE_CORE_PRODUCER_ID: &str = "go.language.core";
pub const RUST_LANGUAGE_CORE_PRODUCER_ID: &str = "rust.language.core";
pub const JAVA_LANGUAGE_CORE_PRODUCER_ID: &str = "java.language.core";
pub const C_LANGUAGE_CORE_PRODUCER_ID: &str = "c.language.core";
pub const RUBY_LANGUAGE_CORE_PRODUCER_ID: &str = "ruby.language.core";
pub const SWIFT_LANGUAGE_CORE_PRODUCER_ID: &str = "swift.language.core";
pub const CSS_LANGUAGE_CORE_PRODUCER_ID: &str = "css.language.core";
pub const HTML_EMBEDDED_LANGUAGE_CORE_PRODUCER_ID: &str = "html-embedded.language.core";

/// A builtin language profile. Keep this cheap and copyable; callers use it as a
/// named semantic boundary around currently-supported language behavior.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LanguageProfile {
    lang: Lang,
}

pub fn semantics(lang: Lang) -> LanguageProfile {
    LanguageProfile { lang }
}

pub fn language_source_fact_provenance(lang: Lang) -> (&'static str, &'static str) {
    let pack_id = builtin_language_pack_id(lang);
    match lang {
        Lang::Python => (pack_id, PYTHON_SOURCE_FACT_PRODUCER_ID),
        Lang::JavaScript | Lang::TypeScript => (pack_id, JS_TS_SOURCE_FACT_PRODUCER_ID),
        Lang::Go => (pack_id, GO_SOURCE_FACT_PRODUCER_ID),
        Lang::Rust => (pack_id, RUST_SOURCE_FACT_PRODUCER_ID),
        Lang::Java => (pack_id, JAVA_SOURCE_FACT_PRODUCER_ID),
        Lang::C => (pack_id, C_SOURCE_FACT_PRODUCER_ID),
        Lang::Ruby => (pack_id, RUBY_SOURCE_FACT_PRODUCER_ID),
        Lang::Swift => (pack_id, SWIFT_SOURCE_FACT_PRODUCER_ID),
        Lang::Css => (pack_id, CSS_SOURCE_FACT_PRODUCER_ID),
        Lang::Vue | Lang::Svelte | Lang::Html => (pack_id, HTML_EMBEDDED_SOURCE_FACT_PRODUCER_ID),
    }
}

pub fn language_core_evidence_provenance(lang: Lang) -> (&'static str, &'static str) {
    let pack_id = builtin_language_pack_id(lang);
    match lang {
        Lang::Python => (pack_id, PYTHON_LANGUAGE_CORE_PRODUCER_ID),
        Lang::JavaScript | Lang::TypeScript => (pack_id, JS_TS_LANGUAGE_CORE_PRODUCER_ID),
        Lang::Go => (pack_id, GO_LANGUAGE_CORE_PRODUCER_ID),
        Lang::Rust => (pack_id, RUST_LANGUAGE_CORE_PRODUCER_ID),
        Lang::Java => (pack_id, JAVA_LANGUAGE_CORE_PRODUCER_ID),
        Lang::C => (pack_id, C_LANGUAGE_CORE_PRODUCER_ID),
        Lang::Ruby => (pack_id, RUBY_LANGUAGE_CORE_PRODUCER_ID),
        Lang::Swift => (pack_id, SWIFT_LANGUAGE_CORE_PRODUCER_ID),
        Lang::Css => (pack_id, CSS_LANGUAGE_CORE_PRODUCER_ID),
        Lang::Vue | Lang::Svelte | Lang::Html => (pack_id, HTML_EMBEDDED_LANGUAGE_CORE_PRODUCER_ID),
    }
}

pub(crate) fn language_core_evidence_provenance_hashes(lang: Lang) -> (u64, u64) {
    let (pack_id, producer_id) = language_core_evidence_provenance(lang);
    (stable_symbol_hash(pack_id), stable_symbol_hash(producer_id))
}

pub fn builtin_language_pack_id(lang: Lang) -> &'static str {
    match lang {
        Lang::Python => PYTHON_LANGUAGE_PACK_ID,
        Lang::JavaScript | Lang::TypeScript => JS_TS_LANGUAGE_PACK_ID,
        Lang::Go => GO_LANGUAGE_PACK_ID,
        Lang::Rust => RUST_LANGUAGE_PACK_ID,
        Lang::Java => JAVA_LANGUAGE_PACK_ID,
        Lang::C => C_LANGUAGE_PACK_ID,
        Lang::Ruby => RUBY_LANGUAGE_PACK_ID,
        Lang::Swift => SWIFT_LANGUAGE_PACK_ID,
        Lang::Css => CSS_LANGUAGE_PACK_ID,
        Lang::Vue | Lang::Svelte | Lang::Html => HTML_EMBEDDED_LANGUAGE_PACK_ID,
    }
}

pub fn is_builtin_language_pack_hash(pack_hash: u64) -> bool {
    pack_hash == stable_symbol_hash(PYTHON_LANGUAGE_PACK_ID)
        || pack_hash == stable_symbol_hash(JS_TS_LANGUAGE_PACK_ID)
        || pack_hash == stable_symbol_hash(GO_LANGUAGE_PACK_ID)
        || pack_hash == stable_symbol_hash(RUST_LANGUAGE_PACK_ID)
        || pack_hash == stable_symbol_hash(JAVA_LANGUAGE_PACK_ID)
        || pack_hash == stable_symbol_hash(C_LANGUAGE_PACK_ID)
        || pack_hash == stable_symbol_hash(RUBY_LANGUAGE_PACK_ID)
        || pack_hash == stable_symbol_hash(SWIFT_LANGUAGE_PACK_ID)
        || pack_hash == stable_symbol_hash(CSS_LANGUAGE_PACK_ID)
        || pack_hash == stable_symbol_hash(HTML_EMBEDDED_LANGUAGE_PACK_ID)
}

impl LanguageProfile {
    pub fn lang(self) -> Lang {
        self.lang
    }

    /// Whether the language is dynamically typed — a bare parameter carries no static type, so
    /// it could be a float at runtime (#342). Used to decide that an untyped `+`/`*` chain is
    /// POSSIBLY float and must not be reassociated (float `+`/`*` is non-associative). The
    /// statically-typed languages (Rust/Go/C/Java) instead carry per-param domain evidence, so
    /// their float-ness is decided by the proven domain, not by this.
    pub fn is_dynamically_typed(self) -> bool {
        matches!(
            self.lang,
            Lang::Python
                | Lang::Ruby
                | Lang::JavaScript
                | Lang::TypeScript
                | Lang::Vue
                | Lang::Svelte
                | Lang::Html
        )
    }

    pub fn pack_id(self) -> &'static str {
        builtin_language_pack_id(self.lang)
    }

    pub fn trust(self) -> PackTrust {
        PackTrust::BuiltinDefault
    }

    pub fn operators(self) -> OperatorSemantics {
        OperatorSemantics { lang: self.lang }
    }

    pub fn effects(self) -> EffectSemantics {
        EffectSemantics { lang: self.lang }
    }

    pub fn modules(self) -> ModuleSemantics {
        ModuleSemantics { lang: self.lang }
    }

    pub fn stdlib(self) -> StdlibSemantics {
        StdlibSemantics { lang: self.lang }
    }

    pub fn collections(self) -> CollectionSemantics {
        CollectionSemantics { lang: self.lang }
    }

    pub fn exact_fragments(self) -> FragmentSemantics {
        FragmentSemantics { lang: self.lang }
    }
}

pub(crate) fn js_like_lang(lang: Lang) -> bool {
    matches!(
        lang,
        Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html
    )
}
