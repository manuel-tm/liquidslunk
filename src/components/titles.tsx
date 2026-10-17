import { cn } from "cn"

import { artistUrl, Names } from "@/components/names"
import { textProps } from "@/lib/languages"
import {
  type AlternateTitle,
  artistsIn,
  listedTitle,
  type NamedLink,
} from "@/lib/titles"

/** A title, marked with its language (slunk's 3_alternate_names.md §7.1). */
export function Title({
  title,
  language,
  className,
}: {
  title: string
  language?: string | null
  className?: string
}) {
  return (
    <span className={className} {...textProps(language)}>
      {title}
    </span>
  )
}

/**
 * The lines under a main title, one per alternate title, smaller and muted
 * (slunk's 3_alternate_names.md §7.1). With `artists`, the ones the main line names,
 * a line repeats them in its title's language when they'd read differently
 * there (§7.2): "福居良, 福居良トリオ – シーナリィ". There's no "=".
 */
export function AlternateTitleLines({
  titles,
  artists,
  className,
}: {
  titles: AlternateTitle[]
  artists?: NamedLink[]
  className?: string
}) {
  if (titles.length === 0) return null
  // Spans, laid out as a column, so it can go inside a heading
  return (
    <span className={cn("flex flex-col text-muted-foreground", className)}>
      {titles.map((title) => {
        const shown = artists ? artistsIn(artists, title.language) : null
        return (
          <span key={title.title}>
            {shown && (
              <>
                <Names items={shown} to={artistUrl} /> –{" "}
              </>
            )}
            <AlternateTitleText title={title} />
          </span>
        )
      })}
    </span>
  )
}

/**
 * An alternate title. One added in liquidslunk, like a translation, is in italics,
 * and its tooltip says so.
 */
function AlternateTitleText({ title }: { title: AlternateTitle }) {
  return (
    <span
      className={cn(!title.printed && "italic")}
      title={
        title.printed
          ? undefined
          : "Added in liquidslunk: it isn't on the release"
      }
      {...textProps(title.language)}
    >
      {title.title}
    </span>
  )
}

/**
 * The alternate title a list row shows under its main one, cut to one line
 * (slunk's 3_alternate_names.md §7.4): the one the search matched, or the first.
 */
export function ListedAlternateTitle({
  main,
  titles,
  q,
}: {
  main: string
  titles: AlternateTitle[]
  q?: string | null
}) {
  const title = listedTitle(main, titles, q)
  if (!title) return null
  return (
    <div className="max-w-80 truncate text-xs text-muted-foreground">
      <AlternateTitleText title={title} />
    </div>
  )
}
