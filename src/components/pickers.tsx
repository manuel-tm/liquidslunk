// A picker for each kind of row, searching the backend as the user types.
// Each one owns its command, so it stays typed; they share the Picker view.
import { useMutation } from "@tanstack/react-query"
import { useState } from "react"

import { type Option, Picker, type PickerProps } from "@/components/picker"
import { NewTagDialog } from "@/components/tag-fields"
import type { FileType, ListKind, TagType } from "@/bindings"
import { useDebounced } from "@/hooks/use-debounced"
import { matchedName } from "@/lib/names"
import { call, useCommand } from "@/lib/query"
import { tagValueKey } from "@/lib/tags"

// Pickers return the first 20 matches
const LIMIT = 20

/**
 * An artist as options: the artist, then the artist credited with each
 * alternate name `q` matches, as "Alika · as ALIKA". Only those credit the
 * name, so a search hint like "Bjork" is never printed on a release by mistake.
 */
const withCredits =
  (q: string) =>
  <
    T extends {
      id: string
      name: string
      alternateNames: { id: number; name: string; realName: boolean }[]
    },
  >({
    alternateNames,
    ...artist
  }: T) => {
    const search = q.normalize("NFC").trim().toLowerCase()
    const matching = search
      ? alternateNames.filter((name) =>
          name.name.toLowerCase().includes(search)
        )
      : []
    return [
      artist,
      ...matching.map((name) => ({
        ...artist,
        key: `${artist.id}:${name.id}`,
        creditedAs: name.name,
        detail: name.realName ? "real name" : null,
      })),
    ]
  }

/** A label as an option, showing the alternate name `q` matched. */
const withMatch =
  (q: string) =>
  <T extends { name: string; alternateNames: { name: string }[] }>({
    alternateNames,
    ...option
  }: T) => ({
    ...option,
    detail: matchedName(option.name, alternateNames, q)?.name ?? null,
  })

/** Options' ids are strings, as the Picker's are. */
const artistOption = (artist: {
  id: number
  name: string
  nameLanguage: string | null
  picture: { id: number } | null
  alternateNames: { id: number; name: string; realName: boolean }[]
}) => ({
  id: String(artist.id),
  name: artist.name,
  lang: artist.nameLanguage,
  image: artist.picture,
  alternateNames: artist.alternateNames,
})

export function ArtistPicker(props: PickerProps) {
  const [query, setQuery] = useState(props.defaultQuery ?? "")
  const q = useDebounced(query)
  const { data } = useCommand("artistOptions", { q })
  const create = useMutation({
    mutationFn: (name: string) => call("createArtist", { name }),
  })
  return (
    <Picker
      {...props}
      noun="artist"
      query={query}
      onQuery={setQuery}
      options={(data ?? []).map(artistOption).flatMap(withCredits(q))}
      onCreate={async (name) => {
        const artist = await create.mutateAsync(name)
        return { id: String(artist.id), name: artist.name }
      }}
    />
  )
}

export function LabelPicker(props: PickerProps) {
  const [query, setQuery] = useState("")
  const q = useDebounced(query)
  const { data } = useCommand("labelOptions", { q })
  const create = useMutation({
    mutationFn: (name: string) => call("createLabel", { name }),
  })
  const options = (data ?? []).map((label) => ({
    id: String(label.id),
    name: label.name,
    lang: label.nameLanguage,
    alternateNames: label.alternateNames,
  }))
  return (
    <Picker
      {...props}
      noun="label"
      query={query}
      onQuery={setQuery}
      options={options.map(withMatch(q))}
      onCreate={async (name) => {
        const label = await create.mutateAsync(name)
        return { id: String(label.id), name: label.name }
      }}
    />
  )
}

export const LIST_NOUNS: Record<ListKind, string> = {
  release_formats: "format",
  release_media: "medium",
}

/** A release format or release medium. */
export function ListItemPicker({
  kind,
  ...props
}: PickerProps & { kind: ListKind }) {
  const [query, setQuery] = useState("")
  const q = useDebounced(query)
  const { data } = useCommand("listItemOptions", { kind, q })
  const create = useMutation({
    mutationFn: (name: string) => call("createListItem", { kind, name }),
  })
  const option = (item: { id: number; name: string }) => ({
    id: String(item.id),
    name: item.name,
  })
  return (
    <Picker
      {...props}
      noun={LIST_NOUNS[kind]}
      query={query}
      onQuery={setQuery}
      options={(data ?? []).map(option)}
      onCreate={async (name) => option(await create.mutateAsync(name))}
    />
  )
}

