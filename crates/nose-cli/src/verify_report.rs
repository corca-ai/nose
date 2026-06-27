use crate::legacy_prelude::*;

/// Behavioral ground truth for the value-add evaluator: each interpretable unit with
/// a stable hash of its behavior battery (equal hash ⟺ behaviorally equal on the
/// battery) and whether that behavior is trivial (constant / all-Err — coincidental,
/// not evidence of a real clone). The evaluator groups by behavior to form gold clone
/// pairs, then scores jscpd and nose against them on equal footing.
pub(super) fn print_verify_json(oracle: &VerifyOracle) -> Result<()> {
    let recs_json: Vec<_> = oracle
        .recs
        .iter()
        .map(|r| {
            serde_json::json!({
                "file": r.file,
                "start_line": r.start,
                "end_line": r.end,
                "tokens": r.tokens,
                "behavior": format!("{:016x}", behavior_hash(&r.beh)),
                "trivial": is_trivial_behavior(&r.beh),
            })
        })
        .collect();
    let excluded_json: Vec<_> = oracle
        .exclusions
        .units
        .iter()
        .map(|u| {
            serde_json::json!({
                "file": u.file,
                "start_line": u.start,
                "end_line": u.end,
                "tokens": u.tokens,
                "reason": u.reason.label(),
            })
        })
        .collect();
    println!(
        "{}",
        serde_json::to_string(&serde_json::json!({
            "units": recs_json,
            "exclusions": {
                "core-missing": oracle.exclusions.core_missing,
                "battery-bail": oracle.exclusions.battery_bail,
                "empty-fingerprint": oracle.exclusions.empty_fingerprint,
                "uninterpretable": oracle.exclusions.uninterpretable,
                "path-bail": oracle.exclusions.path_bail,
            },
            "excluded_units": excluded_json,
        }))?
    );
    Ok(())
}

pub(super) fn print_verify_exclusions(exclusions: &VerifyExclusions) {
    if exclusions.total() == 0 {
        return;
    }
    println!("\nEXCLUSIONS — fail-closed units by reason:");
    println!("  core-missing: {}", exclusions.core_missing);
    println!(
        "  battery-bail: {} (>{} node-rows)",
        exclusions.battery_bail, VERIFY_BATTERY_NODE_ROW_BUDGET
    );
    println!("  empty-fingerprint: {}", exclusions.empty_fingerprint);
    println!("  uninterpretable: {}", exclusions.uninterpretable);
    println!(
        "  path-bail: {} (> {} symbolic branch sites)",
        exclusions.path_bail,
        nose_normalize::MAX_SYM_BRANCH_SITES
    );
}

/// Soundness: fingerprint-equal ⟹ behavior-equal. Prints the section and returns the
/// HARD false-merge count (the input to the `--max-violations` gate). A disagreement
/// where either behavior carries a symbolic value is reported separately as an
/// ADVISORY lead: symbolic identity is keyed on pre-canon syntax, so a proof-backed
/// canonicalization (AC ordering, distribution) can legitimately make two equivalent
/// units' symbolic traces differ — those need a human look, not a red gate.
pub(super) fn report_verify_soundness(recs: &[VerifyRec]) -> usize {
    let has_sym = |r: &VerifyRec| r.beh.iter().any(nose_normalize::behavior_has_sym);
    let mut by_fp: std::collections::HashMap<&[u64], Vec<&VerifyRec>> =
        std::collections::HashMap::new();
    for r in recs {
        by_fp.entry(&r.fp).or_default().push(r);
    }
    let mut fp_groups = 0usize;
    let mut violations: Vec<(String, String, usize)> = Vec::new();
    let mut advisory: Vec<(String, String, usize)> = Vec::new();
    let mut lossy: Vec<(String, String, usize)> = Vec::new();
    for members in by_fp.values() {
        if members.len() < 2 {
            continue;
        }
        fp_groups += 1;
        let first = members[0];
        for r in &members[1..] {
            if r.beh != first.beh {
                let diff = r.beh.iter().zip(&first.beh).filter(|(a, b)| a != b).count();
                let rec = (first.loc.clone(), r.loc.clone(), diff);
                if has_sym(first) || has_sym(r) || first.domain_sig != r.domain_sig {
                    advisory.push(rec);
                } else if first.claimable && r.claimable {
                    violations.push(rec);
                } else {
                    lossy.push(rec);
                }
            }
        }
    }
    println!("\nSOUNDNESS — fingerprint-equal ⟹ behavior-equal (exact claim surface):");
    println!("  fingerprint groups (≥2): {fp_groups}");
    let n_violations = violations.len();
    if violations.is_empty() {
        println!("  SOUND: no false merges ✓");
    } else {
        println!("  [!] {n_violations} VIOLATION(S) (false merges):");
        for (a, b, d) in violations.iter().take(20) {
            println!("    {a}  ≡?  {b}   ({d} differing inputs)");
        }
    }
    if !lossy.is_empty() {
        lossy.sort();
        println!(
            "  lossy-fingerprint collisions (outside the exact claim — diagnostics, not gated): {}",
            lossy.len()
        );
        for (a, b, d) in lossy.iter().take(10) {
            println!("    {a}  ≠  {b}   ({d} differing inputs)");
        }
    }
    if !advisory.is_empty() {
        advisory.sort();
        println!(
            "  advisory (symbolic-trace disagreements — divergence, not gated): {}",
            advisory.len()
        );
        for (a, b, d) in advisory.iter().take(10) {
            println!("    {a}  ≢?  {b}   ({d} differing inputs)");
        }
    }
    n_violations
}

