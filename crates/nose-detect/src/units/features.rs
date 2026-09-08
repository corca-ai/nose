use super::UnitExtractCtx;
use crate::abstraction;
use nose_il::NodeId;
use nose_normalize::node_tag;

const SEED: u64 = 0x9E37_79B9_7F4A_7C15;

#[inline]
fn combine(a: u64, b: u64) -> u64 {
    (a.rotate_left(7) ^ b).wrapping_mul(SEED)
}

pub(super) fn unit_shape_features(
    ctx: &UnitExtractCtx<'_>,
    pre: &[NodeId],
) -> (Vec<u64>, Vec<u64>, Vec<u64>, Vec<abstraction::WitnessToken>) {
    let il = ctx.il;
    let interner = ctx.interner;
    let features = ctx.features;
    if features.shape_features {
        let mut shapes = Vec::with_capacity(pre.len());
        let mut linear = Vec::with_capacity(pre.len());
        let mut abstraction_tokens = if features.abstraction_witnesses {
            Vec::with_capacity(pre.len())
        } else {
            Vec::new()
        };
        for &nid in pre {
            let n = il.node(nid);
            let tag = node_tag(n.kind, n.payload, interner);
            linear.push(tag);
            if features.abstraction_witnesses {
                abstraction_tokens.push(abstraction::token_for(il, interner, nid, tag));
            }
            let mut shape = tag;
            for &c in il.children(nid) {
                let cn = il.node(c);
                shape = combine(shape, node_tag(cn.kind, cn.payload, interner));
            }
            shapes.push(shape);
        }
        shapes.sort_unstable();
        let signature = ctx
            .seeds
            .map(|seeds| {
                let mut distinct = shapes.clone();
                distinct.dedup();
                crate::minhash::sign(&distinct, seeds)
            })
            .unwrap_or_default();
        (shapes, signature, linear, abstraction_tokens)
    } else if features.abstraction_witnesses {
        let abstraction_tokens = pre
            .iter()
            .map(|&nid| {
                let n = il.node(nid);
                let tag = node_tag(n.kind, n.payload, interner);
                abstraction::token_for(il, interner, nid, tag)
            })
            .collect();
        (Vec::new(), Vec::new(), Vec::new(), abstraction_tokens)
    } else {
        (Vec::new(), Vec::new(), Vec::new(), Vec::new())
    }
}

pub(super) fn unit_minhash(
    value: &[u64],
    shapes: &[u64],
    shape_features: bool,
    seeds: &[u64],
) -> Vec<u64> {
    if value.is_empty() && !shape_features {
        Vec::new()
    } else {
        let mut distinct = if value.is_empty() {
            shapes.to_vec()
        } else {
            value.to_vec()
        };
        distinct.dedup();
        crate::minhash::sign(&distinct, seeds)
    }
}
