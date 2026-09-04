mod base_json;
mod group;

pub(super) use group::{render_query_group, QueryGroupView};

use super::query_model::*;
use crate::baseline;
use crate::baseline_comparison::BaselineComparison;
use crate::divergence;
use crate::family_display::{removable_lines, representative_lines};
use crate::query_baseline_gate::family_status;
use crate::query_family_text::print_member_proposal;
use crate::query_opportunities::{proposal_action_label, OpportunityGroups};
use crate::query_semantic_packs::with_semantic_packs;
use crate::query_terms::Query;
use crate::report_text::plural;
use crate::schema_versions;
use crate::source_lines::FileLineCache;
use crate::style;
use crate::surfaces::{effective_surface, SurfaceOverrides};

pub(super) fn print_query_prelude() {
    println!("nose finds duplication in code and docs.");
    println!("nose finds; you judge. Filter, group, sort, or open families to explore.");
}

/// The location cell — `file:line  name` — coloured with its visible width for alignment.
pub(super) fn loc_cell(f: &nose_detect::RefactorFamily) -> (String, usize) {
    let l = &f.locations[0];
    let pos = format!("{}:{}", l.file, l.start_line);
    let name = l
        .name
        .as_deref()
        .map(|n| format!("  {n}"))
        .unwrap_or_default();
    let width = pos.chars().count() + name.chars().count();
    (format!("{}{}", style::dim(&pos), style::bold(&name)), width)
}

/// The payoff-economics cell — copies, shared/varying lines, removable lines, witness — with
/// the removable count bold and the witness coloured by confidence. Returns the coloured
/// string and its *visible* width for alignment.
pub(super) fn metrics_cell(f: &nose_detect::RefactorFamily) -> (String, usize) {
    let (shared, params) = all_copies_shared(f);
    let removable = query_removable_lines(f, shared);
    let witness = witness_label(f.witness.as_ref().map(|w| w.kind()));
    // Flag non-production scope inline so a test/mixed family isn't mistaken for prod.
    let scope = if f.scope == "prod" {
        String::new()
    } else {
        format!(" · {}", f.scope)
    };
    if f.languages > 1 {
        let plain = format!(
            "{} copies · cross-language · ~{removable} repeated · {witness}{scope}",
            f.members,
        );
        let colored = format!(
            "{} copies · cross-language · ~{} repeated · {}{}",
            f.members,
            style::bold(&removable.to_string()),
            witness_styled(f.witness.as_ref().map(|w| w.kind())),
            style::yellow(&scope),
        );
        return (colored, plain.chars().count());
    }
    let rep = representative_lines(f);
    let plain = format!(
        "{} copies · {shared}/{rep} shared, {params}p · ~{removable} removable · {witness}{scope}",
        f.members,
    );
    let colored = format!(
        "{} copies · {shared}/{rep} shared, {params}p · ~{} removable · {}{}",
        f.members,
        style::bold(&removable.to_string()),
        witness_styled(f.witness.as_ref().map(|w| w.kind())),
        style::yellow(&scope),
    );
    (colored, plain.chars().count())
}

/// One concise list row: where the largest copy is, what it is, and the **payoff
/// economics** an agent needs to triage without opening the family — how much is shared,
/// how many spots vary, how many lines an extraction removes (counts, not a verdict).
fn query_row(f: &nose_detect::RefactorFamily) -> String {
    let (loc, _) = loc_cell(f);
    let (metrics, _) = metrics_cell(f);
    format!("{loc}  {metrics}")
}

