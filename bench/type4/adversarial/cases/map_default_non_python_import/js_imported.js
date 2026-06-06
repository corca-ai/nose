import { LOOKUP } from './js_tables';

export function lookup(key, other) {
  return LOOKUP.get(key) ?? 0;
}
