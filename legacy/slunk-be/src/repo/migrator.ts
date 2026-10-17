// The migration runner (design §5.2). Kysely's own Migrator runs SQLite
// migrations outside a transaction, so this one applies them all in a single
// BEGIN IMMEDIATE … COMMIT and checks foreign keys before committing.
import { existsSync, mkdirSync, readdirSync, rmSync } from "node:fs"
import { dirname, join } from "node:path"
import { pathToFileURL } from "node:url"
import Database from "better-sqlite3"
import { type Generated, Kysely, SqliteDialect, sql } from "kysely"
import { openSqlite } from "./db.ts"
import type { DB } from "./db_types.ts"

export const MIGRATIONS_DIR = join(
  import.meta.dirname,
  "../../priv/repo/migrations"
)

const MIGRATION_FILE = /^(\d{14}_[a-z0-9_]+)\.ts$/

// The backups this runner makes, which are the only files it ever removes
const BACKUP_FILE = /^slunk-(\d{14})(?:-(\d+))?\.db$/

type MigrationsDB = {
  migrations: {
    id: Generated<number>
    name: string
    applied_at: Generated<number>
  }
}

type Migration = {
  up(db: Kysely<unknown>): Promise<void>
  down(db: Kysely<unknown>): Promise<void>
}

/**
 * What a run did: the migrations it ran, the backup it made first, and the
 * old backups that removed.
 */
export type Result = {
  names: string[]
  backup: string | null
  removed: string[]
}

/** How many backups to keep (`SLUNK_KEEP_BACKUPS`). */
export type BackupOptions = { keepBackups: number }

/** `YYYYMMDDHHMMSS` in UTC, the timestamp in migration file names. */
export function timestamp(date = new Date()): string {
  return date.toISOString().replace(/\D/g, "").slice(0, 14)
}

/** The migrations in priv/repo/migrations/, in the order they apply. */
export function migrationNames(): string[] {
  return readdirSync(MIGRATIONS_DIR)
    .filter((file) => !file.startsWith("."))
    .map((file) => {
      const match = MIGRATION_FILE.exec(file)
      if (!match) throw new Error(`Not a migration file name: ${file}`)
      return match[1]
    })
    .sort()
}

/** Backs up, then applies every pending migration. */
export function migrate(
  dbPath: string,
  options: BackupOptions
): Promise<Result> {
  return run(dbPath, options, "up", (applied, files) =>
    files.filter((name) => !applied.includes(name))
  )
}

/** Backs up, then reverts the last `n` applied migrations, newest first. */
export function rollback(
  dbPath: string,
  n: number,
  options: BackupOptions
): Promise<Result> {
  if (!Number.isInteger(n) || n < 1) {
    throw new Error(`Not a number of migrations: ${n}`)
  }
  return run(dbPath, options, "down", (applied) => applied.slice(-n).reverse())
}

/** Every migration with when it was applied, plus applied ones whose file is gone. */
export async function status(
  dbPath: string
): Promise<{ name: string; appliedAt: number | null; missing: boolean }[]> {
  const files = migrationNames()
  const applied = existsSync(dbPath)
    ? await withDb(
        new Database(dbPath, { readonly: true, fileMustExist: true }),
        appliedRows
      )
    : []
  const appliedAt = new Map(applied.map((row) => [row.name, row.applied_at]))
  return [...new Set([...files, ...appliedAt.keys()])].sort().map((name) => ({
    name,
    appliedAt: appliedAt.get(name) ?? null,
    missing: !files.includes(name),
  }))
}

/** Copies the database into slunk-backups/, next to it, and removes old copies. */
export function backup(
  dbPath: string,
  options: BackupOptions
): Promise<{ backup: string; removed: string[] }> {
  return withDb(new Database(dbPath, { fileMustExist: true }), (db) =>
    backupInto(db, dbPath, options)
  )
}

/** The latest applied migration, or null before the first one. */
export async function latestMigration(db: Kysely<DB>): Promise<string | null> {
  const row = await db
    .selectFrom("migrations")
    .select("name")
    .orderBy("name", "desc")
    .executeTakeFirst()
  return row?.name ?? null
}

