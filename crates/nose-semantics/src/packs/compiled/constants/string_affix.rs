use super::*;

pub(in crate::packs::compiled) const STRING_AFFIX_PREDICATE_PROTOCOL_PRODUCER_IDS: &[&str] =
    &[STRING_AFFIX_PREDICATE_PROTOCOL_PRODUCER_ID];
pub(in crate::packs::compiled) const STRING_AFFIX_PREDICATE_PROTOCOL_CONTRACT_IDS: &[&str] =
    &[STRING_AFFIX_PREDICATE_CONTRACT_ID];
pub(in crate::packs::compiled) const STRING_AFFIX_PREDICATE_PROTOCOL_CONFORMANCE_REFS: &[&str] = &[
    "string-affix-predicate-python-startswith-positive",
    "string-affix-predicate-python-endswith-positive",
    "string-affix-predicate-java-startswith-positive",
    "string-affix-predicate-java-endswith-positive",
    "string-affix-predicate-rust-starts-with-positive",
    "string-affix-predicate-rust-ends-with-positive",
    "string-affix-predicate-swift-has-prefix-positive",
    "string-affix-predicate-swift-has-suffix-positive",
    "string-affix-predicate-typescript-startswith-positive",
    "string-affix-predicate-typescript-endswith-positive",
    "string-affix-predicate-javascript-startswith-positive",
    "string-affix-predicate-javascript-endswith-positive",
    "string-affix-predicate-direction-mismatch-hard-negative",
    "string-affix-predicate-missing-receiver-proof-hard-negative",
    "string-affix-predicate-non-string-receiver-hard-negative",
    "string-affix-predicate-wrong-pack-hard-negative",
    "string-affix-predicate-wrong-producer-hard-negative",
    "string-affix-predicate-unsupported-arity-hard-negative",
    "string-affix-predicate-unsupported-offset-hard-negative",
];
