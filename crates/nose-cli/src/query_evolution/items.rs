use super::{
    details::Details,
    navigation::Navigation,
    selection::{self, Observations},
    sources::Sources,
    AnalysisArgs,
};
use anyhow::Result;
use nose_detect::regions::evolution::{AnalysisSnapshot, Change};
use nose_il::ContentDigest;
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::PathBuf};

pub(super) struct Items<'a> {
    pub index: &'a Observations<'a>,
    pub details: Option<&'a Details<'a>>,
    pub navigation: &'a Navigation<'a>,
    pub options: &'a AnalysisArgs,
    pub before: &'a AnalysisSnapshot,
    pub after: &'a AnalysisSnapshot,
    pub source_bases: [PathBuf; 2],
    pub assessments: &'a BTreeMap<ContentDigest, Vec<Value>>,
}
impl Items<'_> {
    pub(super) fn rows(&mut self, rows: &[&Change]) -> Result<Vec<Value>> {
        let mut before_source = self
            .options
            .before_source
            .as_ref()
            .map(|p| Sources::new(p, self.before))
            .transpose()?;
        let mut after_source = self
            .options
            .after_source
            .as_ref()
            .map(|p| Sources::new(p, self.after))
            .transpose()?;
        Ok(rows
            .iter()
            .map(|row| {
                let mut item = row_json(
                    row,
                    self.index,
                    self.details,
                    self.navigation
                        .selected(vec![format!("change={}", row.id.hex()), "full".into()]),
                );
                if self.details.is_some() {
                    item["actions"] = json!([self.source_action(&item)]);
                }
                item["reviews"] = json!(self.assessments[&row.id]);
                item["review_status"] = json!(super::reviews::status(&self.assessments[&row.id]));
                if before_source.is_some() || after_source.is_some() {
                    super::sources::attach(&mut item, &mut before_source, &mut after_source);
                }
                item
            })
            .collect())
    }
    fn source_action(&self, item: &Value) -> Value {
        let quote = crate::path_utils::shell_quote;
        let before = self
            .options
            .before_source
            .as_ref()
            .unwrap_or(&self.source_bases[0]);
        let after = self
            .options
            .after_source
            .as_ref()
            .unwrap_or(&self.source_bases[1]);
        json!({"kind":"inspect-source","label":"Verify source against captured addresses (replace directories for historical checkouts)",
            "command":format!("{} --before-source {} --after-source {}", item["next"][0].as_str().unwrap(),
                quote(&before.to_string_lossy()),
                quote(&after.to_string_lossy()))})
    }
}

fn row_json(
    row: &Change,
    index: &Observations<'_>,
    details: Option<&Details<'_>>,
    next: String,
) -> Value {
    let mut output = serde_json::to_value(row).expect("change serializes");
    output["next"] = json!([next]);
    output["reason_details"] = json!(row
        .reasons
        .iter()
        .map(|code| json!({
            "code":code, "meaning":super::render::reason(code),
        }))
        .collect::<Vec<_>>());
    output["scope"] = json!(selection::values(row, "scope", index));
    output["paths"] = json!(selection::values(row, "path", index));
    if let Some(details) = details {
        output["member_changes"] = details.summarize(row, index);
        output["before_observation"] = json!(row.before.and_then(|id| index.before.get(&id)));
        output["after_observations"] = json!(row
            .after
            .iter()
            .filter_map(|id| index.after.get(id))
            .collect::<Vec<_>>());
        output["source_body_status"] = json!("not-stored");
    }
    output
}