/// Falsification search (#317): for each fingerprint-equal group the FIXED battery found
/// equal-and-hard-gate-eligible, search a value-kind-rich input domain (`falsify::falsify_pair`)
/// for a distinguishing input. A hit is a false merge the battery's input starvation missed.
/// Re-normalizes each member's file to the pre-canon CORE IL on demand (deterministic, cached)
/// and re-interprets. Returns the count of newly-found false merges (added to the gate).
pub(super) fn report_falsify(
    corpus: &Corpus,
    opts: &nose_normalize::NormalizeOptions,
    recs: &[VerifyRec],
    probes: &[nose_normalize::Value],
) -> usize {
    use std::collections::HashMap;
    const PER_PAIR_BUDGET: usize = 4096;
    let oracle_opts = nose_normalize::NormalizeOptions {
        oracle: true,
        ..*opts
    };
    let mut by_fp: HashMap<&[u64], Vec<&VerifyRec>> = HashMap::new();
    for r in recs {
        by_fp.entry(&r.fp).or_default().push(r);
    }
    let mut core_cache: HashMap<usize, nose_il::Il> = HashMap::new();
    let mut found: Vec<(String, String)> = Vec::new();
    for members in by_fp.values() {
        if members.len() < 2 {
            continue;
        }
        let first = members[0];
        for r in &members[1..] {
            // The battery already found these EQUAL; only such groups need a deeper search.
            // Restrict to hard-gate-eligible pairs (claimable, comparable declarations) so a hit
            // is a real false merge, not an advisory/lossy diagnostic.
            if r.beh != first.beh
                || !(first.claimable && r.claimable && first.domain_sig == r.domain_sig)
            {
                continue;
            }
            for &idx in &[first.file_idx, r.file_idx] {
                core_cache.entry(idx).or_insert_with(|| {
                    nose_normalize::normalize(&corpus.files[idx], &corpus.interner, &oracle_opts)
                });
            }
            let il_a = &core_cache[&first.file_idx];
            let il_b = &core_cache[&r.file_idx];
            if falsify::falsify_pair(
                il_a,
                first.core_root,
                il_b,
                r.core_root,
                &corpus.interner,
                probes,
                PER_PAIR_BUDGET,
            )
            .is_some()
            {
                found.push((first.loc.clone(), r.loc.clone()));
            }
        }
    }
    println!("\nFALSIFICATION SEARCH (#317) — distinguishing inputs beyond the fixed battery:");
    if found.is_empty() {
        println!(
            "  no new distinguishers — the fixed battery already separates every checked group ✓"
        );
    } else {
        found.sort();
        println!(
            "  [!] {} false merge(s) found by SEARCH that the fixed battery missed:",
            found.len()
        );
        for (a, b) in found.iter().take(20) {
            println!("    {a}  ≡?  {b}   (distinguisher found by search)");
        }
    }
    found.len()
}

