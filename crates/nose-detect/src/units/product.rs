use super::{
    collect_unit_roots, exact_safe_for_unit, large_test_file, post_value_fingerprint_rejection,
    pre_value_fingerprint_rejection, value_fingerprint_context_for_roots, ExtractFeatures,
    PreValueFingerprintGate, ProductUnitAdmission, StrictFacts, UnitExtractCtx,
};
use crate::fragment::FragmentContract;
use nose_il::{Il, Interner, NodeId, NodeKind, UnitKind, UnitOrigin};

/// Keep whole function/method/class units for cross-file matches, but do not expand
/// every nested `if`/loop into extra block units inside dependency code or very
/// large files. The syntax channel still covers exact copy-paste spans there.
const LARGE_FILE_BLOCK_NODE_CUTOFF: usize = 5_000;

pub(crate) fn block_units_for_file(il: &Il, opts: &crate::DetectOptions) -> bool {
    opts.block_units
        && !is_bulk_dependency_path(&il.meta.path)
        && il.nodes.len() <= LARGE_FILE_BLOCK_NODE_CUTOFF
}

pub(crate) fn raw_il_is_empty_module(il: &Il) -> bool {
    il.units.is_empty() && il.kind(il.root) == NodeKind::Module && il.children(il.root).is_empty()
}

fn is_bulk_dependency_path(path: &str) -> bool {
    let p = crate::test_paths::lowercase_path(path);
    [
        "vendor/",
        "third_party/",
        "third-party/",
        "/deps/",
        "node_modules/",
        "/dist/",
        "/build/",
        "/external/",
        ".min.",
        ".pb.",
        "_pb2",
        ".g.dart",
        ".d.ts",
        "generated/",
        "/gen/",
        ".generated.",
    ]
    .iter()
    .any(|m| p.contains(m))
}

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

/// An exact-fragment candidate together with the data the offline behavioral oracle needs to
/// check it. [`product_admission`](Self::product_admission) says whether it survived today's
/// default extraction funnel. Keeping this owner in `nose-detect` prevents the Lab from
/// approximating fragment admission with a second set of gates that can drift from the shipped
/// detector.
#[derive(Clone)]
pub struct ProductOracleFragment {
    pub root: NodeId,
    pub contract: FragmentContract,
    pub token_count: usize,
    pub exact_safe: bool,
    /// Whether the current default product extraction path admits this fragment. The offline
    /// Lab also enumerates rejected candidates so it can re-check claims frozen by an older
    /// release without pretending those candidates are current product claims.
    pub product_admission: ProductUnitAdmission,
    pub value: Vec<u64>,
    /// Pointer-length contracts translated from the fragment value graph's canonical-id
    /// coordinates into the synthesized wrapper's positional-input coordinates. `None` is a
    /// fail-closed signal: the value graph relied on a coordinate the wrapper cannot represent.
    pub oracle_contracts: Option<Vec<(u32, u32)>>,
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
    let opts = crate::DetectOptions::default();
    let block_units = block_units_for_file(il, &opts);
    let (roots, _) = collect_unit_roots(il, interner, block_units);
    value_fingerprint_context_for_roots(il, interner, roots.len())
}

/// Exact sub-function fragments admitted by the product's default extraction path.
///
/// This deliberately returns only fragments, not ordinary function/class units (which the
/// verifier already owns). It runs the same root collection, exact-safety, size, and semantic
/// richness gates as the product extractor, and computes fingerprints under the same shared
/// per-file context. The function is an offline audit surface; it does not run on `nose query`.
#[cfg(test)]
pub(crate) fn default_product_oracle_fragments(
    raw_il: &Il,
    normalized_il: &Il,
    interner: &Interner,
) -> Vec<ProductOracleFragment> {
    default_product_oracle_fragment_candidates(raw_il, normalized_il, interner)
        .into_iter()
        .filter(|fragment| fragment.product_admission.admitted() && fragment.exact_safe)
        .collect()
}

