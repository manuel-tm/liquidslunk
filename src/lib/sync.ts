import { Channel } from "@tauri-apps/api/core"
import { open } from "@tauri-apps/plugin-dialog"
import { useSyncExternalStore } from "react"
import { toast } from "sonner"

import type { SyncOutcome, SyncProgress } from "@/bindings"
import { call, queryClient } from "@/lib/query"

/**
 * The sync running, or the last one, for one external library
 * (4_external_libraries.md §8.3). It's kept outside the pages, so leaving
 * the page doesn't lose it: the sync goes on in the backend, and its page
 * shows it again.
 */
export type SyncRun = {
  libraryId: number
  running: boolean
  progress: SyncProgress | null
  outcome: SyncOutcome | null
  error: Error | null
}

let current: SyncRun | null = null
const listeners = new Set<() => void>()

function set(next: SyncRun | null) {
  current = next
  for (const listener of listeners) listener()
}

function subscribe(listener: () => void) {
  listeners.add(listener)
  return () => listeners.delete(listener)
}

/**
 * Runs the plan `syncPlan` showed, hearing its progress as it goes. On an
 * iPod with tracks from somewhere else, `erase` is the user's agreement to
 * erase them first.
 */
export async function startSync(
  libraryId: number,
  planId: number,
  erase = false
) {
  if (current?.running) return
  set({ libraryId, running: true, progress: null, outcome: null, error: null })
  const progress = new Channel<SyncProgress>()
  progress.onmessage = (message) => {
    if (current?.libraryId === libraryId && current.running) {
      set({ ...current, progress: message })
    }
  }
  try {
    const outcome = await call(
      "sync",
      { id: libraryId, planId, erase },
      progress
    )
    set({ libraryId, running: false, progress: null, outcome, error: null })
  } catch (error) {
    set({
      libraryId,
      running: false,
      progress: null,
      outcome: null,
      error: error as Error,
    })
  } finally {
    // Its last sync, and what's connected
    void queryClient.invalidateQueries()
  }
}

/** Stops the sync after the file in hand. */
export async function cancelSync() {
  await call("cancelSync")
}

/** Forgets a library's finished sync, once its result has been seen. */
export function clearSync(libraryId: number) {
  if (current?.libraryId === libraryId && !current.running) set(null)
}

/** The sync running, or the last one. */
export function useSyncRun(): SyncRun | null {
  return useSyncExternalStore(subscribe, () => current)
}

/**
 * Ejects the disk an external library's files are on, a folder library's
 * folder or an iPod, so it can be unplugged. `what` names it in the toast.
 */
export async function ejectLibrary(
  id: number,
  what: { folder: string } | { ipod: string }
) {
  await call("ejectExternalLibrary", { id })
  toast.success(
    "ipod" in what
      ? `Ejected ${what.ipod}`
      : `Ejected the disk ${what.folder} is on`
  )
  void queryClient.invalidateQueries()
}

/**
 * Asks for a folder library's folder where it is now, a folder that moved or a
 * card that mounted under another name, and points the library at it. False
 * when nothing was chosen.
 */
export async function locateLibrary(id: number, folder: string | null) {
  const chosen = await open({
    directory: true,
    title: "Where the folder library's folder is now",
    defaultPath: folder ?? undefined,
  })
  if (typeof chosen !== "string") return false
  await call("locateExternalLibrary", { id, folder: chosen })
  toast.success(`Found it at ${chosen}`)
  void queryClient.invalidateQueries()
  return true
}
