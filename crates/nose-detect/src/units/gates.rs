use crate::exact_policy::dense_unit_admitted;
use crate::test_paths::is_test_path;
use nose_il::{Il, UnitBodyKind, UnitEvidenceFlag, UnitKind, UnitOrigin};

/// Upper bound (pre-order node count) for a *block* unit. Blocks are meant to surface
/// sub-function fragments; broad nested bodies are covered by their enclosing unit and
/// can multiply value extraction cost across almost-identical regions.
const MAX_BLOCK_TOKENS: usize = 160;
/// Upper bound for a *class* container unit. Ordinary class/type clones stay eligible,
/// while very large class bodies are delegated to their method/function units.
const MAX_CLASS_TOKENS: usize = 8_000;
/// Dense-function admission exists for real compact code (`return sum(...)`), not
/// generated/data-like mega expressions. Syntax copy-paste still covers exact repeats.
const DATA_LIKE_FUNCTION_MIN_TOKENS: usize = 2_000;
const DATA_LIKE_FUNCTION_MIN_TOKENS_PER_LINE: usize = 120;
/// Huge test fixtures stay covered by the syntax channel for exact copy-paste. Small
/// tests still participate in semantic matching; very large test files are usually
/// data/scenario corpora where Type-4 value extraction is less actionable than the cost.
const LARGE_TEST_FILE_NODE_CUTOFF: usize = 5_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductUnitAdmission {
    Admitted,
    DeclarationOnly,
    SemanticContainerTooLarge,
    DataLikeFunction,
    LargeTestFile,
    BelowSizeFloor,
}

impl ProductUnitAdmission {
    pub fn label(self) -> &'static str {
        match self {
            Self::Admitted => "admitted",
            Self::DeclarationOnly => "declaration-only",
            Self::SemanticContainerTooLarge => "semantic-container-too-large",
            Self::DataLikeFunction => "data-like-function",
            Self::LargeTestFile => "large-test-file",
            Self::BelowSizeFloor => "below-size-floor",
        }
    }

    pub fn admitted(self) -> bool {
        self == Self::Admitted
    }
}

#[derive(Clone, Copy)]
pub(super) struct PreValueFingerprintGate {
    pub(super) kind: UnitKind,
    pub(super) origin: UnitOrigin,
    pub(super) tokens: usize,
    pub(super) lines: u32,
    pub(super) syntactically_small: bool,
    pub(super) declarative: bool,
    pub(super) exact_fragment: bool,
    pub(super) large_test_file: bool,
}

pub(super) fn skip_before_value_fingerprint(gate: PreValueFingerprintGate) -> bool {
    pre_value_fingerprint_rejection(gate).is_some()
}

pub(super) fn pre_value_fingerprint_rejection(
    gate: PreValueFingerprintGate,
) -> Option<ProductUnitAdmission> {
    // Cheap structural gates run before strict/value extraction; syntax copy-paste coverage
    // still handles exact repeats from generated-like mega-functions and huge test fixtures.
    if declaration_only_callable(gate.kind, gate.origin) {
        return Some(ProductUnitAdmission::DeclarationOnly);
    }
    if semantic_container_token_cap(gate.kind).is_some_and(|cap| gate.tokens > cap) {
        return Some(ProductUnitAdmission::SemanticContainerTooLarge);
    }
    if data_like_function_unit(gate.kind, gate.tokens, gate.lines) {
        return Some(ProductUnitAdmission::DataLikeFunction);
    }
    if test_structural_unit(gate.large_test_file, gate.kind) {
        return Some(ProductUnitAdmission::LargeTestFile);
    }
    let can_use_dense_gate = gate.declarative
        || matches!(gate.kind, UnitKind::Function | UnitKind::Method)
        || gate.exact_fragment;
    (gate.syntactically_small && !can_use_dense_gate)
        .then_some(ProductUnitAdmission::BelowSizeFloor)
}

pub(super) fn post_value_fingerprint_rejection(
    kind: UnitKind,
    exact_fragment: bool,
    declarative: bool,
    syntactically_small: bool,
    exact_safe: bool,
    value_len: usize,
) -> Option<ProductUnitAdmission> {
    let dense = dense_unit_admitted(kind, exact_fragment, declarative, exact_safe, value_len);
    ((syntactically_small || exact_fragment) && !dense)
        .then_some(ProductUnitAdmission::BelowSizeFloor)
}

