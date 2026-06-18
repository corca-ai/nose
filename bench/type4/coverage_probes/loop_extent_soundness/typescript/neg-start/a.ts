function axisCase(xs: number[]): number {
  let total = 0;
  for (let i = 0; i < xs.length; i++) {
    total += xs[i];
  }
  return total;
}
