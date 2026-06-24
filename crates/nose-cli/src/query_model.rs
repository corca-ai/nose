use crate::baseline;
use crate::baseline_comparison::BaselineComparison;
use crate::family_display::representative_lines;
use crate::query_baseline_gate::family_status;
use crate::query_opportunities::OpportunityGroups;
use crate::query_terms::{QFilter, QOp};
use crate::source_lines::{anti_unify_all, read_lines, FileLineCache};
use crate::style;
use crate::surfaces::{effective_surface, SurfaceOverrides};

/// Canonical `witness.kind` for a friendly filter token (`exact`→`exact-value-graph`, …).
fn witness_alias(v: &str) -> &str {
    match v {
        "exact" => "exact-value-graph",
        "subdag" | "shared-core" => "shared-sub-dag",
        "copy-paste" | "copypaste" => "copy-paste-run",
        "similar" | "structural" => "structural-similarity",
        other => other,
    }
}

/// The friendly token for a `witness.kind` — the machine value (`--format json`,
/// `group=witness` keys, filter matching). Stable; do not change without a schema bump.
pub(super) fn witness_token(kind: Option<&str>) -> &'static str {
    match kind {
        Some("exact-value-graph") => "exact",
        Some("shared-sub-dag") => "subdag",
        Some("copy-paste-run") => "copy-paste",
        Some("structural-similarity") => "similar",
        _ => "?",
    }
}

/// The human-facing DISPLAY label for a `witness.kind` — same as [`witness_token`] except
/// the opaque `subdag` reads as `shared-core` for people. Used only in the terminal report;
/// the machine token (`witness_token`) stays `subdag`, and `witness=shared-core` is an
/// accepted filter alias (see [`witness_alias`]), so the two spellings never collide.
pub(super) fn witness_label(kind: Option<&str>) -> &'static str {
    match kind {
        Some("shared-sub-dag") => "shared-core",
        other => witness_token(other),
    }
}

/// The witness label coloured by confidence: proven channels (exact, shared-core) green,
/// copy-paste yellow, similar blue. Plain text when colour is off.
pub(super) fn witness_styled(kind: Option<&str>) -> String {
    let label = witness_label(kind);
    match kind {
        Some("exact-value-graph") => style::bold_green(label),
        Some("shared-sub-dag") => style::green(label),
        Some("copy-paste-run") => style::yellow(label),
        Some("structural-similarity") => style::blue(label),
        _ => label.to_string(),
    }
}

/// The directory a family lives in (the parent of its largest copy) — nose's spatial unit.
pub(super) fn family_dir(f: &nose_detect::RefactorFamily) -> String {
    f.locations
        .first()
        .and_then(|l| {
            std::path::Path::new(&l.file)
                .parent()
                .and_then(|p| p.to_str())
        })
        .filter(|s| !s.is_empty())
        .unwrap_or(".")
        .to_string()
}

/// Whether every copy is the **same named symbol** in a different place — the decidable
/// signature of a parallel-variant family (e.g. one `lower_for`/`stmt_as_block` per language
/// frontend). Evidence, not a verdict: combine with negation (`same_symbol!=true`) to drop the
/// parallel-by-design class, or `same_symbol=true` to divergence it.
pub(super) fn family_same_symbol(f: &nose_detect::RefactorFamily) -> bool {
    let mut names = f.locations.iter().map(|l| l.name.as_deref());
    match names.next().flatten() {
        Some(first) if f.locations.len() > 1 => names.all(|n| n == Some(first)),
        _ => false,
    }
}