/// Completeness: behavior-equal ⟹ fingerprint-equal (the under-merge / recall
/// direction). Restricted to *non-trivial* behaviors (the return value varies across
/// inputs and isn't uniformly Err/Null) — trivial functions agree coincidentally and
/// aren't evidence of a missed clone. A behavior group split across ≥2 fingerprints
/// is a real Type-4 clone the value graph fails to recognize. Behavior-equal on the
/// battery is necessary-not-sufficient for equivalence, so this is a lower bound on
/// completeness / upper bound on misses — but each surfaced pair is a concrete lead.
pub(super) fn report_verify_completeness(
    recs: &[VerifyRec],
    leads: Option<&std::path::Path>,
) -> Result<()> {
    let mut by_beh: std::collections::HashMap<&[nose_normalize::Behavior], Vec<&VerifyRec>> =
        std::collections::HashMap::new();
    for r in recs {
        // Concrete behaviors only: symbolic equality says "same opaque operations on
        // equal operands", which is too weak a witness for a MISSED-clone claim (two
        // wrappers calling same-NAMED but different functions would coincide). The
        // under-merge direction keeps its §BC meaning; symbolic coverage serves the
        // soundness direction.
        if !is_trivial_behavior(&r.beh) && !r.beh.iter().any(nose_normalize::behavior_has_sym) {
            by_beh.entry(&r.beh).or_default().push(r);
        }
    }
    let (mut beh_pairs, mut fp_equal_pairs, mut split_groups) = (0usize, 0usize, 0usize);
    // Each surfaced under-merge carries the *max cross-fingerprint vj* in its group: the
    // structural near-ness the behavioral oracle would gate. High vj + behavior-equal =
    // a real structural/loop clone the exact-fingerprint detector misses (e.g. join
    // index-loop vs iterator); low vj + behavior-equal = a coincidental skeleton match
    // (null-guard passthrough) we must NOT merge. This is the two-tier discriminator.
    let mut misses: Vec<(String, String, f64)> = Vec::new();
    let mut near_groups = 0usize; // split groups whose max cross-fp vj ≥ 0.7
    for members in by_beh.values() {
        if members.len() < 2 {
            continue;
        }
        let k = members.len();
        beh_pairs += k * (k - 1) / 2;
        // partition by fingerprint
        let mut by_fp2: std::collections::HashMap<&[u64], Vec<&&VerifyRec>> =
            std::collections::HashMap::new();
        for r in members {
            by_fp2.entry(&r.fp).or_default().push(r);
        }
        for sub in by_fp2.values() {
            let s = sub.len();
            fp_equal_pairs += s * (s - 1) / 2;
        }
        if by_fp2.len() > 1 {
            split_groups += 1;
            let (a, b, vj) = best_split_pair(by_fp2.values().map(|v| *v[0]).collect());
            if vj >= 0.7 {
                near_groups += 1;
            }
            misses.push((a, b, vj));
        }
    }
    // Total order: vj desc, then the two locations — `misses` is collected in `HashMap`
    // iteration order, so ties must break on stable keys for byte-identical output.
    misses.sort_by(|a, b| {
        b.2.partial_cmp(&a.2)
            .unwrap()
            .then(a.0.cmp(&b.0))
            .then(a.1.cmp(&b.1))
    });
    println!("\nCOMPLETENESS — behavior-equal ⟹ fingerprint-equal (non-trivial only):");
    println!(
        "  behavior groups (≥2): {}",
        by_beh.values().filter(|m| m.len() >= 2).count()
    );
    if beh_pairs > 0 {
        println!(
            "  completeness: {fp_equal_pairs}/{beh_pairs} = {:.0}% of behavior-equal pairs also converge",
            100.0 * fp_equal_pairs as f64 / beh_pairs as f64
        );
    }
    println!("  under-merged behavior groups (missed clones): {split_groups}");
    println!(
        "  of which structurally-near (max cross-fp vj ≥ 0.7 → behavior-gated near-match would recover): {near_groups}"
    );
    for (a, b, vj) in misses.iter().take(30) {
        println!("    vj={vj:.2}  {a}  ↮  {b}");
    }
    if let Some(path) = leads {
        write_verify_leads(path, &misses)?;
    }
    Ok(())
}

