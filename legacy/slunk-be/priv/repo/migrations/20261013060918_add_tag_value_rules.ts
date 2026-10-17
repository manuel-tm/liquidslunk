// Tag values the baseline didn't check (Appendix D of the design): a date
// tag's value is a day at midnight UTC, and a number tag's value is finite.
import { type Kysely, sql } from "kysely"

const RULES = [
  {
    table: "release_date_tags",
    name: "value_is_day",
    // Days only, like release dates (design §4.3 Dates)
    broken: "NEW.value % 86400 <> 0",
    message: "a date tag's value must be a day, at midnight UTC",
  },
  {
    table: "release_real_number_tags",
    name: "value_is_finite",
    // SQLite stores infinity, which GraphQL can't send. NaN is already NULL.
    broken: "abs(NEW.value) = 9e999",
    message: "a number tag's value must be finite",
  },
]

export async function up(db: Kysely<unknown>): Promise<void> {
  for (const { table, name, broken, message } of RULES) {
    // The triggers only check new writes, so rows already there are checked now
    const { rows } = await sql
      .raw<{ count: number }>(
        `SELECT count(*) AS count FROM ${table}
        WHERE ${broken.replaceAll("NEW.", "")}`
      )
      .execute(db)
    const { count } = rows[0]!
    if (count > 0) {
      throw new Error(
        `${table} has ${count} row(s) that break this: ${message}`
      )
    }

    for (const [event, when] of [
      ["insert", "INSERT"],
      ["update", "UPDATE OF value"],
    ]) {
      await sql
        .raw(
          `CREATE TRIGGER ${table}_${name}_${event}
          BEFORE ${when} ON ${table}
          WHEN ${broken}
          BEGIN
            SELECT RAISE(ABORT, '${message.replaceAll("'", "''")}');
          END`
        )
        .execute(db)
    }
  }
}

export async function down(db: Kysely<unknown>): Promise<void> {
  for (const { table, name } of RULES) {
    for (const event of ["insert", "update"]) {
      await sql.raw(`DROP TRIGGER ${table}_${name}_${event}`).execute(db)
    }
  }
}