/// Render the `base=` divergence view: query's schema envelope around divergence's shared finding
/// JSON, or a concise human report keyed on which copy changed and whether the edit touched
/// shared logic (the propagation hazard).
pub(super) fn render_query_base(
    flagged: &[divergence::Divergence],
    changed_files: usize,
    base_ref: &str,
    path: &str,
    top: Option<usize>,
    json: bool,
    semantic_packs: &[serde_json::Value],
) {
    let limit = query_row_limit(top);
    let fire_eligible = flagged.iter().filter(|d| d.fire_eligible).count();
    let strict = flagged.iter().filter(|d| d.gate_fail_default()).count();
    if json {
        base_json::render(flagged, changed_files, base_ref, path, top, semantic_packs);
        return;
    }
    print_query_prelude();
    if flagged.is_empty() {
        println!(
            "no divergent edits vs `{base_ref}` ({changed_files} {} changed).",
            plural(changed_files, "file", "files")
        );
        return;
    }
    println!(
        "{} divergent {} vs `{base_ref}` ({changed_files} {} changed; {strict} strict, {fire_eligible} legacy fire-eligible):",
        flagged.len(),
        plural(flagged.len(), "family", "families"),
        plural(changed_files, "file", "files"),
    );
    let site = |s: &divergence::Site| {
        let name = s
            .name
            .as_deref()
            .map(|n| format!("  {n}"))
            .unwrap_or_default();
        format!("{}:{}-{}{name}", s.file, s.start_line, s.end_line)
    };
    for d in flagged.iter().take(limit) {
        let decision = d.policy_decision();
        let propagation = match (decision.tier, d.lane, decision.taxonomy_hint) {
            (divergence::DivergenceTier::Strict, _, _) => "strict (likely missed propagation)",
            (divergence::DivergenceTier::Review, _, "no_propagation_needed") => {
                "review (shared logic not touched)"
            }
            (divergence::DivergenceTier::Review, _, _) => "review (shared logic unproven)",
            (_, divergence::DivergenceLane::NewCopy, _) => "report-only (new current-tree copy)",
            (_, _, "test_scaffolding") => "report-only (test/mixed scope)",
            _ => "report-only (advisory)",
        };
        let lane = divergence::lane_value(d.lane);
        println!(
            "  {}  {} · {} · {lane} · {propagation}",
            short_id(&d.family_id),
            witness_styled(d.witness_kind),
            d.scope,
        );
        match d.lane {
            divergence::DivergenceLane::BaseDivergence => {
                for s in &d.changed {
                    println!("    changed:      {}", site(s));
                    print_semantic_change(s);
                }
                for s in &d.not_updated {
                    println!("    not updated:  {}", site(s));
                }
                for target in &d.targets {
                    println!(
                        "    target {}: {} <- {}  ({} {:.3})",
                        short_id(&target.target_id),
                        site(&target.skipped),
                        site(&target.changed),
                        target.direct_witness.kind,
                        target.direct_witness.similarity,
                    );
                    if let Some(label) = target.variant_evidence.concise_label() {
                        println!("      variant:    {label}");
                    }
                }
            }
            divergence::DivergenceLane::NewCopy => {
                for s in &d.changed {
                    println!("    new/changed:  {}", site(s));
                }
                for s in &d.not_updated {
                    println!("    sibling:      {}", site(s));
                }
            }
        }
    }
    println!("\nnext:");
    println!("  nose query {path} base={base_ref} --fail-on any   # fail CI on strict divergences");
}

fn print_semantic_change(site: &divergence::Site) {
    if let Some(witness) = &site.semantic_change {
        println!("      semantic:   {}", witness.concise_label());
    }
}

