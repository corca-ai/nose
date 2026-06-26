//! Method-family selectors and library API wrappers.

use super::*;

pub fn method_fold_name(lang: Lang, name: &str) -> bool {
    matches!(
        (lang, name),
        (
            Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html,
            "reduce"
        ) | (Lang::Ruby, "inject" | "reduce")
            | (Lang::Rust, "fold")
            | (Lang::Java, "reduce")
    )
}

pub fn method_bool_reduction_builtin(lang: Lang, name: &str) -> Option<Builtin> {
    Some(match (lang, name) {
        (Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html, "some") => {
            Builtin::Any
        }
        (Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html, "every") => {
            Builtin::All
        }
        (Lang::Rust, "any") | (Lang::Ruby, "any?") | (Lang::Java, "anyMatch") => Builtin::Any,
        (Lang::Rust, "all") | (Lang::Ruby, "all?") | (Lang::Java, "allMatch") => Builtin::All,
        _ => return None,
    })
}

pub fn method_hof_contract(lang: Lang, name: &str) -> Option<HoFKind> {
    Some(match (lang, name) {
        (Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html, "map")
        | (Lang::Rust, "map")
        | (Lang::Java, "map")
        | (Lang::Swift, "map")
        | (Lang::Ruby, "map" | "collect") => HoFKind::Map,
        (
            Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html,
            "flatMap",
        )
        | (Lang::Rust, "flat_map")
        | (Lang::Java, "flatMap")
        | (Lang::Swift, "flatMap") => HoFKind::FlatMap,
        (Lang::Rust, "filter_map") => HoFKind::FilterMap,
        (Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html, "filter")
        | (Lang::Rust, "filter")
        | (Lang::Java, "filter")
        | (Lang::Swift, "filter")
        | (Lang::Ruby, "filter" | "select") => HoFKind::Filter,
        (Lang::Ruby, "reject") => HoFKind::Reject,
        _ => return None,
    })
}

pub fn method_collection_reduction_builtin(lang: Lang, name: &str) -> Option<Builtin> {
    Some(match (lang, name) {
        (Lang::Rust, "sum") => Builtin::Sum,
        (Lang::Rust, "min") => Builtin::Min,
        (Lang::Rust, "max") => Builtin::Max,
        (Lang::Rust, "count") => Builtin::Len,
        (Lang::Java, "count") => Builtin::Len,
        _ => return None,
    })
}

pub fn property_builtin_contract(lang: Lang, name: &str) -> Option<Builtin> {
    library_property_builtin_contract(lang, name).map(|contract| contract.result)
}

pub(super) fn property_builtin_contract_shape(
    lang: Lang,
    name: &str,
) -> Option<(Builtin, MethodReceiverContract)> {
    Some(match (lang, name) {
        (Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html, "length") => {
            (Builtin::Len, MethodReceiverContract::ExactCollection)
        }
        (Lang::Java, "length") => (Builtin::Len, MethodReceiverContract::ExactCollection),
        (Lang::Swift, "count") => (Builtin::Len, MethodReceiverContract::ExactCollection),
        (Lang::Swift, "isEmpty") => (Builtin::IsEmpty, MethodReceiverContract::ExactCollection),
        _ => return None,
    })
}

pub fn library_property_builtin_contract(
    lang: Lang,
    name: &str,
) -> Option<LibraryPropertyBuiltinContract> {
    let (result, receiver) = property_builtin_contract_shape(lang, name)?;
    let property = library_property_selector_name(name)?;
    Some(LibraryPropertyBuiltinContract {
        pack_id: PROPERTY_BUILTIN_PROTOCOL_PACK_ID,
        id: LibraryApiContractId::PropertyBuiltin(result),
        callee: LibraryApiCalleeContract::Property { property, receiver },
        result,
    })
}

pub(super) fn library_property_selector_name(name: &str) -> Option<&'static str> {
    Some(match name {
        "length" => "length",
        "count" => "count",
        "isEmpty" => "isEmpty",
        _ => return None,
    })
}

pub fn library_scalar_integer_method_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<LibraryScalarIntegerMethodContract> {
    let result = scalar_integer_method_contract_shape(lang, method, arg_count)?;
    let method = library_method_selector_name(method)?;
    let pack_id = match (lang, result.receiver) {
        (Lang::Rust, MethodReceiverContract::ExactInteger) => RUST_STDLIB_INTEGER_METHOD_PACK_ID,
        (Lang::Java, MethodReceiverContract::UnshadowedGlobal("Math")) => JAVA_STDLIB_MATH_PACK_ID,
        _ => unreachable!("scalar integer method contract has no broad builtin pack"),
    };
    Some(LibraryScalarIntegerMethodContract {
        pack_id,
        id: LibraryApiContractId::ScalarIntegerMethod(result.semantic),
        callee: LibraryApiCalleeContract::Method {
            method,
            receiver: result.receiver,
        },
        result,
    })
}

