//! Lowering-coverage measurement. A construct the frontend doesn't handle lands
//! as a `Raw` node (its children are still lowered, so a `Raw` marks one
//! *unhandled site*). The `Raw` ratio and a histogram of which surface kinds hit
//! `Raw` per language tell us exactly where the IL is weak — and what to fix next.

use crate::lower::is_intentional_raw_boundary_tag;
use nose_il::{Corpus, Interner, Node, NodeKind, Payload};
use rustc_hash::FxHashMap;
use serde::Serialize;

#[derive(Serialize)]
pub struct LangCoverage {
    pub lang: String,
    pub files: usize,
    pub nodes: usize,
    pub raw_nodes: usize,
    pub raw_ratio: f64,
    /// `Raw` nodes that are deliberate fail-closed boundaries (protocol/preprocessor) —
    /// by design, not a lowering gap. `raw_nodes - boundary_raw` is the genuine fixable surface.
    pub boundary_raw: usize,
}

#[derive(Serialize)]
pub struct UnhandledKind {
    pub lang: String,
    pub surface_kind: String,
    pub count: usize,
    /// This surface kind is a deliberate fail-closed boundary, not a fixable lowering gap.
    pub boundary: bool,
}

#[derive(Serialize)]
pub struct CoverageReport {
    pub files: usize,
    pub total_nodes: usize,
    pub raw_nodes: usize,
    pub raw_ratio: f64,
    /// Of `raw_nodes`, how many are deliberate fail-closed boundaries (by design). The genuine
    /// fixable lowering gap is `raw_nodes - boundary_raw`.
    pub boundary_raw: usize,
    pub per_lang: Vec<LangCoverage>,
    pub top_unhandled: Vec<UnhandledKind>,
}

pub struct RawSurface {
    pub surface_kind: String,
    pub boundary: bool,
}

pub fn raw_node_surface(interner: &Interner, node: &Node) -> Option<RawSurface> {
    if node.kind != NodeKind::Raw {
        return None;
    }
    let surface_kind = match node.payload {
        Payload::Name(sym) => interner.resolve(sym).to_string(),
        _ => "<unknown>".to_string(),
    };
    let boundary = is_intentional_raw_boundary_tag(&surface_kind);
    Some(RawSurface {
        surface_kind,
        boundary,
    })
}

/// Compute coverage over a (raw) corpus. `top` caps the unhandled-kind histogram.
pub fn coverage(corpus: &Corpus, top: usize) -> CoverageReport {
    let mut total_nodes = 0usize;
    let mut raw_nodes = 0usize;
    let mut boundary_raw = 0usize;

    // per-language aggregates and (lang, surface-kind) -> count
    let mut lang_files: FxHashMap<&'static str, usize> = FxHashMap::default();
    let mut lang_nodes: FxHashMap<&'static str, usize> = FxHashMap::default();
    let mut lang_raw: FxHashMap<&'static str, usize> = FxHashMap::default();
    let mut lang_boundary: FxHashMap<&'static str, usize> = FxHashMap::default();
    let mut unhandled: FxHashMap<(&'static str, String), usize> = FxHashMap::default();

    for il in &corpus.files {
        let lang = il.meta.lang.name();
        *lang_files.entry(lang).or_default() += 1;
        *lang_nodes.entry(lang).or_default() += il.nodes.len();
        total_nodes += il.nodes.len();

        for node in &il.nodes {
            if let Some(raw) = raw_node_surface(&corpus.interner, node) {
                raw_nodes += 1;
                *lang_raw.entry(lang).or_default() += 1;
                if raw.boundary {
                    boundary_raw += 1;
                    *lang_boundary.entry(lang).or_default() += 1;
                }
                *unhandled.entry((lang, raw.surface_kind)).or_default() += 1;
            }
        }
    }

    let mut per_lang: Vec<LangCoverage> = lang_nodes
        .keys()
        .map(|&lang| {
            let nodes = lang_nodes[lang];
            let raw = lang_raw.get(lang).copied().unwrap_or(0);
            LangCoverage {
                lang: lang.to_string(),
                files: lang_files.get(lang).copied().unwrap_or(0),
                nodes,
                raw_nodes: raw,
                raw_ratio: ratio(raw, nodes),
                boundary_raw: lang_boundary.get(lang).copied().unwrap_or(0),
            }
        })
        .collect();
    per_lang.sort_by(|a, b| b.raw_ratio.total_cmp(&a.raw_ratio));

    let mut top_unhandled: Vec<UnhandledKind> = unhandled
        .into_iter()
        .map(|((lang, surface_kind), count)| UnhandledKind {
            lang: lang.to_string(),
            boundary: is_intentional_raw_boundary_tag(&surface_kind),
            surface_kind,
            count,
        })
        .collect();
    top_unhandled.sort_by_key(|u| std::cmp::Reverse(u.count));
    top_unhandled.truncate(top);

    CoverageReport {
        files: corpus.files.len(),
        total_nodes,
        raw_nodes,
        raw_ratio: ratio(raw_nodes, total_nodes),
        boundary_raw,
        per_lang,
        top_unhandled,
    }
}

fn ratio(num: usize, den: usize) -> f64 {
    if den == 0 {
        0.0
    } else {
        (num as f64 / den as f64 * 100_000.0).round() / 100_000.0
    }
}
