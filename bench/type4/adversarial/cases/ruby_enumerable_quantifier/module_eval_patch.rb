Enumerable.module_eval do
  def any?
    false
  end
end

def ruby_any_module_eval_patched(a, b)
  [a, b].any? { |x| x > 0 }
end
