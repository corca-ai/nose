function mapError(xs) {
  return xs.map((x) => 1 / 0);
}

function mapErrorLoop(xs) {
  const out = [];
  for (const x of xs) {
    out.push(1 / 0);
  }
  return out;
}

function emptyFlatMap(xs) {
  return [].flatMap((x) => [1 / 0]);
}

function scalarFlatMap(xs) {
  return xs.flatMap((x) => x);
}
