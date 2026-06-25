use super::*;

pub(in crate::library_api) fn js_like_array_hof_method_call(
    lang: Lang,
    contract: MethodCallContract,
) -> bool {
    js_like_lang(lang)
        && matches!(
            (contract.semantic, contract.receiver, contract.args),
            (
                MethodSemanticContract::HoF(HoFKind::Map | HoFKind::Filter | HoFKind::FlatMap),
                MethodReceiverContract::ExactArray,
                MethodBuiltinArgs::Hof,
            ) | (
                MethodSemanticContract::Builtin(Builtin::Any | Builtin::All),
                MethodReceiverContract::ExactArray,
                MethodBuiltinArgs::BoolReduction,
            )
        )
}

pub(super) fn rust_sequence_hof_method_call(lang: Lang, contract: MethodCallContract) -> bool {
    lang == Lang::Rust
        && matches!(
            (contract.semantic, contract.receiver, contract.args),
            (
                MethodSemanticContract::HoF(
                    HoFKind::Map | HoFKind::Filter | HoFKind::FilterMap | HoFKind::FlatMap,
                ),
                MethodReceiverContract::ExactProtocol,
                MethodBuiltinArgs::Hof,
            ) | (
                MethodSemanticContract::Builtin(Builtin::Any | Builtin::All),
                MethodReceiverContract::ExactProtocol,
                MethodBuiltinArgs::BoolReduction,
            ) | (
                MethodSemanticContract::Builtin(Builtin::Len),
                MethodReceiverContract::ExactProtocol,
                MethodBuiltinArgs::CollectionReduction,
            )
        )
}
