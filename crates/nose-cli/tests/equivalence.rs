//! IL-equivalence tests — the heart of correctness. Semantically-equivalent
//! snippets must normalize to the same structural hash; genuinely different code
//! must not. Also covers provenance and an end-to-end detection smoke test.

use nose_detect::{detect, DetectOptions, StructuralDetector};
use nose_il::{Corpus, FileId, Interner, Lang, NodeId, UnitKind};
use nose_normalize::{normalize, subtree_hashes, NormalizeOptions};

/// Normalize `src` and return the structural hash of its first function/method
/// unit. A shared `interner` keeps field-name symbols comparable across calls.
fn unit_hash(interner: &Interner, src: &str, lang: Lang) -> u64 {
    let il = nose_frontend::lower_source(FileId(0), "t", src.as_bytes(), lang, interner).unwrap();
    let n = normalize(&il, interner, &NormalizeOptions::default());
    let hashes = subtree_hashes(&n, interner);
    let root = first_func(&n);
    hashes[root.0 as usize]
}

fn first_func(il: &nose_il::Il) -> NodeId {
    il.units
        .iter()
        .find(|u| matches!(u.kind, UnitKind::Function | UnitKind::Method))
        .map(|u| u.root)
        .expect("expected a function unit")
}

fn count_nodes(il: &nose_il::Il, root: NodeId, kind: Option<nose_il::NodeKind>) -> usize {
    let own = match kind {
        Some(kind) => usize::from(il.node(root).kind == kind),
        None => 1,
    };
    own + il
        .children(root)
        .iter()
        .map(|child| count_nodes(il, *child, kind))
        .sum::<usize>()
}

#[path = "equivalence/algebra_laws.rs"]
mod algebra_laws;
#[path = "equivalence/branch_selection.rs"]
mod branch_selection;
#[path = "equivalence/call_binding_boundaries.rs"]
mod call_binding_boundaries;
#[path = "equivalence/collection_builders.rs"]
mod collection_builders;
#[path = "equivalence/collection_membership.rs"]
mod collection_membership;
#[path = "equivalence/collection_streams.rs"]
mod collection_streams;
#[path = "equivalence/convergence_probes.rs"]
mod convergence_probes;
#[path = "equivalence/css_surfaces.rs"]
mod css_surfaces;
#[path = "equivalence/imported_collection_membership.rs"]
mod imported_collection_membership;
#[path = "equivalence/imported_js_ts_collection_membership.rs"]
mod imported_js_ts_collection_membership;
#[path = "equivalence/imported_js_ts_map_defaults.rs"]
mod imported_js_ts_map_defaults;
#[path = "equivalence/inline_and_anchors.rs"]
mod inline_and_anchors;
#[path = "equivalence/iteration_contracts.rs"]
mod iteration_contracts;
#[path = "equivalence/language_operator_boundaries.rs"]
mod language_operator_boundaries;
#[path = "equivalence/language_surfaces.rs"]
mod language_surfaces;
#[path = "equivalence/literal_map_defaults.rs"]
mod literal_map_defaults;
#[path = "equivalence/literal_value_boundaries.rs"]
mod literal_value_boundaries;
#[path = "equivalence/loops_and_reductions.rs"]
mod loops_and_reductions;
#[path = "equivalence/map_default_boundaries.rs"]
mod map_default_boundaries;
#[path = "equivalence/markup_surfaces.rs"]
mod markup_surfaces;
#[path = "equivalence/numeric_law_boundaries.rs"]
mod numeric_law_boundaries;
#[path = "equivalence/numeric_scalars.rs"]
mod numeric_scalars;
#[path = "equivalence/operator_boundaries.rs"]
mod operator_boundaries;
#[path = "equivalence/option_boundaries.rs"]
mod option_boundaries;
#[path = "equivalence/protocol_boundaries.rs"]
mod protocol_boundaries;
#[path = "equivalence/receiver_membership.rs"]
mod receiver_membership;
#[path = "equivalence/recursion_iteration.rs"]
mod recursion_iteration;
#[path = "equivalence/reinvented_helper_boundaries.rs"]
mod reinvented_helper_boundaries;
#[path = "equivalence/semantic_law_boundaries.rs"]
mod semantic_law_boundaries;
#[path = "equivalence/source_identity_boundaries.rs"]
mod source_identity_boundaries;
#[path = "equivalence/syntax_surfaces.rs"]
mod syntax_surfaces;
#[path = "equivalence/value_graph_boundaries.rs"]
mod value_graph_boundaries;
#[path = "equivalence/value_graph_core.rs"]
mod value_graph_core;
#[path = "equivalence/value_graph_try_effects.rs"]
mod value_graph_try_effects;

