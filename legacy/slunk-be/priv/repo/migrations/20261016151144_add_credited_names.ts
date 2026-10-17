// Credited names (3_alternate_names.md §4.2, §5): each artist link says which
// of the artist's alternate names was printed there, or NULL for the main
// name. A key over (artist_id, credited_name_id) keeps the name the linked
// artist's own. ADD COLUMN can't add a key over two columns, so the four link
// tables are rebuilt. Every existing link keeps the main name, as it shows now.
import { type CreateTableBuilder, type Kysely, sql } from "kysely"
import { rebuildTable } from "../../../src/repo/rebuild_table.ts"

/** Each artist link table: the release, track or credit line it links to. */
const LINKS = [
  { table: "artists_releases", owner: "release_id", parent: "releases.id" },
  { table: "artists_tracks", owner: "track_id", parent: "tracks.id" },
  {
    table: "artists_release_credits",
    owner: "credit_id",
    parent: "release_credits.id",
  },
  {
    table: "artists_track_credits",
    owner: "credit_id",
    parent: "track_credits.id",
  },
] as const

type Link = (typeof LINKS)[number]

export async function up(db: Kysely<unknown>): Promise<void> {
  for (const link of LINKS) {
    // The rebuild brings indexes back as they were. This one changes.
    await db.schema.dropIndex(`${link.table}_artist_id_index`).execute()
    await rebuildTable(db, link.table, {
      define: (table) => define(table, link, { creditedName: true }),
    })
    await db.schema
      .createIndex(`${link.table}_artist_id_credited_name_id_index`)
      .on(link.table)
      .columns(["artist_id", "credited_name_id"])
      .execute()
  }
}

export async function down(db: Kysely<unknown>): Promise<void> {
  for (const link of LINKS) {
    await db.schema
      .dropIndex(`${link.table}_artist_id_credited_name_id_index`)
      .execute()
    await rebuildTable(db, link.table, {
      define: (table) => define(table, link, { creditedName: false }),
    })
    await db.schema
      .createIndex(`${link.table}_artist_id_index`)
      .on(link.table)
      .column("artist_id")
      .execute()
  }
}

/** A link table as the baseline and 2_credits.md made it, and its name. */
function define(
  table: CreateTableBuilder<string>,
  link: Link,
  { creditedName }: { creditedName: boolean }
) {
  let builder = table
    .addColumn("id", "integer", (col) => col.primaryKey().notNull())
    .addColumn("artist_id", "integer", (col) =>
      col.notNull().references("artists.id").onDelete("restrict")
    )
    .addColumn(link.owner, "integer", (col) =>
      col.notNull().references(link.parent).onDelete("cascade")
    )
  if (link.table === "artists_tracks") {
    builder = builder.addColumn("featured", "integer", (col) =>
      col
        .notNull()
        .defaultTo(0)
        .check(sql`featured IN (0, 1)`)
    )
  }
  builder = builder.addColumn("position", "integer", (col) =>
    col.notNull().check(sql`position >= 0`)
  )
  if (creditedName) {
    // NULL is the main name, and SQLite doesn't check a key with a NULL in
    // it. RESTRICT, since SET NULL would clear artist_id too.
    builder = builder
      .addColumn("credited_name_id", "integer")
      .addForeignKeyConstraint(
        `${link.table}_credited_name_is_the_artists`,
        ["artist_id", "credited_name_id"],
        "artist_alternate_names",
        ["artist_id", "id"],
        (fk) => fk.onDelete("restrict")
      )
  }
  return builder.modifyEnd(sql`strict`)
}
