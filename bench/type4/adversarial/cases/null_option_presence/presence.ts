function tsMissing(value: unknown | null | undefined, other: unknown | null | undefined): boolean {
  return value == null;
}

function tsPresent(value: unknown | null | undefined, other: unknown | null | undefined): boolean {
  return value != null;
}
