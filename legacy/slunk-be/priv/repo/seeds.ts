// Sample data for development: `npm run db:seed`, or `npm run db:reset` to
// start over. There are no files or tracks, which need real files on disk.
import type { Insertable, Kysely } from "kysely"
import type { DB, Releases } from "../../src/repo/db_types.ts"

export async function seed(db: Kysely<DB>): Promise<void> {
  if (await db.selectFrom("releases").select("id").executeTakeFirst()) {
    console.log("Skipped seeding: the database already has releases.")
    return
  }

  const artist = await insertNamed(db, "artists", [
    "Drake",
    "Majid Jordan",
    "The High School Musical Cast",
    "Zac Efron",
    "Vanessa Anne Hudgens",
    // With the alternate names of 3_alternate_names.md §4.1, below
    "Alika",
    "Ryo Fukui",
    "Ryo Fukui Trio",
    "Jimmy Van Heusen",
    "Ann Ronell",
    "Joseph Kosma",
    "Masataka Ito",
    "Teruo Isono",
  ])
  const label = await insertNamed(db, "labels", [
    "October's Very Own",
    "Young Money",
    "Cash Money",
    "Republic Records",
    "Walt Disney Records",
    "Nadja",
  ])
  const format = await upsertNamed(db, "release_formats", [
    "Album",
    "EP",
    "LP",
    "Soundtrack",
  ])
  const medium = await upsertNamed(db, "release_media", [
    "CD",
    "Digital",
    "Vinyl",
  ])
  const genre = await upsertTag(db, "text_tags", "Genre", true)
  const score = await upsertTag(db, "real_number_tags", "Score", false)

  const nwts = await insertRelease(db, {
    release: {
      title: "Nothing Was the Same",
      release_date: Date.UTC(2013, 8, 24) / 1000,
      release_date_precision: "day",
      catalog_number: "B0019141-02",
    },
    artists: [artist["Drake"]],
    labels: [
      label["October's Very Own"],
      label["Young Money"],
      label["Cash Money"],
      label["Republic Records"],
    ],
    formats: [format["Album"]],
    media: [medium["CD"]],
  })
  const hsm = await insertRelease(db, {
    release: {
      title: "High School Musical (Soundtrack)",
      release_date: Date.UTC(2006, 0, 10) / 1000,
      release_date_precision: "day",
      catalog_number: "365 4622",
      various_artists: 1,
    },
    artists: [artist["The High School Musical Cast"]],
    labels: [label["Walt Disney Records"]],
    formats: [format["Album"], format["Soundtrack"]],
    media: [medium["CD"]],
  })

  // Alternate names (3_alternate_names.md §4.1): "ALIKA" differs from the
  // main name only by case, and "A. Ronnell" is misprinted on "Scenery"
  await db
    .insertInto("artist_alternate_names")
    .values(
      (
        [
          ["Alika", "Alika Milova", null, 1, null],
          ["Alika", "ALIKA", null, 0, null],
          ["Ryo Fukui", "福居良", "ja", 0, null],
          ["Ryo Fukui", "R. Fukui", null, 0, null],
          ["Ryo Fukui Trio", "福居良トリオ", "ja", 0, null],
          ["Jimmy Van Heusen", "J. Van Heusen", null, 0, null],
          ["Ann Ronell", "A. Ronnell", null, 0, "misprint"],
          ["Joseph Kosma", "J. Kosma", null, 0, null],
          ["Masataka Ito", "いとう・まさたか", "ja", 0, null],
          ["Teruo Isono", "いソノてルヲ", "ja", 0, null],
        ] as const
      ).map(([owner, name, language, real_name, note], index, all) => ({
        artist_id: artist[owner],
        name,
        language,
        real_name,
        note,
        // Numbered per artist
        position: all.slice(0, index).filter(([o]) => o === owner).length,
      }))
    )
    .execute()
  await insertRelease(db, {
    release: {
      title: "Alika",
      release_date: Date.UTC(2023, 0, 1) / 1000,
      release_date_precision: "year",
    },
    artists: [artist["Alika"]],
    labels: [],
    formats: [],
    media: [],
  })
  const virmalised = await insertRelease(db, {
    release: {
      title: "virmalised",
      title_language: "et",
      release_date: Date.UTC(2024, 0, 1) / 1000,
      release_date_precision: "year",
    },
    artists: [artist["Alika"]],
    labels: [],
    formats: [format["EP"]],
    media: [medium["Digital"]],
  })
  // Credited as printed (3_alternate_names.md §4.1)
  await db
    .updateTable("artists_releases")
    .set({
      credited_name_id: db
        .selectFrom("artist_alternate_names")
        .select("id")
        .where("artist_id", "=", artist["Alika"])
        .where("name", "=", "ALIKA"),
    })
    .where("release_id", "=", virmalised)
    .execute()
  const scenery = await insertRelease(db, {
    release: {
      title: "Scenery",
      title_language: "en",
      release_date: Date.UTC(1976, 0, 1) / 1000,
      release_date_precision: "year",
      catalog_number: "PA-7148",
    },
    artists: [artist["Ryo Fukui"], artist["Ryo Fukui Trio"]],
    labels: [label["Nadja"]],
    formats: [format["LP"], format["Album"]],
    media: [medium["Vinyl"]],
  })
  // Alternate titles (3_alternate_names.md §4.1): printed on "Scenery", and a
  // translation added in Slunk for "virmalised"
  await db
    .insertInto("release_alternate_titles")
    .values([
      {
        release_id: scenery,
        title: "シーナリィ",
        language: "ja",
        printed: 1,
        position: 0,
      },
      {
        release_id: virmalised,
        title: "Northern Lights",
        language: "en",
        printed: 0,
        position: 0,
      },
    ])
    .execute()

  await db
    .insertInto("release_text_tags")
    .values([
      { release_id: nwts, tag_id: genre, value: "Hip Hop" },
      { release_id: hsm, tag_id: genre, value: "Pop" },
      { release_id: hsm, tag_id: genre, value: "Stage & Screen" },
    ])
    .execute()
  await db
    .insertInto("release_real_number_tags")
    .values([
      { release_id: nwts, tag_id: score, value: 9 },
      { release_id: hsm, tag_id: score, value: 7.5 },
    ])
    .execute()

  console.log("Seeded 5 releases, 13 artists, 6 labels and 2 tags.")
}

