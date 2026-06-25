use super::*;

pub(in crate::packs::compiled) const SEQUENCE_HOF_ADAPTER_PROTOCOL_LANGUAGES: &[&str] =
    &["rust", "swift"];
pub(in crate::packs::compiled) const SEQUENCE_HOF_ADAPTER_PROTOCOL_PACKAGES: &[&str] =
    &["core::iter", "Swift.Collection"];
pub(in crate::packs::compiled) const SEQUENCE_HOF_ADAPTER_PROTOCOL_PRODUCER_IDS: &[&str] =
    &[SEQUENCE_HOF_ADAPTER_PROTOCOL_PRODUCER_ID];
pub(in crate::packs::compiled) const SEQUENCE_HOF_ADAPTER_PROTOCOL_CONTRACT_IDS: &[&str] =
    &[SEQUENCE_HOF_ADAPTER_CONTRACT_ID];
pub(in crate::packs::compiled) const SEQUENCE_HOF_ADAPTER_PROTOCOL_CONFORMANCE_REFS: &[&str] = &[
    "rust-iterator-hof-map-positive",
    "rust-iterator-hof-filter-positive",
    "rust-iterator-hof-filter-map-positive",
    "rust-iterator-hof-flat-map-positive",
    "rust-iterator-hof-any-terminal-positive",
    "rust-iterator-hof-all-terminal-positive",
    "rust-iterator-hof-count-terminal-positive",
    "rust-iterator-hof-custom-method-hard-negative",
    "rust-iterator-hof-missing-receiver-proof-hard-negative",
    "rust-iterator-hof-eager-callback-hard-negative",
    "rust-iterator-hof-missing-terminal-proof-hard-negative",
    "rust-iterator-hof-one-shot-reuse-hard-negative",
    "rust-iterator-hof-collect-vec-hard-negative",
    "rust-iterator-hof-find-unsupported-hard-negative",
    "swift-sequence-hof-map-positive",
    "swift-sequence-hof-filter-positive",
    "swift-sequence-hof-flat-map-positive",
    "swift-sequence-hof-set-order-hard-negative",
    "swift-sequence-hof-dictionary-order-hard-negative",
    "swift-sequence-hof-lazy-hard-negative",
    "swift-sequence-hof-throwing-closure-hard-negative",
    "swift-sequence-hof-mutating-closure-hard-negative",
    "swift-sequence-hof-any-sequence-reuse-hard-negative",
    "swift-sequence-hof-compact-map-unsupported-hard-negative",
];
