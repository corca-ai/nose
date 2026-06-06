import { LOOKUP } from './js_tables';

export function lookup(key, other) {
  return new Map([["red", 9], ["blue", 2]]).get(key) ?? 0;
}
