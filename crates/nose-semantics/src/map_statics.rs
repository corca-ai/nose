//! Map views, static/global APIs, regex, and index-membership contracts.

use super::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MapKeyViewKind {
    Collection,
    Iterator,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MapKeyViewContract {
    pub method: &'static str,
    pub kind: MapKeyViewKind,
}

pub fn map_key_view_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<MapKeyViewContract> {
    library_map_key_view_contract(lang, method, arg_count).map(|contract| contract.result)
}

pub fn map_key_view_contract_by_hash(
    lang: Lang,
    method_hash: u64,
    arg_count: usize,
) -> Option<MapKeyViewContract> {
    ["keys", "keySet"].into_iter().find_map(|method| {
        (stable_symbol_hash(method) == method_hash)
            .then(|| map_key_view_contract(lang, method, arg_count))
            .flatten()
    })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MapKeyViewWrapperContract {
    pub receiver: &'static str,
    pub method: &'static str,
    pub qualified_path: &'static str,
}

pub fn map_key_view_wrapper_contract(
    lang: Lang,
    receiver: &str,
    method: &str,
    arg_count: usize,
) -> Option<MapKeyViewWrapperContract> {
    library_map_key_view_wrapper_contract(lang, receiver, method, arg_count)
        .map(|contract| contract.result)
}

pub fn map_key_view_wrapper_contract_by_hash(
    lang: Lang,
    receiver: &str,
    method_hash: u64,
    arg_count: usize,
) -> Option<MapKeyViewWrapperContract> {
    (method_hash == stable_symbol_hash("from"))
        .then(|| map_key_view_wrapper_contract(lang, receiver, "from", arg_count))
        .flatten()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GoZeroMapLookupContract {
    pub map_literal_tag: &'static str,
    pub entry_tag: &'static str,
    pub canonical_value_tag: &'static str,
}

pub fn go_zero_map_lookup_contract(lang: Lang) -> Option<GoZeroMapLookupContract> {
    (lang == Lang::Go).then_some(GoZeroMapLookupContract {
        map_literal_tag: "composite_literal",
        entry_tag: "keyed_element",
        canonical_value_tag: "go_literal_zero_map",
    })
}

pub fn go_zero_map_literal_contract_for_node(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> Option<GoZeroMapLookupContract> {
    let contract = go_zero_map_lookup_contract(il.meta.lang)?;
    sequence_surface_evidence_matches_node(
        il,
        interner,
        node,
        SequenceSurfaceKind::GoCompositeMapLiteral,
    )
    .then_some(contract)
}

pub fn go_zero_map_entry_contract_for_node(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> Option<GoZeroMapLookupContract> {
    let contract = go_zero_map_lookup_contract(il.meta.lang)?;
    sequence_surface_evidence_matches_node(il, interner, node, SequenceSurfaceKind::GoMapEntry)
        .then_some(contract)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GoZeroMapDefaultKind {
    Int,
    String,
    Bool,
    Float,
    Null,
}

pub fn go_zero_map_default_kind(lang: Lang, payload: Payload) -> Option<GoZeroMapDefaultKind> {
    if lang != Lang::Go {
        return None;
    }
    Some(match payload {
        Payload::LitInt(_) => GoZeroMapDefaultKind::Int,
        Payload::LitStr(_) => GoZeroMapDefaultKind::String,
        Payload::LitBool(_) => GoZeroMapDefaultKind::Bool,
        Payload::LitFloat(_) => GoZeroMapDefaultKind::Float,
        Payload::Lit(LitClass::Null) => GoZeroMapDefaultKind::Null,
        _ => return None,
    })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MapGetContract {
    pub method: &'static str,
    pub receiver: MethodReceiverContract,
}

pub fn map_get_contract(lang: Lang, method: &str, arg_count: usize) -> Option<MapGetContract> {
    library_map_get_contract(lang, method, arg_count).map(|contract| contract.result)
}

pub fn map_get_contract_by_hash(
    lang: Lang,
    method_hash: u64,
    arg_count: usize,
) -> Option<MapGetContract> {
    (method_hash == stable_symbol_hash("get"))
        .then(|| map_get_contract(lang, "get", arg_count))
        .flatten()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TypeofOperatorContract {
    pub name: &'static str,
    pub required_source_fact: SourceFactKind,
}

pub fn typeof_operator_contract(
    lang: Lang,
    name: &str,
    arg_count: usize,
) -> Option<TypeofOperatorContract> {
    (js_like_lang(lang) && name == "typeof" && arg_count == 1).then_some(TypeofOperatorContract {
        name: "typeof",
        required_source_fact: SourceFactKind::Operator(SourceOperatorKind::Typeof),
    })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StaticGlobalMethodContract {
    pub receiver: &'static str,
    pub method: &'static str,
    pub qualified_path: &'static str,
    pub requires_unshadowed_receiver: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StaticGlobalFunctionContract {
    pub function: &'static str,
    pub requires_unshadowed_function: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StaticGlobalSymbolContract {
    pub name: &'static str,
    pub requires_unshadowed: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct QualifiedGlobalSymbolContract {
    pub path: &'static str,
    pub root: &'static str,
    pub requires_unshadowed_root: bool,
}

pub fn static_global_symbol_contract(lang: Lang, name: &str) -> Option<StaticGlobalSymbolContract> {
    if !js_like_lang(lang) {
        return None;
    }
    let name = match name {
        "Array" => "Array",
        "Boolean" => "Boolean",
        "Map" => "Map",
        "Math" => "Math",
        "Object" => "Object",
        "Promise" => "Promise",
        "Set" => "Set",
        "console" => "console",
        "undefined" => "undefined",
        _ => return None,
    };
    Some(StaticGlobalSymbolContract {
        name,
        requires_unshadowed: true,
    })
}

pub fn qualified_global_symbol_contract(
    lang: Lang,
    path: &str,
) -> Option<QualifiedGlobalSymbolContract> {
    if !js_like_lang(lang) {
        return None;
    }
    let (path, root) = match path {
        "Array.from" => ("Array.from", "Array"),
        "Array.isArray" => ("Array.isArray", "Array"),
        "Object.hasOwn" => ("Object.hasOwn", "Object"),
        "Object.keys" => ("Object.keys", "Object"),
        "Object.prototype.hasOwnProperty.call" => {
            ("Object.prototype.hasOwnProperty.call", "Object")
        }
        "Promise.resolve" => ("Promise.resolve", "Promise"),
        _ => return None,
    };
    Some(QualifiedGlobalSymbolContract {
        path,
        root,
        requires_unshadowed_root: true,
    })
}

pub fn js_boolean_coercion_contract(
    lang: Lang,
    function: &str,
    arg_count: usize,
) -> Option<StaticGlobalFunctionContract> {
    library_js_boolean_coercion_contract(lang, function, arg_count).map(|contract| contract.result)
}

pub fn js_array_is_array_contract(
    lang: Lang,
    receiver: &str,
    method: &str,
    arg_count: usize,
) -> Option<StaticGlobalMethodContract> {
    library_js_array_is_array_contract(lang, receiver, method, arg_count)
        .map(|contract| contract.result)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RegexTestContract {
    pub method: &'static str,
    pub required_receiver_fact: SourceFactKind,
}

pub fn regex_test_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<RegexTestContract> {
    library_regex_test_contract(lang, method, arg_count).map(|contract| contract.result)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StaticIndexMembershipKind {
    IndexOf,
    FindIndex,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StaticIndexMembershipReceiverContract {
    StaticNonFloatLiteralCollection,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StaticIndexMembershipContract {
    pub method: &'static str,
    pub kind: StaticIndexMembershipKind,
    pub receiver: StaticIndexMembershipReceiverContract,
}

pub fn static_index_membership_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<StaticIndexMembershipContract> {
    if !js_like_lang(lang) || arg_count != 1 {
        return None;
    }
    Some(match method {
        "indexOf" => StaticIndexMembershipContract {
            method: "indexOf",
            kind: StaticIndexMembershipKind::IndexOf,
            receiver: StaticIndexMembershipReceiverContract::StaticNonFloatLiteralCollection,
        },
        "findIndex" => StaticIndexMembershipContract {
            method: "findIndex",
            kind: StaticIndexMembershipKind::FindIndex,
            receiver: StaticIndexMembershipReceiverContract::StaticNonFloatLiteralCollection,
        },
        _ => return None,
    })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ImportedNamespaceFunctionSemantic {
    ProductReduction { op: Op, identity: u32 },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ImportedNamespaceFunctionContract {
    pub module: &'static str,
    pub function: &'static str,
    pub receiver: MethodReceiverContract,
    pub semantic: ImportedNamespaceFunctionSemantic,
}

pub fn imported_namespace_function_contract(
    lang: Lang,
    function: &str,
    arg_count: usize,
) -> Option<ImportedNamespaceFunctionContract> {
    library_imported_namespace_function_contract(lang, function, arg_count)
        .map(|contract| contract.result)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NullishGlobalContract {
    pub name: &'static str,
    pub requires_unshadowed: bool,
}

pub fn nullish_global_contract(lang: Lang, name: &str) -> Option<NullishGlobalContract> {
    (js_like_lang(lang) && name == "undefined").then_some(NullishGlobalContract {
        name: "undefined",
        requires_unshadowed: true,
    })
}
