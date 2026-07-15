use super::{
    collect_unit_roots, large_test_file, post_value_fingerprint_rejection,
    pre_value_fingerprint_rejection, value_fingerprint_context_for_roots, PreValueFingerprintGate,
    ProductUnitAdmission,
};
use nose_il::{Il, Interner, NodeId, NodeKind, UnitKind, UnitOrigin};

/// Per-unit facts needed to ask whether the default product detector would admit an
/// ordinary frontend-tagged unit to semantic extraction.
#[derive(Clone, Copy)]
pub struct ProductUnitAdmissionInput {
    pub root: NodeId,
    pub kind: UnitKind,
    pub origin: UnitOrigin,
    pub tokens: usize,
    pub exact_safe: bool,
    pub value_len: usize,
}

/// Build the same file-level value context used by default product extraction.
///
/// The trigger counts frontend units plus default sub-function block/fragment roots.
/// Offline product censuses must use this owner rather than approximating the trigger
/// from function counts, because the context can change same-file inline fingerprints.
pub fn default_product_value_fingerprint_context(
    il: &Il,
    interner: &Interner,
) -> Option<nose_normalize::ValueFingerprintContext> {
    let block_units = crate::DetectOptions::default().block_units;
    let (roots, _) = collect_unit_roots(il, interner, block_units);
    value_fingerprint_context_for_roots(il, interner, roots.len())
}

/// Whether an ordinary frontend-tagged unit can enter the product's default semantic
/// extraction surface. This is the shared owner for offline soundness censuses: a unit that
/// the default detector drops must not contribute "product-claimable" merge mass merely
/// because its value fingerprint is exact-safe and non-trivial.
pub fn default_product_unit_admission(
    raw_il: &Il,
    normalized_il: &Il,
    input: ProductUnitAdmissionInput,
) -> ProductUnitAdmission {
    let opts = crate::DetectOptions::default();
    let span = normalized_il.node(input.root).span;
    let syntactically_small = span.line_count() < opts.min_lines || input.tokens < opts.min_tokens;
    let declarative = matches!(
        normalized_il.kind(input.root),
        NodeKind::CssRule | NodeKind::HtmlElement
    );
    let gate = PreValueFingerprintGate {
        kind: input.kind,
        origin: input.origin,
        tokens: input.tokens,
        lines: span.line_count(),
        syntactically_small,
        declarative,
        exact_fragment: false,
        large_test_file: large_test_file(raw_il) || large_test_file(normalized_il),
    };
    if let Some(rejection) = pre_value_fingerprint_rejection(gate) {
        return rejection;
    }
    post_value_fingerprint_rejection(
        input.kind,
        false,
        declarative,
        syntactically_small,
        input.exact_safe,
        input.value_len,
    )
    .unwrap_or(ProductUnitAdmission::Admitted)
}
