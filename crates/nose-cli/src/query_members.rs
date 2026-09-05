//! Member facets narrow a family view without changing the detected family.
use crate::{baseline, path_utils::shell_quote, query_terms::Query};
use nose_detect::{Loc, RefactorFamily};
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Default)]
pub(crate) struct Members {
    pub group: Option<String>,
    pub path: Option<String>,
    pub dir: Option<String>,
    pub lang: Option<String>,
    pub scope: Option<String>,
}
impl Members {
    pub(crate) fn parse(&mut self, term: &str) -> anyhow::Result<bool> {
        let Some(term) = term.strip_prefix("member-") else {
            return Ok(false);
        };
        if let Some(value) = term.strip_prefix("path~") {
            anyhow::ensure!(!value.is_empty(), "member-path~ needs a substring");
            self.path = Some(value.into());
            return Ok(true);
        }
        let (field, value) = term.split_once('=').ok_or_else(|| {
            anyhow::anyhow!(
                "expected member-group=, member-dir=, member-lang=, member-scope= or member-path~"
            )
        })?;
        anyhow::ensure!(!value.is_empty(), "member filter needs a value");
        match field {
            "group" => {
                anyhow::ensure!(
                    ["dir", "lang", "scope"].contains(&value),
                    "member-group= expects dir, lang or scope"
                );
                self.group = Some(value.into());
            }
            "dir" => self.dir = Some(value.into()),
            "lang" => self.lang = Some(value.into()),
            "scope" => {
                anyhow::ensure!(
                    ["prod", "test"].contains(&value),
                    "member-scope= expects prod or test"
                );
                self.scope = Some(value.into());
            }
            _ => anyhow::bail!(
                "unknown member field `{field}`; use group, dir, lang, scope or path~"
            ),
        }
        Ok(true)
    }
    pub(crate) fn active(&self) -> bool {
        self.dir.is_some()
            || self.group.is_some()
            || self.path.is_some()
            || self.lang.is_some()
            || self.scope.is_some()
    }
    pub(crate) fn keeps(&self, loc: &Loc) -> bool {
        self.dir.as_ref().is_none_or(|d| directory(loc) == *d)
            && self.path.as_ref().is_none_or(|p| loc.file.contains(p))
            && self.lang.as_ref().is_none_or(|l| &loc.lang == l)
            && self.scope.as_ref().is_none_or(|s| {
                s == if nose_detect::is_test_loc(loc) {
                    "test"
                } else {
                    "prod"
                }
            })
    }
    fn terms(&self) -> Vec<String> {
        let mut terms = Vec::new();
        if let Some(d) = &self.dir {
            terms.push(format!("member-dir={d}"));
        }
        if let Some(p) = &self.path {
            terms.push(format!("member-path~{p}"));
        }
        if let Some(l) = &self.lang {
            terms.push(format!("member-lang={l}"));
        }
        if let Some(s) = &self.scope {
            terms.push(format!("member-scope={s}"));
        }
        terms
    }
}
pub(crate) fn view(f: &RefactorFamily, q: &Query, args: &crate::cli_args::QueryArgs) -> Value {
    let selected: Vec<_> = f
        .locations
        .iter()
        .filter(|l| q.member_view.keeps(l))
        .collect();
    let top = if q.id_full || q.top == Some(0) {
        usize::MAX
    } else {
        q.top.unwrap_or(30)
    };
    let mut words = crate::query_navigation::words(args);
    words.extend([
        format!("id={}", baseline::family_id(f)),
        "--format".into(),
        if args.format == crate::query_options::ReportFormat::Json {
            "json"
        } else {
            "human"
        }
        .into(),
    ]);
    let base = words
        .iter()
        .map(|w| shell_quote(w))
        .collect::<Vec<_>>()
        .join(" ");
    let command = |suffix: Vec<String>| {
        let mut terms = q.member_view.terms();
        terms.extend(suffix);
        format!(
            "{base} {}",
            terms
                .iter()
                .map(|t| shell_quote(t))
                .collect::<Vec<_>>()
                .join(" ")
        )
    };
    let mut groups = BTreeMap::new();
    if let Some(field) = &q.member_view.group {
        for loc in &selected {
            let key = match field.as_str() {
                "dir" => directory(loc),
                "lang" => loc.lang.clone(),
                _ => if nose_detect::is_test_loc(loc) {
                    "test"
                } else {
                    "prod"
                }
                .into(),
            };
            *groups.entry(key).or_insert(0usize) += 1;
        }
    }
    let group_rows: Vec<_> = groups
        .iter()
        .take(top)
        .map(|(key, count)| {
            let field = q.member_view.group.as_deref().unwrap();
            let term = if field == "dir" {
                format!("member-dir={key}")
            } else {
                format!("member-{field}={key}")
            };
            json!({"key":key,"count":count,"next":[command(vec![term])]})
        })
        .collect();
    let mut expand = q
        .member_view
        .group
        .iter()
        .map(|g| format!("member-group={g}"))
        .collect::<Vec<_>>();
    expand.push("top=0".into());
    json!({"total":f.locations.len(),"selected":selected.len(),"shown":if q.member_view.group.is_some() {0} else {selected.len().min(top)},
        "group":q.member_view.group,"groups":group_rows,"groups_total":groups.len(),
        "locations":if q.member_view.group.is_some() {Vec::new()} else {selected.into_iter().take(top).map(|l| json!({"id":baseline::member_id(l),"file":l.file,"start":l.start_line,"end":l.end_line,"name":l.name,"lang":l.lang,"region":l.source_region,"scope_evidence":crate::query_assessment::scope(l)})).collect::<Vec<_>>()},
        "next":[command(vec!["member-group=dir".into()]),command(vec!["member-group=lang".into()]),command(vec!["member-group=scope".into()]),command(expand),base],
        "meaning":"Member selection only; family identity, evidence, metrics and assessment describe the complete family."})
}
pub(crate) fn render(view: &Value) {
    println!(
        "  members: {} selected / {} total",
        view["selected"], view["total"]
    );
    for group in view["groups"].as_array().unwrap() {
        println!(
            "    {} · {} copies\n      next: {}",
            group["key"].as_str().unwrap(),
            group["count"],
            group["next"][0].as_str().unwrap()
        );
    }
    for loc in view["locations"].as_array().unwrap() {
        println!(
            "    {}:{}-{} · {}",
            loc["file"].as_str().unwrap(),
            loc["start"],
            loc["end"],
            loc["scope_evidence"]
        );
    }
    println!("  {}", view["meaning"].as_str().unwrap());
    println!("next:");
    for command in view["next"].as_array().unwrap() {
        println!("  {}", command.as_str().unwrap());
    }
}

fn directory(loc: &Loc) -> String {
    std::path::Path::new(&loc.file)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(std::path::Path::new("."))
        .to_string_lossy()
        .into_owned()
}

pub(crate) fn capabilities() -> Value {
    json!({"requires":["id=ID", "at=FILE:LINE"],"terms":["member-group=dir|lang|scope", "member-dir=DIR", "member-path~TEXT", "member-lang=LANG", "member-scope=prod|test", "top=N", "full"],
        "formats":["human","json"],"metrics_scope":"complete-family","default_top":30})
}
