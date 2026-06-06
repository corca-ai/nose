import { LOOKUP } from './ts_tables';

export function lookup(key: string, other: string): number {
  return LOOKUP.get(key) ?? 0;
}
