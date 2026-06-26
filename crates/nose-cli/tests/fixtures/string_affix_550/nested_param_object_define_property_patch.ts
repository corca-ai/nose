function unrelated(Object: unknown): boolean {
  return Boolean(Object);
}

Object.defineProperty(String.prototype, "startsWith", {
  value: function() { return true; },
});

function nestedParamObjectPatch(value: string): boolean {
  return value.startsWith("pre");
}
