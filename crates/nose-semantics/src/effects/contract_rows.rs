//! First-party effect contract row tables.

use super::*;
use crate::builtin_language_pack_id;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct MethodEffectContractSet {
    id: MethodEffectContractId,
    lang: Lang,
    methods: &'static [&'static str],
    arity: MethodEffectArity,
    receiver: MethodEffectReceiverContract,
    effect: EffectEvidenceKind,
    channel: ChannelEligibility,
}

const BUILDER_APPEND_METHOD_EFFECTS: &[MethodEffectContractSet] = &[
    MethodEffectContractSet {
        id: MethodEffectContractId::ExactBuilderAppendCall,
        lang: Lang::Python,
        methods: &["append"],
        arity: MethodEffectArity::Exact(1),
        receiver: MethodEffectReceiverContract::ActiveCollectionBuilder,
        effect: EffectEvidenceKind::BuilderAppendCall,
        channel: ChannelEligibility::ExactProven,
    },
    MethodEffectContractSet {
        id: MethodEffectContractId::ExactBuilderAppendCall,
        lang: Lang::JavaScript,
        methods: &["push"],
        arity: MethodEffectArity::Exact(1),
        receiver: MethodEffectReceiverContract::ActiveCollectionBuilder,
        effect: EffectEvidenceKind::BuilderAppendCall,
        channel: ChannelEligibility::ExactProven,
    },
    MethodEffectContractSet {
        id: MethodEffectContractId::ExactBuilderAppendCall,
        lang: Lang::Java,
        methods: &["add"],
        arity: MethodEffectArity::Exact(1),
        receiver: MethodEffectReceiverContract::ActiveCollectionBuilder,
        effect: EffectEvidenceKind::BuilderAppendCall,
        channel: ChannelEligibility::ExactProven,
    },
    MethodEffectContractSet {
        id: MethodEffectContractId::ExactBuilderAppendCall,
        lang: Lang::Rust,
        methods: &["push"],
        arity: MethodEffectArity::Exact(1),
        receiver: MethodEffectReceiverContract::ActiveCollectionBuilder,
        effect: EffectEvidenceKind::BuilderAppendCall,
        channel: ChannelEligibility::ExactProven,
    },
];

