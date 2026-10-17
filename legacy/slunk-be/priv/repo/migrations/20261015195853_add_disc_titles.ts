// Disc titles: a release's disc can have a title of its own, as printed
// ("Live At Madison Square Garden"). Only discs with a title have a row. A
// disc exists while it has tracks, so a title needs a track on its disc, and
// goes when the disc's last track leaves it.
import { type Kysely, sql } from "kysely"

export async function up(db: Kysely<unknown>): Promise<void> {
  await db.schema
    .createTable("release_disc_titles")
    .addColumn("id", "integer", (col) => col.primaryKey().notNull())
    .addColumn("release_id", "integer", (col) =>
      col.notNull().references("releases.id").onDelete("cascade")
    )
    .addColumn("cd_number", "integer", (col) =>
      col.notNull().check(sql`cd_number > 0`)
    )
    .addColumn("title", "text", (col) => col.notNull().check(trimmed("title")))
    .modifyEnd(sql`strict`)
    .execute()
  await db.schema
    .createIndex("release_disc_titles_release_id_cd_number_index")
    .on("release_disc_titles")
    .columns(["release_id", "cd_number"])
    .unique()
    .execute()

  for (const [event, when] of [
    ["insert", "INSERT"],
    ["update", "UPDATE OF release_id, cd_number"],
  ]) {
    await sql
      .raw(
        `CREATE TRIGGER release_disc_titles_disc_has_tracks_${event}
        BEFORE ${when} ON release_disc_titles
        WHEN NOT EXISTS (
          SELECT 1 FROM tracks
          WHERE release_id = NEW.release_id AND cd_number = NEW.cd_number
        )
        BEGIN
          SELECT RAISE(ABORT, 'a disc title needs a track on its disc');
        END`
      )
      .execute(db)
  }

  // The last track leaving a disc, deleted or moved, takes its title along
  for (const [event, when, moved] of [
    ["delete", "DELETE", ""],
    [
      "update",
      "UPDATE OF release_id, cd_number",
      "WHEN OLD.release_id <> NEW.release_id OR OLD.cd_number <> NEW.cd_number",
    ],
  ]) {
    await sql
      .raw(
        `CREATE TRIGGER tracks_empty_disc_loses_title_${event}
        AFTER ${when} ON tracks
        ${moved}
        BEGIN
          DELETE FROM release_disc_titles
          WHERE release_id = OLD.release_id AND cd_number = OLD.cd_number
            AND NOT EXISTS (
              SELECT 1 FROM tracks
              WHERE release_id = OLD.release_id AND cd_number = OLD.cd_number
            );
        END`
      )
      .execute(db)
  }
}

export async function down(db: Kysely<unknown>): Promise<void> {
  for (const event of ["delete", "update"]) {
    await sql
      .raw(`DROP TRIGGER tracks_empty_disc_loses_title_${event}`)
      .execute(db)
  }
  // Its own triggers go with it
  await db.schema.dropTable("release_disc_titles").execute()
}

/** Non-empty, with no leading or trailing spaces, as in the baseline. */
function trimmed(column: string) {
  const ref = sql.ref(column)
  return sql`${ref} <> '' AND ${ref} = trim(${ref})`
}
