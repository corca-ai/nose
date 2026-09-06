//! Summarize already-computed correspondence without another search or source read.
use nose_detect::regions::{
    evolution::{AnalysisSnapshot, Change, MemberObservation},
    ChangeKind, Correspondence,
};
use nose_il::ContentDigest;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

pub(super) struct Details<'a> {
    after: BTreeMap<ContentDigest, &'a MemberObservation>,
    mapping: BTreeMap<ContentDigest, &'a Correspondence>,
}

impl<'a> Details<'a> {
    pub(super) fn new(after: &'a AnalysisSnapshot, rows: &'a [Correspondence]) -> Self {
        let members = |s: &'a AnalysisSnapshot| {
            s.families
                .iter()
                .flat_map(|f| &f.members)
                .filter_map(|m| Some((m.observation_id()?, m)))
                .collect()
        };
        Self {
            after: members(after),
            mapping: rows.iter().filter_map(|r| Some((r.before?, r))).collect(),
        }
    }

    pub(super) fn summarize(
        &self,
        change: &Change,
        index: &super::selection::Observations<'_>,
    ) -> Value {
        let before = change.before.and_then(|id| index.before.get(&id)).copied();
        let after: Vec<_> = change
            .after
            .iter()
            .filter_map(|id| index.after.get(id).copied())
            .collect();
        let after_ids: BTreeSet<_> = after
            .iter()
            .flat_map(|f| &f.members)
            .filter_map(|m| m.observation_id())
            .collect();
        let mut referenced = BTreeSet::new();
        let mut members = Vec::new();
        for member in before.iter().flat_map(|f| &f.members) {
            let mapped = member
                .observation_id()
                .and_then(|id| self.mapping.get(&id).copied());
            let candidates: Vec<_> = mapped
                .into_iter()
                .flat_map(|r| &r.after)
                .filter(|id| after_ids.contains(id))
                .copied()
                .collect();
            referenced.extend(&candidates);
            let next: Vec<_> = candidates
                .iter()
                .filter_map(|id| self.after.get(id).copied())
                .collect();
            let kind = mapped.map(|r| r.kind);
            let exact = matches!(kind, Some(ChangeKind::Unchanged | ChangeKind::ContentMatch))
                && next.len() == 1;
            let status = if exact {
                let current = next[0];
                if member.file != current.file
                    || member.start_line != current.start_line
                    || member.end_line != current.end_line
                {
                    "same-content-new-location"
                } else {
                    "same-content"
                }
            } else {
                match kind {
                    Some(ChangeKind::Ambiguous) => "ambiguous",
                    Some(ChangeKind::BudgetExceeded) => "budget-exceeded",
                    Some(ChangeKind::Unresolved) => "unresolved",
                    None => "unavailable",
                    _ => "candidate",
                }
            };
            members.push(json!({"status":status,"before":location(member),"after":next.into_iter().map(location).collect::<Vec<_>>()}));
        }
        for id in after_ids.difference(&referenced) {
            members.push(json!({"status":"unmatched-current", "before":Value::Null,"after":[location(self.after[id])]}));
        }
        for member in after
            .iter()
            .flat_map(|f| &f.members)
            .filter(|m| m.observation_id().is_none())
        {
            members.push(
                json!({"status":"unavailable", "before":Value::Null,"after":[location(member)]}),
            );
        }
        // Source addresses, not hash or input enumeration order, determine reading order.
        members.sort_by_key(|row| {
            (
                row["before"].is_null(),
                row["before"]["file"].as_str().unwrap_or("").to_owned(),
                row["before"]["start_line"].as_u64().unwrap_or(0),
                row["after"].to_string(),
            )
        });
        json!({"before_members":before.map(|f| f.members.len()),
            "after_member_counts":after.iter().map(|f| f.members.len()).collect::<Vec<_>>(),
            "members":members,
            "meaning":"Observed source correspondence within these candidate families; no ancestry, deletion or refactoring-success assertion."})
    }
}

fn location(member: &MemberObservation) -> Value {
    json!({"observation_id":member.observation_id(),"file":member.file,"start_line":member.start_line,"end_line":member.end_line,"name":member.name,"lang":member.lang})
}
