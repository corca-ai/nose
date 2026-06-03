//! Criterion benchmarks for the detection pipeline on a synthetic, deterministic
//! corpus (no external repos needed). Tracks regressions in the hot paths.

use criterion::{criterion_group, criterion_main, Criterion};
use nose_detect::{detect, DetectOptions, StructuralDetector};
use nose_il::{Corpus, FileId, Interner, Lang};

/// Build an N-function Python corpus with a few recurring shapes (so detection has
/// real candidates), parameterized to avoid trivial identity.
fn synthetic_corpus(n: usize) -> Corpus {
    let interner = Interner::new();
    let shapes = [
        |k: usize| {
            format!("def f{k}(items):\n    total = 0\n    for x in items:\n        if x > 0:\n            total = total + x * x\n    return total\n")
        },
        |k: usize| {
            format!("def g{k}(xs, n):\n    c = 0\n    for w in xs:\n        if len(w) > n:\n            c += 1\n    return c\n")
        },
        |k: usize| {
            format!("def h{k}(a, b):\n    s = 0\n    for i in range(len(a)):\n        s += a[i] * b[i]\n    return s\n")
        },
    ];
    let files = (0..n)
        .map(|i| {
            let src = shapes[i % shapes.len()](i);
            let path = format!("f{i}.py");
            nose_frontend::lower_source(
                FileId(i as u32),
                &path,
                src.as_bytes(),
                Lang::Python,
                &interner,
            )
            .unwrap()
        })
        .collect();
    Corpus::new(interner, files)
}

fn bench_detect(c: &mut Criterion) {
    let opts = DetectOptions {
        min_tokens: 12,
        ..Default::default()
    };
    let mut group = c.benchmark_group("detect");
    for &n in &[200usize, 1000] {
        let corpus = synthetic_corpus(n);
        group.bench_function(format!("strict/{n}"), |b| {
            b.iter(|| {
                detect(
                    &corpus,
                    &opts,
                    &StructuralDetector::strict(opts.jaccard_weight),
                )
            })
        });
        group.bench_function(format!("candidates/{n}"), |b| {
            b.iter(|| {
                detect(
                    &corpus,
                    &opts,
                    &StructuralDetector::candidates(opts.jaccard_weight),
                )
            })
        });
    }
    group.finish();
}

criterion_group!(benches, bench_detect);
criterion_main!(benches);