const RECEIVER_MUTATION_METHOD_EFFECTS: &[MethodEffectContractSet] = &[
    MethodEffectContractSet {
        id: MethodEffectContractId::ReceiverMutationRisk,
        lang: Lang::JavaScript,
        methods: &[
            "add",
            "clear",
            "copyWithin",
            "delete",
            "fill",
            "pop",
            "push",
            "reverse",
            "set",
            "shift",
            "sort",
            "splice",
            "unshift",
        ],
        arity: MethodEffectArity::Any,
        receiver: MethodEffectReceiverContract::PotentiallyMutableReceiver,
        effect: EffectEvidenceKind::ReceiverMutation,
        channel: ChannelEligibility::ExactProven,
    },
    MethodEffectContractSet {
        id: MethodEffectContractId::ReceiverMutationRisk,
        lang: Lang::Python,
        methods: &[
            "add",
            "append",
            "clear",
            "extend",
            "insert",
            "pop",
            "remove",
            "reverse",
            "setdefault",
            "sort",
            "update",
        ],
        arity: MethodEffectArity::Any,
        receiver: MethodEffectReceiverContract::PotentiallyMutableReceiver,
        effect: EffectEvidenceKind::ReceiverMutation,
        channel: ChannelEligibility::ExactProven,
    },
    MethodEffectContractSet {
        id: MethodEffectContractId::ReceiverMutationRisk,
        lang: Lang::Ruby,
        methods: &[
            "add", "append", "clear", "delete", "merge!", "pop", "push", "reverse!", "shift",
            "sort!", "store", "unshift", "update",
        ],
        arity: MethodEffectArity::Any,
        receiver: MethodEffectReceiverContract::PotentiallyMutableReceiver,
        effect: EffectEvidenceKind::ReceiverMutation,
        channel: ChannelEligibility::ExactProven,
    },
    MethodEffectContractSet {
        id: MethodEffectContractId::ReceiverMutationRisk,
        lang: Lang::Java,
        methods: &[
            "add",
            "addAll",
            "clear",
            "compute",
            "computeIfAbsent",
            "computeIfPresent",
            "merge",
            "put",
            "putAll",
            "remove",
            "removeAll",
            "removeIf",
            "replace",
            "replaceAll",
            "retainAll",
            "set",
            "sort",
        ],
        arity: MethodEffectArity::Any,
        receiver: MethodEffectReceiverContract::PotentiallyMutableReceiver,
        effect: EffectEvidenceKind::ReceiverMutation,
        channel: ChannelEligibility::ExactProven,
    },
    MethodEffectContractSet {
        id: MethodEffectContractId::ReceiverMutationRisk,
        lang: Lang::Rust,
        methods: &[
            "clear",
            "extend",
            "insert",
            "pop",
            "push",
            "remove",
            "retain",
            "reverse",
            "sort",
            "sort_by",
            "sort_unstable",
        ],
        arity: MethodEffectArity::Any,
        receiver: MethodEffectReceiverContract::PotentiallyMutableReceiver,
        effect: EffectEvidenceKind::ReceiverMutation,
        channel: ChannelEligibility::ExactProven,
    },
    MethodEffectContractSet {
        id: MethodEffectContractId::ReceiverMutationRisk,
        lang: Lang::Swift,
        methods: &[
            "append",
            "formIntersection",
            "formSymmetricDifference",
            "formUnion",
            "insert",
            "merge",
            "popLast",
            "remove",
            "removeAll",
            "removeFirst",
            "removeLast",
            "removeSubrange",
            "removeValue",
            "replaceSubrange",
            "reverse",
            "shuffle",
            "sort",
            "subtract",
            "swapAt",
            "update",
            "updateValue",
            "withContiguousMutableStorageIfAvailable",
            "withUnsafeMutableBufferPointer",
            "withUnsafeMutableBytes",
        ],
        arity: MethodEffectArity::Any,
        receiver: MethodEffectReceiverContract::PotentiallyMutableReceiver,
        effect: EffectEvidenceKind::ReceiverMutation,
        channel: ChannelEligibility::ExactProven,
    },
];

const MAP_BUILDER_INDEX_WRITE_CONTRACTS: &[IndexWriteContract] = &[IndexWriteContract {
    pack_id: crate::PYTHON_LANGUAGE_PACK_ID,
    id: IndexWriteContractId::MapBuilderEntryWrite,
    lang: Lang::Python,
    receiver: IndexWriteReceiverContract::ActiveMapBuilder,
    required_effect: EffectEvidenceKind::BindingWrite,
    channel: ChannelEligibility::ExactProven,
}];

fn method_effect_contract_lang(requested: Lang, contract_lang: Lang) -> Option<Lang> {
    if requested == contract_lang || (js_like_lang(requested) && contract_lang == Lang::JavaScript)
    {
        Some(requested)
    } else {
        None
    }
}

pub(super) fn method_effect_contracts(lang: Lang) -> impl Iterator<Item = MethodEffectContract> {
    BUILDER_APPEND_METHOD_EFFECTS
        .iter()
        .chain(RECEIVER_MUTATION_METHOD_EFFECTS.iter())
        .copied()
        .filter_map(move |set| {
            let lang = method_effect_contract_lang(lang, set.lang)?;
            Some((lang, set))
        })
        .flat_map(|(lang, set)| {
            set.methods
                .iter()
                .copied()
                .map(move |method| MethodEffectContract {
                    pack_id: builtin_language_pack_id(lang),
                    id: set.id,
                    lang,
                    method,
                    arity: set.arity,
                    receiver: set.receiver,
                    effect: set.effect,
                    channel: set.channel,
                })
        })
}

pub(super) fn map_builder_index_write_contract(lang: Lang) -> Option<IndexWriteContract> {
    MAP_BUILDER_INDEX_WRITE_CONTRACTS
        .iter()
        .copied()
        .find(|contract| contract.lang == lang)
}
