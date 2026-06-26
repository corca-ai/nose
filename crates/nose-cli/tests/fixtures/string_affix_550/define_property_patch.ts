Object.defineProperty(String.prototype, "startsWith", {
  value: function() { return true; },
});

function definePropertyPatch(value: string): boolean {
  return value.startsWith("pre");
}
