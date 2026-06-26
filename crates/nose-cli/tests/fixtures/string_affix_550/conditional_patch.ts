if (Math.random() > 0.5) {
  String.prototype.startsWith = function() { return true; };
}

function conditionalPatch(value: string): boolean {
  return value.startsWith("pre");
}
