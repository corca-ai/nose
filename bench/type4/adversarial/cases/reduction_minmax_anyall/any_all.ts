function tsAnyLoop(xs: number[]): boolean {
  let found = false;
  for (const x of xs) {
    if (x > 0) {
      found = true;
      break;
    }
  }
  return found;
}

function tsAnySome(xs: number[]): boolean {
  return xs.some((x) => x > 0);
}

function tsAnyWrongPredicate(xs: number[]): boolean {
  return xs.some((x) => x < 0);
}

function tsAllLoop(a: number, b: number, c: number, min: number): boolean {
  for (const x of [a, b, c]) {
    if (!(x >= min)) {
      return false;
    }
  }
  return true;
}

function tsAllEvery(a: number, b: number, c: number, min: number): boolean {
  return [a, b, c].every((x) => x >= min);
}

function tsAllParamLoop(xs: number[]): boolean {
  for (const x of xs) {
    if (!(x >= 0)) {
      return false;
    }
  }
  return true;
}

function tsAllParamEvery(xs: number[]): boolean {
  return xs.every((x) => x >= 0);
}

function tsAllWrongEmptyTruth(): boolean {
  const xs: number[] = [];
  for (const x of xs) {
    if (!(x >= 0)) {
      return false;
    }
  }
  return false;
}

function tsAllChangedPredicate(a: number, b: number, c: number, min: number): boolean {
  return [a, b, c].every((x) => x > min);
}

function tsAllDifferentSource(
  a: number,
  b: number,
  c: number,
  d: number,
  min: number,
): boolean {
  const ys = [a, b, d];
  for (const y of ys) {
    if (!(y >= min)) {
      return false;
    }
  }
  return true;
}

function tsAllEveryPureWithSeen(seen: number[], bad: number, ok: number): boolean {
  const xs = [bad, ok];
  return xs.every((x) => x >= 0);
}

function tsAllEveryCallbackEffect(seen: number[], bad: number, ok: number): boolean {
  const xs = [bad, ok];
  return xs.every((x) => {
    seen.push(x);
    return x >= 0;
  });
}

function tsAllLoopWithObservedEffect(seen: number[], bad: number, ok: number): boolean {
  const xs = [bad, ok];
  for (const x of xs) {
    if (!(x >= 0)) {
      seen.push(x);
      return false;
    }
  }
  return true;
}

function tsEveryBooleanAnd(): boolean {
  const xs = [0, 1, 2];
  return xs.every((x) => x >= 0 && x <= 10);
}

function tsEveryValueReturningAnd(): boolean {
  const xs = [0, 1, 2];
  return xs.every((x) => x && x <= 10);
}

function tsEveryIndexShort(): boolean {
  return [10, 20].every((_x, index) => index < 2);
}

function tsEveryIndexLong(): boolean {
  return [10, 20, 30].every((_x, index) => index < 2);
}

function tsEverySourceArrayShort(): boolean {
  return [10, 20].every((_x, _index, source) => source.length === 2);
}

function tsEverySourceArrayLong(): boolean {
  return [10, 20, 30].every((_x, _index, source) => source.length === 2);
}
