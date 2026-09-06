use super::query_model::*;
use crate::baseline;
use crate::family_display::representative_lines;
use crate::query_opportunities::{family_hint, hint_reasons, OpportunityGroups};
use crate::query_semantic_packs::with_semantic_packs;
use crate::schema_versions;
use crate::style;

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
            witness_styled(f.witness.as_ref().map(|w| w.kind())),
            f.scope,
            f.members,
            removable,
        );
    } else {
        println!(
            "{} — {} · {} · {} copies · {}/{} shared, {}p · ~{} removable",
            short_id(id),
            witness_styled(f.witness.as_ref().map(|w| w.kind())),
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
/// with `full` — the bounded source comparison (#360). Plus navigation links.
pub(super) fn render_query_family(
    ctx: &crate::query_output::QueryOutput<'_>,
    idv: &str,
    json: bool,
) {
    let families = ctx.families;
    let ov = ctx.overrides;
    let opp = ctx.opp;
    let query = ctx.q;
    let path = ctx.path_arg;
    let baseline_cmp = ctx.baseline_comparison;
    let since = ctx.since;
    let semantic_packs = ctx.semantic_packs;
    let full = query.id_full;
    let f = crate::query_terms::family_by_id(families, idv)
        .expect("family selection validated before rendering");
    let id = baseline::family_id(f);
    // Overlap-fold provenance: a slice points at its richer primary; a primary lists what
    // it subsumes (so the agent doesn't triage the same region twice).
    let member_view = crate::query_members::view(f, query, ctx.args, ctx.terms);
    if json {
        let mut family = query_family_json(f, ov, opp, full, baseline_cmp, since);
        if query.member_view.active() {
            family["locations"] = member_view["locations"].clone();
        }
        println!(
            "{}",
            with_semantic_packs(
                serde_json::json!({
                    "schema_version": schema_versions::QUERY_JSON_SCHEMA_VERSION,
                    "tool": "nose",
                    "view": "family",
                    "analysis": crate::query_context::describe(ctx.args, ctx.settings, ctx.scope),
                    "path": path,
                    "hint": family_hint(f),
                    "hint_reasons": hint_reasons(f),
                    "family": family,
                    "member_view": member_view,
                    "next": member_view["next"],
                }),
                semantic_packs
            )
        );
        return;
    }
    print_family_header(&id, f);
    println!(
        "  evidence: {}",
        crate::query_assessment::relation_explanation(f)
    );
    print!("{}", fold_note(f, opp, &id));
    println!("  → {}", family_hint(f));
    print_hint_reasons(f);
    let (shared, params) = all_copies_shared(f);
    let assessment = crate::query_assessment::assessment(f, shared, params);
    println!(
        "  source support: {} — {}",
        assessment["support"].as_str().unwrap(),
        assessment["explanation"].as_str().unwrap()
    );
    if full {
        crate::query_source_evidence::render_structural(f);
    }

    if query.member_view.active() {
        crate::query_members::render(&member_view);
        return;
    }
    print_copies(f, full);
    if let Some(locations) = member_view["locations"].as_array() {
        for loc in locations.iter().take(8) {
            println!(
                "  Open {}:{}: {}",
                loc["file"].as_str().unwrap(),
                loc["start"],
                loc["next"][0].as_str().unwrap()
            );
        }
    }
    crate::query_source_evidence::render(&crate::query_source_evidence::collect(f, full), false);

    let path = crate::query_navigation::path(ctx.args, path);
    println!("\nnext:");
    for command in member_view["next"].as_array().unwrap().iter().take(2) {
        println!("  {}", command.as_str().unwrap());
    }
    println!(
        "  nose query {path} {}   # other duplication in this directory",
        crate::path_utils::shell_quote(&format!("path~{}", family_dir(f)))
    );
    println!(
        "  nose query {path} witness={}   {}",
        witness_label(f.witness.as_ref().map(|w| w.kind())),
        style::dim("# other families of the same confidence")
    );
}

fn fold_note(f: &nose_detect::RefactorFamily, opp: &OpportunityGroups, id: &str) -> String {
    if let Some(primary) = opp.primary_of.get(id) {
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
    }
}

fn print_copies(f: &nose_detect::RefactorFamily, full: bool) {
    println!("  copies:");
    let helper = family_existing_helper(f);
    let member_limit = if full { usize::MAX } else { 30 };
    for l in f.locations.iter().take(member_limit) {
        let name = l
            .name
            .as_deref()
            .map(|n| format!("  {n}"))
            .unwrap_or_default();
        // Flag the member that *is* the existing helper, so it isn't mistaken for a copy
        // to fold; its call contract still needs separate inspection.
        let role = if helper.is_some_and(|h| std::ptr::eq(h, l)) {
            "  ← existing helper candidate (callability unassessed)"
        } else {
            ""
        };
        println!("    {}:{}-{}{name}{role}", l.file, l.start_line, l.end_line);
        let boundary = crate::query_assessment::boundary(l);
        println!("      boundary: {}", boundary["meaning"].as_str().unwrap());
        if let Some(unit) = &l.enclosing_unit {
            println!(
                "      enclosing: {}:{}-{} {}",
                unit.file,
                unit.start_line,
                unit.end_line,
                unit.name.as_deref().unwrap_or("")
            );
        }
        if full {
            println!(
                "      scope evidence: {}",
                crate::query_assessment::scope(l)
            );
        }
    }
    if !full && f.locations.len() > member_limit {
        println!(
            "    … {} more copies; add `full` to show every location",
            f.locations.len() - member_limit
        );
    }
}
