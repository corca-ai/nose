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
    // Cheap structural gates run before strict/value extraction; syntax copy-paste coverage
    // still handles exact repeats from generated-like mega-functions and huge test fixtures.
    if declaration_only_callable(gate.kind, gate.origin) {
        return true;
    }
    if semantic_container_token_cap(gate.kind).is_some_and(|cap| gate.tokens > cap) {
        return true;
    }
    if data_like_function_unit(gate.kind, gate.tokens, gate.lines)
        || test_structural_unit(gate.large_test_file, gate.kind)
    {
        return true;
    }
    let can_use_dense_gate = gate.declarative
        || matches!(gate.kind, UnitKind::Function | UnitKind::Method)
        || gate.exact_fragment;
    gate.syntactically_small && !can_use_dense_gate
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
