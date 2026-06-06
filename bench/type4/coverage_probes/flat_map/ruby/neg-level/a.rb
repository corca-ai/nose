def f(xss)
  out = []
  xss.each { |xs| xs.each { |y| out << y } }
  out
end
