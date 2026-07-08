function tsCoalesce(value: number | null | undefined, fallback: number): number {
  return value ?? fallback;
}

function tsTernary(value: number | null | undefined, fallback: number): number {
  return value == null ? fallback : value;
}
