// Rebuilds a table with a new definition, for the schema changes SQLite's
// ALTER TABLE can't make: a column's type, CHECK, default or foreign key
// (design §5.3). Committed migrations call it, so its behavior must never
// change: extend it with new options instead.
import {
  type CreateTableBuilder,
  type Kysely,
  type RawBuilder,
  sql,
} from "kysely"

type Options = {
  /**
   * The new definition, complete. It's built under a temporary name, and
   * renamed once the old table is gone.
   */
  define: (
    table: CreateTableBuilder<string>
  ) => CreateTableBuilder<string, string>
  /**
   * SQL expressions over the old table, for new or changed columns. Columns
   * both definitions share are copied as they are, and any other new column
   * gets its default.
   */
  copy?: Record<string, RawBuilder<unknown>>
}

/**
 * Rebuilds `table` with SQLite's recipe, keeping its rows, its indexes, its
 * triggers and its AUTOINCREMENT counter. Indexes and triggers come back as
 * they were: to change one, drop it before the rebuild and create the new one
 * after.
 *
 * Runs only in a migration, where the runner has turned foreign keys off and
 * checks them before committing.
 */
export async function rebuildTable(
  db: Kysely<unknown>,
  table: string,
  { define, copy = {} }: Options
): Promise<void> {
  // With foreign keys on, DROP TABLE would first delete every row, and
  // ON DELETE CASCADE would wipe the rows that point at them
  const { rows } = await sql<{ foreign_keys: number }>`
    pragma foreign_keys
  `.execute(db)
  if (rows[0]?.foreign_keys !== 0) {
    throw new Error(`rebuildTable(${table}) needs foreign keys off`)
  }

  const temp = `new_${table}`
  const kept = await sql<{ sql: string }>`
    select sql from sqlite_schema
    where tbl_name = ${table} and type in ('index', 'trigger') and sql is not null
  `.execute(db)
  const counter = await sequence(db, table)

  // Copy the rows into the new definition
  await define(db.schema.createTable(temp)).execute()
  const oldColumns = await columns(db, table)
  const newColumns = await columns(db, temp)
  const unknown = Object.keys(copy).filter((c) => !newColumns.includes(c))
  if (unknown.length > 0) {
    throw new Error(`rebuildTable(${table}): no column ${unknown.join(", ")}`)
  }
  const targets = newColumns.filter(
    (column) => Object.hasOwn(copy, column) || oldColumns.includes(column)
  )
  await sql`
    insert into ${sql.table(temp)} (${sql.join(targets.map((c) => sql.ref(c)))})
    select ${sql.join(targets.map((c) => copy[c] ?? sql.ref(c)))}
    from ${sql.table(table)}
  `.execute(db)

  // Swap the tables. Other tables' triggers still name the dropped table,
  // which would make the rename fail its schema check: legacy_alter_table
  // skips that check. Their foreign keys keep pointing at the name.
  await sql`drop table ${sql.table(table)}`.execute(db)
  await sql`pragma legacy_alter_table = on`.execute(db)
  try {
    await sql`
      alter table ${sql.table(temp)} rename to ${sql.table(table)}
    `.execute(db)
  } finally {
    await sql`pragma legacy_alter_table = off`.execute(db)
  }

  for (const statement of kept.rows) {
    await sql.raw(statement.sql).execute(db)
  }

  // Copying rows set the counter to the highest id left. Put back the old
  // one, so ids of deleted rows are never reused (design §4.2).
  if (counter !== null) {
    const now = (await sequence(db, table)) ?? 0
    await sql`delete from sqlite_sequence where name = ${table}`.execute(db)
    await sql`
      insert into sqlite_sequence (name, seq)
      values (${table}, ${Math.max(counter, now)})
    `.execute(db)
  }
}

async function columns(db: Kysely<unknown>, table: string) {
  const { rows } = await sql<{ name: string }>`
    select name from pragma_table_info(${table})
  `.execute(db)
  return rows.map((row) => row.name)
}

/** The table's AUTOINCREMENT counter, or null if it has none. */
async function sequence(db: Kysely<unknown>, table: string) {
  const exists = await sql`
    select 1 from sqlite_schema where name = 'sqlite_sequence'
  `.execute(db)
  if (exists.rows.length === 0) return null
  const { rows } = await sql<{ seq: number }>`
    select seq from sqlite_sequence where name = ${table}
  `.execute(db)
  return rows[0]?.seq ?? null
}
