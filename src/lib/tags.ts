import type { TagFilterInput, TagRefInput, TagType } from "@/gql/graphql"
import { formatReleased } from "@/lib/format"

export const TAG_TYPES: TagType[] = ["text", "number", "date"]

export const TAG_TYPE_LABELS: Record<TagType, string> = {
  text: "Text",
  number: "Number",
  date: "Date",
}

/** What each type's inputs take, as placeholder text. */
export const TAG_PLACEHOLDERS: Record<TagType, string> = {
  text: "Value",
  number: "8.5",
  date: "2024, 2024-03 or 2024-03-15",
}

export function isTagType(type: string | undefined): type is TagType {
  return TAG_TYPES.includes(type as TagType)
}

/** A value as the page shows it: a date as `Mar 1, 2024`, else as it is. */
export function formatTagValue(type: TagType, value: string): string {
  return type === "date" ? formatReleased(value) : value
}

/** Text values compare ignoring ASCII case, as the database does. */
export function tagValueKey(value: string): string {
  return value.trim().replace(/[A-Z]/g, (c) => c.toLowerCase())
}

// Release list filters in the query string (slunk's design §7.6), one parameter each:
// `text.3=Rock` (equals), `text.3.contains=ro`, `number.5=8`,
// `number.5.min=8`, `number.5.max=10`, `date.2=2024` (anytime in 2024),
// `date.2.min=2024-03` and `date.2.max=2024`. They all have to match.

export type TagFilterOp = "equals" | "contains" | "min" | "max"

export type TagFilter = {
  key: string
  value: string
  type: TagType
  tagId: string
  op: TagFilterOp
}

const FILTER_KEY = /^(text|number|date)\.(\d+)(?:\.(contains|min|max))?$/

export function tagFilterKey(type: TagType, tagId: string, op: TagFilterOp) {
  return op === "equals" ? `${type}.${tagId}` : `${type}.${tagId}.${op}`
}

/** The tag filters in a list's query string, in order. */
export function tagFiltersOf(params: URLSearchParams): TagFilter[] {
  return [...params].flatMap(([key, value]) => {
    const match = FILTER_KEY.exec(key)
    if (!match) return []
    const [, type, tagId, op] = match
    return [
      {
        key,
        value,
        type: type as TagType,
        tagId,
        op: (op ?? "equals") as TagFilterOp,
      },
    ]
  })
}

/** A filter as the API takes it. */
export function tagFilterInput(filter: TagFilter): TagFilterInput {
  return { type: filter.type, tagId: filter.tagId, [filter.op]: filter.value }
}

/** "Genre = Rock", "Genre contains ro", "Score ≥ 8", "Bought ≤ 2023". */
export function describeTagFilter(name: string, filter: TagFilter): string {
  const value =
    filter.type === "date" ? formatReleased(filter.value) : filter.value
  const op = { equals: "=", contains: "contains", min: "≥", max: "≤" }[
    filter.op
  ]
  return `${name} ${op} ${value}`
}

/** The tags a list's filters name, once each, for `tagsByRef`. */
export function tagFilterRefs(filters: TagFilter[]): TagRefInput[] {
  const refs = new Map(
    filters.map((f) => [`${f.type}:${f.tagId}`, { type: f.type, id: f.tagId }])
  )
  return [...refs.values()]
}

/** Tag names by `type:id`, from `tagsByRef`. */
export function tagNames(
  tags: { id: string; type: TagType; name: string }[] | undefined
): Map<string, string> {
  return new Map((tags ?? []).map((t) => [`${t.type}:${t.id}`, t.name]))
}

/** A list filtered to what has this value: releases, or tracks. */
export function tagValueUrl(
  type: TagType,
  tagId: string,
  value: string,
  list: "/releases" | "/tracks" = "/releases"
) {
  return `${list}?${new URLSearchParams({ [tagFilterKey(type, tagId, "equals")]: value })}`
}