/** Inserts rows by name, returning their ids by name. */
async function insertNamed(
  db: Kysely<DB>,
  table: "artists" | "labels",
  names: string[]
): Promise<Record<string, number>> {
  const rows = await db
    .insertInto(table)
    .values(names.map((name) => ({ name })))
    .returning(["id", "name"])
    .execute()
  return Object.fromEntries(rows.map((row) => [row.name, row.id]))
}

/** Like insertNamed, for lists whose names are unique: keeps existing rows. */
async function upsertNamed(
  db: Kysely<DB>,
  table: "release_formats" | "release_media",
  names: string[]
): Promise<Record<string, number>> {
  await db
    .insertInto(table)
    .values(names.map((name) => ({ name })))
    .onConflict((oc) => oc.column("name").doNothing())
    .execute()
  const rows = await db
    .selectFrom(table)
    .select(["id", "name"])
    .where("name", "in", names)
    .execute()
  return Object.fromEntries(rows.map((row) => [row.name, row.id]))
}

/**
 * A tag by name, created if it's missing. Without releases, no tag has
 * values yet, so `multiple` can be set on an existing one.
 */
async function upsertTag(
  db: Kysely<DB>,
  table: "text_tags" | "real_number_tags",
  name: string,
  multiple: boolean
): Promise<number> {
  // Both tables have the same columns
  const tags = table as "text_tags"
  await db
    .insertInto(tags)
    .values({ name, multiple: multiple ? 1 : 0 })
    .onConflict((oc) =>
      oc.column("name").doUpdateSet({ multiple: multiple ? 1 : 0 })
    )
    .execute()
  const { id } = await db
    .selectFrom(tags)
    .select("id")
    .where("name", "=", name)
    .executeTakeFirstOrThrow()
  return id
}

async function insertRelease(
  db: Kysely<DB>,
  {
    release,
    artists,
    labels,
    formats,
    media,
  }: {
    release: Insertable<Releases>
    artists: number[]
    labels: number[]
    formats: number[]
    media: number[]
  }
): Promise<number> {
  const { id } = await db
    .insertInto("releases")
    .values(release)
    .returning("id")
    .executeTakeFirstOrThrow()
  await db
    .insertInto("artists_releases")
    .values(
      artists.map((artist_id, position) => ({
        artist_id,
        release_id: id,
        position,
      }))
    )
    .execute()
  // A release can have no labels, formats or media yet
  if (labels.length > 0) {
    await db
      .insertInto("labels_releases")
      .values(
        labels.map((label_id, position) => ({
          label_id,
          release_id: id,
          position,
        }))
      )
      .execute()
  }
  if (formats.length > 0) {
    await db
      .insertInto("release_formats_releases")
      .values(formats.map((format_id) => ({ format_id, release_id: id })))
      .execute()
  }
  if (media.length > 0) {
    await db
      .insertInto("release_media_releases")
      .values(media.map((medium_id) => ({ medium_id, release_id: id })))
      .execute()
  }
  return id
}
