use super::{
    details::Details,
    navigation::Navigation,
    selection::{self, Observations},
    source_view,
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
                let next = self
                    .navigation
                    .selected(vec![format!("change={}", row.id.hex()), "full".into()]);
                let mut item = row_json(row, self.index, &next);
                if let Some(details) = self.details {
                    item["actions"] = json!([self.source_action(&next)]);
                    item["member_changes"] = details.summarize(row, self.index);
                    item["before_observation"] = row
                        .before
                        .and_then(|id| self.index.before.get(&id))
                        .map(|family| source_view::family(family, before_source.as_mut()))
                        .unwrap_or(Value::Null);
                    item["after_observations"] = Value::Array(
                        row.after
                            .iter()
                            .filter_map(|id| self.index.after.get(id))
                            .map(|family| source_view::family(family, after_source.as_mut()))
                            .collect(),
                    );
                    item["source_body_status"] = json!("not-stored");
                }
                item["reviews"] = json!(self.assessments[&row.id]);
                item["review_status"] = json!(super::reviews::status(&self.assessments[&row.id]));
                if before_source.is_some() || after_source.is_some() {
                    source_view::summarize(&mut item);
                    item["actions"].as_array_mut().expect("detailed source actions").push(json!({
                        "kind":"inspect-source-json",
                        "label":"Inspect complete verified source bodies as JSON",
                        "command":self.navigation.source_json(vec![format!("change={}", row.id.hex()), "full".into()])
                    }));
                }
                item
            })
            .collect())
    }
    fn source_action(&self, next: &str) -> Value {
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
            "command":format!("{} --before-source {} --after-source {}", next,
                quote(&before.to_string_lossy()),
                quote(&after.to_string_lossy()))})
    }
}

fn row_json(row: &Change, index: &Observations<'_>, next: &str) -> Value {
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
    output
}
