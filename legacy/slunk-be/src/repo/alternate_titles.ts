import { type Kysely, sql } from "kysely"
import type { AlternateTitle } from "../slunk/catalog/repository.ts"
import type { DB } from "./db_types.ts"
import { chunks, contains, groupBy } from "./helpers.ts"

/** Each owner's table of alternate titles, and its column. */
const TABLES = {
  release: { table: "release_alternate_titles", column: "release_id" },
  track: { table: "track_alternate_titles", column: "track_id" },
  disc: { table: "release_disc_alternate_titles", column: "disc_title_id" },
} as const

type Owner = keyof typeof TABLES

/** Alternate titles by owner, in order (3_alternate_names.md §4.2). */
export async function alternateTitlesOf(
  db: Kysely<DB>,
  owner: Owner,
  ids: readonly number[]
): Promise<Map<number, AlternateTitle[]>> {
  const { table, column } = TABLES[owner]
  const rows: {
    ownerId: number
    title: string
    language: string | null
    printed: number
  }[] = []
  for (const chunk of chunks(ids)) {
    const { rows: found } = await sql<(typeof rows)[number]>`
      select ${sql.ref(column)} as ownerId, title, language, printed
      from ${sql.table(table)}
      where ${sql.ref(column)} in (${sql.join(chunk)})
      order by ${sql.ref(column)}, position
    `.execute(db)
    rows.push(...found)
  }
  return groupBy(
    rows,
    (row) => row.ownerId,
    ({ title, language, printed }) => ({
      title,
      language,
      printed: printed === 1,
    })
  )
}

/** Replaces an owner's alternate titles, in order. */
export async function setAlternateTitles(
  db: Kysely<DB>,
  owner: Owner,
  id: number,
  titles: AlternateTitle[]
): Promise<void> {
  const { table, column } = TABLES[owner]
  await sql`
    delete from ${sql.table(table)} where ${sql.ref(column)} = ${id}
  `.execute(db)
  for (const [position, { title, language, printed }] of titles.entries()) {
    await sql`
      insert into ${sql.table(table)}
        (${sql.ref(column)}, title, language, printed, position)
      values (${id}, ${title}, ${language}, ${printed ? 1 : 0}, ${position})
    `.execute(db)
  }
}

/**
 * The owners whose title or any alternate title contains the search, for a
 * list's `q` (3_alternate_names.md §6).
 */
export function titleMatches(owner: "release" | "track", search: string) {
  const { table, column } = TABLES[owner]
  return sql<boolean>`(${contains("title", search)} or id in (
    select ${sql.ref(column)} from ${sql.table(table)}
    where ${contains("title", search)}
  ))`
}
