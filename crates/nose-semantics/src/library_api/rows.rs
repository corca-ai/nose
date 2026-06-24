//! Builtin library/API row constructors.

use super::*;

mod methods;

pub use methods::*;

fn library_collection_factory_pack_id(id: LibraryApiContractId) -> &'static str {
    match id {
        LibraryApiContractId::PythonBuiltinCollectionFactory => {
            PYTHON_BUILTIN_COLLECTION_FACTORY_PACK_ID
        }
        LibraryApiContractId::PythonImportedCollectionFactory => {
            PYTHON_STDLIB_COLLECTION_FACTORY_PACK_ID
        }
        LibraryApiContractId::RustVecMacroFactory | LibraryApiContractId::RustVecNewFactory => {
            RUST_STDLIB_VEC_PACK_ID
        }
        LibraryApiContractId::RustStdCollectionFactory => RUST_STDLIB_COLLECTION_FACTORY_PACK_ID,
        LibraryApiContractId::JavaCollectionFactory(kind) => java_collection_factory_pack_id(kind),
        LibraryApiContractId::JavaCollectionConstructor(_) => {
            JAVA_STDLIB_COLLECTION_CONSTRUCTOR_PACK_ID
        }
        LibraryApiContractId::RubySetFactory => RUBY_STDLIB_SET_PACK_ID,
        LibraryApiContractId::JsLikeSetConstructor => {
            JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PACK_ID
        }
        _ => unreachable!("collection-factory contract has no broad builtin pack"),
    }
}

fn library_map_factory_pack_id(id: LibraryApiContractId) -> &'static str {
    match id {
        LibraryApiContractId::RustStdMapFactory => RUST_STDLIB_MAP_FACTORY_PACK_ID,
        LibraryApiContractId::JavaMapFactory(kind) => java_map_factory_pack_id(kind),
        LibraryApiContractId::JavaMapEntryFactory => JAVA_STDLIB_MAP_ENTRY_PACK_ID,
        LibraryApiContractId::JsLikeMapConstructor => {
            JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PACK_ID
        }
        _ => unreachable!("map-factory contract has no broad builtin pack"),
    }
}

fn java_collection_factory_pack_id(kind: JavaCollectionFactoryKind) -> &'static str {
    match kind {
        JavaCollectionFactoryKind::ListOf
        | JavaCollectionFactoryKind::SetOf
        | JavaCollectionFactoryKind::ArraysAsList => JAVA_STDLIB_COLLECTION_FACTORY_PACK_ID,
        JavaCollectionFactoryKind::GuavaImmutableListOf
        | JavaCollectionFactoryKind::GuavaImmutableSetOf => {
            JAVA_GUAVA_IMMUTABLE_COLLECTION_FACTORY_PACK_ID
        }
    }
}

fn java_map_factory_pack_id(kind: JavaMapFactoryKind) -> &'static str {
    match kind {
        JavaMapFactoryKind::Of | JavaMapFactoryKind::OfEntries => JAVA_STDLIB_MAP_FACTORY_PACK_ID,
        JavaMapFactoryKind::GuavaImmutableMapOf => JAVA_GUAVA_IMMUTABLE_COLLECTION_FACTORY_PACK_ID,
    }
}

pub fn library_free_name_collection_factory_contract(
    lang: Lang,
    name: &str,
) -> Option<LibraryCollectionFactoryContract> {
    FREE_NAME_COLLECTION_FACTORIES
        .iter()
        .find(|row| row.lang.is_none_or(|row_lang| row_lang == lang) && row.names.contains(&name))
        .and_then(|row| {
            let matched_name = row
                .names
                .iter()
                .copied()
                .find(|candidate| *candidate == name)?;
            let id = match lang {
                Lang::Python => LibraryApiContractId::PythonBuiltinCollectionFactory,
                Lang::Rust => LibraryApiContractId::RustStdCollectionFactory,
                _ => return None,
            };
            Some(LibraryCollectionFactoryContract {
                pack_id: library_collection_factory_pack_id(id),
                id,
                callee: LibraryApiCalleeContract::FreeName {
                    name: matched_name,
                    shadow: library_free_name_shadow_policy(lang, row.shadow_guard),
                },
                result: LibraryCollectionFactoryResult::SequenceArgument,
            })
        })
}

