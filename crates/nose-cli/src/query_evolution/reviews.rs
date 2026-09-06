//! Explicit review files are caller decisions, independent of detector acceptance.
use anyhow::{ensure, Context, Result};
use nose_detect::regions::evolution::{AnalysisSnapshot, Change, FamilyObservation};
use nose_il::ContentDigest;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    path::Path,
};

#[derive(Clone, Copy, Debug, clap::ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Decision {
    KeepSeparate,
    Refactor,
    Defer,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    schema: String,
    analysis: ContentDigest,
    family: ContentDigest,
    review_key: ContentDigest,
    scope: String,
    decision: Decision,
    reason: String,
}
fn identity(snapshot: &AnalysisSnapshot) -> ContentDigest {
    ContentDigest::derive(
        b"nose.review-analysis/v1",
        &[&serde_json::to_vec(snapshot).expect("snapshot serializes")],
    )
}

pub(super) struct Reviews {
    records: Vec<(String, Record)>,
    before: ContentDigest,
    after: ContentDigest,
}
impl Reviews {
    pub(super) fn load(
        paths: &[std::path::PathBuf],
        before: &AnalysisSnapshot,
        after: &AnalysisSnapshot,
    ) -> Result<Self> {
        ensure!(
            paths.len() <= 128,
            "at most 128 explicit review files are supported"
        );
        let before_id = identity(before);
        let after_id = identity(after);
        let records = paths
            .iter()
            .map(|path| {
                let mut bytes = Vec::new();
                std::fs::File::open(path)
                    .with_context(|| format!("opening review {}", path.display()))?
                    .take(1024 * 1024 + 1)
                    .read_to_end(&mut bytes)?;
                ensure!(bytes.len() <= 1024 * 1024, "review exceeds 1 MiB");
                let record: Record =
                    serde_json::from_slice(&bytes).context("expected nose.review/v1 record")?;
                ensure!(
                    record.schema == "nose.review/v1" && !record.reason.trim().is_empty(),
                    "invalid review schema or empty reason"
                );
                Ok((path.to_string_lossy().into_owned(), record))
            })
            .collect::<Result<_>>()?;
        let records: Vec<(String, Record)> = records;
        for (_, record) in &records {
            for (snapshot, id) in [(before, before_id), (after, after_id)] {
                if record.analysis == id {
                    let target = snapshot
                        .families
                        .iter()
                        .find(|f| f.id == record.family)
                        .context("review target missing from its bound analysis")?;
                    ensure!(
                        target.review_key == Some(record.review_key)
                            && target.scope == record.scope,
                        "review conditions do not match its original target"
                    );
                }
            }
        }
        Ok(Self {
            records,
            before: before_id,
            after: after_id,
        })
    }
    pub(super) fn evaluate(
        &self,
        comparison: &nose_detect::regions::evolution::Comparison,
        index: &super::selection::Observations<'_>,
    ) -> std::collections::BTreeMap<ContentDigest, Vec<Value>> {
        comparison
            .changes
            .iter()
            .map(|row| {
                (
                    row.id,
                    self.assessments(
                        row,
                        index,
                        comparison.complete && comparison.profile_matches,
                    ),
                )
            })
            .collect()
    }
    pub(super) fn assessments(
        &self,
        row: &Change,
        index: &super::selection::Observations<'_>,
        complete: bool,
    ) -> Vec<Value> {
        let mut assessments: Vec<Value> = self.records.iter().filter_map(|(path, record)| {
            let old = row.before.and_then(|id| index.before.get(&id).copied());
            let current: Vec<_> = row.after.iter().filter_map(|id| index.after.get(id).copied()).collect();
            let at_before = record.analysis == self.before && old.is_some_and(|f| f.id == record.family);
            let at_after = record.analysis == self.after && current.iter().any(|f| f.id == record.family);
            if !at_before && !at_after { return None }
            let target = if at_before { old } else { current.iter().copied().find(|f| f.id == record.family) };
            let valid_record = target.is_some_and(|f| f.review_key == Some(record.review_key) && f.scope == record.scope);
            let direct = at_after && current.len() == 1 && source_complete(current[0]);
            let applicable = valid_record && (direct || (complete && row.unchanged_evidence))
                && current.len() == 1 && current[0].review_key == Some(record.review_key) && current[0].scope == record.scope;
            Some(json!({"file":path,"decision":record.decision,"reason":record.reason,
                "status":if applicable { "applicable" } else { "recheck" },
                "basis":if applicable { if direct { "Caller decision is bound to this exact current observation; no cross-capture evidence reuse is asserted." } else { "Explicit target relation, review evidence and scope satisfy the recorded conditions." } }
                    else { "Target correspondence, review evidence, scope or coverage no longer satisfies the recorded conditions; inspect change reasons." }}))
        }).collect();
        if assessments
            .windows(2)
            .any(|pair| pair[0]["decision"] != pair[1]["decision"])
        {
            for assessment in &mut assessments {
                assessment["status"] = json!("recheck");
                assessment["basis"] = json!("Conflicting caller decisions target this observation; reconcile the explicit records.");
            }
        }
        assessments
    }
    pub(super) fn unrelated(&self) -> Vec<&str> {
        self.records
            .iter()
            .filter(|(_, r)| r.analysis != self.before && r.analysis != self.after)
            .map(|(p, _)| p.as_str())
            .collect()
    }
}

fn source_complete(family: &FamilyObservation) -> bool {
    family.review_key.is_some()
        && family
            .members
            .iter()
            .all(|m| m.source.is_some() && m.content_key.is_some())
}

pub(super) fn write(
    path: &Path,
    snapshot: &AnalysisSnapshot,
    family: &FamilyObservation,
    decision: Decision,
    reason: &str,
) -> Result<()> {
    ensure!(!reason.trim().is_empty(), "review reason must not be empty");
    ensure!(reason.len() <= 64 * 1024, "review reason exceeds 64 KiB");
    ensure!(
        source_complete(family),
        "review requires complete captured source evidence for the selected family; inspect its member evidence and missing-source diagnostics. Other families do not block this decision"
    );
    let record = Record {
        schema: "nose.review/v1".into(),
        analysis: identity(snapshot),
        family: family.id,
        review_key: family.review_key.context("family lacks a review key")?,
        scope: family.scope.clone(),
        decision,
        reason: reason.into(),
    };
    let bytes = serde_json::to_vec_pretty(&record)?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .with_context(|| {
            format!(
                "creating review {}; choose a new file if it already exists",
                path.display()
            )
        })?;
    if let Err(error) = file.write_all(&bytes) {
        drop(file);
        let _ = std::fs::remove_file(path);
        return Err(error.into());
    }
    Ok(())
}

pub(super) fn status(assessments: &[Value]) -> &str {
    if assessments.is_empty() {
        "unreviewed"
    } else if assessments.iter().any(|r| r["status"] == "recheck") {
        "recheck"
    } else {
        "applicable"
    }
}
