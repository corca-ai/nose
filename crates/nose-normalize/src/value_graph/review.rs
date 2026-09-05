//! Separate review evidence from occurrence salts used to prevent false merges.
use super::*;

/// Analysis values for review identity only, with occurrence coordinates replaced
/// by deterministic first-use ordinals. Never an equivalence or candidate key.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct ReviewValueFingerprint {
    pub values: Vec<u64>,
    pub returns: Vec<u64>,
    pub cond_sinks: Vec<u64>,
}

/// Build the unchanged detection features and, only when source salts were used,
/// replay their analysis with position-independent occurrence labels for reviews.
/// Equal source occurrences remain separate; repeated uses of one span share a label.
pub fn value_fingerprint_with_review(
    il: &Il,
    root: NodeId,
    interner: &Interner,
    context: Option<&ValueFingerprintContext>,
) -> (FingerprintLawBundle, Option<ReviewValueFingerprint>) {
    if let Some((v, l, r)) = crate::declarative_fingerprint(il, root, interner) {
        return (
            (v, l, r, Vec::new(), Vec::new(), (false, Vec::new(), false)),
            None,
        );
    }
    let build = |review: bool| {
        let mut builder = match context {
            Some(context) => Builder::new_with_context(il, interner, context),
            None => Builder::new(il, interner),
        };
        if review {
            builder.review_source_ids = Some(Default::default());
        }
        builder.build_unit_with_context(root, context);
        builder
    };
    let builder = build(false);
    let review = builder.source_salt_used.then(|| {
        let replay = build(true);
        let (values, _, returns) = replay.fingerprint_lits();
        ReviewValueFingerprint {
            values,
            returns,
            cond_sinks: replay.sink_profile().1,
        }
    });
    (api::finish_fingerprint_law_bundle(builder), review)
}
