use crate::units::UnitFeat;
use nose_il::UnitKind;

/// Minimum value-fingerprint node count for any exact behavioral claim.
///
/// This floor keeps trivial units (`return x`, one-literal wrappers, etc.) from
/// becoming exact semantic evidence. Dense units, exact candidates, exact scoring,
/// and report witnesses all share this owner so the product surface cannot drift.
pub(crate) const EXACT_VALUE_MIN: usize = 4;

pub(crate) fn exact_value_rich(value_len: usize) -> bool {
    value_len >= EXACT_VALUE_MIN
}

/// Can this unit ever participate in the exact `semantic` channel's merge claim?
///
/// The product asserts behavioral equality only for strict-exact-safe units whose
/// value fingerprint clears the degenerate-size floor. The verify oracle's hard
/// soundness gate is scoped to this surface; collisions between lossy fingerprints
/// are diagnostics, not product false merges.
pub fn exact_claim_eligible(u: &UnitFeat) -> bool {
    exact_claim_eligible_parts(u.exact_safe, u.value.len())
}

/// The exact-claim gate when the caller already has the two relevant facts.
pub fn exact_claim_eligible_parts(exact_safe: bool, value_len: usize) -> bool {
    exact_safe && exact_value_rich(value_len)
}

pub(crate) fn exact_value_match_eligible(a: &UnitFeat, b: &UnitFeat) -> bool {
    exact_claim_eligible(a) && exact_claim_eligible(b) && a.value == b.value
}

pub(crate) fn dense_unit_admitted(
    kind: UnitKind,
    exact_fragment: bool,
    declarative: bool,
    exact_safe: bool,
    value_len: usize,
) -> bool {
    if exact_fragment {
        return exact_safe && exact_value_rich(value_len);
    }
    (declarative || matches!(kind, UnitKind::Function | UnitKind::Method))
        && exact_value_rich(value_len)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_claim_requires_safety_and_value_floor() {
        assert!(!exact_claim_eligible_parts(false, EXACT_VALUE_MIN));
        assert!(!exact_claim_eligible_parts(true, EXACT_VALUE_MIN - 1));
        assert!(exact_claim_eligible_parts(true, EXACT_VALUE_MIN));
    }

    #[test]
    fn dense_unit_admission_matches_exact_floor_policy() {
        assert!(dense_unit_admitted(
            UnitKind::Function,
            false,
            false,
            false,
            EXACT_VALUE_MIN
        ));
        assert!(dense_unit_admitted(
            UnitKind::Block,
            false,
            true,
            false,
            EXACT_VALUE_MIN
        ));
        assert!(!dense_unit_admitted(
            UnitKind::Block,
            false,
            false,
            true,
            EXACT_VALUE_MIN
        ));
        assert!(!dense_unit_admitted(
            UnitKind::Function,
            true,
            false,
            false,
            EXACT_VALUE_MIN
        ));
        assert!(dense_unit_admitted(
            UnitKind::Function,
            true,
            false,
            true,
            EXACT_VALUE_MIN
        ));
    }
}