async function run(
  dbPath: string,
  options: BackupOptions,
  direction: "up" | "down",
  pick: (applied: string[], files: string[]) => string[]
): Promise<Result> {
  const isNew = !existsSync(dbPath)
  // Foreign keys stay off while the schema changes (design §5.3). The pragma
  // can't change inside a transaction, so it's set when the connection opens.
  return withDb(openSqlite(dbPath, { foreignKeys: false }), async (db) => {
    const files = migrationNames()
    if (pick(await applied(db, files), files).length === 0) {
      return { names: [], backup: null, removed: [] }
    }
    const { backup, removed } = isNew
      ? { backup: null, removed: [] }
      : await backupInto(db, dbPath, options)

    const names = await db.connection().execute(async (conn) => {
      // Takes the write lock up front, so the server and the CLI can't
      // interleave. What's pending is read again under the lock.
      await sql`begin immediate`.execute(conn)
      try {
        await conn.schema
          .createTable("migrations")
          .ifNotExists()
          .addColumn("id", "integer", (col) => col.primaryKey().notNull())
          .addColumn("name", "text", (col) => col.notNull().unique())
          .addColumn("applied_at", "integer", (col) =>
            col.notNull().defaultTo(sql`(unixepoch())`)
          )
          .modifyEnd(sql`strict`)
          .execute()

        const names = pick(await applied(conn, files), files)
        for (const name of names) {
          const file = pathToFileURL(join(MIGRATIONS_DIR, `${name}.ts`))
          const migration: Migration = await import(file.href)
          await migration[direction](conn as Kysely<unknown>)
          if (direction === "up") {
            await conn.insertInto("migrations").values({ name }).execute()
          } else {
            await conn
              .deleteFrom("migrations")
              .where("name", "=", name)
              .execute()
          }
        }

        const { rows } = await sql`pragma foreign_key_check`.execute(conn)
        if (rows.length > 0) {
          const list = rows.map((row) => JSON.stringify(row)).join("\n")
          throw new Error(`Foreign key check failed, rolled back:\n${list}`)
        }
        await sql`commit`.execute(conn)
        return names
      } catch (error) {
        await sql`rollback`.execute(conn)
        throw error
      }
    })
    return { names, backup, removed }
  })
}

/** Applied migration names, in order, after checking them against the files. */
async function applied(
  db: Kysely<MigrationsDB>,
  files: string[]
): Promise<string[]> {
  const names = (await appliedRows(db)).map((row) => row.name)
  const missing = names.filter((name) => !files.includes(name))
  if (missing.length > 0) {
    throw new Error(`Applied migrations have no file: ${missing.join(", ")}`)
  }
  const last = names.at(-1)
  const early = files.filter(
    (name) => last !== undefined && name < last && !names.includes(name)
  )
  if (early.length > 0) {
    throw new Error(
      `Pending migrations sort before the last applied one (${last}): ` +
        `${early.join(", ")}. Give them a later timestamp.`
    )
  }
  return names
}

async function appliedRows(db: Kysely<MigrationsDB>) {
  const { rows } = await sql`
    select 1 from sqlite_schema where type = 'table' and name = 'migrations'
  `.execute(db)
  if (rows.length === 0) return []
  return db
    .selectFrom("migrations")
    .select(["name", "applied_at"])
    .orderBy("name")
    .execute()
}

async function backupInto(
  db: Kysely<MigrationsDB>,
  dbPath: string,
  { keepBackups }: BackupOptions
): Promise<{ backup: string; removed: string[] }> {
  const dir = join(dirname(dbPath), "slunk-backups")
  mkdirSync(dir, { recursive: true })
  const stamp = timestamp()
  let file = join(dir, `slunk-${stamp}.db`)
  for (let n = 2; existsSync(file); n++) {
    file = join(dir, `slunk-${stamp}-${n}.db`)
  }
  await sql`vacuum into ${file}`.execute(db)
  return { backup: file, removed: pruneBackups(dir, keepBackups) }
}

/**
 * Removes all but the newest `keep` backups. Only files named like this
 * runner's backups are touched; anything else in the folder stays.
 */
function pruneBackups(dir: string, keep: number): string[] {
  const backups = readdirSync(dir)
    .flatMap((name) => {
      const match = BACKUP_FILE.exec(name)
      return match ? [{ name, stamp: match[1], n: Number(match[2] ?? 1) }] : []
    })
    // Oldest first. `-2` comes after the plain name of the same second.
    .sort((a, b) => a.stamp.localeCompare(b.stamp) || a.n - b.n)
  const old = backups.slice(0, Math.max(0, backups.length - keep))
  for (const { name } of old) rmSync(join(dir, name))
  return old.map(({ name }) => join(dir, name))
}

async function withDb<T>(
  sqlite: Database.Database,
  fn: (db: Kysely<MigrationsDB>) => Promise<T>
): Promise<T> {
  const db = new Kysely<MigrationsDB>({
    dialect: new SqliteDialect({ database: sqlite }),
  })
  try {
    return await fn(db)
  } finally {
    await db.destroy()
  }
}
