// Artists as credited (slunk's 3_alternate_names.md §4.2): an artist, and the
// alternate name printed on the release, track or credit line, if any.
import type { Option } from "@/components/picker"

/** An artist as credited, as queries return it. */
export type CreditedLink = {
  artist: { id: string; name: string; nameLanguage?: string | null }
  creditedAs?: { name: string; language?: string | null } | null
}

/** A credited artist as an editor's option: `creditedAs` is the name printed. */
export function creditedOption(link: CreditedLink): Option {
  return {
    id: link.artist.id,
    name: link.artist.name,
    lang: link.artist.nameLanguage,
    creditedAs: link.creditedAs?.name ?? null,
  }
}

/** Options as the API's `CreditedArtistInput`s. */
export function creditedInput(options: Option[]) {
  return options.map((option) => ({
    artistId: option.id,
    creditedAs: option.creditedAs ?? null,
  }))
}

/** The name a link shows (as printed), and its language. */
export function shownName(link: CreditedLink) {
  return link.creditedAs
    ? { name: link.creditedAs.name, language: link.creditedAs.language }
    : { name: link.artist.name, language: link.artist.nameLanguage }
}
