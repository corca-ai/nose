use super::support::loc;
use crate::{AcceptedEdge, Group};

fn scalar_winners(input: &[AcceptedEdge]) -> Vec<AcceptedEdge> {
    let mut edges = input
        .iter()
        .filter(|edge| edge.left < 8 && edge.right < 8 && edge.left != edge.right)
        .map(|edge| AcceptedEdge {
            left: edge.left.min(edge.right),
            right: edge.left.max(edge.right),
            ..edge.clone()
        })
        .collect::<Vec<_>>();
    edges.sort_by(|a, b| {
        a.left
            .cmp(&b.left)
            .then(a.right.cmp(&b.right))
            .then_with(|| b.score.total_cmp(&a.score))
            .then(a.witness_kind.cmp(b.witness_kind))
    });
    edges.dedup_by(|a, b| a.left == b.left && a.right == b.right);
    edges
}

#[test]
fn parallel_site_order_preserves_scalar_winners_and_float_bits() {
    let group = Group {
        score: 1.0,
        members: (0..8)
            .map(|i| loc(&format!("{i}.rs"), 1, 10, "rust"))
            .collect(),
        semantic_laws: Vec::new(),
        abstraction_witness: None,
        witness: None,
    };
    let scores = [
        vec![0.125, 0.375, 0.9],
        vec![-0.0, 0.0],
        vec![f64::NEG_INFINITY, -1.0, 1.0, f64::INFINITY],
        vec![
            f64::from_bits(0x7ff8_0000_0000_0001),
            f64::from_bits(0x7ff8_0000_0000_0002),
            f64::from_bits(0xfff8_0000_0000_0001),
        ],
    ];
    let kinds = [
        "structural-similarity",
        "shared-sub-dag",
        "exact-value-graph",
    ];
    let bits = |edges: &[AcceptedEdge]| {
        edges
            .iter()
            .map(|edge| {
                (
                    edge.left,
                    edge.right,
                    edge.score.to_bits(),
                    edge.witness_kind,
                )
            })
            .collect::<Vec<_>>()
    };
    for threads in [1, 3] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        for palette in &scores {
            let mut seed = 0x7369_7465_u64;
            let mut next = || {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                seed
            };
            let edges = (0..8192)
                .map(|_| AcceptedEdge {
                    left: (next() % 12) as u32,
                    right: (next() % 12) as u32,
                    score: palette[(next() as usize) % palette.len()],
                    witness_kind: kinds[(next() as usize) % kinds.len()],
                })
                .collect::<Vec<_>>();
            let expected = scalar_winners(&edges);
            let actual = pool.install(|| {
                crate::report::collapsed_accepted_edges(&group, &group.members, &edges)
            });
            assert_eq!(bits(&actual), bits(&expected));
        }
    }
}
