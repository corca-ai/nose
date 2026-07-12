#[cfg(test)]
use crate::query_opportunities::family_langs;

/// The line count of the family's representative copy — the denominator for "`N of M`
/// shared". It's the *first* (largest) site's own span, not the family-wide `mean_lines`:
/// the two largest members are what got diffed, so a family whose biggest copies run
/// longer than average must not read as "47/43 shared". Floored at `shared_lines` so the
/// fraction is never inverted.
pub(super) fn representative_lines(f: &nose_detect::RefactorFamily) -> u32 {
    f.locations
        .first()
        .map(|l| l.end_line.saturating_sub(l.start_line) + 1)
        .unwrap_or(f.mean_lines)
        .max(f.shared_lines)
}

/// One plain-language line describing a family: how many copies, how much is actually
/// shared vs varies, how many lines you'd remove, and where the duplication lives. No
/// internal ranking numbers — those only order the list, they're not for the reader.
#[cfg(test)]
pub(super) fn family_summary(f: &nose_detect::RefactorFamily) -> String {
    let detail = if f.languages > 1 {
        format!(
            "same logic in {} languages ({})",
            f.languages,
            family_langs(f)
        )
    } else {
        let rep = representative_lines(f);
        match f.params {
            0 => format!("{} of {rep} lines identical", f.shared_lines),
            1 => format!("{} of {rep} lines shared, 1 spot differs", f.shared_lines),
            p => format!("{} of {rep} lines shared, {p} spots differ", f.shared_lines),
        }
    };
    let scope = match f.scope {
        "test" => "  · in test code",
        "mixed" => "  · same code in tests and prod",
        _ => "",
    };
    // WHY the members merged, in reader words (issue #264's "shared decision
    // vs shared shape"): an exact value-graph proof is behavioral evidence; a
    // token run is surface likeness. The JSON has carried this since #222 —
    // the human report should too.
    let evidence = match f.witness.as_ref().map(|w| w.kind) {
        Some("exact-value-graph") => " · exact behavior match",
        Some("shared-sub-dag") => " · shared core computation",
        Some("connected-mapped-sub-dag") => " · connected shared computation",
        Some("copy-paste-run") => " · copy-paste",
        Some("structural-similarity") => " · near-duplicate",
        _ => "",
    };
    format!(
        "{} copies · {detail} · ~{} lines removable{evidence}{scope}",
        f.members,
        removable_lines(f)
    )
}

pub(super) fn abstraction_witness_summary(witness: &nose_detect::AbstractionWitness) -> String {
    let caveats = if witness.caveats.is_empty() {
        "no caveats".to_string()
    } else {
        format!("caveats: {}", witness.caveats.join(", "))
    };
    let holes = witness
        .holes
        .iter()
        .map(|hole| format!("{} {} {}->{}", hole.kind, hole.role, hole.left, hole.right))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{}:{} · {} · {} · {}",
        witness.basis, witness.members_checked, witness.reason_code, holes, caveats
    )
}

/// Lines you'd actually delete by extracting one shared copy. For same-language
/// families this is the *invariant* lines folded out of each redundant copy
/// (`(copies−1) × shared_lines`) — not `(copies−1) × mean_lines`, which counts the
/// varying parts that *survive* extraction and so overstates the win (e.g. four
/// 38-line copies sharing only 15 lines remove ~45, not ~114). Cross-language families
/// have no shared-line count, so they keep the span-based estimate.
pub(super) fn removable_lines(f: &nose_detect::RefactorFamily) -> u32 {
    let copies = f.members.saturating_sub(1) as u32;
    if f.languages == 1 && f.shared_lines > 0 {
        copies * f.shared_lines
    } else {
        f.dup_lines
    }
}

/// The honest similarity cell. A bare `sim 1.00` misleads — two same-language copies
/// can be structurally identical yet share *no* literal lines (a language idiom, or two
/// unrelated type literals with the same shape). For same-language families always
/// report the real shared-line count `18/42 shared · 2p` — 18 invariant lines of the 42
/// in the largest copy, even when it's `0/42` (nothing to extract). Only cross-language
/// families, which have no shared *source* lines to diff, fall back to structural `sim`.
pub(super) fn similarity_cell(f: &nose_detect::RefactorFamily) -> String {
    if f.languages > 1 {
        return format!("sim {:.2}", f.mean_score);
    }
    let rep = representative_lines(f);
    format!("{}/{} shared · {}p", f.shared_lines, rep, f.params)
}
