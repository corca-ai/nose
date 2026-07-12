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
        ..Default::default()
    }
}

fn crosses_files(left: &str, right: &str, a: &str, b: &str) -> bool {
    (left == a && right == b) || (left == b && right == a)
}

#[test]
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
                && pair.left.name.as_deref().is_some_and(|name| name.starts_with("validate_"))
                && pair.right.name.as_deref().is_some_and(|name| name.starts_with("validate_"))
        })
        .expect("the connected validation ladders should be accepted");
    let (left_span, right_span) = if pair.left.file == "left.c" {
        (pair.left.shared_subdag, pair.right.shared_subdag)
    } else {
        (pair.right.shared_subdag, pair.left.shared_subdag)
    };
    assert_eq!(left_span, Some((11, 25)));
    assert_eq!(right_span, Some((9, 23)));
}
