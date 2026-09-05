use anyhow::Result;

use crate::query_options::SortKey;

pub(crate) fn family_by_id<'a>(
    families: &'a [nose_detect::RefactorFamily],
    prefix: &str,
) -> Result<&'a nose_detect::RefactorFamily> {
    anyhow::ensure!(!prefix.is_empty(), "family id must not be empty");
    let mut matches = families
        .iter()
        .filter(|family| crate::baseline::family_id(family).starts_with(prefix));
    let family = matches
        .next()
        .ok_or_else(|| anyhow::anyhow!("no family whose id starts with `{prefix}`"))?;
    anyhow::ensure!(
        matches.next().is_none(),
        "ambiguous family id `{prefix}`; use a longer id"
    );
    Ok(family)
}

/// A parsed query: in-memory filters over the family dataset, plus the chosen view.
#[derive(Default)]
pub(crate) struct Query {
    pub(crate) filters: Vec<QFilter>,
    pub(crate) group: Option<String>,
    pub(crate) id: Option<String>,
    /// `at=file:line` — open the family whose member span covers that source location (a
    /// stable handle across edits, unlike the span-derived `id=`). Resolved to an `id=` open.
    pub(crate) at: Option<String>,
    pub(crate) id_full: bool,
    /// Widen from the curated default surface to the full raw universe (shallow/hidden/
    /// declaration/generated families too) — the `all` token. Default queries stay on the
    /// default surface so `query`'s counts and curation match `analysis`'s.
    pub(crate) all: bool,
    pub(crate) sort: Option<SortKey>,
    pub(crate) top: Option<usize>,
    /// The `reinvented` view — code that reimplements an existing helper (the `reinvented`
    /// channel, surfaced in query), distinct from `shape=call-existing-helper` families.
    pub(crate) reinvented: bool,
    /// `since=<baseline>` — compare the dataset to a saved snapshot and expose each family's
    /// `status` (new/changed/unchanged) as a queryable field (temporal lens, not a gate).
    pub(crate) since: Option<String>,
    /// `base=<git-ref>` — the divergent-edit view: detect families at that ref and flag the
    /// ones a diff changed in one copy but not its siblings (the `nose divergence` pipeline,
    /// surfaced in query). A distinct entity (a divergence), so it's its own view.
    pub(crate) base: Option<String>,
}

pub(crate) enum QOp {
    Eq,
    Gt,
    Lt,
    Has,
}

pub(crate) struct QFilter {
    pub(crate) field: String,
    pub(crate) op: QOp,
    pub(crate) value: String,
    /// `!=`/`!~` — keep families that do NOT match (the filter result is inverted).
    pub(crate) negate: bool,
}

/// String-valued (or substring) filter fields.
const STR_FIELDS: &[&str] = &[
    "scope",
    "witness",
    "lang",
    "language",
    "path",
    "file",
    "dir",
    "surface",
    "shape",
    "extraction_shape",
    "same_symbol",
    "spotclass",
    "status",
];

/// Numeric filter fields (also the only ones that accept `>`/`<`).
const NUM_FIELDS: &[&str] = &[
    "members",
    "copies",
    "sites",
    "files",
    "modules",
    "dirs",
    "value",
    "params",
    "lines",
    "mean_lines",
    "shared",
    "shared_lines",
    "dup",
    "dup_lines",
    "languages",
];

/// Reject an unknown field (so a typo errors loudly instead of silently matching nothing
/// and reading as "this repo is clean"), and a non-numeric `>`/`<`.
fn validate_field(field: &str, op: &QOp) -> Result<()> {
    let numeric = NUM_FIELDS.contains(&field);
    if matches!(op, QOp::Gt | QOp::Lt) && !numeric {
        anyhow::bail!(
            "`{field}` is not numeric — `>`/`<` take one of: {}",
            NUM_FIELDS.join(" ")
        );
    }
    if !numeric && !STR_FIELDS.contains(&field) {
        anyhow::bail!(
            "unknown field `{field}` — valid: {} {}",
            STR_FIELDS.join(" "),
            NUM_FIELDS.join(" ")
        );
    }
    Ok(())
}

fn parse_sort_key(v: &str) -> Option<SortKey> {
    match v {
        "extractability" | "extract" => Some(SortKey::Extractability),
        "value" => Some(SortKey::Value),
        "sites" | "members" | "copies" => Some(SortKey::Sites),
        "hazard" => Some(SortKey::Hazard),
        _ => None,
    }
}

/// Parse free-form query terms. Unknown terms are an error (a typo must not silently
/// widen the result to everything).
fn qfilter(field: &str, op: QOp, value: &str, negate: bool) -> QFilter {
    QFilter {
        field: field.into(),
        op,
        value: value.into(),
        negate,
    }
}

