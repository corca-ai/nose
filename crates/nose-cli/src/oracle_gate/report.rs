use super::*;

/// One interpretable record per generated source file (each holds exactly one function).
pub(super) struct GateUnit {
    fp: Vec<u64>,
    beh_hash: u64,
    trivial: bool,
}

pub(super) fn gate_units(
    corpus: &Corpus,
    battery: &[Vec<nose_normalize::Value>],
) -> std::collections::HashMap<String, GateUnit> {
    let opts = nose_normalize::NormalizeOptions::default();
    let oracle_opts = nose_normalize::NormalizeOptions {
        oracle: true,
        ..opts
    };
    let mut units = std::collections::HashMap::new();
    for il in &corpus.files {
        let n = nose_normalize::normalize(il, &corpus.interner, &opts);
        let core = nose_normalize::normalize(il, &corpus.interner, &oracle_opts);
        let core_func = func_span_index(&core);
        for u in &n.units {
            let root = u.root;
            if n.kind(root) != nose_il::NodeKind::Func {
                continue;
            }
            let span0 = n.node(root).span;
            let Some(&core_root) = core_func.get(&(span0.start_byte, span0.end_byte)) else {
                continue;
            };
            let (fp, contracts) =
                nose_normalize::value_fingerprint_and_contracts(&n, root, &corpus.interner);
            if fp.is_empty() {
                continue;
            }
            let mut path_cap = false;
            let Some(beh) = run_battery(
                &core,
                &corpus.interner,
                core_root,
                battery,
                &contracts,
                &mut path_cap,
            ) else {
                continue;
            };
            units.insert(
                manifest_key(&il.meta.path),
                GateUnit {
                    fp,
                    beh_hash: behavior_hash(&beh),
                    trivial: is_trivial_behavior(&beh),
                },
            );
        }
    }
    units
}

#[derive(serde::Deserialize)]
struct GateSide {
    path: String,
}

#[derive(serde::Deserialize)]
struct GateItem {
    left: GateSide,
    right: GateSide,
    semantic_status: String,
    split: String,
}

#[derive(serde::Deserialize)]
pub(super) struct GateManifest {
    items: Vec<GateItem>,
}

struct GateTally {
    pairs: usize,
    fp_merge: usize,
    beh_merge: usize,
    beh_only: usize,
}

impl GateTally {
    fn new() -> Self {
        Self {
            pairs: 0,
            fp_merge: 0,
            beh_merge: 0,
            beh_only: 0,
        }
    }
}

pub(super) struct GateOutcome {
    pos: GateTally,
    neg: GateTally,
    pos_heldout: usize,
    pos_heldout_beh_only: usize,
    uninterp_pairs: usize,
}

pub(super) fn tally_gate(
    m: &GateManifest,
    units: &std::collections::HashMap<String, GateUnit>,
) -> GateOutcome {
    let mut out = GateOutcome {
        pos: GateTally::new(),
        neg: GateTally::new(),
        pos_heldout: 0,
        pos_heldout_beh_only: 0,
        uninterp_pairs: 0,
    };
    for it in &m.items {
        let (lk, rk) = (manifest_key(&it.left.path), manifest_key(&it.right.path));
        let (Some(lu), Some(ru)) = (units.get(&lk), units.get(&rk)) else {
            out.uninterp_pairs += 1;
            continue;
        };
        let positive = it.semantic_status == "equivalent";
        let tally = if positive { &mut out.pos } else { &mut out.neg };
        tally.pairs += 1;
        let fp_merge = lu.fp == ru.fp;
        let beh_merge = !lu.trivial && !ru.trivial && lu.beh_hash == ru.beh_hash;
        if fp_merge {
            tally.fp_merge += 1;
        }
        if beh_merge {
            tally.beh_merge += 1;
        }
        if beh_merge && !fp_merge {
            tally.beh_only += 1;
            if positive && it.split == "heldout" {
                out.pos_heldout_beh_only += 1;
            }
        }
        if positive && it.split == "heldout" {
            out.pos_heldout += 1;
        }
    }
    out
}

pub(super) fn print_gate_report(
    battery_kind: BatteryKind,
    battery_rows: usize,
    outcome: &GateOutcome,
) {
    let GateOutcome {
        pos,
        neg,
        pos_heldout,
        pos_heldout_beh_only,
        uninterp_pairs,
    } = outcome;
    let kind = match battery_kind {
        BatteryKind::Standard => "standard (leap 2)",
        BatteryKind::Wide => "wide (leap 3)",
    };
    println!("=== behavioral-equivalence acceptance gate — battery: {kind} ===");
    println!("battery rows: {battery_rows}");
    println!(
        "manifest pairs: {} interpretable-both / {} excluded (a unit not interpretable)",
        pos.pairs + neg.pairs,
        uninterp_pairs
    );
    println!();
    println!(
        "POSITIVES (should merge), interpretable slice = {}",
        pos.pairs
    );
    println!(
        "  exact-fingerprint recall : {}/{} ({:.1}%)",
        pos.fp_merge,
        pos.pairs,
        pct(pos.fp_merge, pos.pairs)
    );
    println!(
        "  behavioral-gate recall   : {}/{} ({:.1}%)",
        pos.beh_merge,
        pos.pairs,
        pct(pos.beh_merge, pos.pairs)
    );
    println!(
        "  → RECOVERED beyond fingerprint (leap value): {} (heldout: {}/{})",
        pos.beh_only, pos_heldout_beh_only, pos_heldout
    );
    println!();
    println!(
        "HARD NEGATIVES (must NOT merge), interpretable slice = {}",
        neg.pairs
    );
    println!(
        "  exact-fingerprint false merges: {}/{} ({:.1}%)",
        neg.fp_merge,
        neg.pairs,
        pct(neg.fp_merge, neg.pairs)
    );
    println!(
        "  behavioral-gate false merges  : {}/{} ({:.1}%)  ← the soundness cost",
        neg.beh_merge,
        neg.pairs,
        pct(neg.beh_merge, neg.pairs)
    );
    println!("  → INTRODUCED beyond fingerprint: {}", neg.beh_only);
}
