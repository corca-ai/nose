//! The contract-path recognizer and its differential gate against the shape predicates.
//!
//! Issue #33 steps 4–5. As each fragment family migrates off the standalone shape
//! predicates in [`crate::units`], its recognition is re-expressed here as the
//! construction of a [`FragmentContract`]. [`recognize_contract`] is an *independent*
//! recognizer for the migrated shapes: it matches structure directly and builds a contract,
//! reusing only the shared invalidation-boundary gates (span containment + context safety),
//! which are substrate, not per-shape predicates.
//!
//! The differential test below is the acceptance gate the maintainer required: over a
//! representative corpus, the set of `(span, kind)` the predicate path accepts (restricted
//! to migrated kinds) must equal the set the contract path produces. A migration step that
//! changes which nodes are accepted fails this test. As [`MIGRATED`] grows, the gate keeps
//! the two paths in lockstep until every shape is contract-expressed.

use super::contract::FragmentContract;
use super::oracle::free_input_cids;
use super::{Exit, FragmentKind};
use nose_il::{Il, Interner, NodeId, NodeKind};

/// Fragment kinds that have been migrated onto the contract path. The differential gate
/// compares the predicate and contract paths over exactly this set; everything outside it
/// is still owned solely by the [`crate::units`] predicates.
pub(crate) const MIGRATED: &[FragmentKind] = &[FragmentKind::DirectReturn, FragmentKind::DirectThrow];

/// Recognize `node` as a migrated exact-fragment shape by building its contract directly,
/// independently of [`crate::units::exact_statement_fragment_root`]. Returns `None` for
/// non-fragments and for shapes not yet migrated.
pub(crate) fn recognize_contract(
    il: &Il,
    node: NodeId,
    parents: &[Option<NodeId>],
    interner: &Interner,
) -> Option<FragmentContract> {
    // Shared substrate gates — the invalidation-boundary model, reused (not duplicated).
    if !crate::units::subtree_spans_within(il, node, il.node(node).span) {
        return None;
    }
    if !crate::units::top_level_statement_fragment_context_safe(il, node, parents, interner) {
        return None;
    }
    let kids = il.children(node);
    let computed_unary = || {
        kids.len() == 1 && !matches!(il.kind(kids[0]), NodeKind::Var | NodeKind::Lit)
    };
    match il.kind(node) {
        NodeKind::Return if computed_unary() => {
            Some(contract(FragmentKind::DirectReturn, Exit::Return, il, node))
        }
        NodeKind::Throw if computed_unary() => {
            Some(contract(FragmentKind::DirectThrow, Exit::Throw, il, node))
        }
        _ => None,
    }
}

fn contract(kind: FragmentKind, exit: Exit, il: &Il, node: NodeId) -> FragmentContract {
    FragmentContract {
        kind,
        root: node,
        inputs: free_input_cids(il, node),
        exit,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::{build_parent_index, exact_statement_fragment_root};
    use nose_il::{FileId, Lang, Span};
    use nose_normalize::{normalize, NormalizeOptions};

    /// Walk `il` exactly as the real fragment collector does (skipping `Lambda` subtrees),
    /// applying `classify` to each node and collecting the accepted `(span, kind)` pairs.
    fn index<F>(il: &Il, classify: &F) -> Vec<(Span, FragmentKind)>
    where
        F: Fn(NodeId) -> Option<FragmentKind>,
    {
        fn walk<F: Fn(NodeId) -> Option<FragmentKind>>(
            il: &Il,
            node: NodeId,
            classify: &F,
            out: &mut Vec<(Span, FragmentKind)>,
        ) {
            if il.kind(node) == NodeKind::Lambda {
                return;
            }
            if let Some(kind) = classify(node) {
                out.push((il.node(node).span, kind));
            }
            for &c in il.children(node) {
                walk(il, c, classify, out);
            }
        }
        let mut out = Vec::new();
        walk(il, il.root, classify, &mut out);
        out
    }

    fn sort_key(entry: &(Span, FragmentKind)) -> (u32, u32, &'static str) {
        (entry.0.start_byte, entry.0.end_byte, entry.1.reason_code())
    }

    /// The two paths must agree on the migrated kinds for `src`.
    fn assert_paths_agree(src: &str, lang: Lang) {
        let interner = Interner::new();
        let raw = nose_frontend::lower_source(FileId(0), "t", src.as_bytes(), lang, &interner)
            .expect("lowering should succeed");
        let il = normalize(&raw, &interner, &NormalizeOptions::default());
        let parents = build_parent_index(&il);

        let mut predicate: Vec<(Span, FragmentKind)> =
            index(&il, &|node| exact_statement_fragment_root(&il, node, &parents, &interner))
                .into_iter()
                .filter(|(_, kind)| MIGRATED.contains(kind))
                .collect();
        let mut contract: Vec<(Span, FragmentKind)> = index(&il, &|node| {
            recognize_contract(&il, node, &parents, &interner).map(|c| c.kind)
        });

        predicate.sort_by_key(sort_key);
        contract.sort_by_key(sort_key);
        assert_eq!(
            predicate, contract,
            "predicate and contract paths disagree on migrated fragments in `{src}`"
        );
    }

    #[test]
    fn differential_direct_return_and_throw() {
        // Accepted: top-level computed return / throw.
        assert_paths_agree("function g(b){ return b*b + 1; }", Lang::JavaScript);
        assert_paths_agree("function f(a){ throw a + 1; }", Lang::JavaScript);
        assert_paths_agree(
            "def h(a, c):\n    return a * a + c\n",
            Lang::Python,
        );
    }

    #[test]
    fn differential_rejects_match_for_non_fragments() {
        // `return x` (bare var) and `return 1` (bare lit) are not computed returns;
        // both paths must reject — yielding empty, equal sets.
        assert_paths_agree("function f(a){ return a; }", Lang::JavaScript);
        assert_paths_agree("function f(a){ return 1; }", Lang::JavaScript);
        // A preceding reassignment of the returned input invalidates context safety;
        // both paths must reject the return.
        assert_paths_agree("function f(a){ a = a + 1; return a * a; }", Lang::JavaScript);
    }

    #[test]
    fn differential_ignores_non_migrated_shapes() {
        // Loop/append and conditional effect shapes are accepted by the predicate path
        // under OTHER kinds and must be excluded from the contract path — the migrated
        // intersection stays empty and equal.
        assert_paths_agree(
            "function h(xs){ const out=[]; for(const x of xs){ out.push(x*2); } return out; }",
            Lang::JavaScript,
        );
        assert_paths_agree(
            "def k(xs):\n    out = []\n    for x in xs:\n        out.append(x + 1)\n    return out\n",
            Lang::Python,
        );
    }
}
