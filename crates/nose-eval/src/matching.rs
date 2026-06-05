//! Maximum-weight bipartite matching via min-cost augmenting paths (SPFA on a
//! residual graph), augmenting only while a negative-cost (i.e. weight-adding)
//! path remains. This maximizes total matched weight without forcing maximum
//! cardinality.

const SCALE: f64 = 1_000_000.0;

struct Edge {
    to: usize,
    rev: usize,
    cap: i32,
    cost: i64,
}

/// `edges`: `(left, right, weight>0)`. Returns matched `(left, right, weight)`.
pub(crate) fn max_weight_matching(
    n_left: usize,
    n_right: usize,
    edges: &[(usize, usize, f64)],
) -> Vec<(usize, usize, f64)> {
    if n_left == 0 || n_right == 0 || edges.is_empty() {
        return Vec::new();
    }
    let source = 0;
    let left0 = 1;
    let right0 = 1 + n_left;
    let sink = 1 + n_left + n_right;
    let n = sink + 1;
    let mut g: Vec<Vec<Edge>> = (0..n).map(|_| Vec::new()).collect();

    let add = |g: &mut Vec<Vec<Edge>>, u: usize, v: usize, cap: i32, cost: i64| {
        let ru = g[v].len();
        let rv = g[u].len();
        g[u].push(Edge {
            to: v,
            rev: ru,
            cap,
            cost,
        });
        g[v].push(Edge {
            to: u,
            rev: rv,
            cap: 0,
            cost: -cost,
        });
    };

    for i in 0..n_left {
        add(&mut g, source, left0 + i, 1, 0);
    }
    for j in 0..n_right {
        add(&mut g, right0 + j, sink, 1, 0);
    }
    for &(l, r, w) in edges {
        let cost = -((w * SCALE).round() as i64);
        add(&mut g, left0 + l, right0 + r, 1, cost);
    }

    // Augment along shortest (most negative) paths while they improve cost.
    loop {
        let mut dist = vec![i64::MAX; n];
        let mut in_q = vec![false; n];
        let mut prevv = vec![usize::MAX; n];
        let mut preve = vec![usize::MAX; n];
        dist[source] = 0;
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(source);
        in_q[source] = true;
        while let Some(u) = queue.pop_front() {
            in_q[u] = false;
            let du = dist[u];
            for (ei, e) in g[u].iter().enumerate() {
                if e.cap > 0 && du != i64::MAX && du + e.cost < dist[e.to] {
                    dist[e.to] = du + e.cost;
                    prevv[e.to] = u;
                    preve[e.to] = ei;
                    if !in_q[e.to] {
                        in_q[e.to] = true;
                        queue.push_back(e.to);
                    }
                }
            }
        }
        if dist[sink] == i64::MAX || dist[sink] >= 0 {
            break; // no weight-adding augmenting path
        }
        // augment one unit along the path
        let mut v = sink;
        while v != source {
            let u = prevv[v];
            let ei = preve[v];
            g[u][ei].cap -= 1;
            let rev = g[u][ei].rev;
            g[v][rev].cap += 1;
            v = u;
        }
    }

    // Recover matching: forward left→right edges that are now saturated.
    let mut out = Vec::new();
    for i in 0..n_left {
        for e in &g[left0 + i] {
            if e.cost < 0 && e.cap == 0 && e.to >= right0 && e.to < sink {
                let j = e.to - right0;
                out.push((i, j, (-e.cost) as f64 / SCALE));
            }
        }
    }
    out
}
