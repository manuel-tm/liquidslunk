import { type Kysely, type Selectable, sql } from "kysely"
import type {
  FileRecord,
  FilesRepository,
  FileUse,
  NewFile,
} from "../slunk/media/repository.ts"
import { likePattern } from "../slunk/paging.ts"
import type { DB, Files } from "./db_types.ts"

/** Matches files nothing uses: the uses that block a delete (design §4.3). */
const UNUSED = sql<boolean>`
  not exists (select 1 from tracks where file_id = files.id)
  and not exists (select 1 from releases where cover_picture = files.id)
  and not exists (select 1 from artists where picture = files.id)
  and not exists (select 1 from labels where picture = files.id)
  and not exists (select 1 from files_releases where file_id = files.id)
  and not exists (select 1 from artists_files where file_id = files.id)
  and not exists (select 1 from files_tracks where file_id = files.id)
`

// Stays well under SQLite's limit on bound parameters
const CHUNK = 500

export function createFilesRepository(db: Kysely<DB>): FilesRepository {
  return {
    async list({
      offset,
      limit,
      sort,
      descending,
      search,
      type,
      unused,
      withoutTrack,
    }) {
      let query = db.selectFrom("files")
      if (search) {
        const pattern = likePattern(search)
        query = query.where(sql<boolean>`path like ${pattern} escape '\\'`)
      }
      if (type) query = query.where("type", "=", type)
      if (unused) query = query.where(UNUSED)
      if (withoutTrack) {
        query = query.where(
          sql<boolean>`not exists (select 1 from tracks where file_id = files.id)`
        )
      }

      const { total } = await query
        .select((eb) => eb.fn.countAll<number>().as("total"))
        .executeTakeFirstOrThrow()
      const rows = await query
        .selectAll()
        .orderBy(sort, descending ? "desc" : "asc")
        .limit(limit)
        .offset(offset)
        .execute()
      return { files: rows.map(toRecord), total }
    },

    async get(id) {
      const row = await db
        .selectFrom("files")
        .selectAll()
        .where("id", "=", id)
        .executeTakeFirst()
      return row ? toRecord(row) : null
    },

    async getMany(ids) {
      const found: FileRecord[] = []
      for (let i = 0; i < ids.length; i += CHUNK) {
        const rows = await db
          .selectFrom("files")
          .selectAll()
          .where("id", "in", ids.slice(i, i + CHUNK))
          .execute()
        found.push(...rows.map(toRecord))
      }
      return found
    },

    async findByPaths(paths) {
      const found: FileRecord[] = []
      for (let i = 0; i < paths.length; i += CHUNK) {
        const rows = await db
          .selectFrom("files")
          .selectAll()
          .where("path", "in", paths.slice(i, i + CHUNK))
          .execute()
        found.push(...rows.map(toRecord))
      }
      return found
    },

    async all() {
      const rows = await db
        .selectFrom("files")
        .selectAll()
        .orderBy("path")
        .execute()
      return rows.map(toRecord)
    },

    async insert(files) {
      if (files.length === 0) return []
      const rows = await db
        .insertInto("files")
        .values(files.map((file) => toRow(file)))
        .returningAll()
        .execute()
      return rows.map(toRecord)
    },

    async update(id, changes) {
      const row = await db
        .updateTable("files")
        .set({ ...toRow(changes), updated_at: sql`unixepoch()` })
        .where("id", "=", id)
        .returningAll()
        .executeTakeFirst()
      return row ? toRecord(row) : null
    },

    async delete(id) {
      await db.deleteFrom("files").where("id", "=", id).execute()
    },

    async uses(id) {
      const { rows } = await sql<FileUse>`
        select 'track_audio' as kind, id as entityId, title as name, null as note
        from tracks where file_id = ${id}
        union all
        select 'release_cover', id, title, null
        from releases where cover_picture = ${id}
        union all
        select 'artist_picture', id, name, null
        from artists where picture = ${id}
        union all
        select 'label_picture', id, name, null
        from labels where picture = ${id}
        union all
        select 'release_link', r.id, r.title, l.note
        from files_releases l join releases r on r.id = l.release_id
        where l.file_id = ${id}
        union all
        select 'artist_link', a.id, a.name, l.note
        from artists_files l join artists a on a.id = l.artist_id
        where l.file_id = ${id}
        union all
        select 'track_link', t.id, t.title, l.note
        from files_tracks l join tracks t on t.id = l.track_id
        where l.file_id = ${id}
      `.execute(db)
      return rows
    },

    async counts() {
      const { rows } = await sql<{ total: number; unused: number }>`
        select count(*) as total, count(*) filter (where ${UNUSED}) as unused
        from files
      `.execute(db)
      return rows[0]
    },

    async hasPathStartingWith(prefix, negate = false) {
      // A range on the path index: prefixes end in "/", and "0" sorts right
      // after it
      const end = `${prefix.slice(0, -1)}0`
      const inRange = sql<boolean>`path >= ${prefix} and path < ${end}`
      const row = await db
        .selectFrom("files")
        .select("id")
        .where(negate ? sql<boolean>`not (${inRange})` : inRange)
        .limit(1)
        .executeTakeFirst()
      return row !== undefined
    },
  }
}

function toRecord(row: Selectable<Files>): FileRecord {
  return {
    id: row.id,
    path: row.path,
    type: row.type,
    sizeBytes: row.size_bytes,
    mtime: row.mtime,
    sha256: row.sha256,
    insertedAt: row.inserted_at,
    updatedAt: row.updated_at,
  }
}

function toRow(
  file: NewFile
): Omit<Selectable<Files>, "id" | "inserted_at" | "updated_at">
function toRow(file: Partial<NewFile>): Partial<Selectable<Files>>
function toRow(file: Partial<NewFile>) {
  return {
    path: file.path,
    type: file.type,
    size_bytes: file.sizeBytes,
    mtime: file.mtime,
    sha256: file.sha256,
  }
}
