use super::query_model::*;
use crate::divergence;
use crate::legacy_prelude::*;
use crate::query_family_text::print_member_proposal;

pub(super) fn print_query_prelude() {
    println!("nose finds duplication in code and docs.");
    println!("nose finds; you judge. Filter, group, sort, or open families to explore.");
}

/// The location cell — `file:line  name` — coloured (path dim, symbol bold) with its
/// *visible* width (sans ANSI) for column alignment.
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
    let witness = witness_label(f.witness.as_ref().map(|w| w.kind));
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
            witness_styled(f.witness.as_ref().map(|w| w.kind)),
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
        witness_styled(f.witness.as_ref().map(|w| w.kind)),
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
) {
    let limit = query_row_limit(top);
    let fire_eligible = flagged.iter().filter(|d| d.fire_eligible).count();
    if json {
        let items: Vec<_> = divergence::divergence_items_json(flagged)
            .into_iter()
            .take(limit)
            .collect();
        let limit_value = match top {
            Some(0) => serde_json::Value::Null,
            Some(n) => serde_json::json!(n),
            None => serde_json::json!(30),
        };
        println!(
            "{}",
            serde_json::json!({
                "schema_version": schema_versions::QUERY_JSON_SCHEMA_VERSION,
                "tool": "nose",
                "view": "base",
                "path": path,
                "base": base_ref,
                "summary": {
                    "changed_files": changed_files,
                    "divergences": flagged.len(),
                    "shown_divergences": items.len(),
                    "limit": limit_value,
                    "fire_eligible": fire_eligible,
                },
                "items": items,
                "next": [format!("nose query {path} base={base_ref} --fail-on any")],
            })
        );
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
        "{} divergent {} vs `{base_ref}` ({changed_files} {} changed; {fire_eligible} touch shared logic):",
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
        let propagation = if d.fire_eligible {
            "shared-logic (likely missed propagation)"
        } else {
            "span-only (edit stayed in the varying spots)"
        };
        println!(
            "  {}  {} · {} · {propagation}",
            short_id(&d.family_id),
            witness_styled(d.witness_kind),
            d.scope,
        );
        for s in &d.changed {
            println!("    changed:      {}", site(s));
        }
        for s in &d.not_updated {
            println!("    not updated:  {}", site(s));
        }
    }
    println!("\nnext:");
    println!(
        "  nose query {path} base={base_ref} --fail-on any   # fail CI on a proven divergence"
    );
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
            serde_json::json!({
                "schema_version": schema_versions::QUERY_JSON_SCHEMA_VERSION,
                "tool": "nose",
                "view": "reinvented",
                "path": path,
                "summary": {"findings": shown.len(), "shown": shown.len().min(limit),
                    "in_test": in_test, "test_helper": test_helper},
                "items": items,
                "next": [format!("nose query {path} shape=call-existing-helper")],
            })
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
#[allow(clippy::too_many_arguments)] // dataset + view + selection state for one list render
pub(super) fn render_query_list(
    sel: &[&nose_detect::RefactorFamily],
    ov: &SurfaceOverrides,
    opp: &OpportunityGroups,
    q: &Query,
    terms: &[String],
    path: &str,
    widen: bool,
    json: bool,
    since: Option<&BaselineComparison>,
) {
    let top = query_row_limit(q.top);
    let shown = sel.len().min(top);
    if json {
        let mut lines = FileLineCache::default();
        let fams: Vec<_> = sel
            .iter()
            .take(top)
            .map(|f| {
                let (shared, params) = all_copies_shared_cached(f, &mut lines);
                query_family_json_with_counts(f, ov, opp, q.id_full, since, shared, params)
            })
            .collect();
        println!(
            "{}",
            serde_json::json!({
                "schema_version": schema_versions::QUERY_JSON_SCHEMA_VERSION,
                "tool": "nose",
                "view": "list",
                "path": path,
                "summary": { "families": sel.len(), "shown": shown, "widened": widen },
                "families": fams,
                "next": [format!("nose query {path} group=dir"), format!("nose query {path} group=witness")],
            })
        );
        return;
    }
    println!(
        "{} {}{}{}:",
        sel.len(),
        plural(sel.len(), "family", "families"),
        if widen { " (full surface)" } else { "" },
        if shown < sel.len() {
            format!(" (showing {shown})")
        } else {
            String::new()
        }
    );
    // Align the location and metrics columns across the shown rows so the drill commands
    // line up (widths from the visible text, so colour never skews them — same as the
    // dashboard's `print_candidates`).
    let shown_rows: Vec<&nose_detect::RefactorFamily> = sel.iter().take(top).copied().collect();
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
        let surf = if widen {
            match effective_surface(f, ov) {
                "default" => String::new(),
                s => format!(" [{s}]"),
            }
        } else {
            String::new()
        };
        let fold = match opp.slices(f) {
            Some(s) if !s.is_empty() => format!("\n       ↳ +{} overlapping slice folds", s.len()),
            _ => String::new(),
        };
        // With `since=`, tag the actionable changes (new/changed) so the diff against the
        // snapshot is visible inline; unchanged families stay untagged (the common case).
        let status = match since.map(|c| family_status(f, c)) {
            Some(s @ ("new" | "changed")) => format!(" [{s}]"),
            _ => String::new(),
        };
        let cmd = style::dim(&format!(
            "nose query {path} id={}",
            short_id(&baseline::family_id(f))
        ));
        println!(
            "  {loc}{}  {metrics}{}{surf}{status}   {cmd}{fold}",
            " ".repeat(wl - lw),
            " ".repeat(wm - mw),
        );
        // `full` on a list/filter batches the extraction skeletons — triage N candidates
        // in one stateless call (no per-family id= round-trip).
        if q.id_full {
            print_member_proposal(&f.locations, proposal_action_label(f));
        }
    }
    if !q.id_full {
        println!(
            "  nose query {path} ... full   # add `full` to show the extraction skeletons inline"
        );
    }
    println!("\nnext:");
    if !terms.iter().any(|t| t.starts_with("group=")) {
        println!(
            "  {} group=dir       # where this selection concentrates",
            base_cmd(terms, path)
        );
    }
    println!(
        "  {} group=witness   # by confidence",
        base_cmd(terms, path)
    );
}

