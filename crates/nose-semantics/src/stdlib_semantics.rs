//! Standard-library profile toggles and builtin tags.

use super::*;

/// Internal source marker emitted when Swift declares a conformance that can
/// change the contextual meaning of `nil` away from Optional absence.
pub const SWIFT_NIL_LITERAL_CONFORMANCE_MARKER: &str =
    "__nose_swift_expressible_by_nil_literal_conformance";

/// Internal marker for Swift source constructs that can hide a nil-literal
/// conformance from the current name resolver (imports, type aliases, macros).
pub const SWIFT_NIL_LITERAL_PROOF_BARRIER_MARKER: &str = "__nose_swift_nil_literal_proof_barrier";

/// Internal marker emitted when Swift source can override `compactMap`, `map`,
/// or `filter` dispatch. This includes methods and callable properties, even
/// when parser recovery does not produce a function/method unit, so exact
/// stdlib dispatch must remain closed in its presence.
pub const SWIFT_COMPACT_MAP_DISPATCH_BARRIER_MARKER: &str =
    "__nose_swift_compact_map_dispatch_barrier";

/// Internal marker emitted when Swift source can override `flatMap`, its
/// guarded `filter` source, or its nested `map` producer, or can
/// import/macro-expand such an override. The controlled one-level flatten
/// proof stays closed whenever this marker is visible.
pub const SWIFT_FLAT_MAP_DISPATCH_BARRIER_MARKER: &str = "__nose_swift_flat_map_dispatch_barrier";

/// Internal marker emitted when Swift source can override the eager unary
/// `allSatisfy` terminal with a callback-compatible declaration, or can
/// import/macro-expand such an override. Proven callback-arity-disjoint
/// overloads do not emit it. Exact terminal aggregate admission stays closed
/// whenever this marker is visible.
pub const SWIFT_ALL_SATISFY_DISPATCH_BARRIER_MARKER: &str =
    "__nose_swift_all_satisfy_dispatch_barrier";

/// Internal marker emitted when imports, macros, or a visible extension can
/// change `Dictionary` default-subscript overload resolution. The controlled
/// absence-default slice stays closed whenever this marker is visible.
pub const SWIFT_DICTIONARY_DEFAULT_SUBSCRIPT_BARRIER_MARKER: &str =
    "__nose_swift_dictionary_default_subscript_barrier";

/// Compare a lowered Swift identifier after removing the language's backtick
/// escape syntax. Escaping changes how a keyword is parsed, not the identifier
/// denoted at runtime.
pub fn swift_identifier_matches(actual: &str, expected: &str) -> bool {
    actual == expected
        || actual
            .strip_prefix('`')
            .and_then(|value| value.strip_suffix('`'))
            == Some(expected)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StdlibSemantics {
    pub(super) lang: Lang,
}

impl StdlibSemantics {
    pub fn python_collection_factories(self) -> bool {
        self.lang == Lang::Python
    }

    pub fn python_deque_factory(self) -> bool {
        self.lang == Lang::Python
    }

    pub fn java_collection_factories(self) -> bool {
        self.lang == Lang::Java
    }

    pub fn java_map_factories(self) -> bool {
        self.lang == Lang::Java
    }

    pub fn java_primitive_integer_ops(self) -> bool {
        self.lang == Lang::Java
    }

    pub fn ruby_set_factory(self) -> bool {
        self.lang == Lang::Ruby
    }

    pub fn rust_vec_macro_factory(self) -> bool {
        self.lang == Lang::Rust
    }

    pub fn rust_vec_new_factory(self) -> bool {
        self.lang == Lang::Rust
    }

    pub fn rust_std_collection_factories(self) -> bool {
        self.lang == Lang::Rust
    }

    pub fn rust_std_map_factories(self) -> bool {
        self.lang == Lang::Rust
    }

    pub fn swift_collection_factories(self) -> bool {
        self.lang == Lang::Swift
    }

    pub fn go_literal_zero_map_lookup(self) -> bool {
        self.lang == Lang::Go
    }

    pub fn rust_filter_map_option_contract(self) -> bool {
        self.lang == Lang::Rust
    }

    pub fn swift_compact_map_option_contract(self) -> bool {
        self.lang == Lang::Swift
    }

    pub fn imported_map_factory(self) -> Option<ImportedMapFactoryContract> {
        match self.lang {
            Lang::Java => Some(ImportedMapFactoryContract::JavaMap),
            Lang::Rust => Some(ImportedMapFactoryContract::RustStdMap),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ImportedMapFactoryContract {
    JavaMap,
    RustStdMap,
}

/// The value-graph call tag for a canonical builtin. Tag `0` is reserved for
/// opaque calls, so kernel-owned builtin contracts start at `1`.
pub fn builtin_tag(builtin: Builtin) -> u32 {
    builtin as u32 + 1
}
