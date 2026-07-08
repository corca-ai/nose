function jsMutatedSetMember(value, other) {
  const values = new Set(["red", "blue"]);
  values.add("green");
  return values.has(value);
}
