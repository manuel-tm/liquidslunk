import { useState } from "react"

import { ChipList } from "@/components/chip-list"
import type { Option } from "@/components/picker"
import { ArtistPicker } from "@/components/pickers"
import { QueryError } from "@/components/query-error"
import { textProps } from "@/lib/languages"
import { call } from "@/lib/query"

/** An artist as an alias chip. Options' ids are strings, as the Picker's are. */
export const aliasOption = (artist: {
  id: number
  name: string
  nameLanguage: string | null
}): Option => ({
  id: String(artist.id),
  name: artist.name,
  lang: artist.nameLanguage,
})

/** By name ignoring case, then by id, as the artist page lists them. */
function byName(aliases: Option[]): Option[] {
  return [...aliases].sort(
    (a, b) =>
      a.name.localeCompare(b.name, undefined, { sensitivity: "base" }) ||
      Number(a.id) - Number(b.id)
  )
}

/** An artist picked, and the aliases of its own it brought. */
type Brought = { by: Option; aliases: Option[] }

/**
 * An artist's aliases (frontend.md, "Aliases"): chips, by name, then the
 * artist picker, which leaves out the artist and its aliases. Saving makes them
 * one group with the artist, so picking an artist brings its own aliases too,
 * and a note under the chips says which.
 */
export function AliasesEditor({
  artistId,
  aliases,
  onChange,
}: {
  /** The artist edited, or none for a new one */
  artistId?: number
  aliases: Option[]
  onChange: (aliases: Option[]) => void
}) {
  const [brought, setBrought] = useState<Brought[]>([])
  const [error, setError] = useState<Error | null>(null)
  const ids = new Set(aliases.map((alias) => alias.id))

  async function change(next: Option[]) {
    const picked = next.find((alias) => !ids.has(alias.id))
    onChange(byName(next))
    if (!picked) return
    try {
      const own = await call("artistAliases", { id: Number(picked.id) })
      const have = new Set([...next.map((alias) => alias.id), String(artistId)])
      const more = own.map(aliasOption).filter((alias) => !have.has(alias.id))
      if (more.length === 0) return
      onChange(byName([...next, ...more]))
      setBrought((notes) => [...notes, { by: picked, aliases: more }])
    } catch (caught) {
      setError(caught as Error)
    }
  }

  return (
    <div className="flex flex-col gap-1">
      <ChipList
        items={aliases}
        onChange={change}
        picker={(props) => (
          <ArtistPicker
            {...props}
            exclude={[
              ...(props.exclude ?? []),
              ...(artistId ? [String(artistId)] : []),
            ]}
          />
        )}
      />
      {brought.map(({ by, aliases: more }) => {
        const still = more.filter((alias) => ids.has(alias.id))
        if (!ids.has(by.id) || still.length === 0) return null
        return (
          <p key={by.id} className="text-xs text-muted-foreground">
            With <span {...textProps(by.lang)}>{by.name}</span>'s{" "}
            {still.length === 1 ? "alias" : "aliases"},{" "}
            {still.map((alias, index) => (
              <span key={alias.id}>
                {index > 0 && ", "}
                <span {...textProps(alias.lang)}>{alias.name}</span>
              </span>
            ))}
            .
          </p>
        )
      })}
      {error && <QueryError error={error} title="Can't read its aliases" />}
    </div>
  )
}
