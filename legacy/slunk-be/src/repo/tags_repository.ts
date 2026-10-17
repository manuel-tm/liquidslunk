import { type Expression, type Kysely, type SqlBool, sql } from "kysely"
import {
  type Tag,
  type TagFilter,
  type TagOwner,
  type TagsRepository,
  TAG_TYPES,
  type TagType,
  type TagValueRow,
} from "../slunk/tagging/repository.ts"
import type { DB } from "./db_types.ts"
import { chunks, contains, groupBy } from "./helpers.ts"

/** Each type's definition table, and its value table per owner (design §4.3). */
export const TAG_TABLES = {
  text: {
    tags: "text_tags",
    release: "release_text_tags",
    track: "track_text_tags",
  },
  number: {
    tags: "real_number_tags",
    release: "release_real_number_tags",
    track: "track_real_number_tags",
  },
  date: {
    tags: "date_tags",
    release: "release_date_tags",
    track: "track_date_tags",
  },
} as const

/** A value table, as SQL. */
const valueTable = (owner: TagOwner, type: TagType) =>
  sql.table(TAG_TABLES[type][owner])

/** A value table's owner column: `release_id` or `track_id`. */
const ownerColumn = (owner: TagOwner) => sql.ref(`${owner}_id`)

// The order types are listed in when names tie
const RANK = { text: 0, number: 1, date: 2 }

type TagRow = { id: number; name: string; multiple: number }

/**
 * The three types' tables have the same columns, so one repository serves
 * them. The casts name one table for Kysely's types; all three work the same.
 */
