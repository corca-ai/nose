use crate::{BuiltinTypeDomainAliasContract, PYTHON_STDLIB_TYPE_DOMAIN_PRODUCER_ID};

pub(in crate::packs::compiled) const PYTHON_STDLIB_TYPE_DOMAIN_CONTRACT_IDS: &[&str] =
    &["python.stdlib.type-domain-alias.contract"];
pub(in crate::packs::compiled) const PYTHON_STDLIB_TYPE_DOMAIN_PRODUCER_IDS: &[&str] =
    &[PYTHON_STDLIB_TYPE_DOMAIN_PRODUCER_ID];
pub(in crate::packs::compiled) const PYTHON_STDLIB_TYPE_DOMAIN_HARD_NEGATIVE_REFS: &[&str] =
    &["python-typing-domain-wrong-module-hard-negative"];
pub(in crate::packs::compiled) const NO_TYPE_DOMAIN_ALIAS_CONTRACTS:
    &[BuiltinTypeDomainAliasContract] = &[];