/// Every exact-fragment candidate recognized under the default product's file/block policy.
///
/// This offline-only audit surface retains roots rejected by the current
/// size/richness/exact-safety gates and records their admission result. That distinction is
/// required to replay a frozen release claim: withdrawing a fragment from today's product must
/// not make its historical soundness obligation disappear.
pub fn default_product_oracle_fragment_candidates(
    raw_il: &Il,
    normalized_il: &Il,
    interner: &Interner,
) -> Vec<ProductOracleFragment> {
    let opts = crate::DetectOptions::default();
    if raw_il_is_empty_module(raw_il)
        || large_test_file(raw_il)
        || large_test_file(normalized_il)
        || !block_units_for_file(normalized_il, &opts)
    {
        return Vec::new();
    }

    let (roots, parents) = collect_unit_roots(normalized_il, interner, true);
    let Some(parents) = parents else {
        return Vec::new();
    };
    let facts = StrictFacts::collect(normalized_il, interner);
    let value_context = value_fingerprint_context_for_roots(normalized_il, interner, roots.len());
    let ctx = UnitExtractCtx {
        il: normalized_il,
        interner,
        seeds: &[],
        min_lines: opts.min_lines,
        min_tokens: opts.min_tokens,
        features: ExtractFeatures {
            shape_features: false,
            abstraction_witnesses: false,
            connected_witnesses: false,
        },
        parents: Some(&parents),
        facts: &facts,
        value_context: value_context.as_ref(),
        large_test_file: false,
    };
    let mut fragments = Vec::new();
    for unit_root in roots {
        let Some(fragment_kind) = unit_root.fragment_kind else {
            continue;
        };
        let Some(contract) = crate::fragment::recognize::recognize_contract(
            normalized_il,
            unit_root.root,
            &parents,
            interner,
        ) else {
            continue;
        };
        debug_assert_eq!(contract.kind, fragment_kind);

        let span = normalized_il.node(unit_root.root).span;
        let mut pre = Vec::new();
        super::tree::collect_pre(normalized_il, unit_root.root, &mut pre);
        let syntactically_small = span.line_count() < opts.min_lines || pre.len() < opts.min_tokens;
        let pre_gate = PreValueFingerprintGate {
            kind: unit_root.kind,
            origin: unit_root.origin,
            tokens: pre.len(),
            lines: span.line_count(),
            syntactically_small,
            declarative: false,
            exact_fragment: true,
            large_test_file: false,
        };
        let pre_rejection = pre_value_fingerprint_rejection(pre_gate);
        let exact_safe = exact_safe_for_unit(&ctx, unit_root.root, true);

        // The extraction bundle records only whether a pointer-length contract fired. The Lab
        // needs the actual coordinates, so rebuild this offline-only fragment fingerprint once
        // and assert it is the same product value before translating its coordinates.
        let (value, contracts) = match value_context.as_ref() {
            Some(context) => nose_normalize::value_fingerprint_and_contracts_with_context(
                normalized_il,
                unit_root.root,
                interner,
                context,
            ),
            None => nose_normalize::value_fingerprint_and_contracts(
                normalized_il,
                unit_root.root,
                interner,
            ),
        };
        let product_admission = pre_rejection
            .or_else(|| {
                post_value_fingerprint_rejection(
                    unit_root.kind,
                    true,
                    false,
                    syntactically_small,
                    exact_safe,
                    value.len(),
                )
            })
            .unwrap_or(ProductUnitAdmission::Admitted);
        let oracle_contracts = translate_fragment_contracts(&contract, &contracts);
        fragments.push(ProductOracleFragment {
            root: unit_root.root,
            contract,
            token_count: pre.len(),
            exact_safe,
            product_admission,
            value,
            oracle_contracts,
        });
    }
    fragments
}

fn translate_fragment_contracts(
    contract: &FragmentContract,
    contracts: &[(u32, u32)],
) -> Option<Vec<(u32, u32)>> {
    let positions: std::collections::HashMap<u32, u32> = contract
        .inputs
        .iter()
        .enumerate()
        .map(|(position, &cid)| (cid, position as u32))
        .collect();
    let mut translated = contracts
        .iter()
        .map(|&(array, length)| Some((*positions.get(&array)?, *positions.get(&length)?)))
        .collect::<Option<Vec<_>>>()?;
    translated.sort_unstable();
    translated.dedup();
    Some(translated)
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
