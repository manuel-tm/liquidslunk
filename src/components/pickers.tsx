// A picker for each kind of row, searching the backend as the user types.
// Each one owns its query, so it stays typed; they share the Picker view.
import { useMutation, useQuery } from "@apollo/client/react"
import { useState } from "react"

import { type Option, Picker, type PickerProps } from "@/components/picker"
import { type CreatedTag, NewTagDialog } from "@/components/tag-fields"
import { graphql } from "@/gql"
import type { FileType, ListKind, TagType } from "@/gql/graphql"
import { useDebounced } from "@/hooks/use-debounced"
import { matchedName } from "@/lib/names"
import { tagValueKey } from "@/lib/tags"

// Pickers return the first 20 matches (slunk's design §7.3)
const LIMIT = 20

/**
 * An artist as options (slunk's 3_alternate_names.md §7.4): the artist, then the
 * artist credited with each alternate name `q` matches, as "Alika · as
 * ALIKA". Only those credit the name, so a search hint like "Bjork" is never
 * printed on a release by mistake.
 */
const withCredits =
  (q: string) =>
  <
    T extends {
      id: string
      name: string
      alternateNames: { id: string; name: string; realName: boolean }[]
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

const ArtistOptions = graphql(`
  query ArtistOptions($q: String, $perPage: Int!) {
    artists(q: $q, perPage: $perPage, sort: "name") {
      items {
        id
        name
        lang: nameLanguage
        alternateNames {
          id
          name
          realName
        }
        image: picture {
          id
        }
      }
    }
  }
`)

const CreateArtistOption = graphql(`
  mutation CreateArtistOption($name: String!) {
    createArtist(input: { name: $name }) {
      id
      name
    }
  }
`)

export function ArtistPicker(props: PickerProps) {
  const [query, setQuery] = useState(props.defaultQuery ?? "")
  const q = useDebounced(query)
  const { data } = useQuery(ArtistOptions, { variables: { q, perPage: LIMIT } })
  const [create] = useMutation(CreateArtistOption)
  return (
    <Picker
      {...props}
      noun="artist"
      query={query}
      onQuery={setQuery}
      options={(data?.artists.items ?? []).flatMap(withCredits(q))}
      onCreate={async (name) =>
        (await create({ variables: { name } })).data!.createArtist
      }
    />
  )
}

const LabelOptions = graphql(`
  query LabelOptions($q: String, $perPage: Int!) {
    labels(q: $q, perPage: $perPage, sort: "name") {
      items {
        id
        name
        lang: nameLanguage
        alternateNames {
          name
        }
      }
    }
  }
`)

const CreateLabelOption = graphql(`
  mutation CreateLabelOption($name: String!) {
    createLabel(input: { name: $name }) {
      id
      name
    }
  }
`)

export function LabelPicker(props: PickerProps) {
  const [query, setQuery] = useState("")
  const q = useDebounced(query)
  const { data } = useQuery(LabelOptions, { variables: { q, perPage: LIMIT } })
  const [create] = useMutation(CreateLabelOption)
  return (
    <Picker
      {...props}
      noun="label"
      query={query}
      onQuery={setQuery}
      options={(data?.labels.items ?? []).map(withMatch(q))}
      onCreate={async (name) =>
        (await create({ variables: { name } })).data!.createLabel
      }
    />
  )
}

const ListItemOptions = graphql(`
  query ListItemOptions($kind: ListKind!, $q: String, $perPage: Int!) {
    listItems(kind: $kind, q: $q, perPage: $perPage, sort: "name") {
      items {
        id
        name
      }
    }
  }
`)

const CreateListItemOption = graphql(`
  mutation CreateListItemOption($kind: ListKind!, $name: String!) {
    createListItem(kind: $kind, name: $name) {
      id
      name
    }
  }
`)

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
  const { data } = useQuery(ListItemOptions, {
    variables: { kind, q, perPage: LIMIT },
  })
  const [create] = useMutation(CreateListItemOption)
  return (
    <Picker
      {...props}
      noun={LIST_NOUNS[kind]}
      query={query}
      onQuery={setQuery}
      options={data?.listItems.items ?? []}
      onCreate={async (name) =>
        (await create({ variables: { kind, name } })).data!.createListItem
      }
    />
  )
}