#[test]
fn array_element_swap_does_not_merge_with_clobber() {
    // #337: in-place element mutation. `swap` (`t=a[i]; a[i]=a[j]; a[j]=t`) and `clobber`
    // (`a[i]=a[j]; a[j]=a[i]`) differ: clobber's second `a[i]` read observes the FIRST write,
    // while swap captured the pre-write value in `t`. The value graph forwards a post-write
    // read of `a[i]` to the written value (`index_env`), so the two element-write traces — and
    // thus the fingerprints — differ. They previously SHARED a fingerprint (the no-mutation
    // store model treated every `a[i]` read as the pre-write value): a false merge.
    let i = Interner::new();
    let swap = "def swap(a, i, j):\n    t = a[i]\n    a[i] = a[j]\n    a[j] = t\n";
    let clobber = "def clobber(a, i, j):\n    a[i] = a[j]\n    a[j] = a[i]\n";
    assert_ne!(
        value_fp(&i, swap, Lang::Python),
        value_fp(&i, clobber, Lang::Python),
        "swap must not merge with clobber (post-write reads forward to the written value)"
    );
    // Control — two structurally-identical swaps still converge (read-forwarding is structure-
    // sensitive, not a blanket exclusion of indexed stores, so genuine clones are unaffected).
    let swap2 = "def s2(b, m, n):\n    t = b[m]\n    b[m] = b[n]\n    b[n] = t\n";
    assert_eq!(
        value_fp(&i, swap, Lang::Python),
        value_fp(&i, swap2, Lang::Python),
        "identical swaps still converge"
    );
}

/// Value-graph fingerprint of the first function unit.
fn value_fp(interner: &Interner, src: &str, lang: Lang) -> Vec<u64> {
    let il = nose_frontend::lower_source(FileId(0), "t", src.as_bytes(), lang, interner).unwrap();
    let n = normalize(&il, interner, &NormalizeOptions::default());
    nose_normalize::value_fingerprint(&n, first_func(&n), interner)
}

fn return_fp(interner: &Interner, src: &str, lang: Lang) -> Vec<u64> {
    let il = nose_frontend::lower_source(FileId(0), "t", src.as_bytes(), lang, interner).unwrap();
    let n = normalize(&il, interner, &NormalizeOptions::default());
    nose_normalize::value_fingerprint_lits(&n, first_func(&n), interner).2
}

fn value_anchors(interner: &Interner, src: &str, lang: Lang) -> Vec<u64> {
    let il = nose_frontend::lower_source(FileId(0), "t", src.as_bytes(), lang, interner).unwrap();
    let n = normalize(&il, interner, &NormalizeOptions::default());
    nose_normalize::value_anchors(&n, first_func(&n), interner)
        .into_iter()
        .map(|anchor| anchor.hash)
        .collect()
}

fn shares_any(a: &[u64], b: &[u64]) -> bool {
    a.iter().any(|x| b.contains(x))
}

fn full_anchors(interner: &Interner, src: &str, lang: Lang) -> Vec<nose_normalize::Anchor> {
    let il = nose_frontend::lower_source(FileId(0), "t", src.as_bytes(), lang, interner).unwrap();
    let n = normalize(&il, interner, &NormalizeOptions::default());
    nose_normalize::value_anchors(&n, first_func(&n), interner)
}

