// The baseline schema: every table of design §4.4, plus the rules of its
// Appendix A (text CHECKs, a track's file is music, pictures are images).
import { type CreateTableBuilder, type Kysely, sql } from "kysely"

export async function up(db: Kysely<unknown>): Promise<void> {
  // Files

  await db.schema
    .createTable("files")
    .addColumn("id", "integer", (col) =>
      col.primaryKey().autoIncrement().notNull()
    )
    .addColumn("path", "text", (col) =>
      // Relative to the database's folder (design §4.1)
      col.notNull().check(sql`path <> '' AND substr(path, 1, 1) <> '/'`)
    )
    .addColumn("type", "text", (col) =>
      col.notNull().check(sql`type IN ('music', 'image', 'other')`)
    )
    .addColumn("size_bytes", "integer", (col) =>
      col.notNull().check(sql`size_bytes >= 0`)
    )
    .addColumn("mtime", "integer", (col) => col.notNull())
    .addColumn("sha256", "text", (col) =>
      // Lowercase hex, so equal hashes compare equal
      col
        .notNull()
        .check(sql`length(sha256) = 64 AND sha256 NOT GLOB '*[^0-9a-f]*'`)
    )
    .$call(timestamps)
    .modifyEnd(sql`strict`)
    .execute()
  await index(db, "files", ["path"], { unique: true })
  await index(db, "files", ["sha256"])

  // Artists, labels, releases, tracks

  for (const table of ["artists", "labels"]) {
    await db.schema
      .createTable(table)
      .addColumn("id", "integer", (col) =>
        col.primaryKey().autoIncrement().notNull()
      )
      .addColumn("name", "text", (col) => col.notNull().check(trimmed("name")))
      .addColumn("picture", "integer", (col) =>
        col.references("files.id").onDelete("restrict")
      )
      .$call(timestamps)
      .modifyEnd(sql`strict`)
      .execute()
    await nocaseIndex(db, table, "name")
    await index(db, table, ["picture"])
  }

  await db.schema
    .createTable("releases")
    .addColumn("id", "integer", (col) =>
      col.primaryKey().autoIncrement().notNull()
    )
    .addColumn("title", "text", (col) => col.notNull().check(trimmed("title")))
    .addColumn("release_date", "integer")
    .addColumn("release_date_precision", "text", (col) =>
      col.check(sql`release_date_precision IN ('year', 'month', 'day')`)
    )
    .addColumn("catalog_number", "text", (col) =>
      col.check(trimmed("catalog_number")).modifyEnd(sql`COLLATE NOCASE`)
    )
    .addColumn("cover_picture", "integer", (col) =>
      col.references("files.id").onDelete("restrict")
    )
    .addColumn("various_artists", "integer", (col) =>
      col
        .notNull()
        .defaultTo(0)
        .check(sql`various_artists IN (0, 1)`)
    )
    .$call(timestamps)
    .addCheckConstraint(
      "releases_release_date_precision_set",
      sql`(release_date IS NULL) = (release_date_precision IS NULL)`
    )
    .addCheckConstraint(
      // Midnight UTC, on the 1st for a month, on January 1st for a year
      "releases_release_date_matches_precision",
      sql`release_date IS NULL OR (
        release_date % 86400 = 0
        AND (release_date_precision <> 'month'
             OR strftime('%d', release_date, 'unixepoch') = '01')
        AND (release_date_precision <> 'year'
             OR strftime('%m-%d', release_date, 'unixepoch') = '01-01')
      )`
    )
    .modifyEnd(sql`strict`)
    .execute()
  await nocaseIndex(db, "releases", "title")
  await index(db, "releases", ["release_date"])
  await index(db, "releases", ["catalog_number"])
  await index(db, "releases", ["cover_picture"])

  await db.schema
    .createTable("tracks")
    .addColumn("id", "integer", (col) =>
      col.primaryKey().autoIncrement().notNull()
    )
    .addColumn("title", "text", (col) => col.notNull().check(trimmed("title")))
    .addColumn("duration_ms", "integer", (col) =>
      col.check(sql`duration_ms >= 0`)
    )
    .addColumn("track_number", "integer", (col) =>
      col.notNull().check(sql`track_number > 0`)
    )
    .addColumn("cd_number", "integer", (col) =>
      col
        .notNull()
        .defaultTo(1)
        .check(sql`cd_number > 0`)
    )
    .addColumn("release_id", "integer", (col) =>
      col.notNull().references("releases.id").onDelete("restrict")
    )
    .addColumn("file_id", "integer", (col) =>
      col.notNull().references("files.id").onDelete("restrict")
    )
    .$call(timestamps)
    .modifyEnd(sql`strict`)
    .execute()
  await index(db, "tracks", ["release_id", "cd_number", "track_number"], {
    unique: true,
  })
  await index(db, "tracks", ["file_id"], { unique: true })

  // Release and track artists, labels

  await db.schema
    .createTable("artists_releases")
    .addColumn("id", "integer", (col) => col.primaryKey().notNull())
    .addColumn("artist_id", "integer", (col) =>
      col.notNull().references("artists.id").onDelete("restrict")
    )
    .addColumn("release_id", "integer", (col) =>
      col.notNull().references("releases.id").onDelete("cascade")
    )
    .$call(position)
    .modifyEnd(sql`strict`)
    .execute()
  await index(db, "artists_releases", ["release_id", "artist_id"], {
    unique: true,
  })
  await index(db, "artists_releases", ["artist_id"])

  await db.schema
    .createTable("artists_tracks")
    .addColumn("id", "integer", (col) => col.primaryKey().notNull())
    .addColumn("artist_id", "integer", (col) =>
      col.notNull().references("artists.id").onDelete("restrict")
    )
    .addColumn("track_id", "integer", (col) =>
      col.notNull().references("tracks.id").onDelete("cascade")
    )
    .addColumn("featured", "integer", (col) =>
      col
        .notNull()
        .defaultTo(0)
        .check(sql`featured IN (0, 1)`)
    )
    .$call(position)
    .modifyEnd(sql`strict`)
    .execute()
  await index(db, "artists_tracks", ["track_id", "artist_id"], { unique: true })
  await index(db, "artists_tracks", ["artist_id"])

  await db.schema
    .createTable("labels_releases")
    .addColumn("id", "integer", (col) => col.primaryKey().notNull())
    .addColumn("label_id", "integer", (col) =>
      col.notNull().references("labels.id").onDelete("restrict")
    )
    .addColumn("release_id", "integer", (col) =>
      col.notNull().references("releases.id").onDelete("cascade")
    )
    .$call(position)
    .modifyEnd(sql`strict`)
    .execute()
  await index(db, "labels_releases", ["release_id", "label_id"], {
    unique: true,
  })
  await index(db, "labels_releases", ["label_id"])

  // Credits

  await createNameList(db, "credit_roles")
  for (const owner of ["release", "track"]) {
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

  // Release formats and media

  const lists = [
    {
      list: "release_formats",
      link: "release_formats_releases",
      fk: "format_id",
    },
    { list: "release_media", link: "release_media_releases", fk: "medium_id" },
  ]
  for (const { list, link, fk } of lists) {
    await createNameList(db, list)
    await db.schema
      .createTable(link)
      .addColumn("id", "integer", (col) => col.primaryKey().notNull())
      .addColumn(fk, "integer", (col) =>
        col.notNull().references(`${list}.id`).onDelete("restrict")
      )
      .addColumn("release_id", "integer", (col) =>
        col.notNull().references("releases.id").onDelete("cascade")
      )
      .modifyEnd(sql`strict`)
      .execute()
    await index(db, link, ["release_id", fk], { unique: true })
    await index(db, link, [fk])
  }

  // File links. The linked entity's side cascades, the file's side restricts.

  const fileLinks = [
    { table: "files_releases", owner: "release_id", ownerTable: "releases" },
    { table: "artists_files", owner: "artist_id", ownerTable: "artists" },
    { table: "files_tracks", owner: "track_id", ownerTable: "tracks" },
  ]
  for (const { table, owner, ownerTable } of fileLinks) {
    await db.schema
      .createTable(table)
      .addColumn("id", "integer", (col) => col.primaryKey().notNull())
      .addColumn(owner, "integer", (col) =>
        col.notNull().references(`${ownerTable}.id`).onDelete("cascade")
      )
      .addColumn("file_id", "integer", (col) =>
        col.notNull().references("files.id").onDelete("restrict")
      )
      .addColumn("note", "text", (col) => col.check(trimmed("note")))
      .modifyEnd(sql`strict`)
      .execute()
    await index(db, table, [owner, "file_id"], { unique: true })
    await index(db, table, ["file_id"])
  }

  // Tags: a definition table and a value table per type (design §4.3)

  const tagTypes = [
    { type: "text", valueType: "text" },
    { type: "real_number", valueType: "real" },
    { type: "date", valueType: "integer" },
  ] as const
  for (const { type, valueType } of tagTypes) {
    const tags = `${type}_tags`
    const values = `release_${type}_tags`

    await createNameList(db, tags, { multiple: true })
    await db.schema
      .createTable(values)
      .addColumn("id", "integer", (col) => col.primaryKey().notNull())
      .addColumn("release_id", "integer", (col) =>
        col.notNull().references("releases.id").onDelete("cascade")
      )
      .addColumn("tag_id", "integer", (col) =>
        col.notNull().references(`${tags}.id`).onDelete("restrict")
      )
      .addColumn("value", valueType, (col) =>
        type === "text"
          ? col
              .notNull()
              .check(trimmed("value"))
              .modifyEnd(sql`COLLATE NOCASE`)
          : col.notNull()
      )
      .modifyEnd(sql`strict`)
      .execute()
    await index(db, values, ["release_id", "tag_id", "value"], { unique: true })
    await index(db, values, ["tag_id", "value"])

    // A single-value tag holds at most one value per release (design §4.4)
    await sql
      .raw(
        `CREATE TRIGGER ${values}_single_value_insert
        BEFORE INSERT ON ${values}
        WHEN (SELECT multiple FROM ${tags} WHERE id = NEW.tag_id) = 0
          AND EXISTS (SELECT 1 FROM ${values}
                      WHERE release_id = NEW.release_id AND tag_id = NEW.tag_id)
        BEGIN
          SELECT RAISE(ABORT, 'single-value tag already set on this release');
        END`
      )
      .execute(db)
    await sql
      .raw(
        `CREATE TRIGGER ${values}_single_value_update
        BEFORE UPDATE OF release_id, tag_id ON ${values}
        WHEN (SELECT multiple FROM ${tags} WHERE id = NEW.tag_id) = 0
          AND EXISTS (SELECT 1 FROM ${values}
                      WHERE release_id = NEW.release_id AND tag_id = NEW.tag_id
                        AND id <> NEW.id)
        BEGIN
          SELECT RAISE(ABORT, 'single-value tag already set on this release');
        END`
      )
      .execute(db)
    await sql
      .raw(
        `CREATE TRIGGER ${tags}_multiple_off
        BEFORE UPDATE OF multiple ON ${tags}
        WHEN OLD.multiple = 1 AND NEW.multiple = 0
          AND EXISTS (SELECT 1 FROM ${values} WHERE tag_id = NEW.id
                      GROUP BY release_id HAVING count(*) > 1)
        BEGIN
          SELECT RAISE(ABORT, 'some releases have several values for this tag');
        END`
      )
      .execute(db)
  }

  // A track's file is music (Appendix A). A file_id with no file leaves the
  // subquery NULL, so the trigger stays quiet and the foreign key reports it.

  await sql`
    CREATE TRIGGER tracks_file_is_music_insert
    BEFORE INSERT ON tracks
    WHEN (SELECT type FROM files WHERE id = NEW.file_id) <> 'music'
    BEGIN
      SELECT RAISE(ABORT, 'a track''s file must be music');
    END
  `.execute(db)
  await sql`
    CREATE TRIGGER tracks_file_is_music_update
    BEFORE UPDATE OF file_id ON tracks
    WHEN (SELECT type FROM files WHERE id = NEW.file_id) <> 'music'
    BEGIN
      SELECT RAISE(ABORT, 'a track''s file must be music');
    END
  `.execute(db)
  await sql`
    CREATE TRIGGER files_type_stays_music
    BEFORE UPDATE OF type ON files
    WHEN NEW.type <> 'music'
      AND EXISTS (SELECT 1 FROM tracks WHERE file_id = NEW.id)
    BEGIN
      SELECT RAISE(ABORT, 'a track''s file must stay music');
    END
  `.execute(db)

  // Covers and pictures are images (Appendix A)

  const pictures = [
    { table: "releases", column: "cover_picture", what: "a release cover" },
    { table: "artists", column: "picture", what: "an artist picture" },
    { table: "labels", column: "picture", what: "a label picture" },
  ]
  for (const { table, column, what } of pictures) {
    const events = { insert: "INSERT", update: `UPDATE OF ${column}` }
    for (const [suffix, event] of Object.entries(events)) {
      await sql
        .raw(
          `CREATE TRIGGER ${table}_${column}_is_image_${suffix}
          BEFORE ${event} ON ${table}
          WHEN (SELECT type FROM files WHERE id = NEW.${column}) <> 'image'
          BEGIN
            SELECT RAISE(ABORT, '${what} must be an image');
          END`
        )
        .execute(db)
    }
  }
  await sql`
    CREATE TRIGGER files_type_stays_image
    BEFORE UPDATE OF type ON files
    WHEN NEW.type <> 'image'
      AND (EXISTS (SELECT 1 FROM releases WHERE cover_picture = NEW.id)
           OR EXISTS (SELECT 1 FROM artists WHERE picture = NEW.id)
           OR EXISTS (SELECT 1 FROM labels WHERE picture = NEW.id))
    BEGIN
      SELECT RAISE(ABORT, 'a cover or picture must stay an image');
    END
  `.execute(db)

  // Starter rows. Nothing may depend on them (design §5.5).

  await sql`
    INSERT INTO credit_roles (name)
    VALUES ('composer'), ('conductor'), ('producer'), ('remixer'), ('writer')
  `.execute(db)
  await sql`
    INSERT INTO release_formats (name)
    VALUES ('Album'), ('Compilation'), ('EP'), ('Live'), ('LP'), ('Mixtape'),
           ('Single'), ('Soundtrack')
  `.execute(db)
  await sql`
    INSERT INTO release_media (name)
    VALUES ('Cassette'), ('CD'), ('Digital'), ('Vinyl')
  `.execute(db)
}

export async function down(db: Kysely<unknown>): Promise<void> {
  // Children first. Dropping a table drops its indexes and triggers too.
  const tables = [
    "release_date_tags",
    "date_tags",
    "release_real_number_tags",
    "real_number_tags",
    "release_text_tags",
    "text_tags",
    "files_tracks",
    "artists_files",
    "files_releases",
    "release_media_releases",
    "release_media",
    "release_formats_releases",
    "release_formats",
    "track_credits",
    "release_credits",
    "credit_roles",
    "labels_releases",
    "artists_tracks",
    "artists_releases",
    "tracks",
    "releases",
    "labels",
    "artists",
    "files",
  ]
  for (const table of tables) {
    await db.schema.dropTable(table).execute()
  }
}

/** Non-empty, with no leading or trailing spaces (Appendix A). NULL passes. */
function trimmed(column: string) {
  const ref = sql.ref(column)
  return sql`${ref} <> '' AND ${ref} = trim(${ref})`
}

function timestamps<T extends string, C extends string>(
  table: CreateTableBuilder<T, C>
) {
  return table
    .addColumn("inserted_at", "integer", (col) =>
      col.notNull().defaultTo(sql`(unixepoch())`)
    )
    .addColumn("updated_at", "integer", (col) =>
      col.notNull().defaultTo(sql`(unixepoch())`)
    )
}

/** Display order */
function position<T extends string, C extends string>(
  table: CreateTableBuilder<T, C>
) {
  return table.addColumn("position", "integer", (col) =>
    col.notNull().check(sql`position >= 0`)
  )
}

/** A list the user manages: names are stored as displayed, unique ignoring case. */
async function createNameList(
  db: Kysely<unknown>,
  table: string,
  { multiple = false } = {}
) {
  let builder: CreateTableBuilder<string, string> = db.schema
    .createTable(table)
    .addColumn("id", "integer", (col) => col.primaryKey().notNull())
    .addColumn("name", "text", (col) =>
      col
        .notNull()
        .check(trimmed("name"))
        .modifyEnd(sql`COLLATE NOCASE`)
    )
  if (multiple) {
    builder = builder.addColumn("multiple", "integer", (col) =>
      col
        .notNull()
        .defaultTo(0)
        .check(sql`multiple IN (0, 1)`)
    )
  }
  await builder.modifyEnd(sql`strict`).execute()
  await index(db, table, ["name"], { unique: true })
}

/** An index named `<table>_<columns>_index`, as Ecto names them. */
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

/** An index on `column COLLATE NOCASE`, for searching and sorting by name. */
async function nocaseIndex(db: Kysely<unknown>, table: string, column: string) {
  await db.schema
    .createIndex(`${table}_${column}_index`)
    .on(table)
    .expression(sql`${sql.ref(column)} COLLATE NOCASE`)
    .execute()
}
