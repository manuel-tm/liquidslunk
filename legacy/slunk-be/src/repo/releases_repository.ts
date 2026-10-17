import {
  type Insertable,
  type Kysely,
  type Selectable,
  type Updateable,
  type SqlBool,
  sql,
} from "kysely"
import type {
  Release,
  ReleaseFilters,
  ReleaseLink,
  ReleasesRepository,
  ReleaseValues,
} from "../slunk/catalog/repository.ts"
import {
  alternateTitlesOf,
  setAlternateTitles,
  titleMatches,
} from "./alternate_titles.ts"
import type { DB, Releases } from "./db_types.ts"
import { chunks, groupBy } from "./helpers.ts"
import { tagMatches } from "./tags_repository.ts"

/** Each link's table, its column, and how its rows are ordered. */
const LINKS = {
  artists: { table: "artists_releases", column: "artist_id", positioned: true },
  labels: { table: "labels_releases", column: "label_id", positioned: true },
  formats: {
    table: "release_formats_releases",
    column: "format_id",
    positioned: false,
  },
  media: {
    table: "release_media_releases",
    column: "medium_id",
    positioned: false,
  },
} as const

export function createReleasesRepository(db: Kysely<DB>): ReleasesRepository {
  /** The releases that match a list's search and filters. */
  function filtered(query: ReleaseFilters) {
    let select = db.selectFrom("releases")
    if (query.search) {
      select = select.where(titleMatches("release", query.search))
    }
    for (const [link, id] of [
      ["formats", query.format],
      ["media", query.medium],
      ["labels", query.label],
      ["artists", query.artist],
    ] as const) {
      if (id !== undefined) select = select.where(linkedTo(link, id))
    }
    for (const filter of query.tags ?? []) {
      select = select.where(tagMatches("release", filter))
    }
    return select
  }

  return {
    async list(query) {
      const { offset, limit, sort, descending } = query
      const select = filtered(query)

      const { total } = await select
        .select((eb) => eb.fn.countAll<number>().as("total"))
        .executeTakeFirstOrThrow()
      const direction = descending ? "desc" : "asc"
      const rows = await select
        .selectAll()
        // Undated releases go last either way
        .$if(sort === "released", (q) =>
          q
            .orderBy(sql`release_date is null`)
            .orderBy("release_date", direction)
        )
        .$if(sort === "title", (q) =>
          q.orderBy(sql`title collate nocase`, direction)
        )
        .orderBy("id", direction)
        .limit(limit)
        .offset(offset)
        .execute()
      return { items: rows.map(toRelease), total }
    },

    async ids(query) {
      const rows = await filtered(query)
        .select("id")
        .orderBy("id", "desc")
        .execute()
      return rows.map((row) => row.id)
    },

    async get(id) {
      const row = await db
        .selectFrom("releases")
        .selectAll()
        .where("id", "=", id)
        .executeTakeFirst()
      return row ? toRelease(row) : null
    },

    async getMany(ids) {
      const found: Release[] = []
      for (const chunk of chunks(ids)) {
        const rows = await db
          .selectFrom("releases")
          .selectAll()
          .where("id", "in", chunk)
          .execute()
        found.push(...rows.map(toRelease))
      }
      return found
    },

    async findByTitle(title) {
      const rows = await db
        .selectFrom("releases")
        .selectAll()
        .where(
          sql<SqlBool>`title = ${title.trim()} collate nocase or id in (
            select release_id from release_alternate_titles
            where title = ${title.trim()} collate nocase
          )`
        )
        .execute()
      return rows.map(toRelease)
    },

    alternateTitlesOf: (releaseIds) =>
      alternateTitlesOf(db, "release", releaseIds),
    setAlternateTitles: (releaseId, titles) =>
      setAlternateTitles(db, "release", releaseId, titles),

    async insert(values) {
      const row = await db
        .insertInto("releases")
        .values(toRow(values))
        .returningAll()
        .executeTakeFirstOrThrow()
      return toRelease(row)
    },

    async update(id, values) {
      const row = await db
        .updateTable("releases")
        .set({ ...toRow(values), updated_at: sql`unixepoch()` })
        .where("id", "=", id)
        .returningAll()
        .executeTakeFirst()
      return row ? toRelease(row) : null
    },

    async delete(id) {
      await db.deleteFrom("releases").where("id", "=", id).execute()
    },

    async linksOf(link, releaseIds) {
      const { table, column, positioned } = LINKS[link]
      const rows: { releaseId: number; id: number }[] = []
      for (const chunk of chunks(releaseIds)) {
        const { rows: found } = await sql<{ releaseId: number; id: number }>`
          select release_id as releaseId, ${sql.ref(column)} as id
          from ${sql.table(table)}
          where release_id in (${sql.join(chunk)})
          order by release_id, ${positioned ? sql`position` : sql`id`}
        `.execute(db)
        rows.push(...found)
      }
      return groupBy(
        rows,
        (row) => row.releaseId,
        (row) => row.id
      )
    },

    async artistsOf(releaseIds) {
      const rows: {
        releaseId: number
        artistId: number
        creditedNameId: number | null
      }[] = []
      for (const chunk of chunks(releaseIds)) {
        const found = await db
          .selectFrom("artists_releases")
          .select([
            "release_id as releaseId",
            "artist_id as artistId",
            "credited_name_id as creditedNameId",
          ])
          .where("release_id", "in", chunk)
          .orderBy("release_id")
          .orderBy("position")
          .execute()
        rows.push(...found)
      }
      return groupBy(
        rows,
        (row) => row.releaseId,
        ({ artistId, creditedNameId }) => ({ artistId, creditedNameId })
      )
    },

    async setArtists(releaseId, artists) {
      await db
        .deleteFrom("artists_releases")
        .where("release_id", "=", releaseId)
        .execute()
      if (artists.length === 0) return
      await db
        .insertInto("artists_releases")
        .values(
          artists.map(({ artistId, creditedNameId }, position) => ({
            release_id: releaseId,
            artist_id: artistId,
            credited_name_id: creditedNameId,
            position,
          }))
        )
        .execute()
    },

    async setLinks(link, releaseId, ids) {
      const { table, column, positioned } = LINKS[link]
      await sql`
        delete from ${sql.table(table)} where release_id = ${releaseId}
      `.execute(db)
      for (const [position, id] of ids.entries()) {
        await sql`
          insert into ${sql.table(table)}
            (release_id, ${sql.ref(column)}${positioned ? sql`, position` : sql``})
          values (${releaseId}, ${id}${positioned ? sql`, ${position}` : sql``})
        `.execute(db)
      }
    },

    async discTitlesOf(releaseIds) {
      const rows: {
        id: number
        releaseId: number
        cdNumber: number
        title: string
        language: string | null
      }[] = []
      for (const chunk of chunks(releaseIds)) {
        const found = await db
          .selectFrom("release_disc_titles")
          .select([
            "id",
            "release_id as releaseId",
            "cd_number as cdNumber",
            "title",
            "language",
          ])
          .where("release_id", "in", chunk)
          .orderBy("release_id")
          .orderBy("cd_number")
          .execute()
        rows.push(...found)
      }
      const alternates = await alternateTitlesOf(
        db,
        "disc",
        rows.map((row) => row.id)
      )
      return groupBy(
        rows,
        (row) => row.releaseId,
        ({ id, cdNumber, title, language }) => ({
          cdNumber,
          title,
          language,
          alternateTitles: alternates.get(id) ?? [],
        })
      )
    },

    async setDiscTitles(releaseId, titles) {
      await db
        .deleteFrom("release_disc_titles")
        .where("release_id", "=", releaseId)
        .execute()
      // A disc's alternate titles went with its title (CASCADE)
      for (const { cdNumber, title, language, alternateTitles } of titles) {
        const { id } = await db
          .insertInto("release_disc_titles")
          .values({
            release_id: releaseId,
            cd_number: cdNumber,
            title,
            language,
          })
          .returning("id")
          .executeTakeFirstOrThrow()
        await setAlternateTitles(db, "disc", id, alternateTitles)
      }
    },

    async withoutArtists() {
      const rows = await db
        .selectFrom("releases")
        .selectAll()
        .where(
          sql<SqlBool>`not exists (
            select 1 from artists_releases where release_id = releases.id
          )`
        )
        .orderBy(sql`title collate nocase`)
        .execute()
      return rows.map(toRelease)
    },

    async trackCount(id) {
      const row = await db
        .selectFrom("tracks")
        .select((eb) => eb.fn.countAll<number>().as("count"))
        .where("release_id", "=", id)
        .executeTakeFirstOrThrow()
      return row.count
    },
  }
}

