function flatMapJs(xs, ys) {
  return xs.flatMap((x) => ys.map((y) => x + y));
}

function nestedListJs(xs, ys) {
  return xs.map((x) => ys.map((y) => x + y));
}
