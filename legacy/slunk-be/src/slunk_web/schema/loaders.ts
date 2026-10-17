import DataLoader from "dataloader"
import type { Catalog } from "../../slunk/catalog.ts"
import type { ListKind, Track } from "../../slunk/catalog/repository.ts"
import type { Media } from "../../slunk/media.ts"
import type { Tagging } from "../../slunk/tagging.ts"
import type { TagType } from "../../slunk/tagging/repository.ts"

/**
 * Batches the related rows a page shows into one query per relation (design
 * §7.3). Made fresh for every request, so nothing is cached between them.
 */
export function createLoaders({
  media,
  catalog,
  tagging,
}: {
  media: Media
  catalog: Catalog
  tagging: Tagging
}) {
  const list = (kind: ListKind) =>
    byId((ids) => catalog.listItemsByIds(kind, ids))
  const tag = (type: TagType) => byId((ids) => tagging.tagsByIds(type, ids))

  return {
    file: byId(media.filesByIds),
    artist: byId(catalog.artistsByIds),
    label: byId(catalog.labelsByIds),
    artistNames: grouped(catalog.artistNamesOf),
    artistName: byId(catalog.artistNamesByIds),
    artistNameUses: new DataLoader<number, number>(async (ids) => {
      const uses = await catalog.artistNameUses(ids)
      return ids.map((id) => uses.get(id) ?? 0)
    }),
    labelNames: grouped(catalog.labelNamesOf),
    release: byId(catalog.releasesByIds),
    track: byId(catalog.tracksByIds),
    format: list("release_formats"),
    medium: list("release_media"),
    releaseArtists: grouped(catalog.releaseArtistsOf),
    releaseTitles: grouped(catalog.releaseTitlesOf),
    trackTitles: grouped(catalog.trackTitlesOf),
    releaseLabels: grouped((ids) => catalog.releaseLinksOf("labels", ids)),
    releaseFormats: grouped((ids) => catalog.releaseLinksOf("formats", ids)),
    releaseMedia: grouped((ids) => catalog.releaseLinksOf("media", ids)),
    releaseTracks: grouped(catalog.tracksOfReleases),
    releaseDiscTitles: grouped(catalog.discTitlesOf),
    trackArtists: grouped(catalog.trackArtistsOf),
    releaseCredits: grouped((ids) => catalog.creditsOf("release", ids)),
    trackCredits: grouped((ids) => catalog.creditsOf("track", ids)),
    releaseFiles: grouped((ids) => media.fileLinksOf("release", ids)),
    artistFiles: grouped((ids) => media.fileLinksOf("artist", ids)),
    trackFiles: grouped((ids) => media.fileLinksOf("track", ids)),
    tag: { text: tag("text"), number: tag("number"), date: tag("date") },
    releaseTags: grouped((ids) => tagging.tagsOf("release", ids)),
    trackTags: grouped((ids) => tagging.tagsOf("track", ids)),
    trackOfFile: new DataLoader<number, Track | null>(async (ids) => {
      const tracks = await catalog.tracksOfFiles(ids)
      return ids.map((id) => tracks.get(id) ?? null)
    }),
  }
}

export type Loaders = ReturnType<typeof createLoaders>

/** Loads rows by id. A missing row is null. */
function byId<T extends { id: number }>(
  fetch: (ids: readonly number[]) => Promise<T[]>
) {
  return new DataLoader<number, T | null>(async (ids) => {
    const found = new Map((await fetch(ids)).map((row) => [row.id, row]))
    return ids.map((id) => found.get(id) ?? null)
  })
}

/** Loads lists by owner id. An owner without any has an empty list. */
function grouped<T>(
  fetch: (ids: readonly number[]) => Promise<Map<number, T[]>>
) {
  return new DataLoader<number, T[]>(async (ids) => {
    const found = await fetch(ids)
    return ids.map((id) => found.get(id) ?? [])
  })
}

/** Loads rows by id, leaving out any that are missing. */
export async function loadAll<T>(
  loader: DataLoader<number, T | null>,
  ids: readonly number[]
): Promise<T[]> {
  const rows = await loader.loadMany(ids)
  return rows.filter((row): row is T => row !== null && !(row instanceof Error))
}

/** Loads a row the schema promises exists. */
export async function loadRequired<T>(
  loader: DataLoader<number, T | null>,
  id: number
): Promise<T> {
  const row = await loader.load(id)
  if (row === null) throw new Error(`Missing row ${id}`)
  return row
}