function linkedTo(link: ReleaseLink, id: number) {
  const { table, column } = LINKS[link]
  return sql<SqlBool>`exists (
    select 1 from ${sql.table(table)}
    where release_id = releases.id and ${sql.ref(column)} = ${id}
  )`
}

function toRelease(row: Selectable<Releases>): Release {
  return {
    id: row.id,
    title: row.title,
    titleLanguage: row.title_language,
    releaseDate: row.release_date,
    releaseDatePrecision: row.release_date_precision,
    catalogNumber: row.catalog_number,
    coverPicture: row.cover_picture,
    variousArtists: row.various_artists === 1,
    insertedAt: row.inserted_at,
    updatedAt: row.updated_at,
  }
}

function toRow(values: ReleaseValues): Insertable<Releases>
function toRow(values: Partial<ReleaseValues>): Updateable<Releases>
function toRow(values: Partial<ReleaseValues>): Updateable<Releases> {
  return {
    title: values.title,
    title_language: values.titleLanguage,
    release_date: values.releaseDate,
    release_date_precision: values.releaseDatePrecision,
    catalog_number: values.catalogNumber,
    cover_picture: values.coverPicture,
    various_artists:
      values.variousArtists === undefined
        ? undefined
        : values.variousArtists
          ? 1
          : 0,
  }
}
