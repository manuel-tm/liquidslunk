// What the import makes of artist tags: the artists they pick, the names left
// to settle, and other names to offer.
import type { ArtistSuggestion } from "@/bindings"
import type { Option } from "@/components/picker"

/**
 * The artists the tags pick, as credited, for an editor: each artist once,
 * with the first tag's credited name, as the backend keeps it. `skip` leaves
 * out artists by id.
 */
export function pickedArtists(
  suggestions: ArtistSuggestion[],
  skip: Set<number> = new Set()
): Option[] {
  const picked = new Map<number, Option>()
  for (const { artist, creditedAs } of suggestions) {
    if (!artist || skip.has(artist.id) || picked.has(artist.id)) continue
    picked.set(artist.id, {
      // A string, as pickers' options' ids are
      id: String(artist.id),
      name: artist.name,
      lang: artist.nameLanguage,
      creditedAs: creditedAs ?? null,
    })
  }
  return [...picked.values()]
}

/**
 * A tag's name that picked no artist, because none matched or several did. It
 * waits in the artists list until it's created, chosen or removed.
 */
export type PendingArtist = Pick<ArtistSuggestion, "name" | "matches">

/** The tags that picked no artist, each name once. */
export function unpicked(suggestions: ArtistSuggestion[]): PendingArtist[] {
  const pending = new Map<string, PendingArtist>()
  for (const { name, artist, matches } of suggestions) {
    if (!artist && !pending.has(name)) pending.set(name, { name, matches })
  }
  return [...pending.values()]
}

/** "Not an artist yet", or "2 artists match": why a name picked none. */
export function pendingReason({ matches }: PendingArtist): string {
  return matches > 1 ? `${matches} artists match` : "Not an artist yet"
}

/** Whether a name can be made an artist at once: none has it yet. */
export function creatable({ matches }: PendingArtist): boolean {
  return matches < 2
}

/**
 * The list once a pending name is settled: without the name, and with the
 * artist chosen for it at the end, unless it's there already. A null artist
 * just removes the name.
 */
export function settle(
  values: { artists: Option[]; pending: PendingArtist[] },
  name: string,
  artist: Option | null
): { artists: Option[]; pending: PendingArtist[] } {
  const pending = values.pending.filter((p) => p.name !== name)
  const listed = !artist || values.artists.some((a) => a.id === artist.id)
  return {
    artists: listed ? values.artists : [...values.artists, artist],
    pending,
  }
}

/** A name from a tag, to offer as an alternate name of an artist. */
export type OfferedName = {
  artist: { id: number; name: string }
  name: string
}

/** The other parts of the tags, after " = ", each offered once. */
export function offeredNames(suggestions: ArtistSuggestion[]): OfferedName[] {
  const offered = new Map<string, OfferedName>()
  for (const { artist, otherNames } of suggestions) {
    if (!artist) continue
    for (const name of otherNames) {
      offered.set(`${artist.id}:${name}`, { artist, name })
    }
  }
  return [...offered.values()]
}
