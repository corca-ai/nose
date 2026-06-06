import { LOOKUP } from './js_tables';

LOOKUP.set("red", 9);

export function lookup(key, other) {
  return LOOKUP.get(key) ?? 0;
}
