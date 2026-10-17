import {
  type Insertable,
  type Kysely,
  type Selectable,
  type SqlBool,
  type Updateable,
  sql,
} from "kysely"
import type {
  Track,
  TracksRepository,
  TrackValues,
} from "../slunk/catalog/repository.ts"
import {
  alternateTitlesOf,
  setAlternateTitles,
  titleMatches,
} from "./alternate_titles.ts"
import type { DB, Tracks } from "./db_types.ts"
import { chunks, groupBy } from "./helpers.ts"
import { tagMatches } from "./tags_repository.ts"

export function createTracksRepository(db: Kysely<DB>): TracksRepository {
  return {
    async list({ offset, limit, sort, descending, search, release, tags }) {
      let query = db.selectFrom("tracks")
      if (search) query = query.where(titleMatches("track", search))
      if (release !== undefined) query = query.where("release_id", "=", release)
      for (const filter of tags ?? []) {
        query = query.where(tagMatches("track", filter))
      }
      const { total } = await query
        .select((eb) => eb.fn.countAll<number>().as("total"))
        .executeTakeFirstOrThrow()
      const direction = descending ? "desc" : "asc"
      const rows = await query
        .selectAll()
        .$if(sort === "title", (q) =>
          q.orderBy(sql`title collate nocase`, direction)
        )
        .orderBy("id", direction)
        .limit(limit)
        .offset(offset)
        .execute()
      return { items: rows.map(toTrack), total }
    },

    async get(id) {
      const row = await db
        .selectFrom("tracks")
        .selectAll()
        .where("id", "=", id)
        .executeTakeFirst()
      return row ? toTrack(row) : null
    },

    async getMany(ids) {
      const found: Track[] = []
      for (const chunk of chunks(ids)) {
        const rows = await db
          .selectFrom("tracks")
          .selectAll()
          .where("id", "in", chunk)
          .execute()
        found.push(...rows.map(toTrack))
      }
      return found
    },

    async ofFiles(fileIds) {
      const found = new Map<number, Track>()
      for (const chunk of chunks(fileIds)) {
        const rows = await db
          .selectFrom("tracks")
          .selectAll()
          .where("file_id", "in", chunk)
          .execute()
        for (const row of rows) found.set(row.file_id, toTrack(row))
      }
      return found
    },

    async ofReleases(releaseIds) {
      const found: Track[] = []
      for (const chunk of chunks(releaseIds)) {
        const rows = await db
          .selectFrom("tracks")
          .selectAll()
          .where("release_id", "in", chunk)
          .orderBy("release_id")
          .orderBy("cd_number")
          .orderBy("track_number")
          .execute()
        found.push(...rows.map(toTrack))
      }
      return groupBy(
        found,
        (track) => track.releaseId,
        (track) => track
      )
    },

    async insert(values) {
      const row = await db
        .insertInto("tracks")
        .values(toRow(values))
        .returningAll()
        .executeTakeFirstOrThrow()
      return toTrack(row)
    },

    async update(id, values) {
      const row = await db
        .updateTable("tracks")
        .set({ ...toRow(values), updated_at: sql`unixepoch()` })
        .where("id", "=", id)
        .returningAll()
        .executeTakeFirst()
      return row ? toTrack(row) : null
    },

    async delete(id) {
      await db.deleteFrom("tracks").where("id", "=", id).execute()
    },

    async shiftNumbers(releaseId, by) {
      await db
        .updateTable("tracks")
        .set({ track_number: sql`track_number + ${by}` })
        .where("release_id", "=", releaseId)
        .execute()
    },

    async artistsOf(trackIds) {
      const rows: {
        trackId: number
        artistId: number
        creditedNameId: number | null
        featured: number
      }[] = []
      for (const chunk of chunks(trackIds)) {
        const found = await db
          .selectFrom("artists_tracks")
          .select([
            "track_id as trackId",
            "artist_id as artistId",
            "credited_name_id as creditedNameId",
            "featured",
          ])
          .where("track_id", "in", chunk)
          .orderBy("track_id")
          .orderBy("position")
          .execute()
        rows.push(...found)
      }
      return groupBy(
        rows,
        (row) => row.trackId,
        (row) => ({
          artistId: row.artistId,
          creditedNameId: row.creditedNameId,
          featured: row.featured === 1,
        })
      )
    },

    alternateTitlesOf: (trackIds) => alternateTitlesOf(db, "track", trackIds),
    setAlternateTitles: (trackId, titles) =>
      setAlternateTitles(db, "track", trackId, titles),

    async setArtists(trackId, artists) {
      await db
        .deleteFrom("artists_tracks")
        .where("track_id", "=", trackId)
        .execute()
      if (artists.length === 0) return
      await db
        .insertInto("artists_tracks")
        .values(
          artists.map(({ artistId, creditedNameId, featured }, position) => ({
            track_id: trackId,
            artist_id: artistId,
            credited_name_id: creditedNameId,
            featured: featured ? 1 : 0,
            position,
          }))
        )
        .execute()
    },

    async withoutPerformers() {
      const rows = await db
        .selectFrom("tracks")
        .innerJoin("releases", "releases.id", "tracks.release_id")
        .selectAll("tracks")
        .where("releases.various_artists", "=", 1)
        .where(
          sql<SqlBool>`not exists (
            select 1 from artists_tracks
            where track_id = tracks.id and featured = 0
          )`
        )
        .orderBy(sql`releases.title collate nocase`)
        .orderBy("tracks.release_id")
        .orderBy("tracks.cd_number")
        .orderBy("tracks.track_number")
        .execute()
      return rows.map(toTrack)
    },
  }
}

function toTrack(row: Selectable<Tracks>): Track {
  return {
    id: row.id,
    title: row.title,
    titleLanguage: row.title_language,
    durationMs: row.duration_ms,
    trackNumber: row.track_number,
    cdNumber: row.cd_number,
    releaseId: row.release_id,
    fileId: row.file_id,
    insertedAt: row.inserted_at,
    updatedAt: row.updated_at,
  }
}

function toRow(values: TrackValues): Insertable<Tracks>
function toRow(values: Partial<TrackValues>): Updateable<Tracks>
function toRow(values: Partial<TrackValues>): Updateable<Tracks> {
  return {
    title: values.title,
    title_language: values.titleLanguage,
    duration_ms: values.durationMs,
    track_number: values.trackNumber,
    cd_number: values.cdNumber,
    release_id: values.releaseId,
    file_id: values.fileId,
  }
}