/// The existing helper a `call-existing-helper` family already contains: the lone named
/// function/method whose body the other (inline) members recompute (#374 item 5). `None`
/// for every other extraction shape. The `extraction_shape() == "call-existing-helper"`
/// gate is exactly the predicate `family_hint` uses, so the surfaced helper always matches
/// the hint — naming *which* member it is stops the agent from reading the helper's own
/// body as just another copy (the #373 dogfood confusion).
pub(super) fn family_existing_helper(f: &nose_detect::RefactorFamily) -> Option<&nose_detect::Loc> {
    if f.extraction_shape() != "call-existing-helper" {
        return None;
    }
    f.locations.iter().find(|l| {
        matches!(
            l.kind,
            nose_il::UnitKind::Function | nose_il::UnitKind::Method
        ) && l.name.is_some()
            && !l.is_fragment
    })
}

/// The aggregate value-class of a near family's varying spots, from the #315 graded witness:
/// `leaf-only` (every hole is a clean value-leaf and the witness remains equal modulo
/// those holes — a parameterize/extract candidate) vs `structural` (a demoted witness,
/// shape/arity/unmodeled/decorator divergence, or referent mismatch — genuinely
/// different logic, not just parameters). `None` until the
/// graded witness is attached (near families only, and only when the query asks for it —
/// the enrichment is the dominant cost, so `query` runs it on demand; see `run_query_cmd`).
pub(super) fn family_spotclass(f: &nose_detect::RefactorFamily) -> Option<&'static str> {
    let g = f.witness.as_ref()?.graded.as_ref()?;
    let is_structural = |c: &str| {
        matches!(
            c,
            "arity" | "shape" | "unmodeled" | "extra-sink" | "decorator"
        )
    };
    if !g.equal_modulo_holes
        || !g.referent_mismatches.is_empty()
        || g.spots.iter().any(|s| is_structural(s.class))
    {
        Some("structural")
    } else {
        Some("leaf-only")
    }
}

/// Numeric value of a family field (for `>`/`<`/`=` on numbers).
fn family_num(f: &nose_detect::RefactorFamily, field: &str) -> Option<f64> {
    Some(match field {
        "members" | "copies" | "sites" => f.members as f64,
        "files" => f.files as f64,
        "modules" | "dirs" => f.modules as f64,
        "value" => f.value,
        "params" => f.params as f64,
        "lines" | "mean_lines" => f.mean_lines as f64,
        "shared" | "shared_lines" => f.shared_lines as f64,
        "dup" | "dup_lines" => f.dup_lines as f64,
        "languages" => f.languages as f64,
        _ => return None,
    })
}

/// Whether a family satisfies one filter.
pub(super) fn family_matches(
    f: &nose_detect::RefactorFamily,
    ov: &SurfaceOverrides,
    flt: &QFilter,
    since: Option<&BaselineComparison>,
) -> bool {
    let field = flt.field.as_str();
    // The family's value for string fields, computed once. `None` (e.g. `spotclass` on an
    // unenriched/non-near family, or `status` without `since=`) deliberately matches nothing
    // rather than erroring.
    let fval: Option<String> = match field {
        "scope" => Some(f.scope.to_string()),
        "witness" => Some(witness_token(f.witness.as_ref().map(|w| w.kind)).to_string()),
        "surface" => Some(effective_surface(f, ov).to_string()),
        "shape" | "extraction_shape" => Some(f.extraction_shape().to_string()),
        "dir" => Some(family_dir(f)),
        "same_symbol" => Some(family_same_symbol(f).to_string()),
        "spotclass" => family_spotclass(f).map(str::to_string),
        "status" => since.map(|c| family_status(f, c).to_string()),
        _ => None,
    };
    let path_has = |val: &str| f.locations.iter().any(|l| l.file.contains(val));
    let lang_match = |val: &str, exact: bool| {
        f.locations.iter().any(|l| {
            if exact {
                l.lang.as_str() == val
            } else {
                l.lang.as_str().contains(val)
            }
        })
    };
    // Match one value (one comma-part). `=`/`~` OR over the comma-separated set (membership);
    // `>`/`<` are a range, so they take the whole value as a single number.
    let one = |val: &str| -> bool {
        match flt.op {
            QOp::Has => match field {
                "path" | "file" => path_has(val),
                "lang" | "language" => lang_match(val, false),
                _ => fval.as_deref().is_some_and(|s| s.contains(val)),
            },
            QOp::Eq => match field {
                "path" | "file" => path_has(val),
                "lang" | "language" => lang_match(val, true),
                "witness" => f.witness.as_ref().map(|w| w.kind) == Some(witness_alias(val)),
                _ => {
                    if let Some(s) = &fval {
                        s == val
                    } else {
                        family_num(f, field)
                            .zip(val.parse::<f64>().ok())
                            .is_some_and(|(a, b)| (a - b).abs() < f64::EPSILON)
                    }
                }
            },
            QOp::Gt => family_num(f, field)
                .zip(val.parse::<f64>().ok())
                .is_some_and(|(a, b)| a > b),
            QOp::Lt => family_num(f, field)
                .zip(val.parse::<f64>().ok())
                .is_some_and(|(a, b)| a < b),
        }
    };
    let base = match flt.op {
        // Set-membership OR: `witness=exact,subdag` matches either; `!=` (negate) then drops
        // any family in the set. `>`/`<` stay single-valued.
        QOp::Has | QOp::Eq => flt.value.split(',').any(one),
        QOp::Gt | QOp::Lt => one(flt.value.as_str()),
    };
    base ^ flt.negate
}