/// The `reinvented` view: code that reimplements an existing helper's body (the `reinvented`
/// channel). Each surfaced finding's action is "call the helper instead" — the same action as a
/// `call-existing-helper` family, but for sites the family clusterer did not group (different
/// recall, not a second way to ask the same question). Production containers are shown only when
/// the existing helper is also production; a test-only helper requires rehoming/extracting before
/// production code can call it.
pub(super) fn render_query_reinvented(
    reinvented: &[nose_detect::ReinventedHelper],
    path: &str,
    top: Option<usize>,
    json: bool,
    semantic_packs: &[serde_json::Value],
) {
    let shown: Vec<&nose_detect::ReinventedHelper> = reinvented
        .iter()
        .filter(|r| !r.container_in_test && !r.helper_in_test)
        .collect();
    let in_test = reinvented.iter().filter(|r| r.container_in_test).count();
    let test_helper = reinvented
        .iter()
        .filter(|r| !r.container_in_test && r.helper_in_test)
        .count();
    let limit = query_row_limit(top);
    if json {
        let items: Vec<_> = shown
            .iter()
            .take(limit)
            .map(|r| {
                serde_json::json!({
                    "helper": {"name": r.helper_name, "file": r.helper_file,
                        "start": r.helper_start_line, "end": r.helper_end_line,
                        "in_test": r.helper_in_test},
                    "site": {"file": r.container_file, "container": r.container_name,
                        "container_start": r.container_start_line, "container_end": r.container_end_line,
                        "start": r.site_start_line, "end": r.site_end_line,
                        "container_in_test": r.container_in_test},
                    "value": r.weight,
                    "approximate": r.site_approximate,
                })
            })
            .collect();
        println!(
            "{}",
            with_semantic_packs(
                serde_json::json!({
                    "schema_version": schema_versions::QUERY_JSON_SCHEMA_VERSION,
                    "tool": "nose",
                    "view": "reinvented",
                    "path": path,
                    "summary": {"findings": shown.len(), "shown": shown.len().min(limit),
                        "in_test": in_test, "test_helper": test_helper},
                    "items": items,
                    "next": [format!("nose query {path} shape=call-existing-helper")],
                }),
                semantic_packs
            )
        );
        return;
    }
    if shown.is_empty() {
        println!("no reinvented-helper findings on the production surface.");
        if in_test > 0 {
            println!("  ({in_test} in test code — omitted)");
        }
        if test_helper > 0 {
            println!("  ({test_helper} point at test-only helpers — omitted; rehome a helper before calling it from production)");
        }
        return;
    }
    println!("reinvented helpers — code that reimplements an existing helper; call it instead:");
    for r in shown.iter().take(limit) {
        let approx = if r.site_approximate { " ~approx" } else { "" };
        println!(
            "  {}:{}-{}{}  → call {} ({}:{}-{})  ~{} value nodes",
            r.container_file,
            r.site_start_line,
            r.site_end_line,
            approx,
            r.helper_name.as_deref().unwrap_or("-"),
            r.helper_file,
            r.helper_start_line,
            r.helper_end_line,
            r.weight,
        );
    }
    let hidden = shown.len().saturating_sub(limit);
    if hidden > 0 {
        println!("  … {hidden} more (raise top=N)");
    }
    if in_test > 0 {
        println!("  ({in_test} more in test code — omitted)");
    }
    if test_helper > 0 {
        println!("  ({test_helper} more point at test-only helpers — omitted; rehome a helper before calling it from production)");
    }
    println!("\nnext:");
    println!(
        "  nose query {path} shape=call-existing-helper   # the clustered cases (in clone families)"
    );
}

/// A ranked list of the current selection: each row carries its own `id=` drill link,
/// plus a reasoned `next:`.
pub(super) struct QueryListView<'a> {
    pub(super) selection: &'a [&'a nose_detect::RefactorFamily],
    pub(super) overrides: &'a SurfaceOverrides,
    pub(super) opportunities: &'a OpportunityGroups,
    pub(super) query: &'a Query,
    pub(super) terms: &'a [String],
    pub(super) path: &'a str,
    pub(super) widen: bool,
    pub(super) json: bool,
    pub(super) baseline_comparison: Option<&'a BaselineComparison>,
    pub(super) since: Option<&'a BaselineComparison>,
    pub(super) semantic_packs: &'a [serde_json::Value],
}

fn query_list_json(view: &QueryListView<'_>) -> serde_json::Value {
    let QueryListView {
        selection,
        overrides,
        opportunities,
        query,
        path,
        widen,
        baseline_comparison,
        since,
        semantic_packs,
        ..
    } = view;
    let top = query_row_limit(query.top);
    let shown = selection.len().min(top);
    let mut lines = FileLineCache::default();
    let fams: Vec<_> = selection
        .iter()
        .take(top)
        .map(|f| {
            let (shared, params) = all_copies_shared_cached(f, &mut lines);
            query_family_json_with_counts(
                f,
                overrides,
                opportunities,
                query.id_full,
                *baseline_comparison,
                *since,
                shared,
                params,
            )
        })
        .collect();
    with_semantic_packs(
        serde_json::json!({
            "schema_version": schema_versions::QUERY_JSON_SCHEMA_VERSION,
            "tool": "nose",
            "view": "list",
            "path": path,
            "summary": { "families": selection.len(), "shown": shown, "widened": widen },
            "families": fams,
            "next": [format!("nose query {path} group=dir"), format!("nose query {path} group=witness")],
        }),
        semantic_packs,
    )
}

