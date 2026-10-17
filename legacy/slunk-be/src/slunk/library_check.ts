// The library check (design §8, phase 4): what changed on disk since files
// were registered, and catalog gaps the database can't rule out. Running it
// writes nothing; the fixes are separate calls.
import type { Catalog } from "./catalog.ts"
import type { Media } from "./media.ts"

export type LibraryCheck = ReturnType<typeof createLibraryCheck>

export function createLibraryCheck({
  media,
  catalog,
}: {
  media: Media
  catalog: Catalog
}) {
  return {
    async run() {
      const [files, releasesWithoutArtists, tracksWithoutPerformers] =
        await Promise.all([
          media.checkFiles(),
          catalog.releasesWithoutArtists(),
          catalog.tracksWithoutPerformers(),
        ])
      return { ...files, releasesWithoutArtists, tracksWithoutPerformers }
    },

    /** Points files that moved at their new paths. */
    relinkFiles: (moves: { fileId: number; path: string }[]) =>
      media.relinkFiles(moves),

    /**
     * Reads changed files again: their size, mtime and hash, and the
     * duration of the track whose audio each one is.
     */
    async rehashFiles(ids: number[]) {
      const files = await media.rehashFiles(ids)
      await catalog.readDurations(files.map((file) => file.id))
      return files
    },
  }
}