pub(crate) fn large_test_file(il: &Il) -> bool {
    is_test_path(&il.meta.path) && il.nodes.len() > LARGE_TEST_FILE_NODE_CUTOFF
}

fn semantic_container_token_cap(kind: UnitKind) -> Option<usize> {
    match kind {
        UnitKind::Block => Some(MAX_BLOCK_TOKENS),
        UnitKind::Class => Some(MAX_CLASS_TOKENS),
        UnitKind::Function | UnitKind::Method => None,
    }
}

fn data_like_function_unit(kind: UnitKind, tokens: usize, lines: u32) -> bool {
    matches!(kind, UnitKind::Function | UnitKind::Method)
        && tokens > DATA_LIKE_FUNCTION_MIN_TOKENS
        && tokens / (lines.max(1) as usize) > DATA_LIKE_FUNCTION_MIN_TOKENS_PER_LINE
}

fn declaration_only_callable(kind: UnitKind, origin: UnitOrigin) -> bool {
    matches!(kind, UnitKind::Function | UnitKind::Method)
        && (origin.body_kind == UnitBodyKind::DeclarationOnly
            || origin.has_evidence(UnitEvidenceFlag::DeclarationOnly))
}

fn test_structural_unit(large_test_file: bool, kind: UnitKind) -> bool {
    matches!(
        kind,
        UnitKind::Function | UnitKind::Method | UnitKind::Class | UnitKind::Block
    ) && large_test_file
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gate(kind: UnitKind) -> PreValueFingerprintGate {
        PreValueFingerprintGate {
            kind,
            origin: UnitOrigin::default(),
            tokens: 24,
            lines: 5,
            syntactically_small: false,
            declarative: false,
            exact_fragment: false,
            large_test_file: false,
        }
    }

    #[test]
    fn structured_product_rejections_share_the_live_pre_value_gates() {
        let admitted = gate(UnitKind::Function);
        assert_eq!(pre_value_fingerprint_rejection(admitted), None);
        assert!(!skip_before_value_fingerprint(admitted));

        let mut large_test = admitted;
        large_test.large_test_file = true;
        assert_eq!(
            pre_value_fingerprint_rejection(large_test),
            Some(ProductUnitAdmission::LargeTestFile)
        );
        assert!(skip_before_value_fingerprint(large_test));

        let mut data_like = admitted;
        data_like.tokens = DATA_LIKE_FUNCTION_MIN_TOKENS + 1;
        data_like.lines = 1;
        assert_eq!(
            pre_value_fingerprint_rejection(data_like),
            Some(ProductUnitAdmission::DataLikeFunction)
        );
        assert!(skip_before_value_fingerprint(data_like));

        let mut small_block = gate(UnitKind::Block);
        small_block.syntactically_small = true;
        assert_eq!(
            pre_value_fingerprint_rejection(small_block),
            Some(ProductUnitAdmission::BelowSizeFloor)
        );
        assert!(skip_before_value_fingerprint(small_block));
    }

    #[test]
    fn product_admission_labels_are_stable_evidence_values() {
        assert_eq!(ProductUnitAdmission::Admitted.label(), "admitted");
        assert_eq!(
            ProductUnitAdmission::SemanticContainerTooLarge.label(),
            "semantic-container-too-large"
        );
        assert!(ProductUnitAdmission::Admitted.admitted());
        assert!(!ProductUnitAdmission::LargeTestFile.admitted());
    }

    #[test]
    fn post_value_product_rejection_shares_the_dense_exact_gate() {
        assert_eq!(
            post_value_fingerprint_rejection(UnitKind::Function, false, false, true, true, 4),
            None
        );
        assert_eq!(
            post_value_fingerprint_rejection(UnitKind::Function, false, false, true, true, 3),
            Some(ProductUnitAdmission::BelowSizeFloor)
        );
        assert_eq!(
            post_value_fingerprint_rejection(UnitKind::Block, true, false, true, false, 4),
            Some(ProductUnitAdmission::BelowSizeFloor)
        );
    }
}
