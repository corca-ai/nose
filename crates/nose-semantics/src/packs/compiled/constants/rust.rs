use super::*;

pub(in crate::packs::compiled) const RUST_STDLIB_COLLECTION_FACTORY_PACKAGES: &[&str] =
    &["std::collections"];
pub(in crate::packs::compiled) const RUST_STDLIB_MAP_FACTORY_PACKAGES: &[&str] =
    &["std::collections"];
pub(in crate::packs::compiled) const RUST_STDLIB_OPTION_PACKAGES: &[&str] =
    &["std::option", "core::option"];
pub(in crate::packs::compiled) const RUST_STDLIB_RESULT_PACKAGES: &[&str] =
    &["std::result", "core::result"];
pub(in crate::packs::compiled) const RUST_STDLIB_INTEGER_METHOD_PACKAGES: &[&str] =
    &["core::primitive"];
pub(in crate::packs::compiled) const RUST_STDLIB_VEC_PACKAGES: &[&str] =
    &["std::vec", "alloc::vec"];

pub(in crate::packs::compiled) const RUST_STDLIB_COLLECTION_FACTORY_PRODUCER_IDS: &[&str] =
    &[RUST_STDLIB_COLLECTION_FACTORY_PRODUCER_ID];
pub(in crate::packs::compiled) const RUST_STDLIB_COLLECTION_FACTORY_CONTRACT_IDS: &[&str] =
    &[RUST_STDLIB_COLLECTION_FACTORY_CONTRACT_ID];
pub(in crate::packs::compiled) const RUST_STDLIB_COLLECTION_FACTORY_CONFORMANCE_REFS: &[&str] = &[
    "rust-std-collections-hashset-from-positive",
    "rust-std-collections-btreeset-from-positive",
    "rust-std-collections-vecdeque-from-positive",
    "rust-std-collections-shadowed-std-hard-negative",
    "rust-std-collections-type-alias-std-hard-negative",
];

pub(in crate::packs::compiled) const RUST_STDLIB_MAP_FACTORY_PRODUCER_IDS: &[&str] =
    &[RUST_STDLIB_MAP_FACTORY_PRODUCER_ID];
pub(in crate::packs::compiled) const RUST_STDLIB_MAP_FACTORY_CONTRACT_IDS: &[&str] =
    &[RUST_STDLIB_MAP_FACTORY_CONTRACT_ID];
pub(in crate::packs::compiled) const RUST_STDLIB_MAP_FACTORY_CONFORMANCE_REFS: &[&str] = &[
    "rust-std-map-hashmap-from-positive",
    "rust-std-map-btreemap-from-positive",
    "rust-std-map-shadowed-std-hard-negative",
    "rust-std-map-type-alias-std-hard-negative",
];

pub(in crate::packs::compiled) const RUST_STDLIB_OPTION_PRODUCER_IDS: &[&str] =
    &[RUST_STDLIB_OPTION_PRODUCER_ID];
pub(in crate::packs::compiled) const RUST_STDLIB_OPTION_CONTRACT_IDS: &[&str] = &[
    RUST_STDLIB_OPTION_SOME_CONTRACT_ID,
    RUST_STDLIB_OPTION_NONE_CONTRACT_ID,
    RUST_STDLIB_OPTION_AND_THEN_CONTRACT_ID,
];
pub(in crate::packs::compiled) const RUST_STDLIB_OPTION_CONFORMANCE_REFS: &[&str] = &[
    "rust-option-some-positive",
    "rust-option-none-positive",
    "rust-option-and-then-positive",
    "rust-option-some-shadow-hard-negative",
    "rust-option-none-shadow-hard-negative",
    "rust-option-and-then-non-option-hard-negative",
];

pub(in crate::packs::compiled) const RUST_STDLIB_RESULT_PRODUCER_IDS: &[&str] =
    &[RUST_STDLIB_RESULT_PRODUCER_ID];
pub(in crate::packs::compiled) const RUST_STDLIB_RESULT_CONTRACT_IDS: &[&str] = &[
    RUST_STDLIB_RESULT_OK_CONTRACT_ID,
    RUST_STDLIB_RESULT_ERR_CONTRACT_ID,
    RUST_STDLIB_RESULT_IS_OK_CONTRACT_ID,
    RUST_STDLIB_RESULT_IS_ERR_CONTRACT_ID,
];
pub(in crate::packs::compiled) const RUST_STDLIB_RESULT_CONFORMANCE_REFS: &[&str] = &[
    "rust-result-ok-positive",
    "rust-result-err-positive",
    "rust-result-is-ok-positive",
    "rust-result-is-err-positive",
    "rust-result-ok-shadow-hard-negative",
    "rust-result-err-shadow-hard-negative",
    "rust-result-predicate-non-result-hard-negative",
    "rust-result-local-type-shadow-hard-negative",
    "rust-result-callback-defaulting-hard-negative",
];

pub(in crate::packs::compiled) const RUST_STDLIB_INTEGER_METHOD_PRODUCER_IDS: &[&str] =
    &[RUST_STDLIB_INTEGER_METHOD_PRODUCER_ID];
pub(in crate::packs::compiled) const RUST_STDLIB_INTEGER_METHOD_CONTRACT_IDS: &[&str] = &[
    SCALAR_INTEGER_METHOD_ABS_CONTRACT_ID,
    SCALAR_INTEGER_METHOD_MIN_CONTRACT_ID,
    SCALAR_INTEGER_METHOD_MAX_CONTRACT_ID,
    SCALAR_INTEGER_METHOD_CLAMP_CONTRACT_ID,
];
pub(in crate::packs::compiled) const RUST_STDLIB_INTEGER_METHOD_CONFORMANCE_REFS: &[&str] = &[
    "rust-integer-method-abs-positive",
    "rust-integer-method-min-positive",
    "rust-integer-method-max-positive",
    "rust-integer-method-clamp-positive",
    "rust-integer-method-non-integer-receiver-hard-negative",
    "rust-integer-method-unsupported-arity-hard-negative",
];

pub(in crate::packs::compiled) const RUST_STDLIB_VEC_PRODUCER_IDS: &[&str] =
    &[RUST_STDLIB_VEC_PRODUCER_ID];
pub(in crate::packs::compiled) const RUST_STDLIB_VEC_CONTRACT_IDS: &[&str] = &[
    RUST_STDLIB_VEC_MACRO_CONTRACT_ID,
    RUST_STDLIB_VEC_NEW_CONTRACT_ID,
];
pub(in crate::packs::compiled) const RUST_STDLIB_VEC_CONFORMANCE_REFS: &[&str] = &[
    "rust-vec-macro-factory-positive",
    "rust-vec-new-factory-positive",
    "rust-vec-macro-shadowed-hard-negative",
    "rust-vec-new-shadowed-hard-negative",
];
