import { type Kysely, sql } from "kysely"
import type {
  ListItem,
  ListKind,
  ListsRepository,
} from "../slunk/catalog/repository.ts"
import type { DB } from "./db_types.ts"
import { chunks, contains } from "./helpers.ts"

/** What uses each list's rows. */
const USES = {
  release_formats: sql`(select count(*) from release_formats_releases where format_id = l.id)`,
  release_media: sql`(select count(*) from release_media_releases where medium_id = l.id)`,
}

/**
 * Release formats and release media have the same columns, so they share
 * this repository. The casts name one table for Kysely's types.
 */
export function createListsRepository(db: Kysely<DB>): ListsRepository {
  const table = (kind: ListKind) => kind as "release_formats"
  const item = (kind: ListKind) => (row: { id: number; name: string }) => ({
    kind,
    id: row.id,
    name: row.name,
  })

  return {
    async list(kind, { offset, limit, sort, descending, search }) {
      let query = db.selectFrom(table(kind))
      if (search) query = query.where(contains("name", search))
      const { total } = await query
        .select((eb) => eb.fn.countAll<number>().as("total"))
        .executeTakeFirstOrThrow()
      const rows = await query
        .select(["id", "name"])
        .orderBy(
          sort === "name" ? sql`name collate nocase` : sql`id`,
          descending ? "desc" : "asc"
        )
        .limit(limit)
        .offset(offset)
        .execute()
      return { items: rows.map(item(kind)), total }
    },

    async get(kind, id) {
      const row = await db
        .selectFrom(table(kind))
        .select(["id", "name"])
        .where("id", "=", id)
        .executeTakeFirst()
      return row ? item(kind)(row) : null
    },

    async getMany(kind, ids) {
      const found: ListItem[] = []
      for (const chunk of chunks(ids)) {
        const rows = await db
          .selectFrom(table(kind))
          .select(["id", "name"])
          .where("id", "in", chunk)
          .execute()
        found.push(...rows.map(item(kind)))
      }
      return found
    },

    async insert(kind, name) {
      const row = await db
        .insertInto(table(kind))
        .values({ name })
        .returning(["id", "name"])
        .executeTakeFirstOrThrow()
      return item(kind)(row)
    },

    async update(kind, id, name) {
      const row = await db
        .updateTable(table(kind))
        .set({ name })
        .where("id", "=", id)
        .returning(["id", "name"])
        .executeTakeFirst()
      return row ? item(kind)(row) : null
    },

    async delete(kind, id) {
      await db.deleteFrom(table(kind)).where("id", "=", id).execute()
    },

    async uses(kind, id) {
      const { rows } = await sql<{ uses: number }>`
        select ${USES[kind]} as uses from ${sql.table(kind)} l where l.id = ${id}
      `.execute(db)
      return rows[0]?.uses ?? 0
    },
  }
}
