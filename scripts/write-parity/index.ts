// Write parity (design §10): runs the same steps on two copies of a seeded
// library, one through slunk's GraphQL and one through liquidslunk's
// commands, then compares what each step returned and the two databases,
// leaving out inserted_at, updated_at and applied_at.
//
// From the repository's root, after `npm install --prefix legacy/slunk-be`:
//
//   npm run parity:writes
//
// It builds everything in a temporary folder, and touches nothing else:
// - two libraries, graphql/library/ and commands/library/, with the same
//   files, and shared/ next to each for paths that start with ../;
// - the library migrated by the CLI, seeded, then prepared through GraphQL
//   (`prepare` in each phase's steps), and copied for the commands;
// - slunk's backend on a free port, for the GraphQL side;
// - examples/run_commands.rs in src-tauri, for the commands side.
//
// Each phase adds its steps in a module of this folder, listed in PHASES.
import { execFileSync, spawn } from "node:child_process"
import {
  copyFileSync,
  cpSync,
  mkdirSync,
  mkdtempSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs"
import { createServer } from "node:net"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { DatabaseSync } from "node:sqlite"
import { isDeepStrictEqual } from "node:util"

import { catalog } from "./catalog.ts"
import { type Phase, media } from "./media.ts"
import { releases } from "./releases.ts"
import { tagging } from "./tagging.ts"

const PHASES: Phase[] = [media, catalog, tagging, releases]

// Copies keep their files' times and their symlinks
const COPY = {
  recursive: true,
  preserveTimestamps: true,
  verbatimSymlinks: true,
}

const ROOT = join(import.meta.dirname, "../..")
const BACKEND = join(ROOT, "legacy/slunk-be")
const AUDIO = join(ROOT, "crates/slunk/tests/fixtures/audio")
const TIMESTAMPS = new Set(["inserted_at", "updated_at", "applied_at"])

const temp = mkdtempSync(join(tmpdir(), "liquidslunk-parity-"))
// Each library in a folder of its own, so `..` holds the same on both sides
const graphqlLibrary = join(temp, "graphql/library")
const commandsLibrary = join(temp, "commands/library")
let failed = false

try {
  console.log(`Libraries in ${temp}`)
  const template = join(temp, "template")
  makeFiles(join(template, "library"), join(template, "shared"))
  cpSync(template, join(temp, "graphql"), COPY)
  cpSync(template, join(temp, "commands"), COPY)

  const db = join(graphqlLibrary, "slunk.db")
  run("cargo", [
    "run",
    "--quiet",
    "-p",
    "slunk-cli",
    "--",
    "migrate",
    "--db",
    db,
  ])
  run("cargo", ["run", "--quiet", "-p", "slunk-cli", "--", "seed", "--db", db])
  await withBackend(db, async (graphql) => {
    for (const phase of PHASES) {
      for (const step of phase.prepare)
        await graphql(step.graphql, step.variables)
    }
  })
  copyFileSync(db, join(commandsLibrary, "slunk.db"))

  const steps = PHASES.flatMap((phase) => phase.steps)
  const viaGraphql: Result[] = []
  await withBackend(db, async (graphql) => {
    for (const step of steps) {
      viaGraphql.push(
        await graphql(
          step.graphql,
          step.variables?.(graphqlLibrary),
          step.whole
        )
      )
    }
  })
  const viaCommands = runCommands(
    join(commandsLibrary, "slunk.db"),
    steps.map((step) => ({
      command: step.command,
      args: step.args?.(commandsLibrary) ?? null,
    }))
  )

  steps.forEach((step, i) => {
    const graphql = viaGraphql[i]
    if (step.returnsId && "ok" in graphql) graphql.ok = Number(graphql.ok)
    if (step.fromGraphql && "ok" in graphql) {
      graphql.ok = step.fromGraphql(graphql.ok)
    }
    const theirs = normalize(graphql, join(graphqlLibrary, ".."))
    const ours = normalize(viaCommands[i], join(commandsLibrary, ".."))
    if (!isDeepStrictEqual(theirs, ours)) {
      failed = true
      console.log(`✗ ${step.name}`)
      console.log(`  GraphQL:  ${JSON.stringify(theirs)}`)
      console.log(`  commands: ${JSON.stringify(ours)}`)
    } else {
      // A failure both sides share is a match too, but worth seeing
      const error = "error" in graphql ? `: fails with "${graphql.error}"` : ""
      console.log(`✓ ${step.name}${error}`)
    }
  })

  const theirs = dump(db)
  const ours = dump(join(commandsLibrary, "slunk.db"))
  for (const table of new Set([...Object.keys(theirs), ...Object.keys(ours)])) {
    if (!isDeepStrictEqual(theirs[table], ours[table])) {
      failed = true
      console.log(`✗ table ${table} differs`)
      console.log(`  GraphQL:  ${JSON.stringify(theirs[table])}`)
      console.log(`  commands: ${JSON.stringify(ours[table])}`)
    }
  }
  console.log(
    failed
      ? "The two libraries differ."
      : `Same results and same databases after ${steps.length} steps.`
  )
} finally {
  if (failed && process.env.KEEP) {
    console.log(`Kept ${temp}`)
  } else {
    rmSync(temp, { recursive: true, force: true })
  }
}
process.exitCode = failed ? 1 : 0

/** What a step returned: its data, or its error's message. */
type Result = { ok: unknown } | { error: string }

/**
 * The library's files, in `folder`, and `shared/` above it. Copies keep
 * their times and symlinks, so both libraries have the same files.
 */
function makeFiles(folder: string, shared: string) {
  const audio = (name: string) => join(AUDIO, name)
  const files: [string, string | Buffer][] = [
    ["music/Ryo Fukui/2 Early Summer.flac", audio("flac.flac")],
    ["music/Ryo Fukui/10 Mellow Dream.flac", audio("untagged.flac")],
    ["music/Ryo Fukui/CD1/1 Scenery.mp3", audio("id3v24.mp3")],
    ["music/Ryo Fukui/cover.jpg", Buffer.from("not really a jpeg")],
    ["music/Ryo Fukui/rip.log", "Exact Audio Copy"],
    ["music/Ryo Fukui/.DS_Store", ""],
    ["music/Alika/01 virmalised.m4a", audio("alac.m4a")],
    ["music/Alika/50%_off.txt", "a LIKE pattern"],
    ["music/Alika/Café.flac", audio("flac-variants.flac")],
    ["music/moved/2 Early Summer.flac", audio("flac.flac")],
    ["music/.hidden/secret.flac", audio("flac.flac")],
    // Phase 2's pictures, logo and file links
    ["music/Drake/drake.jpg", Buffer.from("not really a jpeg")],
    ["music/Drake/logo.png", Buffer.from("not really a png")],
    ["music/Drake/notes.txt", "Notes"],
    ["music/Drake/interview.pdf", "An interview"],
  ]
  for (const [path, content] of files) {
    const file = join(folder, path)
    mkdirSync(join(file, ".."), { recursive: true })
    if (typeof content === "string" && content.startsWith(AUDIO)) {
      copyFileSync(content, file)
    } else {
      writeFileSync(file, content)
    }
    // A few nanoseconds before a second, which Node's mtimeMs rounds up
    execFileSync("touch", ["-d", "2023-11-14T22:13:20.999999999Z", file])
  }
  symlinkSync("../../shared", join(folder, "music/Elsewhere"))
  symlinkSync("2 Early Summer.flac", join(folder, "music/Ryo Fukui/Alias.flac"))
  symlinkSync("gone.flac", join(folder, "music/Ryo Fukui/Broken.flac"))
  mkdirSync(join(shared, "sub"), { recursive: true })
  copyFileSync(audio("aac.m4a"), join(shared, "a.m4a"))
  writeFileSync(join(shared, "sub/b.png"), "not really a png")
}

/** Runs `work` with slunk's backend serving the library, then stops it. */
async function withBackend(
  db: string,
  work: (
    graphql: (
      query: string,
      variables?: object,
      whole?: boolean
    ) => Promise<Result>
  ) => Promise<void>
) {
  const port = await freePort()
  const backend = spawn("node", ["src/server.ts"], {
    cwd: BACKEND,
    env: {
      ...process.env,
      SLUNK_DB: db,
      SLUNK_SKIP_MIGRATE: "1",
      PORT: String(port),
    },
    stdio: ["ignore", "ignore", "inherit"],
  })
  const url = `http://127.0.0.1:${port}/graphql`
  const graphql = async (
    query: string,
    variables?: object,
    whole = false
  ): Promise<Result> => {
    const response = await fetch(url, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ query, variables }),
    })
    const { data, errors } = (await response.json()) as {
      data?: Record<string, unknown> | null
      errors?: { message: string }[]
    }
    if (errors?.length) return { error: errors[0].message }
    return { ok: whole ? data : Object.values(data ?? {})[0] }
  }
  try {
    for (let tries = 0; ; tries++) {
      try {
        await graphql("{ __typename }")
        break
      } catch (error) {
        if (tries > 100) throw error
        await new Promise((resolve) => setTimeout(resolve, 100))
      }
    }
    await work(graphql)
  } finally {
    const exited = new Promise((resolve) => backend.once("exit", resolve))
    backend.kill("SIGTERM")
    await exited
  }
}