/// A short, git-style family handle for `id=` links (the full id accepts any prefix).
pub(super) fn short_id(id: &str) -> &str {
    &id[..id.len().min(10)]
}

/// One family as the structured `nose query --format json` object: all
/// the evidence a consumer needs to triage without re-parsing a human row. `shared`/`params`
/// are the all-copies counts (the same the human row shows); `skeleton` is the all-copies
/// extraction proposal, included only on `full`.
pub(super) fn query_family_json(
    f: &nose_detect::RefactorFamily,
    ov: &SurfaceOverrides,
    opp: &OpportunityGroups,
    full: bool,
    baseline_cmp: Option<&BaselineComparison>,
    since: Option<&BaselineComparison>,
) -> serde_json::Value {
    let (shared, params) = all_copies_shared(f);
    query_family_json_with_counts(f, ov, opp, full, baseline_cmp, since, shared, params)
}

fn query_family_metrics_json(f: &nose_detect::RefactorFamily) -> serde_json::Value {
    serde_json::json!({
        "mean_sem": f.mean_sem,
        "members": f.members,
        "modules": f.modules,
        "files": f.files,
        "languages": f.languages,
        "mean_score": f.mean_score,
        "mean_lines": f.mean_lines,
        "shared_weight": f.shared_weight,
        "params": f.params,
        "scope": f.scope,
        "value": f.value,
        "dup_lines": f.dup_lines,
        "shared_lines": f.shared_lines,
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn query_family_json_with_counts(
    f: &nose_detect::RefactorFamily,
    ov: &SurfaceOverrides,
    opp: &OpportunityGroups,
    full: bool,
    baseline_cmp: Option<&BaselineComparison>,
    since: Option<&BaselineComparison>,
    shared: u32,
    params: u32,
) -> serde_json::Value {
    let removable = query_removable_lines(f, shared);
    let helper = family_existing_helper(f);
    let locations: Vec<_> = f
        .locations
        .iter()
        .map(|l| {
            let mut o = serde_json::json!({
                "id": baseline::member_id(l),
                "file": l.file, "start": l.start_line, "end": l.end_line,
                "name": l.name, "lang": l.lang.as_str(),
            });
            // Mark the member that is itself the existing helper, so the agent does not read
            // it as one more copy to fold (#374 item 5).
            if helper.is_some_and(|h| std::ptr::eq(h, l)) {
                o["role"] = serde_json::Value::from("existing-helper");
            }
            // For a shared-sub-dag (partial) clone, where the proven shared computation lives
            // at this site — so the caller can see what is provably equal, not just the unit.
            if let Some((s, e)) = l.shared_subdag {
                o["shared_subdag"] = serde_json::json!([s, e]);
            }
            if !l.origin.is_unknown() {
                o["origin"] =
                    serde_json::to_value(l.origin).expect("UnitOrigin JSON serialization");
            }
            o
        })
        .collect();
    let mut obj = serde_json::json!({
        "id": baseline::family_id(f),
        "scope": f.scope,
        "witness": witness_token(f.witness.as_ref().map(|w| w.kind)),
        "surface": effective_surface(f, ov),
        "members": f.members,
        "files": f.files,
        "dirs": f.modules,
        "languages": f.languages,
        "source_comparable": f.languages == 1,
        "metrics": query_family_metrics_json(f),
        "shared": shared,
        "rep_lines": representative_lines(f),
        "params": params,
        "removable": removable,
        "value": f.value,
        "extraction_shape": f.extraction_shape(),
        "same_symbol": family_same_symbol(f),
        "folds": opp.slices(f).map(<[_]>::len).unwrap_or(0),
        "locations": locations,
    });
    // Proof depth: for the exact channel, how much is proven identical — the size of the shared
    // value multiset. Lets a caller act now on the strongest evidence (subdag families carry the
    // proven span per location instead). Evidence, not a verdict.
    if let Some(n) = f.witness.as_ref().and_then(|w| w.value_nodes) {
        obj["value_nodes"] = serde_json::Value::from(n);
    }
    // Temporal status against a `since=` snapshot (new/changed/unchanged), when one was given.
    if let Some(cmp) = since {
        obj["status"] = serde_json::Value::from(family_status(f, cmp));
    }
    if let Some(cmp) = baseline_cmp {
        if let Some(status) = cmp.statuses.get(&baseline::family_key(f)) {
            obj["baseline_status"] = serde_json::Value::from(status.status.as_str());
            obj["baseline_match"] = serde_json::Value::from(status.baseline_match.as_str());
            obj["matched_baseline_ids"] = serde_json::Value::from(
                status
                    .matched_baseline_ids
                    .iter()
                    .map(|id| baseline::format_key(*id))
                    .collect::<Vec<_>>(),
            );
            obj["accepted_member_count"] = serde_json::Value::from(status.accepted_member_count);
            obj["new_member_count"] = serde_json::Value::from(status.new_member_count);
        }
    }
    // Fold-graph navigation: the actual related family ids, not just a count — so a caller can
    // jump to the fuller overlapping family or open the slices it subsumes (HATEOAS).
    let id = baseline::family_id(f);
    if let Some(primary) = opp.primary_of.get(&id) {
        obj["subsumed_by"] = serde_json::Value::from(short_id(primary));
    }
    if let Some(slices) = opp.slices(f).filter(|s| !s.is_empty()) {
        let ids: Vec<&str> = slices.iter().map(|s| short_id(s)).collect();
        obj["subsumes"] = serde_json::Value::from(ids);
    }
    // `call-existing-helper` families: name the helper to call (the action is "call it", not
    // "extract a new one"). Omitted for every other shape (#374 item 5).
    if let Some(h) = helper {
        obj["existing_helper"] = serde_json::json!({
            "name": h.name, "file": h.file, "start": h.start_line, "end": h.end_line,
        });
    }
    // Spot value-class evidence: present only on near families whose graded witness has been
    // enriched (the query path runs that on demand — see `run_query_cmd`); omitted otherwise
    // rather than emitted as a misleading null (#374 item 2).
    if let Some(sc) = family_spotclass(f) {
        obj["spotclass"] = serde_json::Value::from(sc);
    }
    if let Some(graded) = f.witness.as_ref().and_then(|w| w.graded.as_ref()) {
        obj["graded"] = serde_json::to_value(graded).expect("graded witness is JSON serializable");
    }
    if let Some(pair) = graded_pair_json(f) {
        obj["graded_pair"] = pair;
    }
    if full {
        if let Some(skeleton) = family_skeleton(f) {
            obj["skeleton"] = serde_json::Value::from(skeleton);
        }
    }
    obj
}

fn graded_pair_json(f: &nose_detect::RefactorFamily) -> Option<serde_json::Value> {
    let (a_idx, b_idx) = f.witness.as_ref()?.graded_pair?;
    let (Some(a), Some(b)) = (f.locations.get(a_idx), f.locations.get(b_idx)) else {
        return None;
    };
    Some(serde_json::json!({
        "a_index": a_idx,
        "b_index": b_idx,
        "a_member_id": baseline::member_id(a),
        "b_member_id": baseline::member_id(b),
    }))
}

pub(super) fn query_removable_lines(f: &nose_detect::RefactorFamily, shared: u32) -> u32 {
    if f.languages == 1 {
        u32::try_from(f.members.saturating_sub(1)).unwrap_or(0) * shared
    } else {
        f.dup_lines
    }
}

/// The all-copies extraction-skeleton lines (the `--show proposal` artifact) for the `full`
/// JSON contract. `None` when fewer than two copies read. Capped at the same 8 members as
/// `all_copies_shared`, so the skeleton and the `shared`/`params` counts agree.
fn family_skeleton(f: &nose_detect::RefactorFamily) -> Option<Vec<String>> {
    let members: Vec<Vec<String>> = f
        .locations
        .iter()
        .take(8)
        .filter_map(|l| read_lines(&l.file, l.start_line, l.end_line))
        .collect();
    (members.len() >= 2).then(|| anti_unify_all(&members).0)
}

/// Shared-line and parameter counts aligned across **all** copies (not the pairwise
/// representative `shared_lines`, which over-counts a family whose 3rd+ copies diverge —
/// e.g. 25 serializer methods that pairwise share 11 lines but all-25 share 2). Reads the
/// copies and runs the N-way anti-unification (#360); only ever called on the rows
/// actually displayed, so it is bounded. The honest `~removable` is then `(copies − 1) ×
/// all-copies-shared`, which is never more than is truly shared and `0` when nothing is.
/// Cross-language or unreadable families fall back to the detector's structural estimate.
pub(super) fn all_copies_shared(f: &nose_detect::RefactorFamily) -> (u32, u32) {
    let mut cache = FileLineCache::default();
    all_copies_shared_cached(f, &mut cache)
}

pub(super) fn all_copies_shared_cached(
    f: &nose_detect::RefactorFamily,
    cache: &mut FileLineCache,
) -> (u32, u32) {
    if f.languages != 1 {
        return (f.shared_lines, f.params);
    }
    // Same MEMBER_CAP (8) as `shared_lines_of`, so the two surfaces compute the
    // all-copies count over the same member set and report identical numbers.
    let members: Vec<Vec<String>> = f
        .locations
        .iter()
        .take(8)
        .filter_map(|l| cache.slice(&l.file, l.start_line, l.end_line))
        .collect();
    if members.len() < 2 {
        return (f.shared_lines, f.params);
    }
    let (_skeleton, shared, params) = anti_unify_all(&members);
    (shared, params)
}

pub(super) fn is_default_surface(f: &nose_detect::RefactorFamily, ov: &SurfaceOverrides) -> bool {
    effective_surface(f, ov) == "default"
}

/// Resolve how many rows a query view shows. `top=N` shows N; `top=0` means *all*
/// (matching `analysis --top 0`, and the `top: Some(0)` the dataset build already uses for
/// "every family"); an absent `top=` defaults to 30.
pub(super) fn query_row_limit(top: Option<usize>) -> usize {
    match top {
        Some(0) => usize::MAX,
        Some(n) => n,
        None => 30,
    }
}
