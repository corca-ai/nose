function borrowed(value) {
  return String.prototype.startsWith.call(value, "pre");
}