function runCommands(
  db: string,
  steps: { command: string; args: unknown }[]
): Result[] {
  const file = join(temp, "steps.json")
  writeFileSync(file, JSON.stringify(steps))
  const output = execFileSync(
    "cargo",
    [
      "run",
      "--quiet",
      "-p",
      "liquidslunk",
      "--example",
      "run_commands",
      "--",
      db,
      file,
    ],
    { cwd: ROOT, encoding: "utf8", stdio: ["ignore", "pipe", "inherit"] }
  )
  const results = JSON.parse(output) as (
    { ok: unknown } | { error: { message: string } }
  )[]
  return results.map((result) =>
    "error" in result ? { error: result.error.message } : result
  )
}

/**
 * A result as both sides can match it: ids as numbers, since GraphQL's are
 * strings, without the times things were saved, and with the side's folder,
 * which differs, as `<side>`.
 */
function normalize(value: unknown, side: string): unknown {
  if (typeof value === "string") return value.replaceAll(side, "<side>")
  if (Array.isArray(value)) return value.map((item) => normalize(item, side))
  if (value === null || typeof value !== "object") return value
  return Object.fromEntries(
    Object.entries(value)
      .filter(([key]) => key !== "insertedAt" && key !== "updatedAt")
      .map(([key, item]) => [
        key,
        (key === "id" || key === "entityId") && typeof item === "string"
          ? Number(item)
          : normalize(item, side),
      ])
  )
}