pub fn library_free_name_collection_factory_contracts(
    lang: Lang,
) -> impl Iterator<Item = LibraryCollectionFactoryContract> {
    FREE_NAME_COLLECTION_FACTORIES
        .iter()
        .filter(move |row| row.lang.is_none_or(|row_lang| row_lang == lang))
        .flat_map(move |row| {
            row.names
                .iter()
                .filter_map(move |name| library_free_name_collection_factory_contract(lang, name))
        })
}

pub fn library_free_function_builtin_contract(
    lang: Lang,
    name: &str,
    arg_count: usize,
) -> Option<LibraryFreeFunctionBuiltinContract> {
    let result = free_function_builtin_contract(lang, name, arg_count)?;
    Some(LibraryFreeFunctionBuiltinContract {
        id: LibraryApiContractId::FreeFunctionBuiltin(result.builtin),
        callee: LibraryApiCalleeContract::FreeName {
            name: result.name,
            shadow: library_free_name_shadow_policy(lang, result.requires_unshadowed),
        },
        result,
    })
}

pub fn library_imported_collection_factory_contract(
    lang: Lang,
    module: &str,
    exported: &str,
) -> Option<LibraryCollectionFactoryContract> {
    IMPORTED_COLLECTION_FACTORIES
        .iter()
        .find(|row| {
            row.lang.is_none_or(|row_lang| row_lang == lang)
                && row.module == module
                && row.exported == exported
        })
        .map(|row| LibraryCollectionFactoryContract {
            pack_id: library_collection_factory_pack_id(
                LibraryApiContractId::PythonImportedCollectionFactory,
            ),
            id: LibraryApiContractId::PythonImportedCollectionFactory,
            callee: LibraryApiCalleeContract::ImportedBinding {
                module: row.module,
                exported: row.exported,
            },
            result: LibraryCollectionFactoryResult::SequenceArgument,
        })
}

pub fn library_imported_collection_factory_contracts(
    lang: Lang,
) -> impl Iterator<Item = LibraryCollectionFactoryContract> {
    IMPORTED_COLLECTION_FACTORIES
        .iter()
        .filter(move |row| row.lang.is_none_or(|row_lang| row_lang == lang))
        .filter_map(move |row| {
            library_imported_collection_factory_contract(lang, row.module, row.exported)
        })
}

pub fn library_free_name_map_factory_contract(
    lang: Lang,
    name: &str,
) -> Option<LibraryMapFactoryContract> {
    FREE_NAME_MAP_FACTORIES
        .iter()
        .find(|row| row.lang.is_none_or(|row_lang| row_lang == lang) && row.names.contains(&name))
        .and_then(|row| {
            let matched_name = row
                .names
                .iter()
                .copied()
                .find(|candidate| *candidate == name)?;
            let id = match lang {
                Lang::Rust => LibraryApiContractId::RustStdMapFactory,
                _ => return None,
            };
            Some(LibraryMapFactoryContract {
                pack_id: library_map_factory_pack_id(id),
                id,
                callee: LibraryApiCalleeContract::FreeName {
                    name: matched_name,
                    shadow: library_free_name_shadow_policy(lang, false),
                },
                result: LibraryMapFactoryResult::EntrySequence {
                    entry_seq_tag: row.entry_seq_tag,
                },
            })
        })
}

pub fn library_free_name_map_factory_contracts(
    lang: Lang,
) -> impl Iterator<Item = LibraryMapFactoryContract> {
    FREE_NAME_MAP_FACTORIES
        .iter()
        .filter(move |row| row.lang.is_none_or(|row_lang| row_lang == lang))
        .flat_map(move |row| {
            row.names
                .iter()
                .filter_map(move |name| library_free_name_map_factory_contract(lang, name))
        })
}