pub fn library_rust_option_some_constructor_contract(
    lang: Lang,
    name: &str,
    arg_count: usize,
) -> Option<LibraryRustOptionConstructorContract> {
    if arg_count != 1 {
        return None;
    }
    let name = rust_option_some_selector_name(lang, name)?;
    let shadow = rust_option_some_constructor_contract(lang, name)?;
    Some(LibraryRustOptionConstructorContract {
        pack_id: RUST_STDLIB_OPTION_PACK_ID,
        id: LibraryApiContractId::RustOptionSomeConstructor,
        callee: LibraryApiCalleeContract::FreeName {
            name,
            shadow: LibraryApiShadowPolicy::ExplicitRoot(shadow.shadow_root),
        },
        result_domain: DomainEvidence::Option,
    })
}

pub fn library_rust_option_none_sentinel_contract(
    lang: Lang,
    name: &str,
) -> Option<LibraryRustOptionSentinelContract> {
    let name = rust_option_none_selector_name(lang, name)?;
    let shadow = rust_option_none_sentinel_contract(lang, name)?;
    Some(LibraryRustOptionSentinelContract {
        pack_id: RUST_STDLIB_OPTION_PACK_ID,
        id: LibraryApiContractId::RustOptionNoneSentinel,
        callee: LibraryApiCalleeContract::FreeName {
            name,
            shadow: LibraryApiShadowPolicy::ExplicitRoot(shadow.shadow_root),
        },
        result_domain: DomainEvidence::Option,
    })
}

pub fn library_rust_result_ok_constructor_contract(
    lang: Lang,
    name: &str,
    arg_count: usize,
) -> Option<LibraryRustResultConstructorContract> {
    if arg_count != 1 {
        return None;
    }
    let name = rust_result_ok_selector_name(lang, name)?;
    let shadow = rust_result_ok_constructor_contract(lang, name)?;
    Some(LibraryRustResultConstructorContract {
        pack_id: RUST_STDLIB_RESULT_PACK_ID,
        id: LibraryApiContractId::RustResultOkConstructor,
        callee: LibraryApiCalleeContract::FreeName {
            name,
            shadow: LibraryApiShadowPolicy::ExplicitRoot(shadow.shadow_root),
        },
        result_domain: DomainEvidence::Result,
    })
}

pub fn library_rust_result_err_constructor_contract(
    lang: Lang,
    name: &str,
    arg_count: usize,
) -> Option<LibraryRustResultConstructorContract> {
    if arg_count != 1 {
        return None;
    }
    let name = rust_result_err_selector_name(lang, name)?;
    let shadow = rust_result_err_constructor_contract(lang, name)?;
    Some(LibraryRustResultConstructorContract {
        pack_id: RUST_STDLIB_RESULT_PACK_ID,
        id: LibraryApiContractId::RustResultErrConstructor,
        callee: LibraryApiCalleeContract::FreeName {
            name,
            shadow: LibraryApiShadowPolicy::ExplicitRoot(shadow.shadow_root),
        },
        result_domain: DomainEvidence::Result,
    })
}

pub fn library_rust_result_predicate_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<LibraryReceiverMethodApiContract> {
    if lang != Lang::Rust || arg_count != 0 {
        return None;
    }
    let (method, id) = match method {
        "is_ok" => ("is_ok", LibraryApiContractId::RustResultIsOk),
        "is_err" => ("is_err", LibraryApiContractId::RustResultIsErr),
        _ => return None,
    };
    Some(LibraryReceiverMethodApiContract {
        pack_id: RUST_STDLIB_RESULT_PACK_ID,
        id,
        callee: LibraryApiCalleeContract::Method {
            method,
            receiver: MethodReceiverContract::ExactResult,
        },
        rule: RUST_STDLIB_RESULT_PRODUCER_ID,
        result_domain: Some(DomainEvidence::Boolean),
    })
}

pub fn library_rust_option_and_then_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<LibraryRustOptionAndThenContract> {
    if lang != Lang::Rust || method != "and_then" || arg_count != 1 {
        return None;
    }
    Some(LibraryRustOptionAndThenContract {
        pack_id: RUST_STDLIB_OPTION_PACK_ID,
        id: LibraryApiContractId::RustOptionAndThen,
        callee: LibraryApiCalleeContract::Method {
            method: "and_then",
            receiver: MethodReceiverContract::RustMapGetOrExactOption,
        },
        result: RustOptionAndThenContract {
            receiver: MethodReceiverContract::RustMapGetOrExactOption,
        },
    })
}
