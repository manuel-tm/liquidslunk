import { useState } from "react"

import { Cover } from "@/components/cover"
import { textProps } from "@/lib/languages"
import {
  Combobox,
  ComboboxContent,
  ComboboxEmpty,
  ComboboxInput,
  ComboboxItem,
  ComboboxList,
} from "@/components/ui/combobox"

/**
 * Something a picker offers: an id, and what it shows. An option with an
 * `image` key shows it before the name, or a placeholder when it's null. `lang`
 * is the name's language, and `detail` shows after it, muted: the alternate
 * name a search matched.
 *
 * An artist can also be offered as credited with one of its names, as
 * "Alika · as ALIKA": `creditedAs` is that name, and `key` tells the option
 * apart from the plain artist, which has the same id.
 */
export type Option = {
  id: string
  name: string
  image?: { id: number | string } | null
  lang?: string | null
  detail?: string | null
  creditedAs?: string | null
  key?: string
}

const keyOf = (option: Option) => option.key ?? option.id

const CREATE = "__create__"

export type PickerProps<T extends Option = Option> = {
  value: T | null
  onChange: (option: T | null) => void
  /** Ids to leave out, like those already in a list */
  exclude?: string[]
  placeholder?: string
  id?: string
  /** For a picker without a visible label, like one in a table row */
  "aria-label"?: string
  disabled?: boolean
  /** Typed in to start with, like a tag name the import couldn't pick */
  defaultQuery?: string
}

/**
 * A combobox whose options come from the backend as the user types. With
 * `onCreate`, it also offers to create what was typed.
 */
export function Picker<T extends Option>({
  value,
  onChange,
  exclude = [],
  placeholder,
  id,
  "aria-label": ariaLabel,
  disabled,
  defaultQuery,
  options,
  onQuery,
  query,
  noun,
  onCreate,
  createLabel = (typed) => `Create ${noun} "${typed}"`,
}: PickerProps<T> & {
  options: T[]
  query: string
  onQuery: (query: string) => void
  /** "artist", for "No artist found" and "Create artist …" */
  noun: string
  /** Null when creating was cancelled */
  onCreate?: (name: string) => Promise<T | null>
  createLabel?: (typed: string) => string
}) {
  const [creating, setCreating] = useState(false)
  const typed = query.trim()
  const items = options.filter((option) => !exclude.includes(option.id))
  // An alternate name counts, so the picker doesn't offer a second artist
  const exact = options.some((option) =>
    [option.name, option.creditedAs ?? option.detail].some(
      (name) => name?.toLowerCase() === typed.toLowerCase()
    )
  )
  if (onCreate && typed && !exact) {
    items.push({ id: CREATE, name: createLabel(typed) } as T)
  }

  return (
    <Combobox<T>
      items={items}
      filter={null}
      value={value}
      disabled={disabled || creating}
      defaultInputValue={defaultQuery}
      itemToStringLabel={(option) => option.name}
      isItemEqualToValue={(a, b) => keyOf(a) === keyOf(b)}
      onInputValueChange={onQuery}
      onValueChange={async (option) => {
        if (option?.id !== CREATE) {
          onChange(option)
          return
        }
        setCreating(true)
        try {
          const created = await onCreate!(typed)
          if (created) onChange(created)
        } finally {
          setCreating(false)
        }
      }}
    >
      <ComboboxInput
        id={id}
        aria-label={ariaLabel}
        placeholder={placeholder ?? `Search ${noun}s`}
        showClear={value !== null}
        className="w-full"
      />
      <ComboboxContent>
        <ComboboxEmpty>No {noun} found</ComboboxEmpty>
        <ComboboxList>
          {(option: T) => (
            <ComboboxItem key={keyOf(option)} value={option}>
              {option.image !== undefined && (
                <Cover file={option.image} alt="" className="size-8 shrink-0" />
              )}
              <span {...textProps(option.lang)}>{option.name}</span>
              {option.creditedAs ? (
                <span className="text-muted-foreground">
                  · as <span dir="auto">{option.creditedAs}</span>
                  {option.detail && ` (${option.detail})`}
                </span>
              ) : (
                option.detail && (
                  <span className="text-muted-foreground" dir="auto">
                    · {option.detail}
                  </span>
                )
              )}
            </ComboboxItem>
          )}
        </ComboboxList>
      </ComboboxContent>
    </Combobox>
  )
}
