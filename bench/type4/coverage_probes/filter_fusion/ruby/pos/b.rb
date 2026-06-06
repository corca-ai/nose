def f(xs)
  xs.select { |x| x > 0 }.select { |y| y < 10 }
end
