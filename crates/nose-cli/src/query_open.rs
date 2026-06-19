use super::query_model::*;
use crate::legacy_prelude::*;
use crate::query_family_text::{print_member_diff, print_member_proposal};

/// Render the origin-derived "why this hint" reasons (#453) under a family's hint, if any.
fn print_hint_reasons(f: &nose_detect::RefactorFamily) {
    let reasons = hint_reasons(f);
    if !reasons.is_empty() {
        println!("  why this hint:");
        for reason in reasons {
            println!("    - {reason}");
        }
    }
}

fn print_family_header(id: &str, f: &nose_detect::RefactorFamily) {
    let (shared, params) = all_copies_shared(f);
    let removable = query_removable_lines(f, shared);
    if f.languages > 1 {
        println!(
            "{} — {} · {} · {} copies · cross-language · ~{} repeated",
            short_id(id),
            witness_styled(f.witness.as_ref().map(|w| w.kind)),
            f.scope,
            f.members,
            removable,
        );
    } else {
        println!(
            "{} — {} · {} · {} copies · {}/{} shared, {}p · ~{} removable",
            short_id(id),
            witness_styled(f.witness.as_ref().map(|w| w.kind)),
            f.scope,
            f.members,
            shared,
            representative_lines(f),
            params,
            removable,
        );
    }
}

/// Open one family: its copies, the extraction hint, the representative-pair diff, and —
/// with `full` — the all-copies extraction skeleton (#360). Plus navigation links.
#[allow(clippy::too_many_arguments)] // dataset + view + selection state for one family render
pub(super) fn render_query_family(
    families: &[nose_detect::RefactorFamily],
    ov: &SurfaceOverrides,
    opp: &OpportunityGroups,
    idv: &str,
    full: bool,
    path: &str,
    json: bool,
    since: Option<&BaselineComparison>,
) {
    let Some(f) = families
        .iter()
        .find(|f| baseline::family_id(f).starts_with(idv))
    else {
        println!("no family whose id starts with `{idv}` — run `nose query` for the dashboard");
        return;
    };
    let id = baseline::family_id(f);
    // Overlap-fold provenance: a slice points at its richer primary; a primary lists what
    // it subsumes (so the agent doesn't triage the same region twice).
    let fold_note = if let Some(primary) = opp.primary_of.get(&id) {
        format!(
            "  ↳ subsumed by id={} (the fuller overlapping family)\n",
            short_id(primary)
        )
    } else if let Some(s) = opp.slices(f).filter(|s| !s.is_empty()) {
        let ids: Vec<&str> = s.iter().take(6).map(|x| short_id(x)).collect();
        let more = s.len().saturating_sub(ids.len());
        let tail = if more > 0 {
            format!(" +{more}")
        } else {
            String::new()
        };
        format!(
            "  ↳ subsumes {} overlapping slice families: {}{tail}  (open with id=)\n",
            s.len(),
            ids.join(" ")
        )
    } else {
        String::new()
    };
    if json {
        println!(
            "{}",
            serde_json::json!({
                "schema_version": schema_versions::QUERY_JSON_SCHEMA_VERSION,
                "tool": "nose",
                "view": "family",
                "path": path,
                "hint": family_hint(f),
                "hint_reasons": hint_reasons(f),
                "family": query_family_json(f, ov, opp, full, since),
            })
        );
        return;
    }
    print_family_header(&id, f);
    print!("{fold_note}");
    println!("  → {}", family_hint(f));
    print_hint_reasons(f);
    println!("  copies:");
    let helper = family_existing_helper(f);
    for l in f.locations.iter().take(30) {
        let name = l
            .name
            .as_deref()
            .map(|n| format!("  {n}"))
            .unwrap_or_default();
        // Flag the member that *is* the existing helper, so it isn't mistaken for a copy
        // to fold — the action is to call it (#374 item 5).
        let role = if helper.is_some_and(|h| std::ptr::eq(h, l)) {
            "  ← existing helper (call it)"
        } else {
            ""
        };
        println!("    {}:{}-{}{name}{role}", l.file, l.start_line, l.end_line);
    }
    // Lead with the decision-grade artifact: the extraction skeleton aligned across ALL
    // copies (#360), with the differing spots as parameters — not a raw 2-copy token diff.
    if f.locations.len() >= 2 {
        print_member_proposal(&f.locations, proposal_action_label(f));
    }
    if full && f.locations.len() >= 2 {
        print_member_diff(&f.locations[0], &f.locations[1]);
    } else if !full && f.locations.len() >= 2 {
        println!(
            "    nose query {path} id={} full   # also show the raw token diff of two copies",
            short_id(&id)
        );
    }
    println!("\nnext:");
    println!(
        "  nose query {path} path~{}   # other duplication in this directory",
        family_dir(f)
    );
    println!(
        "  nose query {path} witness={}   {}",
        witness_label(f.witness.as_ref().map(|w| w.kind)),
        style::dim("# other families of the same confidence")
    );
}
