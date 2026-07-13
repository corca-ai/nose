//! End-to-end contracts for candidate pairs promoted by a connected mapped witness.
//!
//! These fixtures keep the candidate stream and the near threshold fixed. A positive
//! must already be a raw candidate, then earn acceptance only by carrying one ordered,
//! source-bounded region on both sides.

use nose_detect::{detect_with_dump, DetectOptions, StructuralDetector};
use nose_il::{Corpus, FileId, Interner, Lang};

fn corpus(files: &[(&str, &str, Lang)]) -> Corpus {
    let interner = Interner::new();
    let files = files
        .iter()
        .enumerate()
        .map(|(index, (path, source, lang))| {
            nose_frontend::lower_source(
                FileId(index as u32),
                path,
                source.as_bytes(),
                *lang,
                &interner,
            )
            .expect("lower connected-witness fixture")
        })
        .collect();
    Corpus::new(interner, files)
}

fn candidate_options() -> DetectOptions {
    DetectOptions {
        threshold: 0.70,
        min_lines: 3,
        min_tokens: 12,
        shape_candidates: true,
        connected_witnesses: true,
        ..Default::default()
    }
}

fn crosses_files(left: &str, right: &str, a: &str, b: &str) -> bool {
    (left == a && right == b) || (left == b && right == a)
}

#[test]
#[allow(clippy::too_many_lines)] // The two source fixtures are intentionally readable in full.
fn connected_validation_ladder_promotes_an_existing_candidate() {
    let left = r#"
int validate_left(Item *items, int count) {
  int problem = 0;
  prepare_left(items);
  audit_left(count);
  for(int i = 0; i < count; i++) {
    Address *actual = lookup_left(items[i]);
    char text[64] = {0};
    int port = 0;
    if(!actual && !items[i].expected) break;
    if(items[i].skip) continue;
    if(actual && !convert(actual, text, &port)) {
      report("left conversion", i); problem = 1; break;
    }
    if(actual && !items[i].expected) {
      report("unexpected address", i); problem = 1; break;
    }
    if(!actual && items[i].expected) {
      report("missing address", i); problem = 1; break;
    }
    if(!same_address(text, items[i].expected)) {
      report("different address", i); problem = 1; break;
    }
    if(port != items[i].port) {
      report("different port", i); problem = 1; break;
    }
    if(items[i].permanent) record_permanent(i);
  }
  cleanup_left(items);
  return problem;
}
"#;
    let right = r#"
int validate_right(Item *items, int count) {
  int problem = 0;
  initialize_right(count);
  for(int i = 0; i < count; i++) {
    Address *actual = lookup_right(items[i]);
    char text[64] = {0};
    int port = 0;
    if(!actual && !items[i].expected) break;
    if(actual && !convert(actual, text, &port)) {
      report("right conversion", i); problem = 1; break;
    }
    if(actual && !items[i].expected) {
      report("unexpected address", i); problem = 1; break;
    }
    if(!actual && items[i].expected) {
      report("missing address", i); problem = 1; break;
    }
    if(!same_address(text, items[i].expected)) {
      report("different address", i); problem = 1; break;
    }
    if(port != items[i].port) {
      report("different port", i); problem = 1; break;
    }
    if(!actual) break;
    actual = actual->next;
  }
  cleanup_right(items);
  finish_right(problem);
  return problem;
}
"#;
    let opts = candidate_options();
    let (report, dump) = detect_with_dump(
        &corpus(&[("left.c", left, Lang::C), ("right.c", right, Lang::C)]),
        &opts,
        &StructuralDetector::candidates(opts.jaccard_weight).with_threshold(opts.threshold),
    );

    let left_unit = dump
        .units
        .iter()
        .position(|unit| unit.name.as_deref() == Some("validate_left"))
        .expect("left function unit");
    let right_unit = dump
        .units
        .iter()
        .position(|unit| unit.name.as_deref() == Some("validate_right"))
        .expect("right function unit");
    let left_outer = &dump.units[left_unit];
    let right_outer = &dump.units[right_unit];
    assert!(
        dump.candidates.iter().any(|&(left, right)| {
            let left = &dump.units[left as usize];
            let right = &dump.units[right as usize];
            let inside = |unit: &nose_detect::UnitLoc, outer: &nose_detect::UnitLoc| {
                unit.path == outer.path
                    && unit.start_line >= outer.start_line
                    && unit.end_line <= outer.end_line
            };
            (inside(left, left_outer) && inside(right, right_outer))
                || (inside(left, right_outer) && inside(right, left_outer))
        }),
        "the fixture must exercise acceptance of an existing raw candidate"
    );
    let pair = report
        .duplicates
        .iter()
        .find(|pair| {
            crosses_files(&pair.left.file, &pair.right.file, "left.c", "right.c")
                && pair.left.shared_subdag.is_some()
                && pair.right.shared_subdag.is_some()
        })
        .expect("the connected validation ladders should be accepted");
    let (left_span, right_span) = if pair.left.file == "left.c" {
        (pair.left.shared_subdag, pair.right.shared_subdag)
    } else {
        (pair.right.shared_subdag, pair.left.shared_subdag)
    };
    assert_eq!(left_span, Some((12, 26)));
    assert_eq!(right_span, Some((10, 24)));
    assert_eq!(
        (pair.left.start_line, pair.left.end_line),
        pair.left.shared_subdag.unwrap()
    );
    assert_eq!(
        (pair.right.start_line, pair.right.end_line),
        pair.right.shared_subdag.unwrap()
    );
}

