{
  const String = { prototype: {} };
  String.prototype.startsWith = function() { return false; };
}

String.prototype.startsWith = function() { return true; };

function blockScopedStringPatch(value: string): boolean {
  return value.startsWith("pre");
}
