use super::*;
use crate::test_support::scoring_units;

fn reference_parents(units: &[UnitFeat]) -> Vec<Option<usize>> {
    units
        .iter()
        .map(|child| {
            if child.fragment_kind.is_none() && child.kind != UnitKind::Block {
                return None;
            }
            units
                .iter()
                .enumerate()
                .filter(|(_, parent)| {
                    parent.fragment_kind.is_none()
                        && matches!(
                            parent.kind,
                            UnitKind::Function | UnitKind::Method | UnitKind::Class
                        )
                        && parent.path == child.path
                        && parent.start_line <= child.start_line
                        && child.end_line <= parent.end_line
                        && (parent.start_line != child.start_line
                            || parent.end_line != child.end_line
                            || parent.kind != child.kind)
                })
                .min_by_key(|(index, parent)| {
                    (
                        LineSpan::new(parent.start_line, parent.end_line).line_count(),
                        parent.start_line,
                        parent.end_line,
                        *index,
                    )
                })
                .map(|(index, _)| index)
        })
        .collect()
}

fn assert_contexts(units: &[UnitFeat], parents: &[Option<usize>]) {
    let expected: Vec<_> = parents
        .iter()
        .map(|parent| parent.map(|index| enclosing_unit_of(&units[index])))
        .collect();
    let expected = rmp_serde::to_vec_named(&expected).unwrap();
    for threads in [1, 3] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        pool.install(|| {
            assert_eq!(enclosing_unit_indices(units), parents);
            assert_eq!(
                rmp_serde::to_vec_named(&enclosing_units(units)).unwrap(),
                expected
            );
        });
    }
}

#[test]
fn enclosing_context_preserves_kind_boundaries_files_and_first_parent_ties() {
    use UnitKind::{Block, Class, Function, Method};
    let cases = [
        ("a.rs", Function, 1, 100, false),
        ("a.rs", Function, 10, 30, false),
        ("a.rs", Method, 10, 30, false),
        ("a.rs", Function, 10, 30, false),
        ("a.rs", Block, 10, 30, false),
        ("a.rs", Function, 10, 30, true),
        ("a.rs", Block, 15, 18, false),
        ("b.rs", Block, 15, 18, false),
        ("a.rs/more", Block, 15, 18, false),
        ("b.rs", Function, 1, 100, false),
        ("a.rs", Block, 100, 101, false),
        ("a.rs", Block, 35, 34, false),
        ("a.rs", Block, 10, 9, false),
        ("a.rs", Function, 1, 100, true),
        ("a.rs", Class, 1, 100, false),
    ];
    let mut units = scoring_units(cases.len());
    for (index, (unit, (path, kind, start, end, fragment))) in
        units.iter_mut().zip(cases).enumerate()
    {
        unit.path = path.into();
        unit.kind = kind;
        unit.name = Some(format!("unit_{index}"));
        unit.start_line = start;
        unit.end_line = end;
        unit.fragment_kind = fragment.then_some(FragmentKind::DirectReturn);
        unit.source_region = None;
        unit.source_document = None;
    }
    let parents = [
        None,
        None,
        None,
        None,
        Some(1),
        Some(2),
        Some(1),
        Some(9),
        None,
        None,
        None,
        Some(0),
        Some(1),
        Some(14),
        None,
    ];
    assert_eq!(reference_parents(&units), parents);
    assert_contexts(&units, &parents);
}

#[test]
fn enclosing_context_matches_exhaustive_selection_across_parallel_batches() {
    for count in [0, 1, 31, 256, 257, 512, 513] {
        let mut units = scoring_units(count);
        for (index, unit) in units.iter_mut().enumerate() {
            unit.path = format!("src/{:02}.rs", index % 17);
            unit.name = (index % 11 != 0).then(|| format!("unit_{index}"));
            unit.kind = match index % 4 {
                0 => UnitKind::Function,
                1 => UnitKind::Method,
                2 => UnitKind::Class,
                _ => UnitKind::Block,
            };
            unit.start_line = (index * 23 % 150) as u32 + 1;
            unit.end_line = if index % 19 == 0 {
                unit.start_line.saturating_sub(3)
            } else {
                unit.start_line + (index % 45) as u32
            };
            unit.fragment_kind = (index % 7 == 0).then_some(FragmentKind::DirectReturn);
            unit.source_region = None;
            unit.source_document = None;
            if index < 17 {
                unit.kind = UnitKind::Function;
                unit.start_line = 1;
                unit.end_line = 200;
                unit.fragment_kind = None;
            }
        }
        assert_contexts(&units, &reference_parents(&units));
    }
}