/** Every table's rows, in rowid order, without the timestamps. */
function dump(path: string): Record<string, unknown[]> {
  const db = new DatabaseSync(path, { readOnly: true })
  try {
    const tables = db
      .prepare(
        "select name from sqlite_schema where type = 'table' order by name"
      )
      .all() as { name: string }[]
    const result: Record<string, unknown[]> = {
      sqlite_schema: db
        .prepare(
          "select type, name, tbl_name, sql from sqlite_schema order by name"
        )
        .all(),
    }
    for (const { name } of tables) {
      const columns = (
        db.prepare(`select name from pragma_table_info('${name}')`).all() as {
          name: string
        }[]
      )
        .map((column) => column.name)
        .filter((column) => !TIMESTAMPS.has(column))
      const order = name === "sqlite_sequence" ? "name" : "rowid"
      result[name] = db
        .prepare(
          `select ${columns.map((c) => `"${c}"`).join(", ")} from "${name}" order by ${order}`
        )
        .all()
    }
    return result
  } finally {
    db.close()
  }
}

function run(command: string, args: string[]) {
  execFileSync(command, args, {
    cwd: ROOT,
    stdio: ["ignore", "ignore", "inherit"],
  })
}

function freePort(): Promise<number> {
  return new Promise((resolve, reject) => {
    const server = createServer()
    server.listen(0, "127.0.0.1", () => {
      const address = server.address()
      server.close(() =>
        typeof address === "object" && address
          ? resolve(address.port)
          : reject(new Error("No port"))
      )
    })
  })
}
