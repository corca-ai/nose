//! Receiver-method semantic contracts.

use super::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MethodReceiverContract {
    ExactArray,
    ExactArrayOrCollection,
    ExactCollection,
    ExactProtocol,
    ExactProtocolPairArgument,
    ExactOption,
    ExactResult,
    ExactString,
    ExactInteger,
    ExactMap,
    ExactMapLiteral,
    ExactCollectionOrMap,
    ExactCollectionOrMapLiteral,
    ExactCollectionOrJavaKeySet,
    ExactSetOrMap,
    LiteralString,
    UnshadowedGlobal(&'static str),
    ImportedNamespace(&'static str),
    RustMapGetOrExactOption,
}

pub fn method_receiver_domain_requirement(
    receiver: MethodReceiverContract,
) -> Option<DomainRequirement> {
    match receiver {
        MethodReceiverContract::ExactArray => Some(DomainRequirement::ARRAY),
        MethodReceiverContract::ExactArrayOrCollection => {
            Some(DomainRequirement::ARRAY_OR_COLLECTION)
        }
        MethodReceiverContract::ExactCollection
        | MethodReceiverContract::ExactProtocol
        | MethodReceiverContract::ExactProtocolPairArgument
        | MethodReceiverContract::ExactCollectionOrJavaKeySet => {
            Some(DomainRequirement::ARRAY_COLLECTION_OR_SET)
        }
        MethodReceiverContract::ExactOption | MethodReceiverContract::RustMapGetOrExactOption => {
            Some(DomainRequirement::OPTION)
        }
        MethodReceiverContract::ExactResult => Some(DomainRequirement::RESULT),
        MethodReceiverContract::ExactString | MethodReceiverContract::LiteralString => {
            Some(DomainRequirement::STRING)
        }
        MethodReceiverContract::ExactInteger => Some(DomainRequirement::INTEGER),
        MethodReceiverContract::ExactMap => Some(DomainRequirement::MAP),
        MethodReceiverContract::ExactCollectionOrMap
        | MethodReceiverContract::ExactCollectionOrMapLiteral => {
            Some(DomainRequirement::COLLECTION_OR_MAP)
        }
        MethodReceiverContract::ExactSetOrMap => Some(DomainRequirement::SET_OR_MAP),
        MethodReceiverContract::ExactMapLiteral
        | MethodReceiverContract::UnshadowedGlobal(_)
        | MethodReceiverContract::ImportedNamespace(_) => None,
    }
}

pub fn receiver_satisfies_method_domain(
    il: &Il,
    interner: &Interner,
    receiver: NodeId,
    contract: MethodReceiverContract,
) -> bool {
    method_receiver_domain_requirement(contract)
        .is_some_and(|requirement| receiver_satisfies_domain(il, interner, receiver, requirement))
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MethodBuiltinArgs {
    All,
    First,
    ReceiverOnly,
    ReceiverThenAll,
    ReceiverAndFirst,
    FirstThenReceiver,
    GoSliceContains,
    MapGetDefault,
    MapGetDefaultOrZeroArgLambda,
    RustMapGetOrOptionDefault,
    RustOptionDefaultLambda,
    RustOptionMapOrIdentity,
    RustZip,
    Fold,
    BoolReduction,
    Hof,
    CollectionReduction,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MethodSemanticContract {
    Builtin(Builtin),
    HoF(HoFKind),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MethodCallContract {
    pub semantic: MethodSemanticContract,
    pub receiver: MethodReceiverContract,
    pub args: MethodBuiltinArgs,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ScalarIntegerMethod {
    Abs,
    Min,
    Max,
    Clamp,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ScalarIntegerMethodContract {
    pub semantic: ScalarIntegerMethod,
    pub receiver: MethodReceiverContract,
}

pub(super) fn scalar_integer_method_contract_shape(
    lang: Lang,
    name: &str,
    arg_count: usize,
) -> Option<ScalarIntegerMethodContract> {
    use MethodReceiverContract as Receiver;
    use ScalarIntegerMethod as Method;

    let (semantic, receiver) = match (lang, name, arg_count) {
        (Lang::Rust, "abs", 0) => (Method::Abs, Receiver::ExactInteger),
        (Lang::Rust, "min", 1) => (Method::Min, Receiver::ExactInteger),
        (Lang::Rust, "max", 1) => (Method::Max, Receiver::ExactInteger),
        (Lang::Rust, "clamp", 2) => (Method::Clamp, Receiver::ExactInteger),
        (Lang::Java, "abs", 1) => (Method::Abs, Receiver::UnshadowedGlobal("Math")),
        (Lang::Java, "min", 2) => (Method::Min, Receiver::UnshadowedGlobal("Math")),
        (Lang::Java, "max", 2) => (Method::Max, Receiver::UnshadowedGlobal("Math")),
        _ => return None,
    };
    Some(ScalarIntegerMethodContract { semantic, receiver })
}

pub fn scalar_integer_method_contract(
    lang: Lang,
    name: &str,
    arg_count: usize,
) -> Option<ScalarIntegerMethodContract> {
    library_scalar_integer_method_contract(lang, name, arg_count).map(|contract| contract.result)
}

pub fn method_call_contract(
    lang: Lang,
    name: &str,
    arg_count: usize,
) -> Option<MethodCallContract> {
    library_method_call_contract(lang, name, arg_count).map(|contract| contract.result)
}

pub fn method_call_contracts(lang: Lang, name: &str, arg_count: usize) -> Vec<MethodCallContract> {
    library_method_call_contracts(lang, name, arg_count)
        .into_iter()
        .map(|contract| contract.result)
        .collect()
}

type MethodBuiltinShape = (Builtin, MethodReceiverContract, MethodBuiltinArgs);

pub(super) fn method_call_contract_shapes(
    lang: Lang,
    name: &str,
    arg_count: usize,
) -> Vec<MethodCallContract> {
    use MethodBuiltinArgs as Args;
    use MethodReceiverContract as Receiver;
    use MethodSemanticContract as Semantic;

    let shaped = method_append_contract_shape(lang, name, arg_count)
        .or_else(|| method_namespace_call_contract_shape(lang, name, arg_count))
        .or_else(|| method_cardinality_contract_shape(lang, name, arg_count))
        .or_else(|| method_string_affix_contract_shape(lang, name, arg_count))
        .or_else(|| method_membership_contract_shape(lang, name, arg_count))
        .or_else(|| method_lookup_default_contract_shape(lang, name, arg_count))
        .or_else(|| method_numeric_contract_shape(lang, name, arg_count));
    let contract = if let Some(contract) = shaped {
        contract
    } else if method_fold_name(lang, name) && arg_count > 0 {
        (Builtin::Reduce, Receiver::ExactProtocol, Args::Fold)
    } else if js_like_lang(lang)
        && method_bool_reduction_builtin(lang, name).is_some()
        && arg_count == 1
    {
        (
            method_bool_reduction_builtin(lang, name).unwrap(),
            Receiver::ExactArray,
            Args::BoolReduction,
        )
    } else if !js_like_lang(lang)
        && method_bool_reduction_builtin(lang, name).is_some()
        && arg_count > 0
    {
        (
            method_bool_reduction_builtin(lang, name).unwrap(),
            Receiver::ExactProtocol,
            Args::BoolReduction,
        )
    } else if method_collection_reduction_builtin(lang, name).is_some() && arg_count == 0 {
        (
            method_collection_reduction_builtin(lang, name).unwrap(),
            Receiver::ExactProtocol,
            Args::CollectionReduction,
        )
    } else if js_like_lang(lang) && method_hof_contract(lang, name).is_some() && arg_count == 1 {
        return vec![MethodCallContract {
            semantic: Semantic::HoF(method_hof_contract(lang, name).unwrap()),
            receiver: Receiver::ExactArray,
            args: Args::Hof,
        }];
    } else if lang == Lang::Swift
        && matches!(
            method_hof_contract(lang, name),
            Some(HoFKind::Map | HoFKind::Filter | HoFKind::FlatMap)
        )
        && arg_count == 1
    {
        return vec![MethodCallContract {
            semantic: Semantic::HoF(method_hof_contract(lang, name).unwrap()),
            receiver: Receiver::ExactArrayOrCollection,
            args: Args::Hof,
        }];
    } else if !js_like_lang(lang) && method_hof_contract(lang, name).is_some() && arg_count > 0 {
        return vec![MethodCallContract {
            semantic: Semantic::HoF(method_hof_contract(lang, name).unwrap()),
            receiver: Receiver::ExactProtocol,
            args: Args::Hof,
        }];
    } else if matches!((lang, name, arg_count), (Lang::Rust, "abs", 0)) {
        (Builtin::Abs, Receiver::ExactInteger, Args::ReceiverOnly)
    } else {
        return Vec::new();
    };

    let mut contracts = vec![MethodCallContract {
        semantic: Semantic::Builtin(contract.0),
        receiver: contract.1,
        args: contract.2,
    }];
    if matches!((lang, name, arg_count), (Lang::Go, "Contains", 2)) {
        contracts.push(MethodCallContract {
            semantic: Semantic::Builtin(Builtin::StringContains),
            receiver: Receiver::ImportedNamespace("strings"),
            args: Args::All,
        });
    }
    contracts
}

pub(super) fn method_append_contract_shape(
    lang: Lang,
    name: &str,
    arg_count: usize,
) -> Option<MethodBuiltinShape> {
    use MethodBuiltinArgs as Args;
    use MethodReceiverContract as Receiver;

    let contract = match (lang, name, arg_count) {
        (Lang::Python, "append", 1) => (
            Builtin::Append,
            Receiver::ExactCollection,
            Args::ReceiverThenAll,
        ),
        (
            Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html,
            "push",
            1..,
        ) => (
            Builtin::Append,
            Receiver::ExactCollection,
            Args::ReceiverThenAll,
        ),
        (Lang::Java, "add", 1) | (Lang::Rust, "push", 1) => (
            Builtin::Append,
            Receiver::ExactCollection,
            Args::ReceiverThenAll,
        ),
        (Lang::Swift, "append", 1) => (
            Builtin::Append,
            Receiver::ExactCollection,
            Args::ReceiverThenAll,
        ),
        _ => return None,
    };
    Some(contract)
}

pub(super) fn method_namespace_call_contract_shape(
    lang: Lang,
    name: &str,
    arg_count: usize,
) -> Option<MethodBuiltinShape> {
    use MethodBuiltinArgs as Args;
    use MethodReceiverContract as Receiver;

    let contract = match (lang, name, arg_count) {
        (
            Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html,
            "log" | "info" | "debug",
            _,
        ) => (
            Builtin::Print,
            Receiver::UnshadowedGlobal("console"),
            Args::All,
        ),
        (Lang::Go, "Println" | "Printf" | "Print", _) => (
            Builtin::Print,
            Receiver::ImportedNamespace("fmt"),
            Args::All,
        ),
        // Go math.Abs is a float64 API and differs from the ternary abs idiom on -0.0.
        // Keep it closed until a signed-zero-aware float model exists.
        (Lang::Go, "HasPrefix", 2) => (
            Builtin::StartsWith,
            Receiver::ImportedNamespace("strings"),
            Args::All,
        ),
        (Lang::Go, "HasSuffix", 2) => (
            Builtin::EndsWith,
            Receiver::ImportedNamespace("strings"),
            Args::All,
        ),
        (Lang::Go, "Contains", 2) => (
            Builtin::Contains,
            Receiver::ImportedNamespace("slices"),
            Args::GoSliceContains,
        ),
        _ => return None,
    };
    Some(contract)
}

pub(super) fn method_cardinality_contract_shape(
    lang: Lang,
    name: &str,
    arg_count: usize,
) -> Option<MethodBuiltinShape> {
    use MethodBuiltinArgs as Args;
    use MethodReceiverContract as Receiver;

    let contract = match (lang, name, arg_count) {
        (Lang::Rust, "len", 0) | (Lang::Java, "size", 0) | (Lang::Swift, "count", 0) => {
            (Builtin::Len, Receiver::ExactCollection, Args::ReceiverOnly)
        }
        (Lang::Rust, "is_empty", 0)
        | (Lang::Java, "isEmpty", 0)
        | (Lang::Ruby, "empty?", 0)
        | (Lang::Swift, "isEmpty", 0) => (
            Builtin::IsEmpty,
            Receiver::ExactCollection,
            Args::ReceiverOnly,
        ),
        (Lang::Ruby, "nil?", 0) | (Lang::Rust, "is_none", 0) => {
            (Builtin::IsNull, Receiver::ExactOption, Args::ReceiverOnly)
        }
        (Lang::Rust, "is_some", 0) => (
            Builtin::IsNotNull,
            Receiver::RustMapGetOrExactOption,
            Args::ReceiverOnly,
        ),
        _ => return None,
    };
    Some(contract)
}

pub(super) fn method_string_affix_contract_shape(
    lang: Lang,
    name: &str,
    arg_count: usize,
) -> Option<MethodBuiltinShape> {
    use MethodBuiltinArgs as Args;
    use MethodReceiverContract as Receiver;

    let contract = match (lang, name, arg_count) {
        (
            Lang::JavaScript
            | Lang::TypeScript
            | Lang::Vue
            | Lang::Svelte
            | Lang::Html
            | Lang::Java,
            "startsWith",
            1,
        )
        | (Lang::Python, "startswith", 1)
        | (Lang::Rust, "starts_with", 1)
        | (Lang::Ruby, "start_with?", 1)
        | (Lang::Swift, "hasPrefix", 1) => (
            Builtin::StartsWith,
            Receiver::ExactString,
            Args::ReceiverAndFirst,
        ),
        (
            Lang::JavaScript
            | Lang::TypeScript
            | Lang::Vue
            | Lang::Svelte
            | Lang::Html
            | Lang::Java,
            "endsWith",
            1,
        )
        | (Lang::Python, "endswith", 1)
        | (Lang::Rust, "ends_with", 1)
        | (Lang::Ruby, "end_with?", 1)
        | (Lang::Swift, "hasSuffix", 1) => (
            Builtin::EndsWith,
            Receiver::ExactString,
            Args::ReceiverAndFirst,
        ),
        _ => return None,
    };
    Some(contract)
}

pub(super) fn method_membership_contract_shape(
    lang: Lang,
    name: &str,
    arg_count: usize,
) -> Option<MethodBuiltinShape> {
    use MethodBuiltinArgs as Args;
    use MethodReceiverContract as Receiver;

    let contract = match (lang, name, arg_count) {
        (Lang::Java, "containsKey", 1)
        | (Lang::Rust, "contains_key", 1)
        | (Lang::Ruby, "key?" | "has_key?", 1) => (
            Builtin::Contains,
            Receiver::ExactMap,
            Args::FirstThenReceiver,
        ),
        (Lang::Python, "__contains__", 1) => (
            Builtin::Contains,
            Receiver::ExactCollectionOrMap,
            Args::FirstThenReceiver,
        ),
        (
            Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html,
            "includes",
            1,
        )
        | (Lang::Ruby, "include?" | "member?", 1)
        | (Lang::Java | Lang::Rust | Lang::Swift, "contains", 1) => (
            Builtin::Contains,
            Receiver::ExactCollectionOrJavaKeySet,
            Args::FirstThenReceiver,
        ),
        (Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html, "has", 1) => {
            (
                Builtin::Contains,
                Receiver::ExactSetOrMap,
                Args::FirstThenReceiver,
            )
        }
        _ => return None,
    };
    Some(contract)
}

pub(super) fn method_lookup_default_contract_shape(
    lang: Lang,
    name: &str,
    arg_count: usize,
) -> Option<MethodBuiltinShape> {
    use MethodBuiltinArgs as Args;
    use MethodReceiverContract as Receiver;

    let contract = match (lang, name, arg_count) {
        (Lang::Python, "join", 1) => (
            Builtin::Join,
            Receiver::LiteralString,
            Args::ReceiverAndFirst,
        ),
        (Lang::Python, "get", 2) => (
            Builtin::GetOrDefault,
            Receiver::ExactMap,
            Args::MapGetDefault,
        ),
        (Lang::Ruby, "fetch", 2) => (
            Builtin::GetOrDefault,
            Receiver::ExactMap,
            Args::MapGetDefaultOrZeroArgLambda,
        ),
        (Lang::Java, "getOrDefault", 2) => (
            Builtin::GetOrDefault,
            Receiver::ExactMap,
            Args::MapGetDefault,
        ),
        (Lang::Rust, "unwrap_or", 1) => (
            Builtin::ValueOrDefault,
            Receiver::RustMapGetOrExactOption,
            Args::RustMapGetOrOptionDefault,
        ),
        (Lang::Rust, "unwrap_or_else", 1) => (
            Builtin::ValueOrDefault,
            Receiver::ExactOption,
            Args::RustOptionDefaultLambda,
        ),
        (Lang::Rust, "map_or", 2) => (
            Builtin::ValueOrDefault,
            Receiver::ExactOption,
            Args::RustOptionMapOrIdentity,
        ),
        _ => return None,
    };
    Some(contract)
}

pub(super) fn method_numeric_contract_shape(
    lang: Lang,
    name: &str,
    arg_count: usize,
) -> Option<MethodBuiltinShape> {
    use MethodBuiltinArgs as Args;
    use MethodReceiverContract as Receiver;

    let contract = match (lang, name, arg_count) {
        (Lang::Python, "reduce", 2..) => (
            Builtin::Reduce,
            Receiver::ImportedNamespace("functools"),
            Args::All,
        ),
        // Go math.Min/math.Max are float64 APIs and return NaN if either argument is NaN.
        // Keep them closed until a NaN-aware float model exists. Java Math.abs/min/max
        // use scalar-integer method contracts instead, so only integer-overload calls
        // lower to the modeled Abs/Min/Max value nodes.
        // JS-family Math.abs differs from `x >= 0 ? x : -x` on -0.
        // Keep it closed until a signed-zero-aware numeric model exists.
        // JS-family Math.min/Math.max return NaN if any argument is NaN. The value
        // graph's Min/Max nodes model the ternary selection idiom instead, so these
        // calls stay ordinary calls until a NaN-aware numeric model exists.
        (Lang::Rust, "zip", 1) => (
            Builtin::Zip,
            Receiver::ExactProtocolPairArgument,
            Args::RustZip,
        ),
        _ => return None,
    };
    Some(contract)
}
