//! Raw syntactic containment travels with token streams, including syntax-only runs.
use crate::EnclosingUnit;
use nose_il::{Il, Interner, UnitKind};

#[derive(serde::Serialize, serde::Deserialize)]
pub(super) struct Container {
    kind: UnitKind,
    name: Option<String>,
    start: u32,
    end: u32,
}

pub(super) fn collect(il: &Il, interner: &Interner) -> Vec<Container> {
    let mut containers = il
        .units
        .iter()
        .filter(|unit| {
            matches!(
                unit.kind,
                UnitKind::Function | UnitKind::Method | UnitKind::Class
            )
        })
        .map(|unit| {
            let span = il.node(unit.root).span;
            Container {
                kind: unit.kind,
                name: unit.name.map(|name| interner.resolve(name).to_string()),
                start: span.start_line,
                end: span.end_line,
            }
        })
        .collect::<Vec<_>>();
    containers.sort_by_key(|unit| (unit.end.saturating_sub(unit.start), unit.start));
    containers
}

pub(super) fn enclosing(
    containers: &[Container],
    path: &str,
    start: u32,
    end: u32,
) -> Option<EnclosingUnit> {
    let container = containers
        .iter()
        .find(|unit| unit.start <= start && unit.end >= end)?;
    let mut enclosing = EnclosingUnit {
        file: path.into(),
        start_line: container.start,
        end_line: container.end,
        kind: container.kind,
        name: container.name.clone(),
        unit_key: String::new(),
    };
    enclosing.refresh_unit_key();
    Some(enclosing)
}
