// Alternate names (3_alternate_names.md §4, §5): the other names an artist or
// label goes by, each with a language, and the language of their main names.
// Names compare exactly, case included, so "ALIKA" isn't "Alika".
import { type Kysely, sql } from "kysely"

const OWNERS = ["artist", "label"] as const

export async function up(db: Kysely<unknown>): Promise<void> {
  for (const owner of OWNERS) {
    const table = `${owner}_alternate_names`
    let builder = db.schema
      .createTable(table)
      .addColumn("id", "integer", (col) => col.primaryKey().notNull())
      .addColumn(`${owner}_id`, "integer", (col) =>
        col.notNull().references(`${owner}s.id`).onDelete("cascade")
      )
      .addColumn("name", "text", (col) => col.notNull().check(trimmed("name")))
      .addColumn("language", "text", (col) =>
        col.check(languageTag("language"))
      )
    if (owner === "artist") {
      builder = builder.addColumn("real_name", "integer", (col) =>
        col
          .notNull()
          .defaultTo(0)
          .check(sql`real_name IN (0, 1)`)
      )
    }
    await builder
      .addColumn("note", "text", (col) => col.check(trimmed("note")))
      .addColumn("position", "integer", (col) =>
        col.notNull().check(sql`position >= 0`)
      )
      .modifyEnd(sql`strict`)
      .execute()
    await index(db, table, [`${owner}_id`, "name"], { unique: true })

    // Adding a column with a CHECK needs no rebuild
    await db.schema
      .alterTable(`${owner}s`)
      .addColumn("name_language", "text", (col) =>
        col.check(languageTag("name_language"))
      )
      .execute()
  }

  // The parent key of credited names (3_alternate_names.md §4.2), so a link
  // can only be credited with its own artist's name
  await index(db, "artist_alternate_names", ["artist_id", "id"], {
    unique: true,
  })
  // The import looks names up ignoring case
  await db.schema
    .createIndex("artist_alternate_names_name_index")
    .on("artist_alternate_names")
    .expression(sql`name COLLATE NOCASE`)
    .execute()
}

export async function down(db: Kysely<unknown>): Promise<void> {
  for (const owner of OWNERS) {
    await db.schema
      .alterTable(`${owner}s`)
      .dropColumn("name_language")
      .execute()
    // Its indexes go with it
    await db.schema.dropTable(`${owner}_alternate_names`).execute()
  }
}

/** Non-empty, with no leading or trailing spaces, as in the baseline. */
function trimmed(column: string) {
  const ref = sql.ref(column)
  return sql`${ref} <> '' AND ${ref} = trim(${ref})`
}

/**
 * A BCP 47 tag's canonical shape (3_alternate_names.md §4.3): a 2- or
 * 3-letter lowercase language, then letters, digits and single hyphens, not
 * ending in one. `und` means unknown, which is NULL. NULL passes.
 */
function languageTag(column: string) {
  const ref = sql.ref(column)
  return sql`(${ref} GLOB '[a-z][a-z]' OR ${ref} GLOB '[a-z][a-z][a-z]'
    OR ${ref} GLOB '[a-z][a-z]-?*' OR ${ref} GLOB '[a-z][a-z][a-z]-?*')
    AND ${ref} NOT GLOB '*[^A-Za-z0-9-]*'
    AND ${ref} NOT GLOB '*--*' AND ${ref} NOT GLOB '*-'
    AND ${ref} <> 'und' AND ${ref} NOT GLOB 'und-*'`
}

/** An index named `<table>_<columns>_index`, as in the baseline. */
async function index(
  db: Kysely<unknown>,
  table: string,
  columns: string[],
  { unique = false } = {}
) {
  const builder = db.schema
    .createIndex(`${table}_${columns.join("_")}_index`)
    .on(table)
    .columns(columns)
  await (unique ? builder.unique() : builder).execute()
}
