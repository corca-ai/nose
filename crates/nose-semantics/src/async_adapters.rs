//! Async, iterator-adapter, and Rust option path contracts.

use super::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AsyncReceiverContract {
    ExactPromiseLike,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PromiseFactoryKind {
    Resolve,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PromiseFactoryContract {
    pub receiver: &'static str,
    pub method: &'static str,
    pub qualified_path: &'static str,
    pub kind: PromiseFactoryKind,
    pub result_domain: DomainEvidence,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PromiseThenContract {
    pub receiver: AsyncReceiverContract,
    pub demand: DemandEffectProfile,
}

pub fn promise_then_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<PromiseThenContract> {
    library_promise_then_contract(lang, method, arg_count).map(|contract| contract.result)
}

pub fn promise_resolve_contract(
    lang: Lang,
    receiver: &str,
    method: &str,
    arg_count: usize,
) -> Option<PromiseFactoryContract> {
    library_promise_resolve_contract(lang, receiver, method, arg_count)
        .map(|contract| contract.result)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IteratorAdapterReceiverContract {
    ExactIterableValue,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct IteratorIdentityAdapterContract {
    pub receiver: IteratorAdapterReceiverContract,
}

pub fn iterator_identity_adapter_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<IteratorIdentityAdapterContract> {
    library_iterator_identity_adapter_contract(lang, method, arg_count)
        .map(|contract| contract.result)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StaticCollectionAdapterContract {
    pub module: &'static str,
    pub exported: &'static str,
}

pub fn static_collection_adapter_contract(
    lang: Lang,
    receiver: &str,
    method: &str,
    arg_count: usize,
) -> Option<StaticCollectionAdapterContract> {
    library_static_collection_adapter_contract(lang, receiver, method, arg_count)
        .map(|contract| contract.result)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ShadowedPathContract {
    pub shadow_root: &'static str,
}

pub(super) fn rust_option_some_selector_name(lang: Lang, name: &str) -> Option<&'static str> {
    if lang != Lang::Rust {
        return None;
    }
    Some(match name {
        "Some" => "Some",
        "Option::Some" => "Option::Some",
        "std::option::Option::Some" => "std::option::Option::Some",
        "core::option::Option::Some" => "core::option::Option::Some",
        _ => return None,
    })
}

pub(super) fn rust_option_none_selector_name(lang: Lang, name: &str) -> Option<&'static str> {
    if lang != Lang::Rust {
        return None;
    }
    Some(match name {
        "None" => "None",
        "Option::None" => "Option::None",
        "std::option::Option::None" => "std::option::Option::None",
        "core::option::Option::None" => "core::option::Option::None",
        _ => return None,
    })
}

pub(super) fn rust_result_ok_selector_name(lang: Lang, name: &str) -> Option<&'static str> {
    if lang != Lang::Rust {
        return None;
    }
    Some(match name {
        "Ok" => "Ok",
        "Result::Ok" => "Result::Ok",
        "std::result::Result::Ok" => "std::result::Result::Ok",
        "core::result::Result::Ok" => "core::result::Result::Ok",
        _ => return None,
    })
}

pub(super) fn rust_result_err_selector_name(lang: Lang, name: &str) -> Option<&'static str> {
    if lang != Lang::Rust {
        return None;
    }
    Some(match name {
        "Err" => "Err",
        "Result::Err" => "Result::Err",
        "std::result::Result::Err" => "std::result::Result::Err",
        "core::result::Result::Err" => "core::result::Result::Err",
        _ => return None,
    })
}

pub fn rust_option_some_constructor_contract(
    lang: Lang,
    name: &str,
) -> Option<ShadowedPathContract> {
    if lang != Lang::Rust {
        return None;
    }
    let shadow_root = match name {
        "Some" => "Some",
        "Option::Some" => "Option",
        "std::option::Option::Some" => "std",
        "core::option::Option::Some" => "core",
        _ => return None,
    };
    Some(ShadowedPathContract { shadow_root })
}

pub fn rust_option_none_sentinel_contract(lang: Lang, name: &str) -> Option<ShadowedPathContract> {
    if lang != Lang::Rust {
        return None;
    }
    let shadow_root = match name {
        "None" => "None",
        "Option::None" => "Option",
        "std::option::Option::None" => "std",
        "core::option::Option::None" => "core",
        _ => return None,
    };
    Some(ShadowedPathContract { shadow_root })
}

pub fn rust_result_ok_constructor_contract(lang: Lang, name: &str) -> Option<ShadowedPathContract> {
    if lang != Lang::Rust {
        return None;
    }
    let shadow_root = match name {
        "Ok" => "Ok",
        "Result::Ok" => "Result",
        "std::result::Result::Ok" => "std",
        "core::result::Result::Ok" => "core",
        _ => return None,
    };
    Some(ShadowedPathContract { shadow_root })
}

pub fn rust_result_err_constructor_contract(
    lang: Lang,
    name: &str,
) -> Option<ShadowedPathContract> {
    if lang != Lang::Rust {
        return None;
    }
    let shadow_root = match name {
        "Err" => "Err",
        "Result::Err" => "Result",
        "std::result::Result::Err" => "std",
        "core::result::Result::Err" => "core",
        _ => return None,
    };
    Some(ShadowedPathContract { shadow_root })
}

pub fn rust_vec_new_factory_contract(lang: Lang, name: &str) -> Option<ShadowedPathContract> {
    if lang != Lang::Rust {
        return None;
    }
    let shadow_root = match name {
        "Vec::new" => "Vec",
        "std::vec::Vec::new" => "std",
        "alloc::vec::Vec::new" => "alloc",
        _ => return None,
    };
    Some(ShadowedPathContract { shadow_root })
}

pub fn rust_option_and_then_contract(lang: Lang, method: &str, arg_count: usize) -> bool {
    library_rust_option_and_then_contract(lang, method, arg_count).is_some()
}
