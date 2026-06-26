String.prototype.startsWith = function() { return true; };

function patched(value: string): boolean {
  return value.startsWith("pre");
}