/// Facet the current selection by a discrete field, with a top exemplar per bucket and
/// the "see all" command for each.
/// One `group=` bucket's aggregate: how many families, how many removable lines they carry,
/// and an exemplar. Economics-per-bucket is what turns `group=dir`/`group=file` into a
/// duplication hotspot map (where the volume is), not just a tally.
#[derive(Default)]
struct GroupAgg {
    count: usize,
    removable: u32,
    exemplar_id: String,
    exemplar_row: String,
}

pub(super) fn render_query_group(
    sel: &[&nose_detect::RefactorFamily],
    field: &str,
    terms: &[String],
    path: &str,
    json: bool,
    since: Option<&BaselineComparison>,
) {
    use std::collections::HashMap;
    let key = |f: &nose_detect::RefactorFamily| -> String {
        match field {
            "scope" => f.scope.to_string(),
            "witness" => witness_token(f.witness.as_ref().map(|w| w.kind)).to_string(),
            "lang" | "language" => f
                .locations
                .first()
                .map(|l| l.lang.as_str().to_string())
                .unwrap_or_default(),
            "dir" => family_dir(f),
            "file" => f
                .locations
                .first()
                .map(|l| l.file.clone())
                .unwrap_or_default(),
            "shape" | "extraction_shape" => f.extraction_shape().to_string(),
            "same_symbol" => family_same_symbol(f).to_string(),
            "spotclass" => family_spotclass(f).unwrap_or("unwitnessed").to_string(),
            "status" => since.map_or_else(|| "?".to_string(), |c| family_status(f, c).to_string()),
            _ => "?".to_string(),
        }
    };
    // Aggregate each bucket's payoff, not just its count — so the facet ranks by impact and
    // `group=dir`/`group=file` reads as a hotspot map. `removable_lines` is the cheap detector
    // estimate (no source read), so summing over every family stays bounded.
    let mut buckets: HashMap<String, GroupAgg> = HashMap::new();
    for f in sel {
        let e = buckets.entry(key(f)).or_default();
        if e.count == 0 {
            e.exemplar_id = baseline::family_id(f);
            e.exemplar_row = query_row(f);
        }
        e.count += 1;
        e.removable += removable_lines(f);
    }
    let mut rows: Vec<(String, GroupAgg)> = buckets.into_iter().collect();
    // Rank by removable volume (the hotspot order), then count, then key — deterministic.
    rows.sort_by(|a, b| {
        b.1.removable
            .cmp(&a.1.removable)
            .then(b.1.count.cmp(&a.1.count))
            .then(a.0.cmp(&b.0))
    });
    if json {
        let groups: Vec<_> = rows
            .iter()
            .map(|(k, g)| {
                serde_json::json!({
                    "key": k, "count": g.count, "removable": g.removable,
                    "exemplar_id": g.exemplar_id,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::json!({
                "schema_version": schema_versions::QUERY_JSON_SCHEMA_VERSION, "tool": "nose", "view": "group", "path": path,
                "field": field, "groups": groups,
            })
        );
        return;
    }
    println!(
        "{} {} by {field} (most removable first):",
        sel.len(),
        plural(sel.len(), "family", "families")
    );
    let other: Vec<&str> = terms
        .iter()
        .filter(|t| !t.starts_with("group="))
        .map(String::as_str)
        .collect();
    let base = if other.is_empty() {
        format!("nose query {path}")
    } else {
        format!("nose query {path} {}", other.join(" "))
    };
    for (k, g) in &rows {
        // Display the friendly witness label (`subdag` → `shared-core`); the JSON `key`
        // above stays the machine token. `witness=shared-core` is an accepted filter alias.
        let label = if field == "witness" && k == "subdag" {
            "shared-core"
        } else {
            k.as_str()
        };
        println!(
            "  {label:<16} ({:>3} {} · ~{} removable)  e.g. {}",
            g.count,
            plural(g.count, "family", "families"),
            g.removable,
            g.exemplar_row
        );
        println!("        {base} {field}={label}");
    }
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