/// One representative per distinct fingerprint; find the max-vj cross pair.
/// Sort the reps by location so the chosen pair (and so the printed output) is
/// deterministic: `HashMap` iteration is an unspecified order that varies across
/// runs/thread counts, which would otherwise pick a different max-vj pair on ties
/// and break byte-identical output. The pair comes back in canonical orientation
/// (smaller location first) so it reads identically regardless of which rep the
/// analysis happened to encounter first.
fn best_split_pair(mut reps: Vec<&VerifyRec>) -> (String, String, f64) {
    reps.sort_by(|a, b| a.loc.cmp(&b.loc));
    let mut best = (0.0f64, reps[0], reps[0]);
    for i in 0..reps.len() {
        for j in (i + 1)..reps.len() {
            let vj = multiset_jaccard_u64(&reps[i].fp, &reps[j].fp);
            if vj >= best.0 {
                best = (vj, reps[i], reps[j]);
            }
        }
    }
    let (a, b) = if best.1.loc <= best.2.loc {
        (best.1.loc.clone(), best.2.loc.clone())
    } else {
        (best.2.loc.clone(), best.1.loc.clone())
    };
    (a, b, best.0)
}

/// D1: export the under-merged pairs as detection leads — oracle-discovered candidates the
/// detection campaign can turn into convergence proposals. Sorted by vj (already), so the
/// strongest (structurally-near AND behavior-equal) come first.
fn write_verify_leads(path: &std::path::Path, misses: &[(String, String, f64)]) -> Result<()> {
    let items: Vec<_> = misses
        .iter()
        .map(|(a, b, vj)| {
            serde_json::json!({ "a": a, "b": b, "vj": vj, "structurally_near": *vj >= 0.7 })
        })
        .collect();
    let near = misses.iter().filter(|(_, _, vj)| *vj >= 0.7).count();
    std::fs::write(
        path,
        serde_json::to_string_pretty(&serde_json::json!({
            "under_merged_pairs": items.len(),
            "structurally_near": near,
            "leads": items,
        }))?,
    )?;
    println!(
        "\nLEADS: wrote {} under-merged pairs ({near} structurally-near) to {}",
        misses.len(),
        path.display()
    );
    Ok(())
}

/// Calibration: P(behavior-equal | value-Jaccard bin). The detector currently
/// trusts only an *exact* fingerprint match (vj = 1.0). This measures how safe it
/// would be to also accept *near* matches — for each vj band, the fraction of pairs
/// that are actually behavior-equal = the precision of accepting at that band. Pairs
/// are sampled by sorting units by fingerprint and comparing each to a window of
/// neighbors (so high-vj pairs are well represented, unlike uniform random pairs).
pub(super) fn report_verify_calibration(recs: &[VerifyRec]) {
    let mut sorted: Vec<&VerifyRec> = recs.iter().collect();
    sorted.sort_unstable_by(|a, b| a.fp.cmp(&b.fp));
    const BINS: usize = 5; // [.5,.7) [.7,.8) [.8,.9) [.9,1.0) [1.0]
    let mut tot = [0usize; BINS];
    let mut eq = [0usize; BINS];
    let bin = |vj: f64| -> Option<usize> {
        match vj {
            v if v >= 1.0 => Some(4),
            v if v >= 0.9 => Some(3),
            v if v >= 0.8 => Some(2),
            v if v >= 0.7 => Some(1),
            v if v >= 0.5 => Some(0),
            _ => None,
        }
    };
    for (i, a) in sorted.iter().enumerate() {
        for b in sorted.iter().skip(i + 1).take(32) {
            let vj = multiset_jaccard_u64(&a.fp, &b.fp);
            if let Some(bi) = bin(vj) {
                tot[bi] += 1;
                eq[bi] += (a.beh == b.beh) as usize;
            }
        }
    }
    let labels = ["[.5,.7)", "[.7,.8)", "[.8,.9)", "[.9,1.)", "[1.0] "];
    println!("\nCALIBRATION — P(behavior-equal | value-Jaccard) [windowed sample]:");
    println!("  (the detector accepts an exact match [1.0]; this is how safe near-match is)");
    for i in (0..BINS).rev() {
        if tot[i] > 0 {
            println!(
                "  vj {} : {:>5}/{:<5} = {:>3.0}% behavior-equal",
                labels[i],
                eq[i],
                tot[i],
                100.0 * eq[i] as f64 / tot[i] as f64
            );
        }
    }
}

/// Multiset Jaccard over two sorted `u64` vectors (intersection / union by count).
pub(super) fn multiset_jaccard_u64(a: &[u64], b: &[u64]) -> f64 {
    let (mut i, mut j, mut inter) = (0, 0, 0usize);
    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                inter += 1;
                i += 1;
                j += 1;
            }
        }
    }
    let union = a.len() + b.len() - inter;
    if union == 0 {
        1.0
    } else {
        inter as f64 / union as f64
    }
}
