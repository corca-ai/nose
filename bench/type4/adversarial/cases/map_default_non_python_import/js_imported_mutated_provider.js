import { LOOKUP } from './js_mutated_tables';

export function lookup(key, other) {
  return LOOKUP.get(key) ?? 0;
}
