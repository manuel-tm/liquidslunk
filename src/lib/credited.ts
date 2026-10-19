// Artists as credited: an artist, and the alternate name printed on the
// release, track or credit line, if any.
import type { Option } from "@/components/picker"

/** An artist as credited, as commands and queries return it. */
export type CreditedLink = {
  artist: { id: number | string; name: string; nameLanguage?: string | null }
  creditedAs?: { name: string; language?: string | null } | null
}

/** A credited artist as an editor's option: `creditedAs` is the name printed. */
export function creditedOption(link: CreditedLink): Option {
  return {
    // A string, as pickers' options' ids are
    id: String(link.artist.id),
    name: link.artist.name,
    lang: link.artist.nameLanguage,
    creditedAs: link.creditedAs?.name ?? null,
  }
}

/** Options as `CreditedArtistInput`s, for commands. */
export function creditedInput(options: Option[]) {
  return options.map((option) => ({
    artistId: Number(option.id),
    creditedAs: option.creditedAs ?? null,
  }))
}

/** The name a link shows (as printed), and its language. */
export function shownName(link: CreditedLink) {
  return link.creditedAs
    ? { name: link.creditedAs.name, language: link.creditedAs.language }
    : { name: link.artist.name, language: link.artist.nameLanguage }
}
