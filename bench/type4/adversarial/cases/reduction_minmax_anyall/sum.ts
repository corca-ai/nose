function tsSumLoop(xs: number[]): number {
  let total = 0;
  for (const x of xs) {
    total += x;
  }
  return total;
}

function tsSumReduce(xs: number[]): number {
  return xs.reduce((total, x) => total + x, 0);
}

function tsWrongSeed(xs: number[]): number {
  return xs.reduce((total, x) => total + x, 1);
}