#[test]
fn sub_dag_anchor_carries_source_line_range_of_the_shared_computation() {
    // A heavy sub-DAG anchor records WHERE its computation lives (line range), so a partial clone
    // can report the shared lines. The SAME computation placed on different lines in two functions
    // yields the same anchor hash but each unit's own line range.
    let i = Interner::new();
    let body = |head: &str| {
        format!(
            "{head}\n  const totals = items.map(i => i.price * i.qty).reduce((s, x) => s + x, 0);\n  const tax = totals * 0.1;\n  const shipping = totals > 100 ? 0 : 15;\n  const grand = totals + tax + shipping;\n  log(grand);\n  return grand;\n}}\n"
        )
    };
    // Same heavy computation, but two extra lines push it down in `g`.
    let a = body("function f(items) {");
    let b = body("function g(items) {\n  log(1);\n  log(2);");
    let aa = full_anchors(&i, &a, Lang::TypeScript);
    let bb = full_anchors(&i, &b, Lang::TypeScript);
    // At least one anchor carries a real (non-zero) line range.
    assert!(
        aa.iter().any(|x| x.line_start > 0),
        "an anchor should record its source line range, got {aa:?}",
    );
    // The shared computation produces the same hash in both, on each unit's own line.
    let sh = aa
        .iter()
        .filter(|x| bb.iter().any(|y| y.hash == x.hash))
        .max_by_key(|x| x.weight)
        .expect("the shared computation should produce a common anchor hash");
    let in_b = bb.iter().find(|y| y.hash == sh.hash).unwrap();
    // Each unit reports the shared computation at ITS OWN location: the two leading `log` lines in
    // `g` shift every line down by exactly 2.
    assert!(
        sh.line_start > 0 && in_b.line_start > 0,
        "both carry a real line: {sh:?} / {in_b:?}"
    );
    assert_eq!(
        in_b.line_start,
        sh.line_start + 2,
        "g's shared computation is 2 lines below f's (the two extra `log` lines)",
    );
}

fn value_fp_named(interner: &Interner, src: &str, lang: Lang, name: &str) -> Vec<u64> {
    let il = nose_frontend::lower_source(FileId(0), "t", src.as_bytes(), lang, interner).unwrap();
    let n = normalize(&il, interner, &NormalizeOptions::default());
    let root = n
        .units
        .iter()
        .find(|unit| {
            unit.name
                .is_some_and(|symbol| interner.resolve(symbol) == name)
        })
        .map(|unit| unit.root)
        .unwrap_or_else(|| panic!("expected function unit named {name}"));
    nose_normalize::value_fingerprint(&n, root, interner)
}

fn class_value_fp(interner: &Interner, src: &str, lang: Lang, name: &str) -> Vec<u64> {
    let il = nose_frontend::lower_source(FileId(0), "t", src.as_bytes(), lang, interner).unwrap();
    let n = normalize(&il, interner, &NormalizeOptions::default());
    let root = n
        .units
        .iter()
        .find(|unit| {
            matches!(unit.kind, UnitKind::Class)
                && unit
                    .name
                    .is_some_and(|symbol| interner.resolve(symbol) == name)
        })
        .map(|unit| unit.root)
        .unwrap_or_else(|| panic!("expected class unit named {name}"));
    nose_normalize::value_fingerprint(&n, root, interner)
}

fn corpus_value_fp(corpus: &Corpus, path_suffix: &str, name: &str) -> Vec<u64> {
    let il = corpus
        .files
        .iter()
        .find(|il| il.meta.path.ends_with(path_suffix))
        .unwrap_or_else(|| panic!("expected corpus file ending with {path_suffix}"));
    let n = normalize(il, &corpus.interner, &NormalizeOptions::default());
    let root = n
        .units
        .iter()
        .find(|unit| {
            unit.name
                .is_some_and(|symbol| corpus.interner.resolve(symbol) == name)
        })
        .map(|unit| unit.root)
        .unwrap_or_else(|| panic!("expected function unit named {name} in {path_suffix}"));
    nose_normalize::value_fingerprint(&n, root, &corpus.interner)
}

/// Write `files` into a fresh per-process temp dir named after `tag` and lower
/// them together as one corpus. Callers remove the returned dir when done.
fn lower_temp_corpus(tag: &str, files: &[(&str, &str)]) -> (std::path::PathBuf, Corpus) {
    let dir = std::env::temp_dir().join(format!("{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (name, contents) in files {
        std::fs::write(dir.join(name), contents).unwrap();
    }
    let corpus = nose_frontend::lower_corpus_many(&[dir.as_path()]);
    (dir, corpus)
}
