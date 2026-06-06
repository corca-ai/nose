function sumFlat(xs, ys) {
  return xs.flatMap((x) => ys.map((y) => x + y)).reduce((a, v) => a + v, 0);
}
