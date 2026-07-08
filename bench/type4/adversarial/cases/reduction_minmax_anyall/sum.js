function jsSumLoop(xs) {
  let total = 0;
  for (const x of xs) {
    total += x;
  }
  return total;
}

function jsSumReduce(xs) {
  return xs.reduce((total, x) => total + x, 0);
}

function jsWrongSeed(xs) {
  return xs.reduce((total, x) => total + x, 1);
}

function jsProductReduce(xs) {
  return xs.reduce((product, x) => product * x, 1);
}