#[test]
fn connected_edges_remain_pair_local_instead_of_clustering_transitively() {
    let a = r#"
int left(State *s) {
  prepare_left(s);
  if(s->alpha) {
    alpha_open(s, 1); alpha_scan(s); alpha_record(s); alpha_close(s); alpha_commit(s);
  }
  finish_left(s);
  return s->status;
}
"#;
    let b = r#"
int bridge(State *s) {
  prepare_bridge(s);
  if(s->alpha) {
    alpha_open(s, 2); alpha_scan(s); alpha_record(s); alpha_close(s); alpha_commit(s);
  }
  audit_bridge(s);
  if(s->beta) {
    beta_open(s, 3); beta_scan(s); beta_record(s); beta_close(s); beta_commit(s);
  }
  finish_bridge(s);
  return s->status;
}
"#;
    let c = r#"
int right(State *s) {
  prepare_right(s);
  if(s->beta) {
    beta_open(s, 4); beta_scan(s); beta_record(s); beta_close(s); beta_commit(s);
  }
  finish_right(s);
  return s->status;
}
"#;
    let opts = candidate_options();
    let (report, _) = detect_with_dump(
        &corpus(&[
            ("a.c", a, Lang::C),
            ("b.c", b, Lang::C),
            ("c.c", c, Lang::C),
        ]),
        &opts,
        &StructuralDetector::candidates(opts.jaccard_weight).with_threshold(opts.threshold),
    );

    let connected = report
        .duplicates
        .iter()
        .filter(|pair| pair.left.shared_subdag.is_some() && pair.right.shared_subdag.is_some())
        .collect::<Vec<_>>();
    assert!(
        connected
            .iter()
            .any(|pair| crosses_files(&pair.left.file, &pair.right.file, "a.c", "b.c")),
        "A-B should have its own alpha witness"
    );
    assert!(
        connected
            .iter()
            .any(|pair| crosses_files(&pair.left.file, &pair.right.file, "b.c", "c.c")),
        "B-C should have its own beta witness"
    );
    assert!(
        !report.duplicates.iter().any(|pair| crosses_files(
            &pair.left.file,
            &pair.right.file,
            "a.c",
            "c.c"
        )),
        "A-B plus B-C must not manufacture an A-C edge"
    );
    assert!(
        report.groups.iter().all(|group| group.members.len() == 2),
        "connected edges must remain two-member pair-local groups"
    );
}

#[test]
fn same_unit_route_reports_two_bounded_non_exact_locations() {
    let source = r#"
int set_option(const char *name, const char *value) {
  if (!strcmp(name, "progress")) {
    if (!strcmp(value, "true")) options.progress = 1;
    else if (!strcmp(value, "false")) options.progress = 0;
    else return -1;
    return 0;
  }
  if (!strcmp(name, "deepen-relative")) {
    if (!strcmp(value, "true")) options.deepen_relative = 1;
    else if (!strcmp(value, "false")) options.deepen_relative = 0;
    else return -1;
    return 0;
  }
  return 1;
}
"#;
    let opts = candidate_options();
    let (report, _) = detect_with_dump(
        &corpus(&[("options.c", source, Lang::C)]),
        &opts,
        &StructuralDetector::candidates(opts.jaccard_weight).with_threshold(opts.threshold),
    );

    let family = report
        .groups
        .iter()
        .find(|group| {
            group.witness.as_ref().map(|witness| witness.kind) == Some("bounded-same-unit-window")
        })
        .expect("the two branches should be one bounded same-unit family");
    assert_eq!(family.members.len(), 2);
    let left = &family.members[0];
    let right = &family.members[1];
    assert_eq!(left.file, right.file);
    assert!(left.end_line < right.start_line);
    assert!(family.members.iter().all(|location| {
        !location.is_fragment
            && location.fragment_kind.is_none()
            && location.reason_code.is_none()
            && location.name.is_none()
            && location
                .enclosing_unit
                .as_ref()
                .and_then(|unit| unit.name.as_deref())
                == Some("set_option")
    }));
}
