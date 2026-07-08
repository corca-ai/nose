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

def ruby_any_changed_predicate(a, b, c, min)
  [a, b, c].any? { |x| x >= min }
end

def ruby_all_different_source(a, b, c, d, min)
  [a, b, d].all? { |x| x >= min }
end

def ruby_any_pure_with_seen(seen, a, b)
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

def ruby_any_destructure_b(a, b, c)
  [[a, b]].any? { |x, y| y > 0 }
end

def ruby_any_destructure_c(a, b, c)
  [[a, c]].any? { |x, y| y > 0 }
end
