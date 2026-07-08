use super::*;

fn assert_ruby_named_eq(src: &str, left: &str, right: &str, message: &str) {
    let i = Interner::new();
    assert_eq!(
        value_fp_named(&i, src, Lang::Ruby, left),
        value_fp_named(&i, src, Lang::Ruby, right),
        "{message}"
    );
}

fn assert_ruby_named_ne(src: &str, left: &str, right: &str, message: &str) {
    let i = Interner::new();
    assert_ne!(
        value_fp_named(&i, src, Lang::Ruby, left),
        value_fp_named(&i, src, Lang::Ruby, right),
        "{message}"
    );
}

#[test]
fn ruby_any_all_converge_for_literal_receivers_but_not_params() {
    let positive = "\
def ruby_any_literal_loop(a, b, c, min)
  for x in [a, b, c]
    if x > min
      return true
    end
  end
  false
end

def ruby_any_literal_call(a, b, c, min)
  [a, b, c].any? { |x| x > min }
end

def ruby_all_literal_loop(a, b, c, min)
  for x in [a, b, c]
    if !(x >= min)
      return false
    end
  end
  true
end

def ruby_all_literal_call(a, b, c, min)
  [a, b, c].all? { |x| x >= min }
end

def ruby_all_empty_loop
  for x in []
    if !(x >= 0)
      return false
    end
  end
  true
end

def ruby_all_empty_call
  [].all? { |x| x >= 0 }
end
";
    assert_ruby_named_eq(
        positive,
        "ruby_any_literal_loop",
        "ruby_any_literal_call",
        "Ruby literal any? should converge with the same-source early-return loop",
    );
    assert_ruby_named_eq(
        positive,
        "ruby_all_literal_loop",
        "ruby_all_literal_call",
        "Ruby literal all? should converge with the same-source counterexample loop",
    );
    assert_ruby_named_eq(
        positive,
        "ruby_all_empty_loop",
        "ruby_all_empty_call",
        "Ruby literal all? should preserve vacuous truth for empty literal arrays",
    );

    let param_boundary = "\
def ruby_any_param_loop(xs)
  for x in xs
    if x > 0
      return true
    end
  end
  false
end

def ruby_any_param_call(xs)
  xs.any? { |x| x > 0 }
end
";
    assert_ruby_named_ne(
        param_boundary,
        "ruby_any_param_loop",
        "ruby_any_param_call",
        "Ruby any? on an unproven receiver parameter must stay closed",
    );
}

#[test]
fn ruby_quantifiers_keep_predicate_source_and_block_boundaries() {
    let changed_predicate = "\
def ruby_any_literal_loop(a, b, c, min)
  for x in [a, b, c]
    if x > min
      return true
    end
  end
  false
end

def ruby_any_changed_predicate(a, b, c, min)
  [a, b, c].any? { |x| x >= min }
end
";
    assert_ruby_named_ne(
        changed_predicate,
        "ruby_any_literal_loop",
        "ruby_any_changed_predicate",
        "changing the Ruby any? predicate must stay distinct",
    );

    let different_source = "\
def ruby_all_literal_loop(a, b, c, d, min)
  for x in [a, b, c]
    if !(x >= min)
      return false
    end
  end
  true
end

def ruby_all_different_source(a, b, c, d, min)
  [a, b, d].all? { |x| x >= min }
end
";
    assert_ruby_named_ne(
        different_source,
        "ruby_all_literal_loop",
        "ruby_all_different_source",
        "Ruby all? must traverse the same proven source",
    );

    let block_boundaries = "\
def ruby_any_pure(a, b)
  [a, b].any? { |x| x > 0 }
end

def ruby_any_effect(seen, a, b)
  [a, b].any? do |x|
    seen << x
    x > 0
  end
end

def ruby_any_no_block(a, b)
  [a, b].any?
end
";
    assert_ruby_named_ne(
        block_boundaries,
        "ruby_any_pure",
        "ruby_any_effect",
        "Ruby any? blocks with observable effects stay outside the admitted perimeter",
    );
    assert_ruby_named_ne(
        block_boundaries,
        "ruby_any_pure",
        "ruby_any_no_block",
        "Ruby any? without a block has different Enumerable semantics and stays closed",
    );

    let multi_param_boundary = "\
def ruby_any_destructure_b(a, b, c)
  [[a, b]].any? { |x, y| y > 0 }
end

def ruby_any_destructure_c(a, b, c)
  [[a, c]].any? { |x, y| y > 0 }
end
";
    assert_ruby_named_ne(
        multi_param_boundary,
        "ruby_any_destructure_b",
        "ruby_any_destructure_c",
        "Ruby multi-param blocks destructure array elements and stay closed until modeled",
    );
}

#[test]
fn ruby_quantifier_monkey_patch_stays_closed() {
    let monkey_patch = "\
class Array
  def any?
    false
  end
end

def ruby_any_literal_loop(a, b)
  for x in [a, b]
    if x > 0
      return true
    end
  end
  false
end

def ruby_any_monkey_patched(a, b)
  [a, b].any? { |x| x > 0 }
end
";
    assert_ruby_named_ne(
        monkey_patch,
        "ruby_any_literal_loop",
        "ruby_any_monkey_patched",
        "Ruby Array monkey patches must close Enumerable quantifier admission",
    );

    let module_eval_patch = "\
Enumerable.module_eval do
  def any?
    false
  end
end

def ruby_any_literal_loop(a, b)
  for x in [a, b]
    if x > 0
      return true
    end
  end
  false
end

def ruby_any_module_eval_patched(a, b)
  [a, b].any? { |x| x > 0 }
end
";
    assert_ruby_named_ne(
        module_eval_patch,
        "ruby_any_literal_loop",
        "ruby_any_module_eval_patched",
        "Ruby Enumerable.module_eval monkey patches must close quantifier admission",
    );
}
