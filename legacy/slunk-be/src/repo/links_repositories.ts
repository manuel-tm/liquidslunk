import { type Kysely, sql } from "kysely"
import type {
  CreditsRepository,
  StatsRepository,
} from "../slunk/catalog/repository.ts"
import type { FileLinksRepository } from "../slunk/media/repository.ts"
import type { DB } from "./db_types.ts"
import { chunks, contains, groupBy } from "./helpers.ts"

// Each owner's credit lines, and the artists on each line
const CREDITS = {
  release: {
    lines: "release_credits",
    artists: "artists_release_credits",
    column: "release_id",
  },
  track: {
    lines: "track_credits",
    artists: "artists_track_credits",
    column: "track_id",
  },
} as const

export function createCreditsRepository(db: Kysely<DB>): CreditsRepository {
  return {
    async of(owner, ids) {
      const { lines, artists, column } = CREDITS[owner]
      const rows: {
        ownerId: number
        lineId: number
        role: string
        artistId: number
        creditedNameId: number | null
      }[] = []
      for (const chunk of chunks(ids)) {
        const { rows: found } = await sql<(typeof rows)[number]>`
          select l.${sql.ref(column)} as ownerId, l.id as lineId, l.role,
            a.artist_id as artistId, a.credited_name_id as creditedNameId
          from ${sql.table(lines)} l
          join ${sql.table(artists)} a on a.credit_id = l.id
          where l.${sql.ref(column)} in (${sql.join(chunk)})
          order by l.${sql.ref(column)}, l.position, a.position
        `.execute(db)
        rows.push(...found)
      }
      return new Map(
        [
          ...groupBy(
            rows,
            (row) => row.ownerId,
            (row) => row
          ),
        ].map(([ownerId, ownerRows]) => [
          ownerId,
          [
            ...groupBy(
              ownerRows,
              (row) => row.lineId,
              (row) => row
            ),
          ].map(([, line]) => ({
            role: line[0].role,
            artists: line.map(({ artistId, creditedNameId }) => ({
              artistId,
              creditedNameId,
            })),
          })),
        ])
      )
    },

    async set(owner, id, credits) {
      const { lines, artists, column } = CREDITS[owner]
      // Each line's artists go with it (CASCADE)
      await sql`
        delete from ${sql.table(lines)} where ${sql.ref(column)} = ${id}
      `.execute(db)
      for (const [position, { role, artists: links }] of credits.entries()) {
        const { rows } = await sql<{ id: number }>`
          insert into ${sql.table(lines)} (${sql.ref(column)}, role, position)
          values (${id}, ${role}, ${position})
          returning id
        `.execute(db)
        // Numbered from 0: the line's key needs an artist at position 0
        for (const [place, { artistId, creditedNameId }] of links.entries()) {
          await sql`
            insert into ${sql.table(artists)}
              (credit_id, artist_id, credited_name_id, position)
            values (${rows[0].id}, ${artistId}, ${creditedNameId}, ${place})
          `.execute(db)
        }
      }
    },

    async roles({ search, limit }) {
      const where = search ? sql`where ${contains("role", search)}` : sql``
      // `role` ignores case, so grouping by its exact text keeps each
      // spelling apart: "Mixed By" and "Mixed by" are two suggestions
      const { rows } = await sql<{ role: string }>`
        select role from (
          select role from release_credits ${where}
          union all
          select role from track_credits ${where}
        )
        group by role collate binary
        order by count(*) desc, role collate nocase, role collate binary
        limit ${limit}
      `.execute(db)
      return rows.map((row) => row.role)
    },
  }
}

const FILE_LINKS = {
  release: { table: "files_releases", column: "release_id" },
  artist: { table: "artists_files", column: "artist_id" },
  track: { table: "files_tracks", column: "track_id" },
} as const

export function createFileLinksRepository(db: Kysely<DB>): FileLinksRepository {
  return {
    async of(owner, ids) {
      const { table, column } = FILE_LINKS[owner]
      const rows: { ownerId: number; fileId: number; note: string | null }[] =
        []
      for (const chunk of chunks(ids)) {
        const { rows: found } = await sql<(typeof rows)[number]>`
          select ${sql.ref(column)} as ownerId, file_id as fileId, note
          from ${sql.table(table)}
          where ${sql.ref(column)} in (${sql.join(chunk)})
          order by ${sql.ref(column)}, id
        `.execute(db)
        rows.push(...found)
      }
      return groupBy(
        rows,
        (row) => row.ownerId,
        ({ fileId, note }) => ({ fileId, note })
      )
    },

    async set(owner, id, links) {
      const { table, column } = FILE_LINKS[owner]
      await sql`
        delete from ${sql.table(table)} where ${sql.ref(column)} = ${id}
      `.execute(db)
      for (const { fileId, note } of links) {
        await sql`
          insert into ${sql.table(table)} (${sql.ref(column)}, file_id, note)
          values (${id}, ${fileId}, ${note})
        `.execute(db)
      }
    },
  }
}

export function createStatsRepository(db: Kysely<DB>): StatsRepository {
  return {
    async totals() {
      const { rows } = await sql<{
        releases: number
        artists: number
        tracks: number
        labels: number
        durationMs: number
      }>`
        select
          (select count(*) from releases) as releases,
          (select count(*) from artists) as artists,
          (select count(*) from tracks) as tracks,
          (select count(*) from labels) as labels,
          (select coalesce(sum(duration_ms), 0) from tracks) as durationMs
      `.execute(db)
      return rows[0]
    },

    async languages() {
      const { rows } = await sql<{ language: string }>`
        select language from (
          select name_language as language from artists
          union all select name_language from labels
          union all select language from artist_alternate_names
          union all select language from label_alternate_names
          union all select title_language from releases
          union all select title_language from tracks
          union all select language from release_disc_titles
          union all select language from release_alternate_titles
          union all select language from track_alternate_titles
          union all select language from release_disc_alternate_titles
        )
        where language is not null
        group by language
        order by count(*) desc, language
      `.execute(db)
      return rows.map((row) => row.language)
    },
  }
}
