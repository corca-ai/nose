mod source_locations;
use super::{graded_witness, model::MAX_NODES};
use nose_normalize::{ValueDag, VgNode, VgOp, VgReferent, VgSink, VgSinkKind, VG_PROTOCOL_AWAIT};

fn node(op: VgOp, key: u64, args: &[u32], hash: u64) -> VgNode {
    VgNode {
        op,
        key,
        args: args.to_vec(),
        hash,
        line_start: 0,
        line_end: 0,
    }
}

fn name_key(name: &str) -> u64 {
    name.bytes().fold(1469598103934665603u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(1099511628211)
    })
}

fn referent(name: &str, id: Option<u64>) -> VgReferent {
    VgReferent {
        name: name.to_string(),
        name_key: name_key(name),
        referent: id,
    }
}

/// A `Return` over a chain of `len` non-commutative `Un` ops wrapping `Const(key)`,
/// so two such DAGs differ only in the const while staying above the low-substance
/// floor. Node 0 = Const, 1..len = the Un chain, top is the return value.
fn chain(len: u32, const_key: u64, salt: u64) -> ValueDag {
    let mut nodes = vec![node(VgOp::Const, const_key, &[], 1000 + const_key + salt)];
    for i in 1..=len {
        nodes.push(node(VgOp::Un, 7, &[i - 1], 2000 + u64::from(i) + salt));
    }
    ValueDag {
        sinks: vec![VgSink {
            kind: VgSinkKind::Return,
            value: len,
            effect_ord: None,
        }],
        referents: vec![],
        nodes,
    }
}

#[test]
fn identical_dags_are_equal_modulo_zero_holes() {
    let w = graded_witness(&chain(12, 5, 0), &chain(12, 5, 0), false, false).unwrap();
    assert_eq!(w.holes, 0);
    assert!(w.equal_modulo_holes);
    assert!(w.referent_mismatches.is_empty());
    assert!(!w.modeled_caveat);
}

#[test]
fn single_differing_literal_is_one_leaf_hole() {
    // Same chain, different const at the bottom: one literal hole, still clean.
    let w = graded_witness(&chain(12, 5, 0), &chain(12, 9, 1), false, false).unwrap();
    assert_eq!(w.holes, 1);
    assert_eq!(w.spots.len(), 1);
    assert_eq!(w.spots[0].class, "literal");
    assert!(w.equal_modulo_holes);
}

/// An async↔sync twin pair: side A wraps the inner `Call` in `await`
/// (`Opaque(VG_PROTOCOL_AWAIT,[call])`), side B uses the bare call; both return a `len`-deep
/// `Un` chain over that value, so they sit above the low-substance floor and differ ONLY at
/// the await point.
fn async_sync_twin(len: u32) -> (ValueDag, ValueDag) {
    let mk = |nodes: Vec<VgNode>, top: u32| ValueDag {
        sinks: vec![VgSink {
            kind: VgSinkKind::Return,
            value: top,
            effect_ord: None,
        }],
        referents: vec![],
        nodes,
    };
    // sync (B): Const -> Call -> Un chain over the Call.
    let mut sync = vec![
        node(VgOp::Const, 5, &[], 100),
        node(VgOp::Call, 42, &[0], 200),
    ];
    for i in 0..len {
        let arg = sync.len() as u32 - 1;
        sync.push(node(VgOp::Un, 7, &[arg], 300 + u64::from(i)));
    }
    let sync_top = sync.len() as u32 - 1;
    // async (A): Const -> Call -> await(Call) -> Un chain over the await. Same Call hash as
    // B's (so the operand matches after the gate); the Un chain uses distinct hashes so the
    // witness recurses down to the await point instead of fast-matching.
    let mut asy = vec![
        node(VgOp::Const, 5, &[], 100),
        node(VgOp::Call, 42, &[0], 200),
        node(VgOp::Opaque, VG_PROTOCOL_AWAIT, &[1], 250),
    ];
    for i in 0..len {
        let arg = asy.len() as u32 - 1;
        asy.push(node(VgOp::Un, 7, &[arg], 400 + u64::from(i)));
    }
    let asy_top = asy.len() as u32 - 1;
    (mk(asy, asy_top), mk(sync, sync_top))
}

#[test]
fn async_await_aligns_with_sync_twin_as_async_mirror() {
    let (a, b) = async_sync_twin(12);
    let w = graded_witness(&a, &b, false, false).unwrap();
    assert!(
        w.patterns.contains(&"async-mirror"),
        "expected async-mirror, got {:?}",
        w.patterns
    );
    assert!(
        !w.equal_modulo_holes,
        "async↔sync is a transformation twin, never a behavioral equivalence"
    );
    assert!(w.spots.iter().any(|s| s.class == "async-mirror"));
}

#[test]
fn both_sides_await_is_not_async_mirror() {
    // Two identical async copies (both await) align cleanly — no one-sided await, no mirror.
    let (a, _) = async_sync_twin(12);
    let w = graded_witness(&a, &a, false, false).unwrap();
    assert!(!w.patterns.contains(&"async-mirror"));
    assert!(w.equal_modulo_holes);
}

#[test]
fn lossy_lowering_marks_modeled_caveat() {
    let w = graded_witness(&chain(12, 5, 0), &chain(12, 5, 0), true, false).unwrap();
    assert!(w.modeled_caveat);
}

#[test]
fn tiny_units_are_low_substance_not_clean() {
    // A 2-node difference below the substance floor is not an equal-modulo claim.
    let mut a = chain(2, 5, 0);
    let mut b = chain(2, 9, 1);
    a.referents.clear();
    b.referents.clear();
    let w = graded_witness(&a, &b, false, false).unwrap();
    assert!(w.patterns.contains(&"low-substance"));
    assert!(!w.equal_modulo_holes);
}

#[test]
fn disjoint_referents_demote_the_witness() {
    // Identical graphs, but a shared name resolves to different definitions.
    let mut a = chain(12, 5, 0);
    let mut b = chain(12, 5, 0);
    a.referents.push(referent("equals", Some(111)));
    b.referents.push(referent("equals", Some(222)));
    let w = graded_witness(&a, &b, false, false).unwrap();
    assert_eq!(w.referent_mismatches, vec!["equals".to_string()]);
    assert!(w.patterns.contains(&"referent-mismatch"));
    assert!(!w.equal_modulo_holes);
}

#[test]
fn unresolved_shared_name_is_a_scoped_caveat() {
    let mut a = chain(12, 5, 0);
    let mut b = chain(12, 5, 0);
    a.referents.push(referent("globalThing", None));
    b.referents.push(referent("globalThing", None));
    let w = graded_witness(&a, &b, false, false).unwrap();
    assert_eq!(w.caveat_names, vec!["globalThing".to_string()]);
    assert!(w.referent_mismatches.is_empty());
}

#[test]
fn oversized_pair_fails_closed_to_no_witness() {
    let big = chain(MAX_NODES as u32 + 5, 5, 0);
    assert!(graded_witness(&big, &chain(12, 5, 0), false, false).is_none());
}

#[test]
fn extra_return_sink_is_a_superset_pattern() {
    let mut a = chain(12, 5, 0);
    let b = chain(12, 5, 0);
    // Give `a` a second return sink with no counterpart in `b`.
    a.sinks.push(VgSink {
        kind: VgSinkKind::Return,
        value: 0,
        effect_ord: None,
    });
    let w = graded_witness(&a, &b, false, false).unwrap();
    assert!(w.holes >= 1);
    assert!(w.patterns.contains(&"sink-superset-a"));
    assert!(!w.equal_modulo_holes);
}
