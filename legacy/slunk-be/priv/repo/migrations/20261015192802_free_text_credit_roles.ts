// Credits with free-text roles (2_credits.md §4, §5). A credit is a line: a
// role as printed, and its artists in order. `credit_roles` goes away, and
// the old credits are dropped with it rather than carried over.
import { type CreateTableBuilder, type Kysely, sql } from "kysely"

const OWNERS = ["release", "track"] as const

export async function up(db: Kysely<unknown>): Promise<void> {
  for (const owner of OWNERS) {
    await db.schema.dropTable(`${owner}_credits`).execute()
  }
  await db.schema.dropTable("credit_roles").execute()

  for (const owner of OWNERS) {
    const lines = `${owner}_credits`
    const artists = `artists_${owner}_credits`

    await db.schema
      .createTable(lines)
      .addColumn("id", "integer", (col) => col.primaryKey().notNull())
      .addColumn(`${owner}_id`, "integer", (col) =>
        col.notNull().references(`${owner}s.id`).onDelete("cascade")
      )
      .addColumn("role", "text", (col) =>
        col
          .notNull()
          .check(trimmed("role"))
          .modifyEnd(sql`COLLATE NOCASE`)
      )
      .$call(position)
      // Always 0. With the key below, it says the line has an artist at
      // position 0 (2_credits.md §4.2). The key is checked at commit, once
      // the line's artists are in, since they're inserted after it.
      .addColumn("first_artist_position", "integer", (col) =>
        col.notNull().generatedAlwaysAs(sql`0`)
      )
      .addForeignKeyConstraint(
        `${lines}_has_artist`,
        ["id", "first_artist_position"],
        artists,
        ["credit_id", "position"],
        (fk) => fk.deferrable().initiallyDeferred()
      )
      .modifyEnd(sql`strict`)
      .execute()
    await index(db, lines, [`${owner}_id`, "role"], { unique: true })

    await db.schema
      .createTable(artists)
      .addColumn("id", "integer", (col) => col.primaryKey().notNull())
      .addColumn("artist_id", "integer", (col) =>
        col.notNull().references("artists.id").onDelete("restrict")
      )
      .addColumn("credit_id", "integer", (col) =>
        col.notNull().references(`${lines}.id`).onDelete("cascade")
      )
      .$call(position)
      .modifyEnd(sql`strict`)
      .execute()
    await index(db, artists, ["credit_id", "artist_id"], { unique: true })
    // The parent of the line's key
    await index(db, artists, ["credit_id", "position"], { unique: true })
    await index(db, artists, ["artist_id"])
  }
}

export async function down(db: Kysely<unknown>): Promise<void> {
  for (const owner of OWNERS) {
    await db.schema.dropTable(`artists_${owner}_credits`).execute()
    await db.schema.dropTable(`${owner}_credits`).execute()
  }

  // The baseline's tables, empty except for its starter roles
  await db.schema
    .createTable("credit_roles")
    .addColumn("id", "integer", (col) => col.primaryKey().notNull())
    .addColumn("name", "text", (col) =>
      col
        .notNull()
        .check(trimmed("name"))
        .modifyEnd(sql`COLLATE NOCASE`)
    )
    .modifyEnd(sql`strict`)
    .execute()
  await index(db, "credit_roles", ["name"], { unique: true })
  for (const owner of OWNERS) {
    const table = `${owner}_credits`
    await db.schema
      .createTable(table)
      .addColumn("id", "integer", (col) => col.primaryKey().notNull())
      .addColumn(`${owner}_id`, "integer", (col) =>
        col.notNull().references(`${owner}s.id`).onDelete("cascade")
      )
      .addColumn("artist_id", "integer", (col) =>
        col.notNull().references("artists.id").onDelete("restrict")
      )
      .addColumn("role_id", "integer", (col) =>
        col.notNull().references("credit_roles.id").onDelete("restrict")
      )
      .$call(position)
      .modifyEnd(sql`strict`)
      .execute()
    await index(db, table, [`${owner}_id`, "artist_id", "role_id"], {
      unique: true,
    })
    await index(db, table, ["artist_id"])
    await index(db, table, ["role_id"])
  }
  await sql`
    INSERT INTO credit_roles (name)
    VALUES ('composer'), ('conductor'), ('producer'), ('remixer'), ('writer')
  `.execute(db)
}

/** Non-empty, with no leading or trailing spaces, as in the baseline. */
function trimmed(column: string) {
  const ref = sql.ref(column)
  return sql`${ref} <> '' AND ${ref} = trim(${ref})`
}

/** Display order, as in the baseline */
function position<T extends string, C extends string>(
  table: CreateTableBuilder<T, C>
) {
  return table.addColumn("position", "integer", (col) =>
    col.notNull().check(sql`position >= 0`)
  )
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