pub fn library_java_collection_factory_contract(
    lang: Lang,
    receiver: &str,
    method: &str,
) -> Option<LibraryCollectionFactoryContract> {
    let contract = java_collection_factory_contract(lang, receiver, method)?;
    let id = LibraryApiContractId::JavaCollectionFactory(contract.kind);
    let callee = if contract.module == "java.util" {
        LibraryApiCalleeContract::JavaUtilStaticMember {
            receiver: contract.receiver,
            method: contract.method,
        }
    } else {
        LibraryApiCalleeContract::JavaStaticMember {
            module: contract.module,
            receiver: contract.receiver,
            method: contract.method,
            requires_import_for_simple_receiver: true,
            requires_no_local_type_shadow: true,
        }
    };
    Some(LibraryCollectionFactoryContract {
        pack_id: library_collection_factory_pack_id(id),
        id,
        callee,
        result: LibraryCollectionFactoryResult::VariadicElements {
            single_arg_spreads_array: contract.single_arg_spreads_array,
        },
    })
}

pub fn library_java_collection_factory_contract_by_hash(
    lang: Lang,
    receiver: &str,
    method_hash: u64,
) -> Option<LibraryCollectionFactoryContract> {
    ["of", "asList"].into_iter().find_map(|method| {
        (stable_symbol_hash(method) == method_hash)
            .then(|| library_java_collection_factory_contract(lang, receiver, method))
            .flatten()
    })
}

pub fn library_java_collection_constructor_contract(
    lang: Lang,
    type_name: &str,
    arg_count: usize,
) -> Option<LibraryCollectionFactoryContract> {
    let contract = java_collection_constructor_contract(lang, type_name, arg_count)?;
    let id = LibraryApiContractId::JavaCollectionConstructor(contract.kind);
    Some(LibraryCollectionFactoryContract {
        pack_id: library_collection_factory_pack_id(id),
        id,
        callee: LibraryApiCalleeContract::JavaUtilConstructor {
            simple_type: contract.simple_type,
            qualified_type: contract.qualified_type,
            module: contract.module,
            requires_import_for_simple_type: contract.requires_import_for_simple_type,
            requires_no_local_type_shadow: contract.requires_no_local_type_shadow,
        },
        result: LibraryCollectionFactoryResult::EmptySequence,
    })
}

pub fn library_java_map_factory_contract(
    lang: Lang,
    receiver: &str,
    method: &str,
) -> Option<LibraryMapFactoryContract> {
    let contract = java_map_factory_contract(lang, receiver, method)?;
    let id = LibraryApiContractId::JavaMapFactory(contract.kind);
    let callee = if contract.module == "java.util" {
        LibraryApiCalleeContract::JavaUtilStaticMember {
            receiver: contract.receiver,
            method: contract.method,
        }
    } else {
        LibraryApiCalleeContract::JavaStaticMember {
            module: contract.module,
            receiver: contract.receiver,
            method: contract.method,
            requires_import_for_simple_receiver: true,
            requires_no_local_type_shadow: true,
        }
    };
    Some(LibraryMapFactoryContract {
        pack_id: library_map_factory_pack_id(id),
        id,
        callee,
        result: LibraryMapFactoryResult::JavaFactory {
            kind: contract.kind,
        },
    })
}

pub fn library_java_map_factory_contract_by_hash(
    lang: Lang,
    receiver: &str,
    method_hash: u64,
) -> Option<LibraryMapFactoryContract> {
    ["of", "ofEntries"].into_iter().find_map(|method| {
        (stable_symbol_hash(method) == method_hash)
            .then(|| library_java_map_factory_contract(lang, receiver, method))
            .flatten()
    })
}

pub fn library_java_map_entry_contract(
    lang: Lang,
    receiver: &str,
    method: &str,
) -> Option<LibraryMapEntryFactoryContract> {
    java_map_entry_contract(lang, receiver, method).then_some(LibraryMapEntryFactoryContract {
        pack_id: library_map_factory_pack_id(LibraryApiContractId::JavaMapEntryFactory),
        id: LibraryApiContractId::JavaMapEntryFactory,
        callee: LibraryApiCalleeContract::JavaUtilStaticMember {
            receiver: "Map",
            method: "entry",
        },
    })
}

pub fn library_java_map_entry_contract_by_hash(
    lang: Lang,
    receiver: &str,
    method_hash: u64,
) -> Option<LibraryMapEntryFactoryContract> {
    (method_hash == stable_symbol_hash("entry"))
        .then(|| library_java_map_entry_contract(lang, receiver, "entry"))
        .flatten()
}