pub(crate) fn parse_query(terms: &[String]) -> Result<Query> {
    let mut q = Query::default();
    for t in terms {
        if t == "full" {
            q.id_full = true;
        } else if t == "all" {
            q.all = true;
        } else if t == "reinvented" {
            q.reinvented = true;
        } else if let Some(v) = t.strip_prefix("group=") {
            q.group = Some(v.to_string());
        } else if let Some(v) = t.strip_prefix("id=") {
            q.id = Some(v.to_string());
        } else if let Some(v) = t.strip_prefix("at=") {
            q.at = Some(v.to_string());
        } else if let Some(v) = t.strip_prefix("since=") {
            q.since = Some(v.to_string());
        } else if let Some(v) = t.strip_prefix("base=") {
            q.base = Some(v.to_string());
        } else if let Some(v) = t.strip_prefix("sort=") {
            q.sort = Some(parse_sort_key(v).ok_or_else(|| {
                anyhow::anyhow!("unknown sort key `{v}` (extractability|value|sites|hazard)")
            })?);
        } else if let Some(v) = t.strip_prefix("top=") {
            q.top = Some(
                v.parse()
                    .map_err(|_| anyhow::anyhow!("top= needs a number, got `{v}`"))?,
            );
        } else if let Some((f, v)) = t.split_once("!~") {
            q.filters.push(qfilter(f, QOp::Has, v, true));
        } else if let Some((f, v)) = t.split_once("!=") {
            q.filters.push(qfilter(f, QOp::Eq, v, true));
        } else if let Some((f, v)) = t.split_once('~') {
            q.filters.push(qfilter(f, QOp::Has, v, false));
        } else if let Some((f, v)) = t.split_once('>') {
            q.filters.push(qfilter(f, QOp::Gt, v, false));
        } else if let Some((f, v)) = t.split_once('<') {
            q.filters.push(qfilter(f, QOp::Lt, v, false));
        } else if let Some((f, v)) = t.split_once('=') {
            q.filters.push(qfilter(f, QOp::Eq, v, false));
        } else {
            anyhow::bail!(
                "unrecognized term `{t}` — try field=value, field!=value, path~substr, group=FIELD, id=FAM, at=FILE:LINE, sort=KEY, top=N"
            );
        }
    }
    for flt in &q.filters {
        validate_field(&flt.field, &flt.op)?;
        validate_filter_values(flt)?;
    }
    Ok(q)
}

/// Validate an `=`/`!=` filter's value(s) against the field's enum, if it has one — so a value
/// typo errors instead of silently matching nothing (which reads as "no such clones exist").
/// Each comma-part is a member of the OR set and is checked independently.
fn validate_filter_values(flt: &QFilter) -> Result<()> {
    if !matches!(flt.op, QOp::Eq) {
        return Ok(());
    }
    let witness_values: Vec<_> = crate::query_model::WITNESS_ALIASES
        .iter()
        .flat_map(|(alias, kind)| [*alias, *kind])
        .collect();
    let valid: Option<&[&str]> = match flt.field.as_str() {
        "scope" => Some(&["prod", "test", "mixed"]),
        "witness" => Some(&witness_values),
        "shape" | "extraction_shape" => Some(&[
            "call-existing-helper",
            "extract-helper",
            "extract-method-from-block",
            "consolidate-type",
            "extract-base-class",
            "consolidate-cross-language",
        ]),
        "same_symbol" => Some(&["true", "false"]),
        "spotclass" => Some(&["leaf-only", "structural"]),
        "status" => Some(&["new", "changed", "unchanged"]),
        _ => None,
    };
    let Some(vals) = valid else { return Ok(()) };
    for part in flt.value.split(',') {
        if !vals.contains(&part) {
            anyhow::bail!(
                "unknown {} value `{}` — valid: {}",
                flt.field,
                part,
                vals.join(" ")
            );
        }
    }
    Ok(())
}

/// The family whose member span covers `at` (`file:line`) — the `at=` selector's target.
/// A stable handle across edits, unlike the span-derived `id=`.
pub(crate) fn family_at<'f>(
    families: &'f [nose_detect::RefactorFamily],
    at: &str,
    path_arg: &str,
) -> Result<&'f nose_detect::RefactorFamily> {
    let (file, line) = at
        .rsplit_once(':')
        .and_then(|(f, l)| l.parse::<u32>().ok().map(|n| (f, n)))
        .ok_or_else(|| anyhow::anyhow!("at= needs `file:line`, got `{at}`"))?;
    families
        .iter()
        .find(|fam| {
            fam.locations
                .iter()
                .any(|l| l.file.contains(file) && l.start_line <= line && line <= l.end_line)
        })
        .ok_or_else(|| {
            anyhow::anyhow!(
                "no family has a copy covering `{at}` — try `nose query {path_arg} path~{file}` to browse nearby"
            )
        })
}
