use super::*;

fn assert_ts_named_eq(src: &str, left: &str, right: &str, message: &str) {
    let i = Interner::new();
    assert_eq!(
        value_fp_named(&i, src, Lang::TypeScript, left),
        value_fp_named(&i, src, Lang::TypeScript, right),
        "{message}"
    );
}

fn assert_ts_named_ne(src: &str, left: &str, right: &str, message: &str) {
    let i = Interner::new();
    assert_ne!(
        value_fp_named(&i, src, Lang::TypeScript, left),
        value_fp_named(&i, src, Lang::TypeScript, right),
        "{message}"
    );
}

#[test]
fn typescript_every_converges_for_dense_literal_source_but_not_array_param() {
    let positive = "\
function tsAllDenseLoop(a: number, b: number, c: number, min: number): boolean {
  for (const x of [a, b, c]) {
    if (!(x >= min)) {
      return false;
    }
  }
  return true;
}

function tsAllDenseEvery(a: number, b: number, c: number, min: number): boolean {
  return [a, b, c].every((x) => x >= min);
}
";
    assert_ts_named_eq(
        positive,
        "tsAllDenseLoop",
        "tsAllDenseEvery",
        "TypeScript Array.every should converge with a pure same-source dense counterexample loop",
    );

    let array_param_boundary = "\
function tsAllParamLoop(xs: number[]): boolean {
  for (const x of xs) {
    if (!(x >= 0)) {
      return false;
    }
  }
  return true;
}

function tsAllParamEvery(xs: number[]): boolean {
  return xs.every((x) => x >= 0);
}
";
    assert_ts_named_ne(
        array_param_boundary,
        "tsAllParamLoop",
        "tsAllParamEvery",
        "a TypeScript number[] parameter is not dense proof because Array.every skips sparse holes",
    );
}

#[test]
fn typescript_every_keeps_truth_predicate_and_source_boundaries() {
    let wrong_empty_truth = "\
function tsAllEvery(): boolean {
  const xs: number[] = [];
  return xs.every((x) => x >= 0);
}

function tsAllWrongEmptyTruth(): boolean {
  const xs: number[] = [];
  for (const x of xs) {
    if (!(x >= 0)) {
      return false;
    }
  }
  return false;
}
";
    assert_ts_named_ne(
        wrong_empty_truth,
        "tsAllEvery",
        "tsAllWrongEmptyTruth",
        "changing vacuous truth must stay distinct",
    );

    let changed_predicate = "\
function tsAllEvery(a: number, b: number, c: number, min: number): boolean {
  return [a, b, c].every((x) => x >= min);
}

function tsAllChangedPredicate(a: number, b: number, c: number, min: number): boolean {
  return [a, b, c].every((x) => x > min);
}
";
    assert_ts_named_ne(
        changed_predicate,
        "tsAllEvery",
        "tsAllChangedPredicate",
        "changing the every predicate must stay distinct",
    );

    let different_source = "\
function tsAllEvery(a: number, b: number, c: number, d: number, min: number): boolean {
  return [a, b, c].every((x) => x >= min);
}

function tsAllDifferentSource(a: number, b: number, c: number, d: number, min: number): boolean {
  const ys = [a, b, d];
  for (const y of ys) {
    if (!(y >= min)) {
      return false;
    }
  }
  return true;
}
";
    assert_ts_named_ne(
        different_source,
        "tsAllEvery",
        "tsAllDifferentSource",
        "the loop and every receiver must traverse the same source",
    );
}

#[test]
fn typescript_every_keeps_effect_and_value_return_boundaries() {
    let effect_boundary = "\
function tsAllEveryPureWithSeen(seen: number[], bad: number, ok: number): boolean {
  const xs = [bad, ok];
  return xs.every((x) => x >= 0);
}

function tsAllEveryCallbackEffect(seen: number[], bad: number, ok: number): boolean {
  const xs = [bad, ok];
  return xs.every((x) => {
    seen.push(x);
    return x >= 0;
  });
}

function tsAllLoopWithObservedEffect(seen: number[], bad: number, ok: number): boolean {
  const xs = [bad, ok];
  for (const x of xs) {
    if (!(x >= 0)) {
      seen.push(x);
      return false;
    }
  }
  return true;
}
";
    assert_ts_named_ne(
        effect_boundary,
        "tsAllEveryPureWithSeen",
        "tsAllEveryCallbackEffect",
        "callback effects must stay outside the admitted every perimeter",
    );
    assert_ts_named_ne(
        effect_boundary,
        "tsAllEveryPureWithSeen",
        "tsAllLoopWithObservedEffect",
        "loop-body effects must stay outside the admitted every perimeter",
    );

    let value_return_boundary = "\
function tsEveryBooleanAnd(): boolean {
  const xs = [0, 1, 2];
  return xs.every((x) => x >= 0 && x <= 10);
}

function tsEveryValueReturningAnd(): boolean {
  const xs = [0, 1, 2];
  return xs.every((x) => x && x <= 10);
}
";
    assert_ts_named_ne(
        value_return_boundary,
        "tsEveryBooleanAnd",
        "tsEveryValueReturningAnd",
        "value-returning && operands must stay outside boolean-only predicate proof",
    );
}

#[test]
fn typescript_every_keeps_callback_extra_arg_boundaries() {
    let index_arg_boundary = "\
function tsEveryIndexShort(): boolean {
  return [10, 20].every((_x, index) => index < 2);
}

function tsEveryIndexLong(): boolean {
  return [10, 20, 30].every((_x, index) => index < 2);
}
";
    assert_ts_named_ne(
        index_arg_boundary,
        "tsEveryIndexShort",
        "tsEveryIndexLong",
        "callbacks that observe Array.every index arguments must stay outside value-only proof",
    );

    let source_arg_boundary = "\
function tsEverySourceArrayShort(): boolean {
  return [10, 20].every((_x, _index, source) => source.length === 2);
}

function tsEverySourceArrayLong(): boolean {
  return [10, 20, 30].every((_x, _index, source) => source.length === 2);
}
";
    assert_ts_named_ne(
        source_arg_boundary,
        "tsEverySourceArrayShort",
        "tsEverySourceArrayLong",
        "callbacks that observe Array.every source-array arguments must stay outside value-only proof",
    );
}
