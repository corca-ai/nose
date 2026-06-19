use super::query_model::*;
use super::query_views::{loc_cell, metrics_cell};
use crate::legacy_prelude::*;

/// Print a block of candidate rows in aligned columns (location · metrics · drill command),
/// coloured. Widths are computed from each cell's visible length so the ANSI codes never
/// skew the columns. The drill command is dimmed; an overlapping-slice fold note, if any,
/// trails on its own line.
fn print_candidates(rows: &[&nose_detect::RefactorFamily], path: &str, opp: &OpportunityGroups) {
    let cells: Vec<(String, usize, String, usize, String, String)> = rows
        .iter()
        .map(|f| {
            let (loc, lw) = loc_cell(f);
            let (metrics, mw) = metrics_cell(f);
            let cmd = style::dim(&format!(
                "nose query {path} id={}",
                short_id(&baseline::family_id(f))
            ));
            let fold = match opp.slices(f) {
                Some(s) if !s.is_empty() => {
                    format!("\n       ↳ +{} overlapping slice folds", s.len())
                }
                _ => String::new(),
            };
            (loc, lw, metrics, mw, cmd, fold)
        })
        .collect();
    let wl = cells.iter().map(|c| c.1).max().unwrap_or(0);
    let wm = cells.iter().map(|c| c.3).max().unwrap_or(0);
    for (loc, lw, metrics, mw, cmd, fold) in &cells {
        println!(
            "  {loc}{}  {metrics}{}   {cmd}{fold}",
            " ".repeat(wl - lw),
            " ".repeat(wm - mw),
        );
    }
}

