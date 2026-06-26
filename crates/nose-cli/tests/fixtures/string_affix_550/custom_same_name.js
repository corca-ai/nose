function custom(value) {
  const box = { startsWith(prefix) { return prefix.length > 0; } };
  return box.startsWith("pre");
}
