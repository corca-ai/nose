function axisCase(xs) {
  let total = 0;
  for (let i = 1; i < xs.length; i++) {
    total += xs[i];
  }
  return total;
}
