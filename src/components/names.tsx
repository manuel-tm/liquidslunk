import { Fragment } from "react"
import { Link } from "react-router"

import type { CreditedLink } from "@/lib/credited"
import { textProps } from "@/lib/languages"

type Named = { id: number | string; name: string; nameLanguage?: string | null }

/**
 * Names that link to their pages, joined with commas: "A", "A, B", "A, B, C"
 * (slunk's 2_credits.md §7). An "&" is only ever part of a name. Each name has its
 * language's `lang` (slunk's 3_alternate_names.md §7.1).
 *
 * An artist as credited shows the name printed, and hovering it shows the
 * artist's main name: "ALIKA" links to Alika, with no "*".
 */
export function Names({
  items,
  to,
}: {
  items: (Named | CreditedLink)[]
  to: (id: number | string) => string
}) {
  return (
    <>
      {items.map((item, index) => {
        const { id, name, language, main } = shown(item)
        return (
          <Fragment key={id}>
            {index > 0 && ", "}
            <Link
              to={to(id)}
              className="hover:underline"
              title={main ?? undefined}
              {...textProps(language)}
            >
              {name}
            </Link>
          </Fragment>
        )
      })}
    </>
  )
}

/** What an item shows, and the main name when it's credited with another. */
function shown(item: Named | CreditedLink) {
  if (!("artist" in item)) {
    return { id: item.id, name: item.name, language: item.nameLanguage }
  }
  const { artist, creditedAs } = item
  return creditedAs
    ? {
        id: artist.id,
        name: creditedAs.name,
        language: creditedAs.language,
        main: artist.name,
      }
    : { id: artist.id, name: artist.name, language: artist.nameLanguage }
}

/**
 * "as ALIKA", muted, when a link credits its artist under another name, for a
 * row that doesn't show the name itself (slunk's 3_alternate_names.md §7.5).
 */
export function CreditedAsNote({
  creditedAs,
}: {
  creditedAs?: { name: string; language?: string | null } | null
}) {
  if (!creditedAs) return null
  return (
    <span className="text-muted-foreground">
      {" "}
      as <span {...textProps(creditedAs.language)}>{creditedAs.name}</span>
    </span>
  )
}

export const artistUrl = (id: number | string) => `/artists/${id}`
export const labelUrl = (id: number | string) => `/labels/${id}`
