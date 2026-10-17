// Alternate titles (slunk's 3_alternate_names.md §7): the lines under a main title,
// each with the artists' names in that title's language.
import type { CreditedLink } from "@/lib/credited"

/** An alternate title, as queries return it. */
export type AlternateTitle = {
  title: string
  language?: string | null
  printed: boolean
}

/**
 * The alternate title a list row shows under its main title (§7.4): the one
 * the search matched, when the main title didn't, or else the first.
 */
export function listedTitle<T extends { title: string }>(
  main: string,
  alternates: T[],
  q: string | null | undefined
): T | null {
  const search = q?.normalize("NFC").trim().toLowerCase()
  if (search && !main.toLowerCase().includes(search)) {
    const matched = alternates.find((alternate) =>
      alternate.title.toLowerCase().includes(search)
    )
    if (matched) return matched
  }
  return alternates[0] ?? null
}

/**
 * A tag's parts around " = ", as Discogs joins a title in two languages:
 * "Scenery = シーナリィ" (§7.7). The "=" needs spaces on both sides, so
 * "E=MC²" isn't split. Empty and repeated parts are left out. Null when
 * there's only one part.
 */
export function titleParts(text: string): string[] | null {
  const parts = text
    .normalize("NFC")
    .split(/\s+=\s+/u)
    .map((part) => part.trim())
  const unique = [...new Set(parts.filter(Boolean))]
  return unique.length > 1 ? unique : null
}

/** A credited artist, with the artist's names in other languages. */
export type NamedLink = CreditedLink & {
  artist: CreditedLink["artist"] & {
    alternateNames?: { name: string; language?: string | null }[]
  }
}

type Shown = { name: string; language?: string | null }

/**
 * An artist's name in a language (§7.2): the first name in it, the main name
 * or an alternate one. Without one, the name it's credited with, unless
 * that's in another language. Otherwise, the main name.
 */
function nameIn(link: NamedLink, language: string): Shown {
  const { artist, creditedAs } = link
  const main = { name: artist.name, language: artist.nameLanguage }
  if (artist.nameLanguage === language) return main
  const alternate = artist.alternateNames?.find((n) => n.language === language)
  if (alternate) return alternate
  if (creditedAs && (!creditedAs.language || creditedAs.language === language))
    return creditedAs
  return main
}

/**
 * The artists on the line of an alternate title in `language` (§7.2), as
 * links `Names` shows: each in that language, with the main name on hover.
 * Null when the title has no language, or when every artist would read the
 * same as on the main line: the line then shows only the title.
 */
export function artistsIn(
  links: NamedLink[],
  language: string | null | undefined
): CreditedLink[] | null {
  if (!language || links.length === 0) return null
  const shown = links.map((link) => nameIn(link, language))
  const same = links.every(
    (link, i) => (link.creditedAs?.name ?? link.artist.name) === shown[i].name
  )
  if (same) return null
  return links.map(({ artist }, i) => ({
    artist,
    creditedAs:
      shown[i].name === artist.name
        ? null
        : { name: shown[i].name, language: shown[i].language },
  }))
}
