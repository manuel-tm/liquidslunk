import { useId, useState } from "react"

import type { Option } from "@/components/picker"
import { Input } from "@/components/ui/input"
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover"
import { textProps } from "@/lib/languages"
import { useCommand } from "@/lib/query"

/**
 * The name an artist is credited with, as printed (slunk's 3_alternate_names.md
 * §7.6). It suggests the artist's alternate names, like a role suggests the
 * roles in use, and whatever is typed is the name: blank is the main name,
 * and a name the artist doesn't have yet is added to its alternate names.
 */
export function CreditedAsInput({
  artist,
  onChange,
  className,
}: {
  /** An artist option: `creditedAs` is the name printed */
  artist: Option
  onChange: (creditedAs: string | null) => void
  className?: string
}) {
  const id = useId()
  // Only the field being typed in asks for the names
  const [focused, setFocused] = useState(false)
  const { data: names = [] } = useCommand(
    "artistNames",
    // Options' ids are strings, as the Picker's are
    { id: Number(artist.id) },
    { enabled: focused }
  )
  const language = names.find((name) => name.name === artist.creditedAs)
  return (
    <>
      <Input
        aria-label={`${artist.name}, credited as`}
        placeholder={`as ${artist.name}`}
        list={`${id}-names`}
        value={artist.creditedAs ?? ""}
        className={className}
        {...textProps(language?.language)}
        onChange={(e) => onChange(e.target.value || null)}
        onFocus={() => setFocused(true)}
        onBlur={() => setFocused(false)}
      />
      <datalist id={`${id}-names`}>
        {names.map((name) => (
          <option key={name.id} value={name.name} />
        ))}
      </datalist>
    </>
  )
}

/**
 * A chip's label for an artist as credited: the name printed, with the main
 * name on hover. Clicking it opens a popover to change the name.
 */
export function CreditedChipLabel({
  artist,
  onChange,
}: {
  artist: Option
  onChange: (creditedAs: string | null) => void
}) {
  return (
    <Popover>
      <PopoverTrigger
        render={
          <button
            type="button"
            className="hover:underline"
            title={artist.creditedAs ? artist.name : undefined}
            {...textProps(artist.creditedAs ? null : artist.lang)}
          />
        }
      >
        {artist.creditedAs ?? artist.name}
      </PopoverTrigger>
      <PopoverContent align="start" className="w-72 gap-1.5 text-xs">
        <p className="font-medium">Credited as</p>
        <CreditedAsInput artist={artist} onChange={onChange} />
        <p className="text-muted-foreground">
          As printed. Blank shows the main name, {artist.name}.
        </p>
      </PopoverContent>
    </Popover>
  )
}
