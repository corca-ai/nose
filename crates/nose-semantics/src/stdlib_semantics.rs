//! Standard-library profile toggles and builtin tags.

use super::*;

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