/** Roles in use, the most used first, matching what was typed. */
export function useCreditRoles(query: string, skip = false) {
  const q = useDebounced(query)
  const { data } = useCommand("creditRoles", { q }, { enabled: !skip })
  // A disabled query still gives what's cached for its key
  return skip ? [] : (data ?? [])
}

/** A release, searched by title. Its options' ids are strings, as the Picker's are. */
export function ReleasePicker(props: PickerProps) {
  const [query, setQuery] = useState("")
  const q = useDebounced(query)
  const { data } = useCommand("releaseOptions", { q })
  return (
    <Picker
      {...props}
      noun="release"
      query={query}
      onQuery={setQuery}
      options={(data ?? []).map((release) => ({
        id: String(release.id),
        name: release.title,
        image: release.cover,
      }))}
    />
  )
}

/**
 * A registered file, searched by path. Files can't be created here. Its
 * options' ids are strings, as the Picker's are.
 */
export function FilePicker({
  type,
  withoutTrack,
  ...props
}: PickerProps & { type?: FileType; withoutTrack?: boolean }) {
  const [query, setQuery] = useState("")
  const q = useDebounced(query)
  const { data } = useCommand("files", {
    q,
    perPage: LIMIT,
    type,
    withoutTrack,
    sort: "path",
  })
  const options = (data?.items ?? []).map((file) => ({
    id: String(file.id),
    name: file.path,
  }))
  return (
    <Picker
      {...props}
      noun={
        type === "image" ? "image" : type === "music" ? "audio file" : "file"
      }
      query={query}
      onQuery={setQuery}
      options={options}
    />
  )
}

/** A tag of any type. Ids repeat across types, so the option's id has both. */
export type TagOption = Option & {
  /** A string, as the Picker's ids are */
  tagId: string
  type: TagType
  multiple: boolean
}

export function tagOption(tag: {
  id: number | string
  type: TagType
  name: string
  multiple: boolean
}): TagOption {
  return {
    id: `${tag.type}:${tag.id}`,
    tagId: String(tag.id),
    type: tag.type,
    name: tag.name,
    multiple: tag.multiple,
  }
}

/**
 * A tag definition. With `creatable`, a new name opens a dialog that asks
 * for its type, and single or multiple.
 */
export function TagPicker({
  creatable,
  ...props
}: PickerProps<TagOption> & { creatable?: boolean }) {
  const [query, setQuery] = useState("")
  const q = useDebounced(query)
  const { data } = useCommand("tagOptions", { q })
  const [creating, setCreating] = useState<{
    name: string
    resolve: (tag: TagOption | null) => void
  } | null>(null)

  return (
    <>
      <Picker<TagOption>
        {...props}
        noun="tag"
        query={query}
        onQuery={setQuery}
        options={data?.map(tagOption) ?? []}
        onCreate={
          creatable
            ? (name) => new Promise((resolve) => setCreating({ name, resolve }))
            : undefined
        }
      />
      {creating && (
        <NewTagDialog
          name={creating.name}
          onDone={(tag) => {
            creating.resolve(tag && tagOption(tag))
            setCreating(null)
          }}
        />
      )}
    </>
  )
}

/** A text tag's values, the most used first, matching what was typed. */
export function useTagValues(tagId: string, query: string, skip = false) {
  const q = useDebounced(query)
  const { data } = useCommand(
    "tagValueOptions",
    { id: Number(tagId), q },
    { enabled: !skip }
  )
  // A disabled query still gives what's cached for its key
  return skip ? [] : (data ?? [])
}

/**
 * A value for a text tag: one it already has on other releases, so they're
 * spelled alike, or what was typed. The option's id ignores case, as the
 * database does.
 */
export function TagValuePicker({
  tagId,
  ...props
}: PickerProps & { tagId: string }) {
  const [query, setQuery] = useState("")
  const values = useTagValues(tagId, query)
  const option = (value: string) => ({ id: tagValueKey(value), name: value })
  return (
    <Picker
      {...props}
      noun="value"
      query={query}
      onQuery={setQuery}
      options={values.map(option)}
      onCreate={async (value) => option(value.trim())}
      createLabel={(typed) => `Add "${typed}"`}
    />
  )
}
