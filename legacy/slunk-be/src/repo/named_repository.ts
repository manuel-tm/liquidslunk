import { type Kysely, type Selectable, sql } from "kysely"
import type {
  AlternateName,
  Artist,
  NamedRepository,
  NamedValues,
  NameMatch,
} from "../slunk/catalog/repository.ts"
import type { ArtistAlternateNames, Artists, DB } from "./db_types.ts"
import { chunks, contains } from "./helpers.ts"

/**
 * Artists and labels have the same columns, so they share this repository.
 * The casts name one table for Kysely's types; both work the same, except
 * that only artists' alternate names can be real names.
 */
export function createNamedRepository(
  db: Kysely<DB>,
  table: "artists" | "labels"
): NamedRepository<Artist> {
  const t = table as "artists"
  const names = (
    table === "artists" ? "artist_alternate_names" : "label_alternate_names"
  ) as "artist_alternate_names"
  const owner = (table === "artists" ? "artist_id" : "label_id") as "artist_id"
  const hasRealNames = table === "artists"

  return {
    async list({ offset, limit, sort, descending, search }) {
      let query = db.selectFrom(t)
      if (search) {
        query = query.where((eb) =>
          eb.or([
            contains("name", search),
            eb(
              "id",
              "in",
              eb.selectFrom(names).select(owner).where(contains("name", search))
            ),
          ])
        )
      }
      const { total } = await query
        .select((eb) => eb.fn.countAll<number>().as("total"))
        .executeTakeFirstOrThrow()
      const rows = await query
        .selectAll()
        .orderBy(
          sort === "name" ? sql`name collate nocase` : sql`id`,
          descending ? "desc" : "asc"
        )
        .orderBy("id", descending ? "desc" : "asc")
        .limit(limit)
        .offset(offset)
        .execute()
      return { items: rows.map(toNamed), total }
    },

    async get(id) {
      const row = await db
        .selectFrom(t)
        .selectAll()
        .where("id", "=", id)
        .executeTakeFirst()
      return row ? toNamed(row) : null
    },

    async getMany(ids) {
      const found: Artist[] = []
      for (const chunk of chunks(ids)) {
        const rows = await db
          .selectFrom(t)
          .selectAll()
          .where("id", "in", chunk)
          .execute()
        found.push(...rows.map(toNamed))
      }
      return found
    },

    // NOCASE, like the name indexes, so both queries use them
    async findByNames(wanted) {
      const matches: NameMatch<Artist>[] = []
      for (const chunk of chunks([...new Set(wanted)])) {
        const owners = await db
          .selectFrom(t)
          .selectAll()
          .where(sql`name collate nocase`, "in", chunk)
          .execute()
        for (const row of owners) {
          matches.push({ item: toNamed(row), alternate: null })
        }
        const alternates = await db
          .selectFrom(names)
          .innerJoin(t, `${t}.id`, `${names}.${owner}`)
          .selectAll(t)
          .select([
            `${names}.id as alternate_id`,
            `${names}.name as alternate_name`,
            `${names}.language as alternate_language`,
            `${names}.note as alternate_note`,
            ...(hasRealNames
              ? [`${names}.real_name as real_name` as const]
              : []),
          ])
          .where(sql`${sql.ref(`${names}.name`)} collate nocase`, "in", chunk)
          .execute()
        for (const row of alternates) {
          matches.push({
            item: toNamed(row),
            alternate: {
              id: row.alternate_id,
              name: row.alternate_name,
              language: row.alternate_language,
              realName: "real_name" in row && row.real_name === 1,
              note: row.alternate_note,
            },
          })
        }
      }
      return matches
    },

    async insert(values) {
      const row = await db
        .insertInto(t)
        .values({
          name: values.name,
          name_language: values.nameLanguage,
          picture: values.picture,
        })
        .returningAll()
        .executeTakeFirstOrThrow()
      return toNamed(row)
    },

    async update(id, values: Partial<NamedValues>) {
      const row = await db
        .updateTable(t)
        .set({ ...namedRow(values), updated_at: sql`unixepoch()` })
        .where("id", "=", id)
        .returningAll()
        .executeTakeFirst()
      return row ? toNamed(row) : null
    },

    async delete(id) {
      await db.deleteFrom(t).where("id", "=", id).execute()
    },

    async alternateNamesOf(ids) {
      const result = new Map<number, AlternateName[]>()
      for (const chunk of chunks(ids)) {
        const rows = await db
          .selectFrom(names)
          .selectAll()
          .where(owner, "in", chunk)
          .orderBy(owner)
          .orderBy("position")
          .execute()
        for (const row of rows) {
          const list = result.get(row[owner]) ?? []
          list.push(toAlternateName(row))
          result.set(row[owner], list)
        }
      }
      return result
    },

    async setAlternateNames(id, values) {
      const existing = await db
        .selectFrom(names)
        .select(["id", "name"])
        .where(owner, "=", id)
        .execute()
      const current = new Map(existing.map((row) => [row.id, row.name]))
      const kept = new Set<number>()
      for (const value of values) {
        if (value.id == null) continue
        if (!current.has(value.id)) {
          throw new Error(`Name ${value.id} isn't one of its alternate names`)
        }
        kept.add(value.id)
      }
      const removed = existing.filter((row) => !kept.has(row.id))
      for (const chunk of chunks(removed.map((row) => row.id))) {
        await db.deleteFrom(names).where("id", "in", chunk).execute()
      }
      // Names can trade places between rows, so renamed rows first move clear
      // of the unique (owner, name) index, as tracks are renumbered. U+FFFF is
      // never typed, and passes the trim CHECK.
      for (const value of values) {
        if (value.id != null && current.get(value.id) !== value.name) {
          await db
            .updateTable(names)
            .set({ name: `￿${value.id}` })
            .where("id", "=", value.id)
            .execute()
        }
      }
      for (const [position, value] of values.entries()) {
        const row = {
          name: value.name,
          language: value.language,
          note: value.note,
          position,
          ...(hasRealNames ? { real_name: value.realName ? 1 : 0 } : {}),
        }
        if (value.id != null) {
          await db
            .updateTable(names)
            .set(row)
            .where("id", "=", value.id)
            .execute()
        } else {
          await db
            .insertInto(names)
            .values({ ...row, [owner]: id } as typeof row & {
              artist_id: number
            })
            .execute()
        }
      }
    },
  }
}

/** The columns of a named row that are given. */
function namedRow(values: Partial<NamedValues>) {
  return {
    ...(values.name !== undefined && { name: values.name }),
    ...(values.nameLanguage !== undefined && {
      name_language: values.nameLanguage,
    }),
    ...(values.picture !== undefined && { picture: values.picture }),
  }
}

function toNamed(row: Selectable<Artists>): Artist {
  return {
    id: row.id,
    name: row.name,
    nameLanguage: row.name_language,
    picture: row.picture,
    insertedAt: row.inserted_at,
    updatedAt: row.updated_at,
  }
}

/** A label's row has no `real_name`, so it's never a real name. */
export function toAlternateName(
  row: Selectable<ArtistAlternateNames>
): AlternateName {
  return {
    id: row.id,
    name: row.name,
    language: row.language,
    realName: row.real_name === 1,
    note: row.note,
  }
}
