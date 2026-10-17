// Tags on tracks (design §3.2, §8 phase 4): a value table per type, built
// like the releases' (§4.3 Tags, Appendix D) and using the same tag
// definitions. Switching a tag to single-value now checks tracks too.
import { type Kysely, sql } from "kysely"

const TYPES = [
  { type: "text", valueType: "text" },
  { type: "real_number", valueType: "real" },
  { type: "date", valueType: "integer" },
] as const

type Type = (typeof TYPES)[number]["type"]

// The rules on values that the release tables got in Appendix D
const VALUE_RULES: Partial<
  Record<Type, { name: string; broken: string; message: string }>
> = {
  real_number: {
    name: "value_is_finite",
    broken: "abs(NEW.value) = 9e999",
    message: "a number tag''s value must be finite",
  },
  date: {
    name: "value_is_day",
    broken: "NEW.value % 86400 <> 0",
    message: "a date tag''s value must be a day, at midnight UTC",
  },
}

export async function up(db: Kysely<unknown>): Promise<void> {
  for (const { type, valueType } of TYPES) {
    const tags = `${type}_tags`
    const values = `track_${type}_tags`

    await db.schema
      .createTable(values)
      .addColumn("id", "integer", (col) => col.primaryKey().notNull())
      .addColumn("track_id", "integer", (col) =>
        col.notNull().references("tracks.id").onDelete("cascade")
      )
      .addColumn("tag_id", "integer", (col) =>
        col.notNull().references(`${tags}.id`).onDelete("restrict")
      )
      .addColumn("value", valueType, (col) =>
        type === "text"
          ? col
              .notNull()
              .check(sql`value <> '' AND value = trim(value)`)
              .modifyEnd(sql`COLLATE NOCASE`)
          : col.notNull()
      )
      .modifyEnd(sql`strict`)
      .execute()
    await db.schema
      .createIndex(`${values}_track_id_tag_id_value_index`)
      .on(values)
      .columns(["track_id", "tag_id", "value"])
      .unique()
      .execute()
    await db.schema
      .createIndex(`${values}_tag_id_value_index`)
      .on(values)
      .columns(["tag_id", "value"])
      .execute()

    // A single-value tag holds at most one value per track (design §4.4)
    await sql
      .raw(
        `CREATE TRIGGER ${values}_single_value_insert
        BEFORE INSERT ON ${values}
        WHEN (SELECT multiple FROM ${tags} WHERE id = NEW.tag_id) = 0
          AND EXISTS (SELECT 1 FROM ${values}
                      WHERE track_id = NEW.track_id AND tag_id = NEW.tag_id)
        BEGIN
          SELECT RAISE(ABORT, 'single-value tag already set on this track');
        END`
      )
      .execute(db)
    await sql
      .raw(
        `CREATE TRIGGER ${values}_single_value_update
        BEFORE UPDATE OF track_id, tag_id ON ${values}
        WHEN (SELECT multiple FROM ${tags} WHERE id = NEW.tag_id) = 0
          AND EXISTS (SELECT 1 FROM ${values}
                      WHERE track_id = NEW.track_id AND tag_id = NEW.tag_id
                        AND id <> NEW.id)
        BEGIN
          SELECT RAISE(ABORT, 'single-value tag already set on this track');
        END`
      )
      .execute(db)

    const rule = VALUE_RULES[type]
    if (rule) {
      for (const [event, when] of [
        ["insert", "INSERT"],
        ["update", "UPDATE OF value"],
      ]) {
        await sql
          .raw(
            `CREATE TRIGGER ${values}_${rule.name}_${event}
            BEFORE ${when} ON ${values}
            WHEN ${rule.broken}
            BEGIN
              SELECT RAISE(ABORT, '${rule.message}');
            END`
          )
          .execute(db)
      }
    }

    // Switching to single-value now also fails while a track has several
    await sql.raw(`DROP TRIGGER ${tags}_multiple_off`).execute(db)
    await sql
      .raw(multipleOff(tags, [`release_${type}_tags`, values]))
      .execute(db)
  }
}

export async function down(db: Kysely<unknown>): Promise<void> {
  for (const { type } of TYPES) {
    const tags = `${type}_tags`
    // Dropping the table drops its indexes and triggers too
    await db.schema.dropTable(`track_${type}_tags`).execute()
    await sql.raw(`DROP TRIGGER ${tags}_multiple_off`).execute(db)
    await sql.raw(multipleOff(tags, [`release_${type}_tags`])).execute(db)
  }
}

/**
 * The baseline's trigger, checking each of the value tables. With only the
 * release table, it's the baseline's own, message included.
 */
function multipleOff(tags: string, valueTables: string[]): string {
  const owners = valueTables.length > 1 ? "releases or tracks" : "releases"
  const several = valueTables
    .map(
      (values) => `EXISTS (SELECT 1 FROM ${values} WHERE tag_id = NEW.id
                    GROUP BY ${values.startsWith("track_") ? "track_id" : "release_id"}
                    HAVING count(*) > 1)`
    )
    .join("\n          OR ")
  return `CREATE TRIGGER ${tags}_multiple_off
    BEFORE UPDATE OF multiple ON ${tags}
    WHEN OLD.multiple = 1 AND NEW.multiple = 0
      AND (${several})
    BEGIN
      SELECT RAISE(ABORT, 'some ${owners} have several values for this tag');
    END`
}
