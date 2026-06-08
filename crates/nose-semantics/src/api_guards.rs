//! Negative API guard contracts.
//!
//! These rows do not admit library semantics. They only describe selector
//! families that are risky to let converge as ordinary opaque calls when the
//! matching `LibraryApi` occurrence proof is absent.

use crate::{ChannelEligibility, FIRST_PARTY_PACK_ID};
use nose_il::Lang;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ApiGuardContractId {
    UnprovenMembershipLikeCall,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct UnprovenMembershipLikeCallContract {
    pub pack_id: &'static str,
    pub id: ApiGuardContractId,
    pub lang: Lang,
    pub method: &'static str,
    pub arg_count: usize,
    pub channel: ChannelEligibility,
}

const UNPROVEN_MEMBERSHIP_LIKE_METHODS: &[&str] = &[
    "Contains",
    "__contains__",
    "contains",
    "containsKey",
    "containsValue",
    "contains_key",
    "contains_value",
    "has",
    "has_key?",
    "include?",
    "includes",
    "key?",
    "member?",
];

pub fn unproven_membership_like_method_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<UnprovenMembershipLikeCallContract> {
    let method = UNPROVEN_MEMBERSHIP_LIKE_METHODS
        .iter()
        .copied()
        .find(|candidate| *candidate == method)?;
    Some(UnprovenMembershipLikeCallContract {
        pack_id: FIRST_PARTY_PACK_ID,
        id: ApiGuardContractId::UnprovenMembershipLikeCall,
        lang,
        method,
        arg_count,
        channel: ChannelEligibility::ExactProven,
    })
}
