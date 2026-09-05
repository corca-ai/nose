use anyhow::{bail, ensure, Result};
use nose_detect::regions::evolution::{AnalysisSnapshot, Change, FamilyObservation};
use nose_il::ContentDigest;
use std::collections::{BTreeMap, BTreeSet};

pub(super) const FIELDS: &[&str] = &[
    "reason",
    "correspondence",
    "evidence",
    "scope",
    "lang",
    "path",
    "witness",
];
pub(super) const REASONS: &[&str] = &[
    "profile-changed",
    "incomplete-coverage",
    "membership-changed",
    "evidence-population-changed",
    "member-content-changed",
    "source-address-changed",
    "scope-changed",
    "witness-changed",
    "analysis-changed",
    "packs-changed",
    "laws-changed",
    "abstraction-changed",
    "review-evidence-changed",
    "evidence-unavailable",
    "review-evidence-retained",
    "candidate",
    "ambiguous",
    "unresolved",
    "unmatched-current",
    "budget-exceeded",
];
pub(super) const KINDS: &[&str] = &[
    "matched",
    "candidate",
    "ambiguous",
    "unresolved",
    "unmatched-current",
    "budget-exceeded",
];

pub(super) const WITNESSES: &[&str] = &[
    "exact-value-graph",
    "shared-sub-dag",
    "copy-paste-run",
    "structural-similarity",
    "connected-mapped-sub-dag",
    "bounded-same-unit-window",
    "unavailable",
];

pub(super) struct Selection {
    pub group: Option<String>,
    pub change: Option<String>,
    pub top: usize,
    pub full: bool,
    pub filters: Vec<Filter>,
}
pub(super) struct Filter {
    pub field: String,
    pub values: Vec<String>,
    pub negate: bool,
    pub contains: bool,
}

impl Selection {
    pub(super) fn parse(terms: &[String]) -> Result<Self> {
        let mut q = Self {
            group: None,
            change: None,
            top: if terms.is_empty() { 5 } else { 30 },
            full: false,
            filters: Vec::new(),
        };
        for t in terms {
            if t == "full" {
                q.full = true;
                continue;
            }
            if t == "all" {
                continue;
            } // This comparison always retains the admitted population.
            if let Some(v) = t.strip_prefix("group=") {
                ensure!(
                    FIELDS.contains(&v),
                    "unknown comparison group `{v}`; valid: {}",
                    FIELDS.join(", ")
                );
                ensure!(q.group.is_none(), "only one group= is supported");
                q.group = Some(v.into());
                continue;
            }
            if let Some(v) = t.strip_prefix("change=") {
                ensure!(
                    !v.is_empty() && v.len() <= 64 && v.bytes().all(|b| b.is_ascii_hexdigit()),
                    "change= needs an observation id prefix"
                );
                ensure!(q.change.is_none(), "only one change= is supported");
                q.change = Some(v.into());
                continue;
            }
            if let Some(v) = t.strip_prefix("top=") {
                q.top = v.parse()?;
                continue;
            }
            let (field, value, negate, contains) = filter_term(t)?;
            ensure!(
                FIELDS.contains(&field),
                "unknown comparison field `{field}`; valid: {}",
                FIELDS.join(", ")
            );
            ensure!(
                !contains || field == "path",
                "substring matching is only supported for path"
            );
            let mut values: Vec<String> = if value.starts_with('"') {
                vec![serde_json::from_str(value)?]
            } else {
                value.split(',').map(str::to_owned).collect()
            };
            ensure!(
                values.iter().all(|v| !v.is_empty()),
                "comparison filter needs a value"
            );
            for value in &mut values {
                if field == "witness" {
                    *value = crate::query_model::witness_alias(value).to_owned();
                }
                validate_value(field, value)?;
            }
            q.filters.push(Filter {
                field: field.into(),
                values,
                negate,
                contains,
            });
        }
        ensure!(
            q.group.is_none() || q.change.is_none(),
            "group= and change= are separate views"
        );
        Ok(q)
    }
    pub(super) fn select<'a>(
        &self,
        changes: &'a [Change],
        index: &Observations<'_>,
    ) -> Result<Vec<&'a Change>> {
        let mut rows: Vec<_> = changes.iter().filter(|r| self.keeps(r, index)).collect();
        if let Some(id) = &self.change {
            rows.retain(|r| r.id.hex().starts_with(id));
            ensure!(
                !rows.is_empty(),
                "no change matching `{id}` in this selection; remove change= to browse"
            );
            ensure!(
                rows.len() == 1,
                "ambiguous change id `{id}`; use a longer prefix"
            );
        }
        Ok(rows)
    }
    pub(super) fn keeps(&self, row: &Change, index: &Observations<'_>) -> bool {
        self.filters.iter().all(|f| {
            let actual = values(row, &f.field, index);
            let matched = actual.iter().any(|a| {
                f.values
                    .iter()
                    .any(|v| if f.contains { a.contains(v) } else { a == v })
            });
            matched != f.negate
        })
    }
}
fn validate_value(field: &str, value: &str) -> Result<()> {
    let valid = match field {
        "reason" => REASONS,
        "correspondence" => KINDS,
        "evidence" => &["retained", "recheck"],
        "scope" => &["prod", "test", "mixed"],
        "witness" => WITNESSES,
        _ => return Ok(()),
    };
    if !valid.contains(&value) {
        bail!(
            "unknown {field} value `{value}`; valid: {}",
            valid.join(", ")
        );
    }
    Ok(())
}

pub(super) struct Observations<'a> {
    pub before: BTreeMap<ContentDigest, &'a FamilyObservation>,
    pub after: BTreeMap<ContentDigest, &'a FamilyObservation>,
}
impl<'a> Observations<'a> {
    pub(super) fn new(before: &'a AnalysisSnapshot, after: &'a AnalysisSnapshot) -> Self {
        Self {
            before: before.families.iter().map(|f| (f.id, f)).collect(),
            after: after.families.iter().map(|f| (f.id, f)).collect(),
        }
    }
    fn families(&self, row: &Change) -> Vec<&'a FamilyObservation> {
        row.before
            .iter()
            .filter_map(|id| self.before.get(id).copied())
            .chain(
                row.after
                    .iter()
                    .filter_map(|id| self.after.get(id).copied()),
            )
            .collect()
    }
}

pub(super) fn values(row: &Change, field: &str, index: &Observations<'_>) -> BTreeSet<String> {
    match field {
        "reason" => row.reasons.iter().cloned().collect(),
        "correspondence" => BTreeSet::from([row.correspondence.clone()]),
        "evidence" => BTreeSet::from([if row.unchanged_evidence {
            "retained"
        } else {
            "recheck"
        }
        .into()]),
        _ => index
            .families(row)
            .iter()
            .flat_map(|f| match field {
                "scope" => vec![f.scope.clone()],
                "witness" => vec![f.witness.clone()],
                "lang" => f.members.iter().map(|m| m.lang.clone()).collect(),
                "path" => f.members.iter().map(|m| m.file.clone()).collect(),
                _ => Vec::new(),
            })
            .collect(),
    }
}

fn filter_term(term: &str) -> Result<(&str, &str, bool, bool)> {
    let (_, op) = ["!=", "!~", "=", "~"].iter().filter_map(|op| term.find(op).map(|i| (i, *op)))
        .min_by_key(|(i, _)| *i)
        .ok_or_else(|| anyhow::anyhow!("unknown comparison term `{term}`; use group=reason, reason=VALUE, change=ID, top=N, full"))?;
    let (field, value) = term.split_once(op).expect("operator found");
    Ok((field, value, op.starts_with('!'), op.ends_with('~')))
}
