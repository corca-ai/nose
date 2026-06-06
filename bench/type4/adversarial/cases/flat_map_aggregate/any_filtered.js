function anyFiltered(xs, ys) {
  return xs
    .filter((x) => x > 0)
    .flatMap((x) => ys.filter((y) => y < 10).map((y) => x + y))
    .some((v) => v > 0);
}
