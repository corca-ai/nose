class Array
  def any?
    false
  end
end

def ruby_any_monkey_patched(a, b)
  [a, b].any? { |x| x > 0 }
end
