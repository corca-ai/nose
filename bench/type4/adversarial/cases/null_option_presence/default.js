function jsCoalesce(value, fallback) {
  return value ?? fallback;
}

function jsTernary(value, fallback) {
  return value == null ? fallback : value;
}

function jsGuard(value, fallback) {
  if (value == null) {
    return fallback;
  }
  return value;
}

function jsTruthy(value, fallback) {
  return value || fallback;
}

function jsWrongFallback(value, fallback, otherDefault) {
  return value ?? otherDefault;
}

function jsStrictNull(value, fallback) {
  return value === null ? fallback : value;
}
