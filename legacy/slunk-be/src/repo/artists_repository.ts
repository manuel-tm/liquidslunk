import { type Kysely, sql } from "kysely"
import type {
  ArtistsRepository,
  LabelsRepository,
} from "../slunk/catalog/repository.ts"
import type { DB } from "./db_types.ts"
import { chunks } from "./helpers.ts"
import { createNamedRepository, toAlternateName } from "./named_repository.ts"

/** Every table that links an artist to a release, a track or a credit line. */
const ARTIST_LINKS = [
  "artists_releases",
  "artists_tracks",
  "artists_release_credits",
  "artists_track_credits",
] as const

export function createArtistsRepository(db: Kysely<DB>): ArtistsRepository {
  return {
    ...createNamedRepository(db, "artists"),

    async alternateNamesByIds(ids) {
      const found = []
      for (const chunk of chunks(ids)) {
        const rows = await db
          .selectFrom("artist_alternate_names")
          .selectAll()
          .where("id", "in", chunk)
          .execute()
        found.push(...rows.map(toAlternateName))
      }
      return found
    },

    async addAlternateName(artistId, values) {
      const row = await db
        .insertInto("artist_alternate_names")
        .values({
          artist_id: artistId,
          name: values.name,
          language: values.language,
          real_name: values.realName ? 1 : 0,
          note: values.note,
          position: sql<number>`(
            select coalesce(max(position) + 1, 0) from artist_alternate_names
            where artist_id = ${artistId}
          )`,
        })
        .returningAll()
        .executeTakeFirstOrThrow()
      return toAlternateName(row)
    },

    async nameUses(nameIds) {
      const uses = new Map<number, number>()
      for (const chunk of chunks(nameIds)) {
        const { rows } = await sql<{ nameId: number; uses: number }>`
          select credited_name_id as nameId, count(*) as uses from (
            ${sql.join(
              ARTIST_LINKS.map(
                (table) => sql`
                  select credited_name_id from ${sql.table(table)}
                  where credited_name_id in (${sql.join(chunk)})`
              ),
              sql` union all `
            )}
          )
          group by credited_name_id
        `.execute(db)
        for (const row of rows) uses.set(row.nameId, row.uses)
      }
      return uses
    },

    async mainNameUses(artistId) {
      const { rows } = await sql<{ uses: number }>`
        select ${sql.join(
          ARTIST_LINKS.map(
            (table) => sql`(
              select count(*) from ${sql.table(table)}
              where artist_id = ${artistId} and credited_name_id is null
            )`
          ),
          sql` + `
        )} as uses
      `.execute(db)
      return rows[0].uses
    },

    async creditMainNameLinks(artistId, nameId) {
      for (const table of ARTIST_LINKS) {
        await sql`
          update ${sql.table(table)} set credited_name_id = ${nameId}
          where artist_id = ${artistId} and credited_name_id is null
        `.execute(db)
      }
    },

    async uncredit(artistId, nameIds) {
      for (const chunk of chunks(nameIds)) {
        for (const table of ARTIST_LINKS) {
          await sql`
            update ${sql.table(table)} set credited_name_id = null
            where artist_id = ${artistId}
              and credited_name_id in (${sql.join(chunk)})
          `.execute(db)
        }
      }
    },

    async usage(id) {
      const { rows } = await sql<{
        releases: number
        tracks: number
        credits: number
      }>`
        select
          (select count(*) from artists_releases where artist_id = ${id}) as releases,
          (select count(*) from artists_tracks where artist_id = ${id}) as tracks,
          (select count(*) from artists_release_credits where artist_id = ${id})
            + (select count(*) from artists_track_credits where artist_id = ${id})
            as credits
      `.execute(db)
      return rows[0]
    },

    async appearances(id) {
      const { rows } = await sql<{ releaseId: number; trackId: number }>`
        select t.release_id as releaseId, t.id as trackId
        from artists_tracks a join tracks t on t.id = a.track_id
        where a.artist_id = ${id}
          and not exists (
            select 1 from artists_releases r
            where r.release_id = t.release_id and r.artist_id = ${id}
          )
        order by t.release_id, t.cd_number, t.track_number
      `.execute(db)
      return rows
    },

    // A row per line the artist is on
    async credits(id) {
      const { rows } = await sql<{
        role: string
        releaseId: number
        trackId: number | null
        creditedNameId: number | null
      }>`
        select l.role, l.release_id as releaseId, null as trackId,
          a.credited_name_id as creditedNameId
        from artists_release_credits a
        join release_credits l on l.id = a.credit_id
        where a.artist_id = ${id}
        union all
        select l.role, t.release_id, t.id, a.credited_name_id
        from artists_track_credits a
        join track_credits l on l.id = a.credit_id
        join tracks t on t.id = l.track_id
        where a.artist_id = ${id}
      `.execute(db)
      return rows
    },

    async formatCounts(id) {
      const { rows } = await sql<{ formatId: number; count: number }>`
        select f.format_id as formatId, count(*) as count
        from release_formats_releases f
        join artists_releases a on a.release_id = f.release_id
        where a.artist_id = ${id}
        group by f.format_id
      `.execute(db)
      return rows
    },
  }
}

export function createLabelsRepository(db: Kysely<DB>): LabelsRepository {
  return {
    ...createNamedRepository(db, "labels"),

    async releaseCount(id) {
      const row = await db
        .selectFrom("labels_releases")
        .select((eb) => eb.fn.countAll<number>().as("count"))
        .where("label_id", "=", id)
        .executeTakeFirstOrThrow()
      return row.count
    },
  }
}
