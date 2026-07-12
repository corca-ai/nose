use crate::{
    model::{EnclosingUnit, Group, LineSpan, Loc, LocInit},
    units::UnitFeat,
    FragmentKind,
};
use nose_il::UnitKind;
use std::collections::HashMap;

pub(crate) fn loc_of(u: &UnitFeat, enclosing_unit: Option<EnclosingUnit>) -> Loc {
    let fragment_kind = u.fragment_kind;
    let mut loc = Loc::new(LocInit {
        file: u.path.clone(),
        source_span: LineSpan::new(u.start_line, u.end_line),
        lang: u.lang.name().to_string(),
        kind: u.kind,
        origin: u.origin,
        name: u.name.clone(),
        sem: u.value.len(),
        span_tokens: u.token_count,
    });
    loc.is_fragment = fragment_kind.is_some();
    loc.fragment_kind = fragment_kind;
    loc.reason_code = fragment_kind.map(FragmentKind::reason_code);
    loc.enclosing_unit = enclosing_unit;
    loc.in_test_module = u.in_test_module;
    loc
}

fn can_enclose_fragment(u: &UnitFeat) -> bool {
    u.fragment_kind.is_none()
        && matches!(
            u.kind,
            UnitKind::Function | UnitKind::Method | UnitKind::Class
        )
}

fn contains_span(parent: &UnitFeat, child: &UnitFeat) -> bool {
    parent.path == child.path
        && parent.start_line <= child.start_line
        && parent.end_line >= child.end_line
        // Strict containment, except that a DIFFERENT-kind parent may share the
        // exact span: a method and its whole-body block are one region in two
        // unit kinds, and the method IS the block's enclosing context (#225).
        // Same-kind twins still never enclose each other.
        && (parent.start_line < child.start_line
            || parent.end_line > child.end_line
            || parent.kind != child.kind)
}

fn enclosing_unit_of(parent: &UnitFeat) -> EnclosingUnit {
    let mut unit = EnclosingUnit {
        file: parent.path.clone(),
        start_line: parent.start_line,
        end_line: parent.end_line,
        kind: parent.kind,
        name: parent.name.clone(),
        unit_key: String::new(),
    };
    unit.refresh_unit_key();
    unit
}

/// Attach enclosing function/method names to copy-paste-run members. Contiguous
/// groups are built from token streams, not from the unit set, so they never
/// passed through `enclosing_units` — and the #216 audit's sampled block
/// locations (all contiguous) carried `name: null` with nothing to anchor a
/// discussion to (#225). A run that crosses unit boundaries keeps `None`.
pub(crate) fn attach_enclosing_units(groups: &mut [Group], units: &[UnitFeat]) {
    let mut by_file: HashMap<&str, Vec<usize>> = HashMap::new();
    for (idx, unit) in units.iter().enumerate() {
        if can_enclose_fragment(unit) {
            by_file.entry(unit.path.as_str()).or_default().push(idx);
        }
    }
    for parents in by_file.values_mut() {
        parents.sort_by_key(|&idx| {
            (
                LineSpan::new(units[idx].start_line, units[idx].end_line).line_count(),
                units[idx].start_line,
            )
        });
    }
    for group in groups {
        for loc in &mut group.members {
            if loc.enclosing_unit.is_some() {
                continue;
            }
            let Some(parents) = by_file.get(loc.file.as_str()) else {
                continue;
            };
            let parent = parents.iter().copied().find(|&idx| {
                let u = &units[idx];
                u.start_line <= loc.start_line && u.end_line >= loc.end_line
            });
            if let Some(idx) = parent {
                loc.enclosing_unit = Some(enclosing_unit_of(&units[idx]));
                loc.in_test_module = units[idx].in_test_module;
            } else {
                // A run crossing unit boundaries is test scaffolding iff EVERY
                // overlapping unit sits in the inline test module (#226 — the
                // alacritty audit run spanned four #[test] fns yet read `prod`).
                let overlapping: Vec<usize> = parents
                    .iter()
                    .copied()
                    .filter(|&idx| {
                        let u = &units[idx];
                        u.start_line <= loc.end_line && loc.start_line <= u.end_line
                    })
                    .collect();
                loc.in_test_module = !overlapping.is_empty()
                    && overlapping.iter().all(|&idx| units[idx].in_test_module);
            }
        }
    }
}

pub(crate) fn enclosing_unit_indices(units: &[UnitFeat]) -> Vec<Option<usize>> {
    let mut by_file: HashMap<&str, Vec<usize>> = HashMap::new();
    for (idx, unit) in units.iter().enumerate() {
        by_file.entry(unit.path.as_str()).or_default().push(idx);
    }

    let mut out = vec![None; units.len()];
    for indices in by_file.values() {
        let mut parents: Vec<usize> = indices
            .iter()
            .copied()
            .filter(|&idx| can_enclose_fragment(&units[idx]))
            .collect();
        parents.sort_by_key(|&idx| {
            (
                LineSpan::new(units[idx].start_line, units[idx].end_line).line_count(),
                units[idx].start_line,
                units[idx].end_line,
            )
        });

        for &idx in indices {
            // Fragments AND plain Block units get their enclosing
            // function/method recovered — an agent cannot even NAME the region
            // of a block family without it (#225: every sampled block location
            // had `name: null`). Whole function/method/class units need none.
            if units[idx].fragment_kind.is_none() && units[idx].kind != UnitKind::Block {
                continue;
            }
            if let Some(parent) = parents
                .iter()
                .copied()
                .find(|&parent_idx| contains_span(&units[parent_idx], &units[idx]))
            {
                out[idx] = Some(parent);
            }
        }
    }
    out
}

pub(crate) fn enclosing_units(units: &[UnitFeat]) -> Vec<Option<EnclosingUnit>> {
    enclosing_unit_indices(units)
        .into_iter()
        .map(|parent| parent.map(|index| enclosing_unit_of(&units[index])))
        .collect()
}

/// Two units from the same file where one span contains the other (e.g. a method
/// and its enclosing class) — exclude these trivial nesting matches.
pub(crate) fn is_nested(a: &UnitFeat, b: &UnitFeat) -> bool {
    a.path == b.path
        && ((a.start_line <= b.start_line && a.end_line >= b.end_line)
            || (b.start_line <= a.start_line && b.end_line >= a.end_line))
}
