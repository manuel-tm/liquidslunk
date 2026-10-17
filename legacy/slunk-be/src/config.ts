import { resolve } from "node:path"

/** The database file from `SLUNK_DB`, as an absolute path. */
export function databasePath(): string {
  const value = process.env.SLUNK_DB
  if (!value) {
    throw new Error(
      "SLUNK_DB is not set. Point it at a library's slunk.db (see .env.example)."
    )
  }
  return resolve(value)
}

/**
 * How many backups slunk-backups/ keeps, from `SLUNK_KEEP_BACKUPS`. Default
 * 10. Each new backup removes the oldest ones beyond it.
 */
export function backupsToKeep(): number {
  const value = process.env.SLUNK_KEEP_BACKUPS
  if (value === undefined || value === "") return 10
  const keep = Number(value)
  if (!Number.isInteger(keep) || keep < 1) {
    throw new Error(
      `SLUNK_KEEP_BACKUPS must be a whole number from 1: ${value}`
    )
  }
  return keep
}

/** The port the server listens on, from `PORT`. */
export function port(): number {
  return Number(process.env.PORT ?? 4000)
}
