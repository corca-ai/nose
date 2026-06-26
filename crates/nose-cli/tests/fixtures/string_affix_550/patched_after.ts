function patchedAfter(value: string): boolean {
  return value.startsWith("pre");
}

String.prototype.startsWith = function() { return true; };
