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

function tsAllLoop(xs: number[]): boolean {
  for (const x of xs) {
    if (!(x >= 0)) {
      return false;
    }
  }
  return true;
}

function tsAllEvery(xs: number[]): boolean {
  return xs.every((x) => x >= 0);
}
