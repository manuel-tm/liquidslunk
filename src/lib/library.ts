import { Channel } from "@tauri-apps/api/core"
import { useSyncExternalStore } from "react"
import { toast } from "sonner"

import type { LibraryStatus, MigrationReport, OpenProgress } from "@/bindings"
import { apollo } from "@/lib/apollo"
import { call, queryClient } from "@/lib/query"

type OnProgress = (progress: OpenProgress) => void

/**
 * Forgets everything cached from the library that was open, Apollo's and
 * TanStack Query's, so nothing from it shows in the next one (design §6.4).
 * Resetting the queries fetches the mounted ones again, like the library's
 * status, which `AppLayout` remounts the pages by.
 */
async function forgetCaches() {
  await apollo.clearStore()
  await queryClient.resetQueries()
}

function channel(onProgress: OnProgress) {
  const progress = new Channel<OpenProgress>()
  progress.onmessage = onProgress
  return progress
}

/**
 * Opens the library at `path`, a `slunk.db`, in place of the open one. It's
 * backed up and upgraded first when it needs to be, which `onProgress` hears.
 */
export async function openLibrary(path: string, onProgress: OnProgress) {
  const opened = await call("openLibrary", path, channel(onProgress))
  await forgetCaches()
  reportMigration(opened.library.name, opened.migration)
  return opened
}

/**
 * Creates a library in `folder` and opens it, or says the folder has one
 * already.
 */
export async function createLibrary(folder: string, onProgress: OnProgress) {
  const created = await call("createLibrary", folder, channel(onProgress))
  if (created.type === "created") {
    await forgetCaches()
    toast.success(`Created the library ${created.library.name}`)
  }
  return created
}

/**
 * Closes the open library, which the next launch still opens. The launch is
 * over, so Welcome doesn't open it again.
 */
export async function closeLibrary() {
  endLaunch()
  await call("closeLibrary")
  await forgetCaches()
}

/** The library's name, its folder's: `…/Music/slunk.db` is "Music". */
export function libraryName(path: string) {
  const parts = path.split(/[\\/]/)
  return parts.at(-2) || path
}

function reportMigration(name: string, migration: MigrationReport) {
  const applied = migration.applied.length
  if (applied === 0) return
  const backup = migration.backup?.split(/[\\/]/).at(-1)
  toast.success(`Upgraded the library ${name}`, {
    description:
      `Applied ${applied} migration${applied === 1 ? "" : "s"}.` +
      (backup ? ` Its backup is slunk-backups/${backup}.` : ""),
  })
}

/**
 * The launch (design §6.1): the library the settings remember opens once per
 * run, without asking. Welcome shows it as it goes.
 */
export type Launch =
  | { state: "waiting" }
  | { state: "opening"; name: string; migrating: boolean }
  | { state: "failed"; error: Error }
  | { state: "over" }

let launch: Launch = { state: "waiting" }
const listeners = new Set<() => void>()

function setLaunch(next: Launch) {
  launch = next
  for (const listener of listeners) listener()
}

export function useLaunch(): Launch {
  return useSyncExternalStore(
    (listener) => {
      listeners.add(listener)
      return () => {
        listeners.delete(listener)
      }
    },
    () => launch
  )
}

/**
 * Opens the saved library, the first time it's called: whether it opened.
 * When a library is open already, or the saved one isn't there, the launch
 * is over without opening anything.
 */
export async function launchLibrary(status: LibraryStatus): Promise<boolean> {
  if (launch.state !== "waiting") return false
  const saved = status.saved
  if (status.library || !saved?.found) {
    setLaunch({ state: "over" })
    return false
  }
  const name = libraryName(saved.path)
  setLaunch({ state: "opening", name, migrating: false })
  try {
    await openLibrary(saved.path, () =>
      setLaunch({ state: "opening", name, migrating: true })
    )
    setLaunch({ state: "over" })
    return true
  } catch (error) {
    setLaunch({ state: "failed", error: error as Error })
    return false
  }
}

/** Ends the launch once the user chooses a library, clearing its error. */
export function endLaunch() {
  if (launch.state !== "over") setLaunch({ state: "over" })
}
