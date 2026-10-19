// What the import makes of artist tags (slunk's 3_alternate_names.md §7.7): the
// artists they pick, the names left to pick, and other names to offer.
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

/** The tags that picked no artist: none matched, or several did. */
export function unpicked(suggestions: ArtistSuggestion[]): ArtistSuggestion[] {
  return suggestions.filter((suggestion) => !suggestion.artist)
}

/** "J. Smith (2 artists)": an unpicked tag, saying why. */
export function unpickedLabel(suggestion: ArtistSuggestion): string {
  return suggestion.matches > 1
    ? `${suggestion.name} (${suggestion.matches} artists)`
    : suggestion.name
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
