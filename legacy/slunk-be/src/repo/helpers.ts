import { type Expression, type SqlBool, sql } from "kysely"
import { likePattern } from "../slunk/paging.ts"

// Stays well under SQLite's limit on bound parameters
export const CHUNK = 500

/** Splits ids into chunks for `in (…)` lists. */
export function chunks<T>(items: readonly T[]): T[][] {
  const result: T[][] = []
  for (let i = 0; i < items.length; i += CHUNK) {
    result.push(items.slice(i, i + CHUNK))
  }
  return result
}

/** `column like '%search%'`, ignoring case, with the pattern escaped. */
export function contains(column: string, search: string): Expression<SqlBool> {
  return sql<SqlBool>`${sql.ref(column)} like ${likePattern(search)} escape '\\'`
}

/** Groups rows into a map of lists, keeping their order. */
export function groupBy<T, K, V>(
  rows: T[],
  key: (row: T) => K,
  value: (row: T) => V
): Map<K, V[]> {
  const map = new Map<K, V[]>()
  for (const row of rows) {
    const list = map.get(key(row))
    if (list) list.push(value(row))
    else map.set(key(row), [value(row)])
  }
  return map
}
