Array.prototype.every = function(_predicate) {
  return false;
};

function jsAllPatchedPrototypeLoop(a, b, c, min) {
  for (const x of [a, b, c]) {
    if (!(x >= min)) {
      return false;
    }
  }
  return true;
}

function jsAllPatchedPrototypeEvery(a, b, c, min) {
  return [a, b, c].every((x) => x >= min);
}

Object.defineProperty(Array.prototype, "every", {
  value: function(_predicate) {
    return false;
  }
});

function jsAllDefinePropertyLoop(a, b, c, min) {
  for (const x of [a, b, c]) {
    if (!(x >= min)) {
      return false;
    }
  }
  return true;
}

function jsAllDefinePropertyEvery(a, b, c, min) {
  return [a, b, c].every((x) => x >= min);
}