export function createTagsRepository(db: Kysely<DB>): TagsRepository {
  const tags = (type: TagType) => TAG_TABLES[type].tags as "text_tags"

  /** Every type's tags as one table, with a `type` column. */
  const allTags = (types: readonly TagType[]) =>
    sql.join(
      types.map(
        (type) => sql`
          select ${type} as type, ${RANK[type]} as rank, id, name, multiple
          from ${sql.table(TAG_TABLES[type].tags)}
        `
      ),
      sql` union all `
    )

  return {
    async list({ offset, limit, sort, descending, search, type }) {
      const from = sql`(${allTags(type ? [type] : TAG_TYPES)})`
      const where = search ? sql`where ${contains("name", search)}` : sql``
      const { rows: counted } = await sql<{ total: number }>`
        select count(*) as total from ${from} ${where}
      `.execute(db)
      const direction = descending ? sql`desc` : sql`asc`
      const { rows } = await sql<TagRow & { type: TagType }>`
        select type, id, name, multiple from ${from} ${where}
        order by ${sort === "name" ? sql`name collate nocase ${direction},` : sql``}
          id ${direction}, rank
        limit ${limit} offset ${offset}
      `.execute(db)
      return {
        items: rows.map((row) => toTag(row.type, row)),
        total: counted[0]?.total ?? 0,
      }
    },

    async get(type, id) {
      const row = await db
        .selectFrom(tags(type))
        .select(["id", "name", "multiple"])
        .where("id", "=", id)
        .executeTakeFirst()
      return row ? toTag(type, row) : null
    },

    async getMany(type, ids) {
      const found: Tag[] = []
      for (const chunk of chunks(ids)) {
        const rows = await db
          .selectFrom(tags(type))
          .select(["id", "name", "multiple"])
          .where("id", "in", chunk)
          .execute()
        found.push(...rows.map((row) => toTag(type, row)))
      }
      return found
    },

    async findByName(name) {
      const { rows } = await sql<TagRow & { type: TagType }>`
        select type, id, name, multiple from (${allTags(TAG_TYPES)})
        where name = ${name} collate nocase
        order by rank
      `.execute(db)
      return rows.map((row) => toTag(row.type, row))
    },

    async insert(type, { name, multiple }) {
      const row = await db
        .insertInto(tags(type))
        .values({ name, multiple: multiple ? 1 : 0 })
        .returning(["id", "name", "multiple"])
        .executeTakeFirstOrThrow()
      return toTag(type, row)
    },

    async update(type, id, { name, multiple }) {
      const row = await db
        .updateTable(tags(type))
        .set({ name, multiple: multiple ? 1 : 0 })
        .where("id", "=", id)
        .returning(["id", "name", "multiple"])
        .executeTakeFirst()
      return row ? toTag(type, row) : null
    },

    async delete(type, id) {
      await db.deleteFrom(tags(type)).where("id", "=", id).execute()
    },

    async usage(type, id) {
      const count = async (owner: TagOwner) => {
        const { rows } = await sql<{ count: number }>`
          select count(distinct ${ownerColumn(owner)}) as count
          from ${valueTable(owner, type)} where tag_id = ${id}
        `.execute(db)
        return rows[0]?.count ?? 0
      }
      return { releases: await count("release"), tracks: await count("track") }
    },

    async values(id, { offset, limit, search }) {
      const where = search ? sql`and ${contains("value", search)}` : sql``
      // Values compare ignoring case, so "Rock" and "rock" count as one
      const both = sql`(
        select value, 1 as release, 0 as track from release_text_tags
        where tag_id = ${id} ${where}
        union all
        select value, 0, 1 from track_text_tags
        where tag_id = ${id} ${where}
      )`
      const { rows: counted } = await sql<{ total: number }>`
        select count(distinct value collate nocase) as total from ${both}
      `.execute(db)
      const { rows } = await sql<{
        value: string
        releases: number
        tracks: number
      }>`
        select value, sum(release) as releases, sum(track) as tracks
        from ${both}
        group by value collate nocase
        order by count(*) desc, value collate nocase
        limit ${limit} offset ${offset}
      `.execute(db)
      return { items: rows, total: counted[0]?.total ?? 0 }
    },

    async multiValueOwners(owner, type, id, { offset, limit }) {
      const several = sql`(
        select ${ownerColumn(owner)} as id from ${valueTable(owner, type)}
        where tag_id = ${id}
        group by ${ownerColumn(owner)}
        having count(*) > 1
      )`
      const { rows: counted } = await sql<{ total: number }>`
        select count(*) as total from ${several}
      `.execute(db)
      const { rows } = await sql<{ id: number }>`
        select id from ${several} order by id limit ${limit} offset ${offset}
      `.execute(db)
      return {
        items: rows.map((row) => row.id),
        total: counted[0]?.total ?? 0,
      }
    },

    async valuesOf(owner, ids) {
      const rows: (TagValueRow & { ownerId: number })[] = []
      for (const chunk of chunks(ids)) {
        const { rows: found } = await sql<TagValueRow & { ownerId: number }>`
          select type, ownerId, tagId, value from (${sql.join(
            TAG_TYPES.map(
              (type) => sql`
                select ${type} as type, ${RANK[type]} as rank,
                  v.${ownerColumn(owner)} as ownerId, v.tag_id as tagId,
                  v.value as value, t.name as name, v.id as valueId
                from ${valueTable(owner, type)} v
                join ${sql.table(TAG_TABLES[type].tags)} t on t.id = v.tag_id
                where v.${ownerColumn(owner)} in (${sql.join(chunk)})
              `
            ),
            sql` union all `
          )})
          order by ownerId, name collate nocase, rank, tagId, valueId
        `.execute(db)
        rows.push(...found)
      }
      return groupBy(
        rows,
        (row) => row.ownerId,
        ({ type, tagId, value }) => ({ type, tagId, value })
      )
    },

    async setValues(owner, id, list) {
      for (const type of TAG_TYPES) {
        await sql`
          delete from ${valueTable(owner, type)}
          where ${ownerColumn(owner)} = ${id}
        `.execute(db)
      }
      // Rows are inserted in order, so ids keep the order they were given in
      for (const { type, tagId, value } of list) {
        await sql`
          insert into ${valueTable(owner, type)}
            (${ownerColumn(owner)}, tag_id, value)
          values (${id}, ${tagId}, ${value})
        `.execute(db)
      }
    },

    async addValue(owner, ids, { type, tagId, value }, replace) {
      let changed = 0
      for (const chunk of chunks(ids)) {
        if (replace) {
          // Same value, ignoring case for text, stays as it is
          await sql`
            delete from ${valueTable(owner, type)}
            where ${ownerColumn(owner)} in (${sql.join(chunk)})
              and tag_id = ${tagId} and value <> ${value}
          `.execute(db)
        }
        // Owners that have the value already are skipped. (INSERT OR IGNORE
        // wouldn't do: the single-value trigger aborts before the index.)
        const result = await sql`
          insert into ${valueTable(owner, type)}
            (${ownerColumn(owner)}, tag_id, value)
          select o.id, ${tagId}, ${value} from ${sql.table(`${owner}s`)} o
          where o.id in (${sql.join(chunk)})
            and not exists (
              select 1 from ${valueTable(owner, type)} v
              where v.${ownerColumn(owner)} = o.id
                and v.tag_id = ${tagId} and v.value = ${value}
            )
          order by o.id
        `.execute(db)
        changed += Number(result.numAffectedRows ?? 0)
      }
      return changed
    },

    async removeValues(owner, ids, { type, tagId, value }) {
      let changed = 0
      const matches = (chunk: number[]) => sql`
        ${ownerColumn(owner)} in (${sql.join(chunk)}) and tag_id = ${tagId}
        ${value === undefined ? sql`` : sql`and value = ${value}`}
      `
      for (const chunk of chunks(ids)) {
        const { rows } = await sql<{ count: number }>`
          select count(distinct ${ownerColumn(owner)}) as count
          from ${valueTable(owner, type)} where ${matches(chunk)}
        `.execute(db)
        changed += rows[0]?.count ?? 0
        await sql`
          delete from ${valueTable(owner, type)} where ${matches(chunk)}
        `.execute(db)
      }
      return changed
    },
  }
}

/** A list filter on a tag's values, for releases or tracks (design §7.6). */
export function tagMatches(owner: TagOwner, filter: TagFilter) {
  const conditions: Expression<unknown>[] = [sql`tag_id = ${filter.tagId}`]
  if (filter.equals !== undefined) {
    conditions.push(sql`value = ${filter.equals}`)
  }
  if (filter.contains !== undefined) {
    conditions.push(contains("value", filter.contains))
  }
  if (filter.min !== undefined) conditions.push(sql`value >= ${filter.min}`)
  if (filter.max !== undefined) conditions.push(sql`value <= ${filter.max}`)
  // Uses the (tag_id, value) index
  return sql<SqlBool>`${sql.ref(`${owner}s.id`)} in (
    select ${ownerColumn(owner)} from ${valueTable(owner, filter.type)}
    where ${sql.join(conditions, sql` and `)}
  )`
}

function toTag(type: TagType, row: TagRow): Tag {
  return {
    type,
    id: row.id,
    name: row.name,
    multiple: row.multiple === 1,
  }
}
