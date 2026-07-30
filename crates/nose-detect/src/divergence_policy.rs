//! Pure product policy for divergent-edit findings.
//!
//! The CLI adapter provides normalized detection evidence through
//! [`DivergencePolicyInput`]. This module owns only the deterministic tier,
//! taxonomy, reason, and gate decision; rendering and process behavior remain
//! at the CLI boundary.

pub const DIVERGENT_EDIT_V2_POLICY: &str = "divergent-edit-v2-strict";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DivergenceLane {
    BaseDivergence,
    NewCopy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DivergenceScope {
    Production,
    TestOrMixed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SharedLogicEvidence {
    Touched,
    NotTouched,
    Unproven,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DivergencePolicyInput {
    pub lane: DivergenceLane,
    pub scope: DivergenceScope,
    pub shared_logic: SharedLogicEvidence,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DivergenceTier {
    Strict,
    Review,
    ReportOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DivergenceGateDecision {
    pub eligible: bool,
    pub fail_default: bool,
    pub policy: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DivergencePolicyDecision {
    pub tier: DivergenceTier,
    pub tier_reasons: Vec<&'static str>,
    pub taxonomy_hint: &'static str,
    pub gate: DivergenceGateDecision,
}

pub fn divergence_policy(input: DivergencePolicyInput) -> DivergencePolicyDecision {
    let production = input.scope == DivergenceScope::Production;
    let tier = if input.lane == DivergenceLane::NewCopy || !production {
        DivergenceTier::ReportOnly
    } else if input.shared_logic == SharedLogicEvidence::Touched {
        DivergenceTier::Strict
    } else {
        DivergenceTier::Review
    };
    let taxonomy_hint = if input.lane == DivergenceLane::NewCopy {
        "unclear"
    } else if !production {
        "test_scaffolding"
    } else {
        match input.shared_logic {
            SharedLogicEvidence::Touched => "missed_propagation",
            SharedLogicEvidence::NotTouched => "no_propagation_needed",
            SharedLogicEvidence::Unproven => "unclear",
        }
    };
    let mut tier_reasons = Vec::with_capacity(3);
    if input.lane == DivergenceLane::NewCopy {
        tier_reasons.push("new_copy_no_base_member");
    } else {
        tier_reasons.push(match input.shared_logic {
            SharedLogicEvidence::Touched => "shared_logic_touched",
            SharedLogicEvidence::NotTouched => "shared_logic_not_touched",
            SharedLogicEvidence::Unproven => "shared_logic_unproven",
        });
    }
    if production {
        tier_reasons.push("non_test_scope");
    } else {
        tier_reasons.push("test_scope");
        tier_reasons.push("test_scaffolding");
    }
    DivergencePolicyDecision {
        tier,
        tier_reasons,
        taxonomy_hint,
        gate: DivergenceGateDecision {
            eligible: matches!(tier, DivergenceTier::Strict | DivergenceTier::Review),
            fail_default: tier == DivergenceTier::Strict,
            policy: DIVERGENT_EDIT_V2_POLICY,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decide(
        lane: DivergenceLane,
        scope: DivergenceScope,
        shared_logic: SharedLogicEvidence,
    ) -> DivergencePolicyDecision {
        divergence_policy(DivergencePolicyInput {
            lane,
            scope,
            shared_logic,
        })
    }

    #[test]
    fn new_copy_is_advisory_without_base_members() {
        let decision = decide(
            DivergenceLane::NewCopy,
            DivergenceScope::Production,
            SharedLogicEvidence::Unproven,
        );
        assert_eq!(decision.tier, DivergenceTier::ReportOnly);
        assert_eq!(decision.taxonomy_hint, "unclear");
        assert_eq!(
            decision.tier_reasons,
            ["new_copy_no_base_member", "non_test_scope"]
        );
        assert!(!decision.gate.eligible);
        assert!(!decision.gate.fail_default);
    }

    #[test]
    fn unproven_product_change_requires_review_but_does_not_fail() {
        let decision = decide(
            DivergenceLane::BaseDivergence,
            DivergenceScope::Production,
            SharedLogicEvidence::Unproven,
        );
        assert_eq!(decision.tier, DivergenceTier::Review);
        assert_eq!(decision.taxonomy_hint, "unclear");
        assert_eq!(
            decision.tier_reasons,
            ["shared_logic_unproven", "non_test_scope"]
        );
        assert!(decision.gate.eligible);
        assert!(!decision.gate.fail_default);
    }

    #[test]
    fn test_or_mixed_scope_stays_report_only_with_shared_logic_evidence() {
        let decision = decide(
            DivergenceLane::BaseDivergence,
            DivergenceScope::TestOrMixed,
            SharedLogicEvidence::Touched,
        );
        assert_eq!(decision.tier, DivergenceTier::ReportOnly);
        assert_eq!(decision.taxonomy_hint, "test_scaffolding");
        assert_eq!(
            decision.tier_reasons,
            ["shared_logic_touched", "test_scope", "test_scaffolding"]
        );
        assert!(!decision.gate.eligible);
        assert!(!decision.gate.fail_default);
    }

    #[test]
    fn product_gate_distinguishes_shared_logic_evidence() {
        for (evidence, tier, taxonomy, eligible, fail_default) in [
            (
                SharedLogicEvidence::Touched,
                DivergenceTier::Strict,
                "missed_propagation",
                true,
                true,
            ),
            (
                SharedLogicEvidence::NotTouched,
                DivergenceTier::Review,
                "no_propagation_needed",
                true,
                false,
            ),
            (
                SharedLogicEvidence::Unproven,
                DivergenceTier::Review,
                "unclear",
                true,
                false,
            ),
        ] {
            let decision = decide(
                DivergenceLane::BaseDivergence,
                DivergenceScope::Production,
                evidence,
            );
            assert_eq!(decision.tier, tier);
            assert_eq!(decision.taxonomy_hint, taxonomy);
            assert_eq!(decision.gate.eligible, eligible);
            assert_eq!(decision.gate.fail_default, fail_default);
            assert_eq!(decision.gate.policy, DIVERGENT_EDIT_V2_POLICY);
        }
    }
}