/// The query summary: analysis scope, candidate counts, a few high-value rows with `id=`
/// links, and the next commands a reader is likely to need.
#[allow(clippy::too_many_lines)]
#[allow(clippy::too_many_arguments)] // a self-describing landing view over several dataset facets
pub(super) fn render_query_dashboard(
    families: &[nose_detect::RefactorFamily],
    ov: &SurfaceOverrides,
    opp: &OpportunityGroups,
    scope: &QueryScope,
    path: &str,
    reinvented_prod: usize,
    json: bool,
    since: Option<&BaselineComparison>,
    markdown: &[nose_markdown::Family],
) {
    // Default surface, slice-folds removed (shown under their primary) — matches analysis.
    let def: Vec<&nose_detect::RefactorFamily> = families
        .iter()
        .filter(|f| is_default_surface(f, ov) && !opp.is_slice(f))
        .collect();
    // Scope-blind ranking: a family is ranked purely by extractability (the order it
    // arrives in), test and production treated alike — scope is a tag and an optional
    // filter, never a demotion.
    let count = |k: &str| {
        def.iter()
            .filter(|f| witness_token(f.witness.as_ref().map(|w| w.kind)) == k)
            .count()
    };
    if json {
        let top: Vec<_> = def
            .iter()
            .take(5)
            .map(|f| query_family_json(f, ov, opp, false, since))
            .collect();
        println!(
            "{}",
            serde_json::json!({
                "schema_version": schema_versions::QUERY_JSON_SCHEMA_VERSION,
                "tool": "nose",
                "view": "dashboard",
                "path": path,
                "summary": {
                    "scanned_files": scope.files,
                    "families": def.len(),
                    "by_confidence": {"exact": count("exact"), "subdag": count("subdag"),
                        "copy_paste": count("copy-paste"), "similar": count("similar")},
                    "reinvented": reinvented_prod,
                    "shown": top.len(),
                },
                "families": top,
                "top_candidates": top,
                // Markdown near-duplicate families (separate prose engine). Additive key —
                // query-JSON consumers that don't know it simply ignore it.
                "markdown": markdown::families_json(markdown),
                "next": [format!("nose query {path} sort=extractability"), format!("nose query {path} group=dir"),
                    format!("nose query {path} witness=exact"), format!("nose query {path} all")],
            })
        );
        return;
    }
    println!("{}", scope.summary());
    let n_proven = count("exact") + count("subdag");
    println!(
        "\n{} duplicated-code {}.",
        style::bold(&def.len().to_string()),
        plural(def.len(), "family", "families"),
    );
    println!(
        "  {} {n_proven} ({} {} · {} {}) · {} {} · {} {}",
        style::bold_green("verified"),
        style::green("exact"),
        count("exact"),
        style::green("shared-core"),
        count("subdag"),
        style::yellow("copy-paste"),
        count("copy-paste"),
        style::blue("similar"),
        count("similar"),
    );
    println!(
        "  {}",
        style::dim("verified = machine-checked evidence · exact = same unit behavior · shared-core = shared computation")
    );
    // The "best candidates" lead only makes sense when the default surface has
    // something on it. With an empty surface we skip it (a `sort=extractability` link into
    // an empty list is a dead end); the closing footer still offers `all` when families
    // were merely held back below the surface — the one genuinely useful next move.
    if !def.is_empty() {
        println!("\n{}", style::bold("best candidates:"));
        let top: Vec<&nose_detect::RefactorFamily> = def.iter().take(3).copied().collect();
        print_candidates(&top, path, opp);
        println!(
            "  nose query {path} sort=extractability       {}",
            style::dim(&format!("# all {}, best first", def.len()))
        );
    }

    let kind_of = |k: &str| {
        def.iter()
            .filter(|f| f.witness.as_ref().map(|w| w.kind) == Some(k))
            .count()
    };
    let n_exact = kind_of("exact-value-graph");
    let n_subdag = kind_of("shared-sub-dag");
    let proven: Vec<_> = def
        .iter()
        .filter(|f| {
            matches!(
                f.witness.as_ref().map(|w| w.kind),
                Some("exact-value-graph") | Some("shared-sub-dag")
            )
        })
        .collect();
    if !proven.is_empty() {
        println!(
            "\n{}",
            style::bold("verified families (exact behavior or shared computation):")
        );
        let top: Vec<&nose_detect::RefactorFamily> = proven.iter().take(3).map(|f| **f).collect();
        print_candidates(&top, path, opp);
        if n_exact > 0 {
            println!(
                "  nose query {path} witness=exact             {}",
                style::dim(&format!(
                    "# the {n_exact} exact whole-unit {}",
                    plural(n_exact, "family", "families")
                ))
            );
        }
        if n_subdag > 0 {
            println!(
                "  nose query {path} witness=shared-core       {}",
                style::dim(&format!(
                    "# the {n_subdag} that share a proven core computation"
                ))
            );
        }
    }

    // Most-duplicated directories.
    use std::collections::HashMap;
    let mut by_dir: HashMap<String, (u32, usize)> = HashMap::new();
    for f in &def {
        let e = by_dir.entry(family_dir(f)).or_default();
        e.0 += f.dup_lines;
        e.1 += 1;
    }
    let mut dirs: Vec<_> = by_dir.into_iter().collect();
    dirs.sort_by(|a, b| b.1 .0.cmp(&a.1 .0).then(a.0.cmp(&b.0)));
    // Only worth surfacing when the duplication actually spans directories — with a single
    // directory both the `path~` slice and the `group=dir` facet just re-run the same analysis.
    if dirs.len() > 1 {
        println!("\n{}", style::bold("most-duplicated directories:"));
        for (d, (_dup, n)) in dirs.iter().take(3) {
            println!(
                "  nose query {path} path~{d}   {}",
                style::dim(&format!("# {n} {}", plural(*n, "family", "families")))
            );
        }
        println!(
            "  nose query {path} group=dir                 {}",
            style::dim("# full breakdown")
        );
    }
    if reinvented_prod > 0 {
        println!(
            "\n{}",
            style::bold(&format!(
                "{reinvented_prod} place{} reimplement an existing helper — call it instead:",
                if reinvented_prod == 1 { "" } else { "s" }
            ))
        );
        println!(
            "  nose query {path} reinvented                 {}",
            style::dim("# the call-the-helper findings")
        );
    }
    // Repo-level magnitude + what the default surface omitted.
    let omitted = surface_omission_note(families, ov);
    println!(
        "\n~{} duplicated lines on the default surface.{}",
        total_dup_lines_refs(&def),
        omitted
            .map(|n| format!(" {n} — add `all` to include them"))
            .unwrap_or_default()
    );
    println!(
        "\n{}",
        style::bold("next commands — replace <path> with your path; terms combine with AND:")
    );
    for (verb, cmd, note) in [
        (
            "filter",
            "nose query <path> witness=exact",
            "keep only the proven-identical families",
        ),
        (
            "",
            "nose query <path> members>3 path~api",
            "compare with > < , ~ (contains), != (negate)",
        ),
        (
            "group",
            "nose query <path> group=dir",
            "totals by directory (or: witness, lang, scope, same_symbol)",
        ),
        (
            "open",
            "nose query <path> id=<id> full",
            "one family: every copy + the extraction skeleton",
        ),
        (
            "sort",
            "nose query <path> sort=value",
            "by duplicated volume (or: extractability [default], members)",
        ),
        (
            "more",
            "nose query <path> all",
            "include families held back below the default surface",
        ),
    ] {
        println!(
            "  {}  {cmd:<37}  {}",
            style::bold(&format!("{verb:<6}")),
            style::dim(note)
        );
    }
    println!(
        "\n  {}",
        style::dim("filter/group fields: scope · witness · lang · path · members · files · value · params · shared · dir · same_symbol")
    );
    // Markdown near-duplicate prose, reported as a query domain (separate `nose-markdown` engine).
    markdown::print_section(markdown, path);
}
