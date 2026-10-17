// Alternate titles (3_alternate_names.md §4, §5): the other titles a release,
// a track or a disc goes by, each with a language and whether it's printed,
// and the language of their main titles. Titles compare exactly, case
// included. A disc's alternate titles belong to its title, so they go when it
// goes.
import { type Kysely, sql } from "kysely"

/** Each owner of alternate titles, and its table. */
const OWNERS = [
  {
    table: "release_alternate_titles",
    owner: "release_id",
    parent: "releases.id",
  },
  { table: "track_alternate_titles", owner: "track_id", parent: "tracks.id" },
  {
    table: "release_disc_alternate_titles",
    owner: "disc_title_id",
    parent: "release_disc_titles.id",
  },
] as const

/** The main titles' languages. Null on a track or disc is the release's. */
const LANGUAGES = [
  { table: "releases", column: "title_language" },
  { table: "tracks", column: "title_language" },
  { table: "release_disc_titles", column: "language" },
] as const

export async function up(db: Kysely<unknown>): Promise<void> {
  for (const { table, owner, parent } of OWNERS) {
    await db.schema
      .createTable(table)
      .addColumn("id", "integer", (col) => col.primaryKey().notNull())
      .addColumn(owner, "integer", (col) =>
        col.notNull().references(parent).onDelete("cascade")
      )
      .addColumn("title", "text", (col) =>
        col.notNull().check(trimmed("title"))
      )
      .addColumn("language", "text", (col) =>
        col.check(languageTag("language"))
      )
      .addColumn("printed", "integer", (col) =>
        col
          .notNull()
          .defaultTo(1)
          .check(sql`printed IN (0, 1)`)
      )
      .addColumn("position", "integer", (col) =>
        col.notNull().check(sql`position >= 0`)
      )
      .modifyEnd(sql`strict`)
      .execute()
    // Also indexes the foreign key
    await db.schema
      .createIndex(`${table}_${owner}_title_index`)
      .on(table)
      .columns([owner, "title"])
      .unique()
      .execute()
  }

  // Adding a column with a CHECK needs no rebuild
  for (const { table, column } of LANGUAGES) {
    await db.schema
      .alterTable(table)
      .addColumn(column, "text", (col) => col.check(languageTag(column)))
      .execute()
  }
}

export async function down(db: Kysely<unknown>): Promise<void> {
  for (const { table, column } of LANGUAGES) {
    await db.schema.alterTable(table).dropColumn(column).execute()
  }
  // Their indexes go with them
  for (const { table } of [...OWNERS].reverse()) {
    await db.schema.dropTable(table).execute()
  }
}

/** Non-empty, with no leading or trailing spaces, as in the baseline. */
function trimmed(column: string) {
  const ref = sql.ref(column)
  return sql`${ref} <> '' AND ${ref} = trim(${ref})`
}

/**
 * A BCP 47 tag's canonical shape, as in 20261016144030_add_alternate_names: a
 * 2- or 3-letter lowercase language, then letters, digits and single hyphens,
 * not ending in one. `und` means unknown, which is NULL. NULL passes.
 */
function languageTag(column: string) {
  const ref = sql.ref(column)
  return sql`(${ref} GLOB '[a-z][a-z]' OR ${ref} GLOB '[a-z][a-z][a-z]'
    OR ${ref} GLOB '[a-z][a-z]-?*' OR ${ref} GLOB '[a-z][a-z][a-z]-?*')
    AND ${ref} NOT GLOB '*[^A-Za-z0-9-]*'
    AND ${ref} NOT GLOB '*--*' AND ${ref} NOT GLOB '*-'
    AND ${ref} <> 'und' AND ${ref} NOT GLOB 'und-*'`
}
