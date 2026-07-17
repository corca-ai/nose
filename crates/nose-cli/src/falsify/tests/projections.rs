use super::*;

fn identity_with_arity(arity: u32, negate: bool) -> (Il, Interner, NodeId) {
    let interner = Interner::new();
    let span = Span::synthetic(FileId(0));
    let mut builder = IlBuilder::new(FileId(0));
    let params: Vec<_> = (0..arity)
        .map(|cid| builder.add(NodeKind::Param, Payload::Cid(cid), span, &[]))
        .collect();
    let value = builder.add(NodeKind::Var, Payload::Cid(0), span, &[]);
    let value = if negate {
        builder.add(NodeKind::UnOp, Payload::Op(Op::Neg), span, &[value])
    } else {
        value
    };
    let ret = builder.add(NodeKind::Return, Payload::None, span, &[value]);
    let mut children = params;
    children.push(ret);
    let root = builder.add(NodeKind::Func, Payload::None, span, &children);
    (finish(builder, root, Lang::Python), interner, root)
}

#[test]
fn falsifier_replays_asymmetric_arity_only_after_a_trailing_unused_proof() {
    use nose_detect::OracleInputProjection::{Declared, UnusedTrailing};

    let (short, interner, short_root) = identity_with_arity(1, false);
    let (long_different, _, long_different_root) = identity_with_arity(2, true);
    assert!(matches!(
        falsify_pair_with_projections(
            &short,
            short_root,
            &long_different,
            long_different_root,
            &interner,
            &[],
            256,
            DEFAULT_FALSIFY_SEED,
            &[Declared],
            &[Declared, UnusedTrailing],
            false,
            true,
        ),
        FalsifyOutcome::Witness(_)
    ));

    let (long_equal, _, long_equal_root) = identity_with_arity(2, false);
    assert!(matches!(
        falsify_pair_with_projections(
            &short,
            short_root,
            &long_equal,
            long_equal_root,
            &interner,
            &[],
            256,
            DEFAULT_FALSIFY_SEED,
            &[Declared],
            &[Declared, UnusedTrailing],
            false,
            true,
        ),
        FalsifyOutcome::Exhausted { cases } if cases > 0
    ));
    assert!(matches!(
        falsify_pair_with_projections(
            &short,
            short_root,
            &long_equal,
            long_equal_root,
            &interner,
            &[],
            256,
            DEFAULT_FALSIFY_SEED,
            &[Declared],
            &[UnusedTrailing, Declared],
            false,
            true,
        ),
        FalsifyOutcome::Skipped { .. }
    ));
}
