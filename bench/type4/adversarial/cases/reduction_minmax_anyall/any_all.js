function jsAllLoop(a, b, c, min) {
  for (const x of [a, b, c]) {
    if (!(x >= min)) {
      return false;
    }
  }
  return true;
}

function jsAllEvery(a, b, c, min) {
  return [a, b, c].every((x) => x >= min);
}

function jsAllParamLoop(xs) {
  for (const x of xs) {
    if (!(x >= 0)) {
      return false;
    }
  }
  return true;
}

function jsAllParamEvery(xs) {
  return xs.every((x) => x >= 0);
}

function jsAllWrongEmptyTruth() {
  const xs = [];
  for (const x of xs) {
    if (!(x >= 0)) {
      return false;
    }
  }
  return false;
}

function jsAllChangedPredicate(a, b, c, min) {
  return [a, b, c].every((x) => x > min);
}

function jsAllDifferentSource(a, b, c, d, min) {
  const ys = [a, b, d];
  for (const y of ys) {
    if (!(y >= min)) {
      return false;
    }
  }
  return true;
}

function jsAllEveryPureWithSeen(seen, bad, ok) {
  const xs = [bad, ok];
  return xs.every((x) => x >= 0);
}

function jsAllEveryCallbackEffect(seen, bad, ok) {
  const xs = [bad, ok];
  return xs.every((x) => {
    seen.push(x);
    return x >= 0;
  });
}

function jsAllLoopWithObservedEffect(seen, bad, ok) {
  const xs = [bad, ok];
  for (const x of xs) {
    if (!(x >= 0)) {
      seen.push(x);
      return false;
    }
  }
  return true;
}

function jsEveryBooleanAnd() {
  const xs = [0, 1, 2];
  return xs.every((x) => x >= 0 && x <= 10);
}

function jsEveryValueReturningAnd() {
  const xs = [0, 1, 2];
  return xs.every((x) => x && x <= 10);
}

function jsEveryIndexShort() {
  return [10, 20].every((_x, index) => index < 2);
}

function jsEveryIndexLong() {
  return [10, 20, 30].every((_x, index) => index < 2);
}

function jsEverySourceArrayShort() {
  return [10, 20].every((_x, _index, source) => source.length === 2);
}

function jsEverySourceArrayLong() {
  return [10, 20, 30].every((_x, _index, source) => source.length === 2);
}
