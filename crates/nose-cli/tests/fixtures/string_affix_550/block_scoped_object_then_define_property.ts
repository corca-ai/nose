{
  const Object = { defineProperty() {} };
  Object.defineProperty(String.prototype, "startsWith", {});
}

Object.defineProperty(String.prototype, "startsWith", {
  value: function() { return true; },
});

function blockScopedObjectPatch(value: string): boolean {
  return value.startsWith("pre");
}