pub(super) fn render_query_list(view: QueryListView<'_>) {
    let top = query_row_limit(view.query.top);
    let shown = view.selection.len().min(top);
    if view.json {
        println!("{}", query_list_json(&view));
        return;
    }
    println!(
        "{} {}{}{}:",
        view.selection.len(),
        plural(view.selection.len(), "family", "families"),
        if view.widen { " (full surface)" } else { "" },
        if shown < view.selection.len() {
            format!(" (showing {shown})")
        } else {
            String::new()
        }
    );
    // Align the location and metrics columns across the shown rows so the drill commands
    // line up (widths from the visible text, so colour never skews them — same as the
    // dashboard's `print_candidates`).
    let shown_rows: Vec<&nose_detect::RefactorFamily> =
        view.selection.iter().take(top).copied().collect();
    let cells: Vec<(String, usize, String, usize)> = shown_rows
        .iter()
        .map(|f| {
            let (loc, lw) = loc_cell(f);
            let (metrics, mw) = metrics_cell(f);
            (loc, lw, metrics, mw)
        })
        .collect();
    let wl = cells.iter().map(|c| c.1).max().unwrap_or(0);
    let wm = cells.iter().map(|c| c.3).max().unwrap_or(0);
    for (f, (loc, lw, metrics, mw)) in shown_rows.iter().zip(&cells) {
        // When widened past the default surface, label why a demoted family is here.
        let surf = if view.widen {
            match effective_surface(f, view.overrides) {
                "default" => String::new(),
                s => format!(" [{s}]"),
            }
        } else {
            String::new()
        };
        let fold = match view.opportunities.slices(f) {
            Some(s) if !s.is_empty() => format!("\n       ↳ +{} overlapping slice folds", s.len()),
            _ => String::new(),
        };
        // With `since=`, tag the actionable changes (new/changed) so the diff against the
        // snapshot is visible inline; unchanged families stay untagged (the common case).
        let status_cmp = view.since.or(view.baseline_comparison);
        let status = match status_cmp.map(|c| family_status(f, c)) {
            Some(s @ ("new" | "changed")) => format!(" [{s}]"),
            _ => String::new(),
        };
        let cmd = style::dim(&format!(
            "nose query {} id={}",
            view.path,
            short_id(&baseline::family_id(f))
        ));
        println!(
            "  {loc}{}  {metrics}{}{surf}{status}   {cmd}{fold}",
            " ".repeat(wl - lw),
            " ".repeat(wm - mw),
        );
        // `full` on a list/filter batches the extraction skeletons — triage N candidates
        // in one stateless call (no per-family id= round-trip).
        if view.query.id_full {
            print_member_proposal(&f.locations, proposal_action_label(f));
        }
    }
    if !view.query.id_full {
        println!(
            "  nose query {} ... full   # add `full` to show the extraction skeletons inline",
            view.path
        );
    }
    println!("\nnext:");
    if !view.terms.iter().any(|term| term.starts_with("group=")) {
        println!(
            "  {} group=dir       # where this selection concentrates",
            base_cmd(view.terms, view.path)
        );
    }
    println!(
        "  {} group=witness   # by confidence",
        base_cmd(view.terms, view.path)
    );
}

/// `nose query` with the current selection's terms minus any view term — the prefix the
/// `next:` links extend.
fn base_cmd(terms: &[String], path: &str) -> String {
    let keep: Vec<&str> = terms
        .iter()
        .filter(|t| !t.starts_with("group=") && !t.starts_with("id=") && *t != "full")
        .map(String::as_str)
        .collect();
    if keep.is_empty() {
        format!("nose query {path}")
    } else {
        format!("nose query {path} {}", keep.join(" "))
    }
}
