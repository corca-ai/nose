def f(xss)
  xss.flat_map { |xs| xs.map { |y| y } }
end
