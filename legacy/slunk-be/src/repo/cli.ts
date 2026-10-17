// The db:* commands (design §5.4). npm runs them from slunk-be/.
import { execFileSync } from "node:child_process"
import { existsSync, rmSync, writeFileSync } from "node:fs"
import { join, relative } from "node:path"
import { seed } from "../../priv/repo/seeds.ts"
import { backupsToKeep, databasePath } from "../config.ts"
import { openDb } from "./db.ts"
import {
  backup,
  migrate,
  MIGRATIONS_DIR,
  migrationNames,
  type Result,
  rollback,
  status,
  timestamp,
} from "./migrator.ts"

const ROOT = join(import.meta.dirname, "../..")

const TEMPLATE = `import { type Kysely, sql } from "kysely"

export async function up(db: Kysely<unknown>): Promise<void> {}

export async function down(db: Kysely<unknown>): Promise<void> {}
`

const [command, arg] = process.argv.slice(2)

switch (command) {
  case "gen":
    gen(arg)
    break
  case "migrate":
    report("Applied", await migrate(databasePath(), backupOptions()))
    break
  case "rollback":
    report(
      "Rolled back",
      await rollback(databasePath(), Number(arg ?? 1), backupOptions())
    )
    break
  case "status":
    await printStatus()
    break
  case "seed":
    await seedDb()
    break
  case "reset":
    await reset()
    break
  default:
    throw new Error(`Unknown command: ${command}`)
}

/** Creates `<timestamp>_<name>.ts` from the template. */
function gen(name: string | undefined) {
  if (!name || !/^[a-z][a-z0-9_]*$/.test(name)) {
    throw new Error("Usage: npm run db:gen -- <snake_case_name>")
  }
  if (migrationNames().some((existing) => existing.slice(15) === name)) {
    throw new Error(`A migration named ${name} already exists.`)
  }
  const file = join(MIGRATIONS_DIR, `${timestamp()}_${name}.ts`)
  writeFileSync(file, TEMPLATE, { flag: "wx" })
  console.log(`Created ${shown(file)}`)
}

/** Prints what ran, then regenerates the DB types if the schema changed. */
function report(verb: string, { names, backup, removed }: Result) {
  if (backup) console.log(`Backed up to ${shown(backup)}`)
  reportRemoved(removed)
  if (names.length === 0) console.log("Nothing to do.")
  for (const name of names) console.log(`${verb} ${name}`)
  if (names.length > 0) generateTypes()
}

/** Regenerates src/repo/db_types.ts (config in .kysely-codegenrc.json). */
function generateTypes() {
  execFileSync(
    join(ROOT, "node_modules/.bin/kysely-codegen"),
    ["--url", databasePath()],
    { cwd: ROOT, stdio: "inherit" }
  )
}

async function printStatus() {
  console.log(`Database: ${databasePath()}`)
  for (const { name, appliedAt, missing } of await status(databasePath())) {
    const state = missing
      ? "MISSING"
      : appliedAt === null
        ? "pending"
        : "applied"
    const at =
      appliedAt === null ? "" : new Date(appliedAt * 1000).toISOString()
    console.log(`${state.padEnd(8)} ${name}  ${at}`)
  }
}

async function seedDb() {
  const db = openDb(databasePath())
  try {
    await db.transaction().execute(seed)
  } finally {
    await db.destroy()
  }
}

/** Backs up and deletes the database, then migrates and seeds a new one. */
async function reset() {
  const dbPath = databasePath()
  if (existsSync(dbPath)) {
    const copy = await backup(dbPath, backupOptions())
    console.log(`Backed up to ${shown(copy.backup)}`)
    reportRemoved(copy.removed)
    for (const suffix of ["", "-wal", "-shm"]) {
      rmSync(dbPath + suffix, { force: true })
    }
    console.log(`Deleted ${dbPath}`)
  }
  report("Applied", await migrate(dbPath, backupOptions()))
  await seedDb()
}

function backupOptions() {
  return { keepBackups: backupsToKeep() }
}

function reportRemoved(removed: string[]) {
  if (removed.length > 0) {
    console.log(
      `Removed ${removed.length} old backup${removed.length === 1 ? "" : "s"}, keeping the last ${backupsToKeep()}`
    )
  }
}

/** A path relative to slunk-be/ when it's inside it, else absolute. */
function shown(path: string): string {
  const inside = relative(ROOT, path)
  return inside.startsWith("..") ? path : inside
}
