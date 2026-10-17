import { Fragment } from "react"

import { languageName, textProps } from "@/lib/languages"
import { matchedName } from "@/lib/names"

type Name = { name: string; language?: string | null; realName?: boolean }

/**
 * An artist's or label's alternate names, under its name
 * (slunk's 3_alternate_names.md §7.5): the real names, then the others, each with its
 * language. A row with nothing in it is hidden.
 */
export function AlternateNames({ names }: { names: Name[] }) {
  const real = names.filter((name) => name.realName)
  const other = names.filter((name) => !name.realName)
  const rows: [string, Name[]][] = [
    [real.length > 1 ? "Real names" : "Real name", real],
    ["Other names", other],
  ]
  const shown = rows.filter(([, list]) => list.length > 0)
  if (shown.length === 0) return null

  return (
    <dl className="grid grid-cols-[minmax(5rem,auto)_1fr] gap-x-2 gap-y-1 text-xs">
      {shown.map(([label, list]) => (
        <Fragment key={label}>
          <dt className="text-muted-foreground">{label}:</dt>
          <dd>
            {list.map((name, index) => (
              <Fragment key={name.name}>
                {index > 0 && ", "}
                <span {...textProps(name.language)}>{name.name}</span>
                {name.language && (
                  <span className="text-muted-foreground">
                    {" "}
                    ({languageName(name.language)})
                  </span>
                )}
              </Fragment>
            ))}
          </dd>
        </Fragment>
      ))}
    </dl>
  )
}

/**
 * The alternate name a search matched, under a row's main name
 * (slunk's 3_alternate_names.md §7.4), so the row shows why it's there.
 */
export function MatchedName({
  main,
  names,
  q,
}: {
  main: string
  names: { name: string; language?: string | null }[]
  q: string | null | undefined
}) {
  const name = matchedName(main, names, q)
  if (!name) return null
  return (
    <div
      className="text-xs text-muted-foreground"
      {...textProps(name.language)}
    >
      {name.name}
    </div>
  )
}