const CreditRoleOptions = graphql(`
  query CreditRoleOptions($q: String) {
    creditRoles(q: $q)
  }
`)

/** Roles in use, the most used first, matching what was typed. */
export function useCreditRoles(query: string, skip = false) {
  const q = useDebounced(query)
  const { data } = useQuery(CreditRoleOptions, { variables: { q }, skip })
  return data?.creditRoles ?? []
}

const ReleaseOptions = graphql(`
  query ReleaseOptions($q: String, $perPage: Int!) {
    releases(q: $q, perPage: $perPage, sort: "title") {
      items {
        id
        name: title
        image: cover {
          id
        }
      }
    }
  }
`)

export function ReleasePicker(props: PickerProps) {
  const [query, setQuery] = useState("")
  const q = useDebounced(query)
  const { data } = useQuery(ReleaseOptions, {
    variables: { q, perPage: LIMIT },
  })
  return (
    <Picker
      {...props}
      noun="release"
      query={query}
      onQuery={setQuery}
      options={data?.releases.items ?? []}
    />
  )
}

const FileOptions = graphql(`
  query FileOptions(
    $q: String
    $perPage: Int!
    $type: FileType
    $withoutTrack: Boolean
  ) {
    files(
      q: $q
      perPage: $perPage
      type: $type
      withoutTrack: $withoutTrack
      sort: "path"
    ) {
      items {
        id
        name: path
      }
    }
  }
`)

/** A registered file, searched by path. Files can't be created here. */
export function FilePicker({
  type,
  withoutTrack,
  ...props
}: PickerProps & { type?: FileType; withoutTrack?: boolean }) {
  const [query, setQuery] = useState("")
  const q = useDebounced(query)
  const { data } = useQuery(FileOptions, {
    variables: { q, perPage: LIMIT, type, withoutTrack },
  })
  return (
    <Picker
      {...props}
      noun={
        type === "image" ? "image" : type === "music" ? "audio file" : "file"
      }
      query={query}
      onQuery={setQuery}
      options={data?.files.items ?? []}
    />
  )
}

const TagOptions = graphql(`
  query TagOptions($q: String, $perPage: Int!) {
    tags(q: $q, perPage: $perPage, sort: "name") {
      items {
        id
        type
        name
        multiple
      }
    }
  }
`)

/** A tag of any type. Ids repeat across types, so the option's id has both. */
export type TagOption = Option & {
  tagId: string
  type: TagType
  multiple: boolean
}

export function tagOption(tag: CreatedTag): TagOption {
  return {
    id: `${tag.type}:${tag.id}`,
    tagId: tag.id,
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
  const { data } = useQuery(TagOptions, { variables: { q, perPage: LIMIT } })
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
        options={data?.tags.items.map(tagOption) ?? []}
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

const TagValueOptions = graphql(`
  query TagValueOptions($id: ID!, $q: String, $perPage: Int!) {
    tag(type: text, id: $id) {
      __typename
      id
      ... on TextTag {
        values(q: $q, perPage: $perPage) {
          items {
            value
          }
        }
      }
    }
  }
`)

/** A text tag's values, the most used first, matching what was typed. */
export function useTagValues(tagId: string, query: string, skip = false) {
  const q = useDebounced(query)
  const { data } = useQuery(TagValueOptions, {
    variables: { id: tagId, q, perPage: LIMIT },
    skip,
  })
  const tag = data?.tag
  return tag?.__typename === "TextTag"
    ? tag.values.items.map((item) => item.value)
    : []
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
