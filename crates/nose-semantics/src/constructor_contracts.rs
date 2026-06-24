//! Collection constructor and factory contracts.

use super::*;
use rustc_hash::FxHashSet;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum JavaCollectionFactoryKind {
    ListOf,
    SetOf,
    ArraysAsList,
    CollectionsEmptyList,
    CollectionsEmptySet,
    CollectionsSingleton,
    CollectionsSingletonList,
    GuavaImmutableListOf,
    GuavaImmutableSetOf,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct JavaCollectionFactoryContract {
    pub module: &'static str,
    pub receiver: &'static str,
    pub method: &'static str,
    pub kind: JavaCollectionFactoryKind,
    pub single_arg_spreads_array: bool,
}

pub fn java_collection_factory_contract(
    lang: Lang,
    receiver: &str,
    method: &str,
) -> Option<JavaCollectionFactoryContract> {
    if lang != Lang::Java {
        return None;
    }
    Some(match (receiver, method) {
        ("List", "of") => JavaCollectionFactoryContract {
            module: "java.util",
            receiver: "List",
            method: "of",
            kind: JavaCollectionFactoryKind::ListOf,
            single_arg_spreads_array: false,
        },
        ("Set", "of") => JavaCollectionFactoryContract {
            module: "java.util",
            receiver: "Set",
            method: "of",
            kind: JavaCollectionFactoryKind::SetOf,
            single_arg_spreads_array: false,
        },
        ("Arrays", "asList") => JavaCollectionFactoryContract {
            module: "java.util",
            receiver: "Arrays",
            method: "asList",
            kind: JavaCollectionFactoryKind::ArraysAsList,
            single_arg_spreads_array: true,
        },
        ("Collections", "emptyList") => JavaCollectionFactoryContract {
            module: "java.util",
            receiver: "Collections",
            method: "emptyList",
            kind: JavaCollectionFactoryKind::CollectionsEmptyList,
            single_arg_spreads_array: false,
        },
        ("Collections", "emptySet") => JavaCollectionFactoryContract {
            module: "java.util",
            receiver: "Collections",
            method: "emptySet",
            kind: JavaCollectionFactoryKind::CollectionsEmptySet,
            single_arg_spreads_array: false,
        },
        ("Collections", "singleton") => JavaCollectionFactoryContract {
            module: "java.util",
            receiver: "Collections",
            method: "singleton",
            kind: JavaCollectionFactoryKind::CollectionsSingleton,
            single_arg_spreads_array: false,
        },
        ("Collections", "singletonList") => JavaCollectionFactoryContract {
            module: "java.util",
            receiver: "Collections",
            method: "singletonList",
            kind: JavaCollectionFactoryKind::CollectionsSingletonList,
            single_arg_spreads_array: false,
        },
        ("ImmutableList", "of") => JavaCollectionFactoryContract {
            module: "com.google.common.collect",
            receiver: "ImmutableList",
            method: "of",
            kind: JavaCollectionFactoryKind::GuavaImmutableListOf,
            single_arg_spreads_array: false,
        },
        ("ImmutableSet", "of") => JavaCollectionFactoryContract {
            module: "com.google.common.collect",
            receiver: "ImmutableSet",
            method: "of",
            kind: JavaCollectionFactoryKind::GuavaImmutableSetOf,
            single_arg_spreads_array: false,
        },
        _ => return None,
    })
}

pub fn java_collection_factory_contract_by_hash(
    lang: Lang,
    receiver: &str,
    method_hash: u64,
) -> Option<JavaCollectionFactoryContract> {
    [
        "of",
        "asList",
        "emptyList",
        "emptySet",
        "singleton",
        "singletonList",
    ]
    .into_iter()
    .find_map(|method| {
        (stable_symbol_hash(method) == method_hash)
            .then(|| java_collection_factory_contract(lang, receiver, method))
            .flatten()
    })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum JavaCollectionConstructorKind {
    EmptyList,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct JavaCollectionConstructorContract {
    pub simple_type: &'static str,
    pub qualified_type: &'static str,
    pub module: &'static str,
    pub kind: JavaCollectionConstructorKind,
    pub requires_import_for_simple_type: bool,
    pub requires_no_local_type_shadow: bool,
}

pub fn java_collection_constructor_contract(
    lang: Lang,
    type_name: &str,
    arg_count: usize,
) -> Option<JavaCollectionConstructorContract> {
    if lang != Lang::Java || arg_count != 0 {
        return None;
    }
    let simple_type = match type_name {
        "ArrayList" | "java.util.ArrayList" => "ArrayList",
        "LinkedList" | "java.util.LinkedList" => "LinkedList",
        _ => return None,
    };
    Some(JavaCollectionConstructorContract {
        simple_type,
        qualified_type: match simple_type {
            "ArrayList" => "java.util.ArrayList",
            "LinkedList" => "java.util.LinkedList",
            _ => return None,
        },
        module: "java.util",
        kind: JavaCollectionConstructorKind::EmptyList,
        requires_import_for_simple_type: true,
        requires_no_local_type_shadow: true,
    })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum JavaMapFactoryKind {
    Of,
    OfEntries,
    CollectionsEmptyMap,
    CollectionsSingletonMap,
    GuavaImmutableMapOf,
}

pub const JAVA_GUAVA_IMMUTABLE_MAP_OF_MAX_ENTRIES: usize = 10;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct JavaMapFactoryContract {
    pub module: &'static str,
    pub receiver: &'static str,
    pub method: &'static str,
    pub kind: JavaMapFactoryKind,
}

pub fn java_map_factory_contract(
    lang: Lang,
    receiver: &str,
    method: &str,
) -> Option<JavaMapFactoryContract> {
    if lang != Lang::Java {
        return None;
    }
    Some(match (receiver, method) {
        ("Map", "of") => JavaMapFactoryContract {
            module: "java.util",
            receiver: "Map",
            method: "of",
            kind: JavaMapFactoryKind::Of,
        },
        ("Map", "ofEntries") => JavaMapFactoryContract {
            module: "java.util",
            receiver: "Map",
            method: "ofEntries",
            kind: JavaMapFactoryKind::OfEntries,
        },
        ("Collections", "emptyMap") => JavaMapFactoryContract {
            module: "java.util",
            receiver: "Collections",
            method: "emptyMap",
            kind: JavaMapFactoryKind::CollectionsEmptyMap,
        },
        ("Collections", "singletonMap") => JavaMapFactoryContract {
            module: "java.util",
            receiver: "Collections",
            method: "singletonMap",
            kind: JavaMapFactoryKind::CollectionsSingletonMap,
        },
        ("ImmutableMap", "of") => JavaMapFactoryContract {
            module: "com.google.common.collect",
            receiver: "ImmutableMap",
            method: "of",
            kind: JavaMapFactoryKind::GuavaImmutableMapOf,
        },
        _ => return None,
    })
}

pub fn java_map_factory_contract_by_hash(
    lang: Lang,
    receiver: &str,
    method_hash: u64,
) -> Option<JavaMapFactoryContract> {
    ["of", "ofEntries", "emptyMap", "singletonMap"]
        .into_iter()
        .find_map(|method| {
            (stable_symbol_hash(method) == method_hash)
                .then(|| java_map_factory_contract(lang, receiver, method))
                .flatten()
        })
}

pub fn java_guava_immutable_map_of_arg_count_supported(arg_count: usize) -> bool {
    arg_count % 2 == 0 && arg_count / 2 <= JAVA_GUAVA_IMMUTABLE_MAP_OF_MAX_ENTRIES
}

pub fn java_map_factory_positional_arg_count_supported(
    kind: JavaMapFactoryKind,
    arg_count: usize,
) -> bool {
    match kind {
        JavaMapFactoryKind::Of => arg_count % 2 == 0,
        JavaMapFactoryKind::CollectionsEmptyMap => arg_count == 0,
        JavaMapFactoryKind::CollectionsSingletonMap => arg_count == 2,
        JavaMapFactoryKind::GuavaImmutableMapOf => {
            java_guava_immutable_map_of_arg_count_supported(arg_count)
        }
        JavaMapFactoryKind::OfEntries => false,
    }
}

pub fn java_map_factory_result_domain_arg_count_supported(
    kind: JavaMapFactoryKind,
    arg_count: usize,
) -> bool {
    match kind {
        JavaMapFactoryKind::Of => arg_count % 2 == 0,
        JavaMapFactoryKind::CollectionsEmptyMap => arg_count == 0,
        JavaMapFactoryKind::CollectionsSingletonMap => arg_count == 2,
        JavaMapFactoryKind::GuavaImmutableMapOf => {
            java_guava_immutable_map_of_arg_count_supported(arg_count)
        }
        JavaMapFactoryKind::OfEntries => true,
    }
}

pub fn java_map_factory_uses_positional_entries(kind: JavaMapFactoryKind) -> bool {
    matches!(
        kind,
        JavaMapFactoryKind::Of
            | JavaMapFactoryKind::CollectionsEmptyMap
            | JavaMapFactoryKind::CollectionsSingletonMap
            | JavaMapFactoryKind::GuavaImmutableMapOf
    )
}

pub fn java_collection_factory_rejects_null_literal(kind: JavaCollectionFactoryKind) -> bool {
    matches!(
        kind,
        JavaCollectionFactoryKind::GuavaImmutableListOf
            | JavaCollectionFactoryKind::GuavaImmutableSetOf
    )
}

pub fn node_is_static_null_literal(il: &Il, node: NodeId) -> bool {
    matches!(
        (il.kind(node), il.node(node).payload),
        (NodeKind::Lit, Payload::Lit(LitClass::Null))
    )
}

pub fn nodes_contain_static_null_literal(il: &Il, nodes: impl IntoIterator<Item = NodeId>) -> bool {
    nodes
        .into_iter()
        .any(|node| node_is_static_null_literal(il, node))
}

pub fn nodes_contain_duplicate_static_literal_keys(
    il: &Il,
    nodes: impl IntoIterator<Item = NodeId>,
) -> bool {
    let mut seen = FxHashSet::default();
    for node in nodes {
        let Some(key) = static_literal_key(il, node) else {
            continue;
        };
        if !seen.insert(key) {
            return true;
        }
    }
    false
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum StaticLiteralKey {
    LiteralClass(LitClass),
    Int(i64),
    Bool(bool),
    Str(u64),
    Float(u64),
}

fn static_literal_key(il: &Il, node: NodeId) -> Option<StaticLiteralKey> {
    if il.kind(node) != NodeKind::Lit {
        return None;
    }
    match il.node(node).payload {
        Payload::Lit(class) => Some(StaticLiteralKey::LiteralClass(class)),
        Payload::LitInt(value) => Some(StaticLiteralKey::Int(value)),
        Payload::LitBool(value) => Some(StaticLiteralKey::Bool(value)),
        Payload::LitStr(value) => Some(StaticLiteralKey::Str(value)),
        Payload::LitFloat(value) => Some(StaticLiteralKey::Float(value)),
        _ => None,
    }
}

pub fn java_map_entry_contract(lang: Lang, receiver: &str, method: &str) -> bool {
    lang == Lang::Java && receiver == "Map" && method == "entry"
}

pub fn java_map_entry_contract_by_hash(lang: Lang, receiver: &str, method_hash: u64) -> bool {
    java_map_entry_contract(lang, receiver, "entry") && method_hash == stable_symbol_hash("entry")
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RubySetFactoryContract {
    pub receiver: &'static str,
    pub method: &'static str,
    pub required_module: &'static str,
    pub shadow_root: &'static str,
}

pub fn ruby_set_factory_contract(
    lang: Lang,
    receiver: &str,
    method: &str,
    arg_count: usize,
) -> Option<RubySetFactoryContract> {
    (lang == Lang::Ruby && receiver == "Set" && method == "new" && arg_count == 1).then_some(
        RubySetFactoryContract {
            receiver: "Set",
            method: "new",
            required_module: "set",
            shadow_root: "Set",
        },
    )
}

pub fn ruby_set_factory_contract_by_hash(
    lang: Lang,
    receiver: &str,
    method_hash: u64,
    arg_count: usize,
) -> Option<RubySetFactoryContract> {
    (method_hash == stable_symbol_hash("new"))
        .then(|| ruby_set_factory_contract(lang, receiver, "new", arg_count))
        .flatten()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConstructorProofRequirement {
    ConstructSyntax,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ClosedConstructorContract {
    pub receiver: &'static str,
    pub required_proof: ConstructorProofRequirement,
    pub requires_unshadowed_global: bool,
    pub entry_seq_tag: Option<u64>,
}

pub fn js_like_set_constructor_contract(
    lang: Lang,
    receiver: &str,
) -> Option<ClosedConstructorContract> {
    (js_like_lang(lang) && receiver == "Set").then_some(ClosedConstructorContract {
        receiver: "Set",
        required_proof: ConstructorProofRequirement::ConstructSyntax,
        requires_unshadowed_global: true,
        entry_seq_tag: None,
    })
}

pub fn js_like_map_constructor_contract(
    lang: Lang,
    receiver: &str,
) -> Option<ClosedConstructorContract> {
    (js_like_lang(lang) && receiver == "Map").then_some(ClosedConstructorContract {
        receiver: "Map",
        required_proof: ConstructorProofRequirement::ConstructSyntax,
        requires_unshadowed_global: true,
        entry_seq_tag: Some(SEQ_VALUE_COLLECTION),
    })
}