pub fn library_ruby_set_factory_contract(
    lang: Lang,
    receiver: &str,
    method: &str,
    arg_count: usize,
) -> Option<LibraryCollectionFactoryContract> {
    let contract = ruby_set_factory_contract(lang, receiver, method, arg_count)?;
    let id = LibraryApiContractId::RubySetFactory;
    Some(LibraryCollectionFactoryContract {
        pack_id: library_collection_factory_pack_id(id),
        id,
        callee: LibraryApiCalleeContract::RubyRequireStaticMember {
            receiver: contract.receiver,
            method: contract.method,
            required_module: contract.required_module,
            shadow_root: contract.shadow_root,
        },
        result: LibraryCollectionFactoryResult::SequenceArgument,
    })
}

pub fn library_ruby_set_factory_contract_by_hash(
    lang: Lang,
    receiver: &str,
    method_hash: u64,
    arg_count: usize,
) -> Option<LibraryCollectionFactoryContract> {
    (method_hash == stable_symbol_hash("new"))
        .then(|| library_ruby_set_factory_contract(lang, receiver, "new", arg_count))
        .flatten()
}

pub fn library_js_like_set_constructor_contract(
    lang: Lang,
    receiver: &str,
) -> Option<LibraryCollectionFactoryContract> {
    let contract = js_like_set_constructor_contract(lang, receiver)?;
    let id = LibraryApiContractId::JsLikeSetConstructor;
    Some(LibraryCollectionFactoryContract {
        pack_id: library_collection_factory_pack_id(id),
        id,
        callee: LibraryApiCalleeContract::JsGlobalConstructor {
            receiver: contract.receiver,
            requires_unshadowed_global: contract.requires_unshadowed_global,
        },
        result: LibraryCollectionFactoryResult::StaticNonFloatSequenceArgument,
    })
}

pub fn library_js_like_map_constructor_contract(
    lang: Lang,
    receiver: &str,
) -> Option<LibraryMapFactoryContract> {
    let contract = js_like_map_constructor_contract(lang, receiver)?;
    let id = LibraryApiContractId::JsLikeMapConstructor;
    Some(LibraryMapFactoryContract {
        pack_id: library_map_factory_pack_id(id),
        id,
        callee: LibraryApiCalleeContract::JsGlobalConstructor {
            receiver: contract.receiver,
            requires_unshadowed_global: contract.requires_unshadowed_global,
        },
        result: LibraryMapFactoryResult::EntrySequence {
            entry_seq_tag: contract.entry_seq_tag?,
        },
    })
}

pub fn library_rust_vec_macro_factory_contract(
    lang: Lang,
    name: &str,
) -> Option<LibraryCollectionFactoryContract> {
    (lang == Lang::Rust && name == "vec").then_some({
        let id = LibraryApiContractId::RustVecMacroFactory;
        LibraryCollectionFactoryContract {
            pack_id: library_collection_factory_pack_id(id),
            id,
            callee: LibraryApiCalleeContract::RustMacro {
                name: "vec",
                shadow: LibraryApiShadowPolicy::SameName,
            },
            result: LibraryCollectionFactoryResult::VariadicElements {
                single_arg_spreads_array: false,
            },
        }
    })
}

pub fn library_rust_vec_new_factory_contract(
    lang: Lang,
    name: &str,
) -> Option<LibraryCollectionFactoryContract> {
    let contract = rust_vec_new_factory_contract(lang, name)?;
    let id = LibraryApiContractId::RustVecNewFactory;
    Some(LibraryCollectionFactoryContract {
        pack_id: library_collection_factory_pack_id(id),
        id,
        callee: LibraryApiCalleeContract::FreeName {
            name: match name {
                "Vec::new" => "Vec::new",
                "std::vec::Vec::new" => "std::vec::Vec::new",
                "alloc::vec::Vec::new" => "alloc::vec::Vec::new",
                _ => return None,
            },
            shadow: LibraryApiShadowPolicy::ExplicitRoot(contract.shadow_root),
        },
        result: LibraryCollectionFactoryResult::EmptySequence,
    })
}

fn library_free_name_shadow_policy(lang: Lang, shadow_guard: bool) -> LibraryApiShadowPolicy {
    if shadow_guard {
        LibraryApiShadowPolicy::SameName
    } else if lang == Lang::Rust {
        LibraryApiShadowPolicy::RustStdRootForStdPath
    } else {
        LibraryApiShadowPolicy::None
    }
}
