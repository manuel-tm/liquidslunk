// Generates the migration parity fixtures (design §5.6) with slunk's own
// migration runner, in legacy/slunk-be/. The tests in tests/parity.rs check
// the Rust migrator against them.
//
// Run from the repository root, after `npm install --prefix legacy/slunk-be`:
//
//   node crates/slunk/tests/fixtures/parity/generate.ts
//
// It writes, next to itself:
// - fresh.schema.json: sqlite_schema of a new database slunk migrated, in
//   rowid order;
// - fresh.sql: that database, dumped;
// - state_<k>.sql, for k from 0 to 7: a database after slunk's first k
//   migrations, each one followed by its seed in seeds/;
// - final_<k>.sql: state_<k>.sql after slunk's runner applied the rest;
// - rolled_back_<k>.sql, for k from 1 to 8: the database after k migrations
//   (state_<k>.sql, or final_7.sql for all 8) after slunk's runner rolled
//   the last one back.
//
// Dumps come from the sqlite3 command-line tool's `.dump`.
import { execFileSync } from "node:child_process"
import {
  cpSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs"
import { createRequire } from "node:module"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { pathToFileURL } from "node:url"

type Migrator = {
  migrate(dbPath: string, options: { keepBackups: number }): Promise<unknown>
  rollback(
    dbPath: string,
    n: number,
    options: { keepBackups: number }
  ): Promise<unknown>
}

type Connection = {
  pragma(source: string): unknown
  exec(source: string): void
  prepare(source: string): { all(): unknown[] }
  close(): void
}

const HERE = import.meta.dirname
const BACKEND = join(HERE, "../../../../../legacy/slunk-be")
const MIGRATIONS = join(BACKEND, "priv/repo/migrations")
const OPTIONS = { keepBackups: 10 }

const require = createRequire(join(BACKEND, "package.json"))
const { default: Database } = (await import(
  pathToFileURL(require.resolve("better-sqlite3")).href
)) as { default: new (path: string) => Connection }

const names = readdirSync(MIGRATIONS)
  .filter((file) => file.endsWith(".ts"))
  .map((file) => file.slice(0, -".ts".length))
  .sort()

const temp = mkdtempSync(join(tmpdir(), "slunk-parity-"))
try {
  // slunk's runner applies every migration it finds. A copy of the backend
  // with only the first i migrations applies up to the i-th.
  const upTo: Migrator[] = []
  for (let i = 1; i < names.length; i++) {
    const copy = join(temp, `backend-${i}`)
    cpSync(join(BACKEND, "src"), join(copy, "src"), { recursive: true })
    cpSync(join(BACKEND, "package.json"), join(copy, "package.json"))
    symlinkSync(join(BACKEND, "node_modules"), join(copy, "node_modules"))
    const migrations = join(copy, "priv/repo/migrations")
    mkdirSync(migrations, { recursive: true })
    for (const name of names.slice(0, i)) {
      cpSync(join(MIGRATIONS, `${name}.ts`), join(migrations, `${name}.ts`))
    }
    upTo[i] = await importMigrator(copy)
  }
  const slunk = await importMigrator(BACKEND)

  for (let k = 0; k < names.length; k++) {
    const state = database(`state-${k}`)
    // An empty file, as a library starts out
    new Database(state).close()
    for (let i = 1; i <= k; i++) {
      await upTo[i].migrate(state, OPTIONS)
      seed(state, names[i - 1])
    }
    const dump = write(`state_${k}.sql`, dumpOf(state))

    const final = database(`final-${k}`)
    execFileSync("sqlite3", [final], { input: dump })
    await slunk.migrate(final, OPTIONS)
    write(`final_${k}.sql`, dumpOf(final))
  }

  for (let k = 1; k <= names.length; k++) {
    const start = k < names.length ? `state_${k}.sql` : `final_${k - 1}.sql`
    const rolledBack = database(`rolled-back-${k}`)
    execFileSync("sqlite3", [rolledBack], {
      input: readFileSync(join(HERE, start), "utf8"),
    })
    await slunk.rollback(rolledBack, 1, OPTIONS)
    write(`rolled_back_${k}.sql`, dumpOf(rolledBack))
  }

  const fresh = database("fresh")
  await slunk.migrate(fresh, OPTIONS)
  const db = new Database(fresh)
  const schema = db
    .prepare(
      "SELECT type, name, tbl_name, sql FROM sqlite_schema ORDER BY rowid"
    )
    .all()
  db.close()
  write("fresh.schema.json", JSON.stringify(schema, null, 2) + "\n")
  write("fresh.sql", dumpOf(fresh))
} finally {
  rmSync(temp, { recursive: true, force: true })
}

async function importMigrator(backend: string): Promise<Migrator> {
  return import(pathToFileURL(join(backend, "src/repo/migrator.ts")).href)
}

/** A path for a database in a folder of its own, for its backups. */
function database(name: string): string {
  const folder = join(temp, name)
  mkdirSync(folder)
  return join(folder, "slunk.db")
}

/** Runs a migration's seed in one transaction, with foreign keys on. */
function seed(dbPath: string, name: string) {
  const file = join(HERE, "seeds", `${name}.sql`)
  if (!existsSync(file)) return
  const db = new Database(dbPath)
  try {
    db.pragma("foreign_keys = ON")
    db.exec("BEGIN")
    db.exec(readFileSync(file, "utf8"))
    db.exec("COMMIT")
  } finally {
    db.close()
  }
}

function dumpOf(dbPath: string): string {
  return execFileSync("sqlite3", [dbPath, ".dump"], { encoding: "utf8" })
}

function write(name: string, contents: string): string {
  writeFileSync(join(HERE, name), contents)
  console.log(`Wrote ${name}`)
  return contents
}
