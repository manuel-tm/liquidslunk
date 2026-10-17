import Database from "better-sqlite3"
import { Kysely, SqliteDialect } from "kysely"
import type { DB } from "./db_types.ts"

type Options = {
  /** The migrator turns foreign keys off (design §5.3). */
  foreignKeys?: boolean
  /** Fail instead of creating a new database. */
  fileMustExist?: boolean
}

/** Opens SQLite with the connection settings of design §4.1. */
export function openSqlite(
  path: string,
  { foreignKeys = true, fileMustExist = false }: Options = {}
): Database.Database {
  const sqlite = new Database(path, { fileMustExist })
  sqlite.pragma(`foreign_keys = ${foreignKeys ? "ON" : "OFF"}`)
  sqlite.pragma("journal_mode = WAL")
  sqlite.pragma("busy_timeout = 5000")
  return sqlite
}

/** The app's connection. The database must exist: `migrate` creates it. */
export function openDb(path: string): Kysely<DB> {
  return new Kysely<DB>({
    dialect: new SqliteDialect({
      database: openSqlite(path, { fileMustExist: true }),
    }),
  })
}
