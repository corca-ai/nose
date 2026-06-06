function sumFiltered(xs, ys) {
  return xs
    .filter((x) => x > 0)
    .flatMap((x) => ys.filter((y) => y < 10).map((y) => x + y))
    .reduce((a, v) => a + v, 0);
}
