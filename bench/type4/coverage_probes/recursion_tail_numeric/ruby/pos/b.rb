def fac(n)
  acc = 1
  while n != 0
    acc = acc * n
    n = n - 1
  end
  acc
end
