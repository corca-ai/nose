const String = { prototype: {} };
String.prototype.startsWith = function() { return false; };

function shadowedPatch(value: string): boolean {
  return value.startsWith("pre");
}
