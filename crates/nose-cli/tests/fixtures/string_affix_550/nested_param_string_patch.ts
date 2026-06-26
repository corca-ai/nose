function unrelated(String: unknown): boolean {
  return Boolean(String);
}

String.prototype.startsWith = function() { return true; };

function nestedParamStringPatch(value: string): boolean {
  return value.startsWith("pre");
}
